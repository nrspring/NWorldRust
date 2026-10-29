//! The editable world: a finite heightmap rendered as a grid of level-of-detail chunks.

use std::time::{Duration, Instant};

use macroquad::miniquad::{BufferId, BufferSource, BufferType, BufferUsage, RenderingBackend};
use macroquad::prelude::*;

use crate::gen_common::ExistingLand;
use crate::render::TerrainVertex;

static NEXT_WORLD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub const SEA_LEVEL: f32 = 0.0;
/// Spacing between heightmap samples, in world units (meters).
pub const CELL_SIZE: f32 = 4.0;
/// Every point of a new world starts this far below sea level.
pub const DEEP_OCEAN: f32 = -200.0;

#[cfg(test)]
mod surface_tests {
    use super::*;

    #[test]
    fn surface_encodes_lowland_and_hill_elevation_without_changing_coast() {
        let mut world = World::new(256.0, 256.0);
        let mut heights = world.heights().to_vec();
        heights[..6].copy_from_slice(&[-1.0, 0.0, 1.0, 7.0, 50.0, 200.0]);
        world.set_heights(heights);
        let texels = world.surface_texels();
        let pixels: Vec<_> = texels.chunks_exact(4).take(6).collect();
        assert_eq!(pixels.iter().map(|p| p[0]).collect::<Vec<_>>(), [0, 128, 255, 255, 255, 255]);
        assert_eq!(pixels[0][1], 0);
        assert_eq!(pixels[1][1], 0);
        assert!(pixels[2][1] < pixels[3][1]);
        assert!(pixels[3][1] < pixels[4][1]);
        assert_eq!(pixels[4][1], 255);
        assert_eq!(pixels[5][1], 255);
    }

    /// A 256 m world of flat 20 m ground with a pit of 14 m at its middle, filled by a lake whose
    /// surface stands at 20 m.
    fn world_with_lake() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![20.0; nx * nz];
        let mut water = vec![SEA_LEVEL; nx * nz];
        for j in 29..=35 {
            for i in 29..=35 {
                water[j * nx + i] = 20.0;
            }
        }
        heights[32 * nx + 32] = 14.0;
        world.set_terrain(heights, water);
        world
    }

    #[test]
    fn a_lake_is_drawn_at_its_own_level() {
        let world = world_with_lake();
        let nx = world.samples().0;
        assert_eq!(world.surface_height(0.0, 0.0), 20.0, "the surface is the lake's, not the pit's");

        let texels = world.surface_texels();
        let texel = |i: usize, j: usize| &texels[(j * nx + i) * 4..(j * nx + i) * 4 + 4];
        assert_eq!(texel(32, 32)[0], 0, "the pit is water");
        assert!(texel(32, 32)[3] > 0, "the pit's water has some depth");
        // Ground at the lake's level is its shore, and ground beyond its water is dry.
        assert_eq!(texel(31, 32)[0], 128);
        assert_eq!(texel(10, 10)[0], 255);
        assert_eq!(texel(10, 10)[3], 255, "20 m above the sea is far above the beach");
        assert_eq!(texel(31, 32)[3], 0, "the lake's shore is level with its water");
        // Beaches are found beside the lake, not far from it.
        assert!(world.near_water[32 * nx + 36]);
        assert!(!world.near_water[10 * nx + 10]);
    }

    #[test]
    fn the_surface_tells_shallow_water_from_deep() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![DEEP_OCEAN; nx * nz];
        let mut water = vec![SEA_LEVEL; nx * nz];
        heights[0] = -3.0;
        // A lake dug 60 m below its surface.
        heights[1] = -40.0;
        water[1] = 20.0;
        world.set_terrain(heights, water);
        let texels = world.surface_texels();
        let depth = |k: usize| texels[k * 4 + 3];
        assert_eq!(depth(2), 255, "the open sea is as deep as it gets");
        assert!(depth(0) > 0 && depth(0) < 80, "shallow sea reads as {}", depth(0));
        assert!(depth(1) <= 70, "a lake is shallow however deep it is dug, not {}", depth(1));
        assert!(depth(0) < depth(1), "3 m is shallower than 4 m");
    }

    #[test]
    fn painting_a_lake_moves_the_beaches_with_it() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![20.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        assert!(!world.near_water[32 * nx + 34]);

        world.heights_mut()[32 * nx + 32] = 14.0;
        world.water_mut()[32 * nx + 32] = 20.0;
        let region = Region { i0: 32, j0: 32, i1: 32, j1: 32 };
        let texels = world.refresh_region(region, Vec3::ZERO);
        assert!(world.near_water[32 * nx + 34], "the new lake has a beach");
        assert!(texels.i0 <= 28 && texels.i1 >= 36 && texels.j0 <= 28 && texels.j1 >= 36, "{texels:?}");
    }
}

/// Heightmap cells along a chunk side at the finest LOD; each coarser LOD halves the vertex count.
const CHUNK_CELLS: usize = 16;
/// Map sides are whole numbers of chunks.
pub const CHUNK_SIZE: f32 = CELL_SIZE * CHUNK_CELLS as f32;
pub const MIN_MAP_SIZE: f32 = 256.0;
pub const MAX_MAP_SIZE: f32 = 12288.0;
/// Chunks along a side of a block, the group of chunks that is kept on the GPU and drawn together.
/// Bigger blocks mean fewer draw calls; smaller ones less to re-upload when a chunk changes. An
/// 8 x 8 block has at most 64 x 357 vertices, well inside 16-bit indices.
const BLOCK_CHUNKS: usize = 8;
/// Distance thresholds (world units) for LOD 0, 1 and 2; anything farther is LOD 3.
/// Generous enough that coastlines stay crisp in the top-down view of a 4 km map.
const LOD_DISTANCES: [f32; 3] = [500.0, 1300.0, 4000.0];

/// Whether a map side of this many heightmap cells is a size the world can be built at.
pub fn cells_valid(cells: usize) -> bool {
    let meters = cells as f32 * CELL_SIZE;
    snap_map_size(meters) == meters
}

/// Rounds a requested map side (meters) to the nearest size the world can be built at.
pub fn snap_map_size(meters: f32) -> f32 {
    ((meters / CHUNK_SIZE).round() * CHUNK_SIZE).clamp(MIN_MAP_SIZE, MAX_MAP_SIZE)
}

struct Chunk {
    lod: u8,
    vertices: Vec<TerrainVertex>,
    indices: Vec<u16>,
    /// Lowest and highest point of the mesh, skirts included.
    y_min: f32,
    y_max: f32,
}

/// A block of chunks as uploaded to the GPU.
struct Block {
    /// A chunk in the block changed since it was last uploaded.
    dirty: bool,
    mesh: Option<GpuMesh>,
    center: Vec3,
    radius: f32,
}

/// Vertex and index buffers on the GPU.
struct GpuMesh {
    vertices: BufferId,
    indices: BufferId,
    count: i32,
}

impl GpuMesh {
    fn delete(self, ctx: &mut dyn RenderingBackend) {
        ctx.delete_buffer(self.vertices);
        ctx.delete_buffer(self.indices);
    }
}

/// A block of terrain to draw: its buffers and how many indices to draw from them.
pub struct BlockDraw {
    pub vertices: BufferId,
    pub indices: BufferId,
    pub count: i32,
}

pub struct World {
    /// Heightmap cells along x (west to east) and z (north to south).
    cells_x: usize,
    cells_z: usize,
    /// (cells_x + 1) × (cells_z + 1) elevation samples, row-major from the north-west corner.
    heights: Vec<f32>,
    /// The level of the water at each sample, which holds wherever the ground is lower: sea level
    /// everywhere except around a lake, where it is the lake's own level.
    water: Vec<f32>,
    /// For each sample, whether water is close enough for the terrain shader to look for a
    /// beach there; see [`near_water`].
    near_water: Vec<bool>,
    /// How much forest there is at each sample, from 0 (none) to 255 (the densest).
    forest: Vec<u8>,
    chunks_x: usize,
    chunks: Vec<Option<Chunk>>,
    /// For each chunk, a number that changes whenever anything its trees depend on does.
    stamps: Vec<u32>,
    blocks_x: usize,
    blocks: Vec<Block>,
    /// Which world this is, and how many times its terrain has changed; see [`World::revision`].
    id: u64,
    version: u64,
}

impl World {
    /// A new world of the given size in meters (snapped with [`snap_map_size`]): deep ocean
    /// everywhere, centered on the origin.
    pub fn new(width: f32, height: f32) -> Self {
        let cells = |meters: f32| (snap_map_size(meters) / CELL_SIZE) as usize;
        let (cells_x, cells_z) = (cells(width), cells(height));
        let (chunks_x, chunks_z) = (cells_x / CHUNK_CELLS, cells_z / CHUNK_CELLS);
        let samples = (cells_x + 1) * (cells_z + 1);
        let (blocks_x, blocks_z) = (chunks_x.div_ceil(BLOCK_CHUNKS), chunks_z.div_ceil(BLOCK_CHUNKS));
        Self {
            cells_x,
            cells_z,
            heights: vec![DEEP_OCEAN; samples],
            water: vec![SEA_LEVEL; samples],
            near_water: vec![true; samples],
            forest: vec![0; samples],
            chunks_x,
            chunks: (0..chunks_x * chunks_z).map(|_| None).collect(),
            stamps: vec![0; chunks_x * chunks_z],
            id: NEXT_WORLD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            version: 0,
            blocks_x,
            blocks: (0..blocks_x * blocks_z)
                .map(|_| Block { dirty: true, mesh: None, center: Vec3::ZERO, radius: 0.0 })
                .collect(),
        }
    }

    /// Identifies this world and the state of its terrain: it differs whenever the heights
    /// have changed since it was last read, or for a different world.
    pub fn revision(&self) -> (u64, u64) {
        (self.id, self.version)
    }

    /// Width (x) and height (z) of the world in meters.
    pub fn size(&self) -> Vec2 {
        vec2(self.cells_x as f32, self.cells_z as f32) * CELL_SIZE
    }

    pub fn half_extents(&self) -> Vec2 {
        self.size() * 0.5
    }

    /// Heightmap cells along x and z.
    pub fn cells(&self) -> (usize, usize) {
        (self.cells_x, self.cells_z)
    }

    /// Every elevation sample, row-major from the north-west corner, (cells_x + 1) per row.
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// The water level at every sample, laid out as in `heights`: wherever the ground is lower
    /// than this it is under water. It is sea level except around lakes.
    pub fn water(&self) -> &[f32] {
        &self.water
    }

    /// How much forest there is at every sample, laid out as in `heights`: 0 for none up to 255
    /// for the densest. Where trees actually stand is worked out from this and the terrain.
    pub fn forest(&self) -> &[u8] {
        &self.forest
    }

    /// Mutable forest densities. After changing some, call
    /// [`forest_changed`](Self::forest_changed) for the part that changed.
    pub fn forest_mut(&mut self) -> &mut [u8] {
        &mut self.forest
    }

    /// Replaces every forest density (laid out as in `heights`).
    pub fn set_forest(&mut self, forest: Vec<u8>) {
        assert_eq!(forest.len(), self.heights.len(), "forest map size mismatch");
        self.forest = forest;
        self.stamps.iter_mut().for_each(|s| *s = s.wrapping_add(1));
    }

    /// Notes that the forest densities of the samples in `region` changed.
    pub fn forest_changed(&mut self, region: Region) {
        let (cx0, cx1) = (region.i0.saturating_sub(1) / CHUNK_CELLS, ((region.i1 + 1) / CHUNK_CELLS).min(self.chunks_x - 1));
        let chunks_z = self.stamps.len() / self.chunks_x;
        let (cz0, cz1) = (region.j0.saturating_sub(1) / CHUNK_CELLS, ((region.j1 + 1) / CHUNK_CELLS).min(chunks_z - 1));
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let stamp = &mut self.stamps[cz * self.chunks_x + cx];
                *stamp = stamp.wrapping_add(1);
            }
        }
    }

    /// Chunks along x and along z.
    pub fn chunk_counts(&self) -> (usize, usize) {
        (self.chunks_x, self.stamps.len() / self.chunks_x)
    }

    /// A number that is different whenever the terrain, water or forest in or around chunk
    /// (`cx`, `cz`) has changed since it was last read.
    pub fn chunk_stamp(&self, cx: usize, cz: usize) -> u32 {
        self.stamps[cz * self.chunks_x + cx]
    }

    /// Whether the sample is close to water, which is where beaches are; see [`near_water`].
    pub fn beach_nearby(&self, idx: usize) -> bool {
        self.near_water[idx]
    }

    /// The forest densities of the samples in `region` as RGBA texels for the terrain shader,
    /// row by row: the density in red.
    pub fn forest_texels_in(&self, region: Region) -> Vec<u8> {
        let nx = self.cells_x + 1;
        let mut texels = Vec::with_capacity(region.width() * region.height() * 4);
        for j in region.j0..=region.j1 {
            texels.extend(self.forest[j * nx + region.i0..=j * nx + region.i1].iter().flat_map(|&d| [d, 0, 0, 255]));
        }
        texels
    }

    /// Replaces every elevation sample (laid out as in `heights`), keeping the water levels, and
    /// drops the built chunks so they are rebuilt from the new terrain.
    pub fn set_heights(&mut self, heights: Vec<f32>) {
        let water = std::mem::take(&mut self.water);
        self.set_terrain(heights, water);
    }

    /// Replaces every elevation sample and water level, and drops the built chunks so they are
    /// rebuilt from the new terrain.
    pub fn set_terrain(&mut self, heights: Vec<f32>, water: Vec<f32>) {
        assert_eq!(heights.len(), self.heights.len(), "heightmap size mismatch");
        assert_eq!(water.len(), self.heights.len(), "water map size mismatch");
        self.near_water = near_water(&heights, &water, self.cells_x + 1, self.cells_z + 1);
        self.heights = heights;
        self.water = water;
        self.version += 1;
        self.stamps.iter_mut().for_each(|s| *s = s.wrapping_add(1));
        self.chunks.iter_mut().for_each(|c| *c = None);
        self.blocks.iter_mut().for_each(|b| b.dirty = true);
    }

    /// Height of the visible surface (land or water) at a world position.
    pub fn surface_height(&self, x: f32, z: f32) -> f32 {
        let half = self.half_extents();
        let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, self.cells_x as f32);
        let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, self.cells_z as f32);
        let (i, j) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let top = self.surface_sample(i, j) + (self.surface_sample(i + 1, j) - self.surface_sample(i, j)) * tx;
        let bottom = self.surface_sample(i, j + 1) + (self.surface_sample(i + 1, j + 1) - self.surface_sample(i, j + 1)) * tx;
        top + (bottom - top) * tz
    }

    /// Height of the visible surface, land or water, at a heightmap sample.
    fn surface_sample(&self, gi: i32, gj: i32) -> f32 {
        let i = gi.clamp(0, self.cells_x as i32) as usize;
        let j = gj.clamp(0, self.cells_z as i32) as usize;
        let idx = j * (self.cells_x + 1) + i;
        self.heights[idx].max(self.water[idx])
    }

    /// Heightmap samples along x and z: one more than the cells.
    pub fn samples(&self) -> (usize, usize) {
        (self.cells_x + 1, self.cells_z + 1)
    }

    /// The heightmap as RGBA texels for the terrain shader, one per sample in the same order.
    /// Red is height around the water level, from 0 (1 m or more under water) through 128 (at the
    /// water) to 255 (1 m or more above); the shader traces the coast where it crosses the
    /// middle, which follows a smooth curve between samples. Green encodes
    /// elevation from 0 to 50 m, which the shader colors: 25 shades of green up to 25 m, grey
    /// rock from 26 m and snow from 44 m. Alpha is how far the ground stands above the water, up
    /// to 8 m, which is as far up as a beach goes, and where the ground is under water, how deep
    /// the water is, up to 64 m (stored as a square root), which tells shallow water from deep. Lakes and rivers, whose
    /// water is above sea level, are shallow however deep they are dug.
    pub fn surface_texels(&self) -> Vec<u8> {
        self.surface_texels_in(Region { i0: 0, j0: 0, i1: self.cells_x, j1: self.cells_z })
    }

    /// The part of [`surface_texels`](Self::surface_texels) inside `region`, row by row.
    pub fn surface_texels_in(&self, region: Region) -> Vec<u8> {
        let nx = self.cells_x + 1;
        let mut texels = Vec::with_capacity(region.width() * region.height() * 4);
        for j in region.j0..=region.j1 {
            let row = j * nx + region.i0..=j * nx + region.i1;
            let near = &self.near_water[row.clone()];
            let water = &self.water[row.clone()];
            texels.extend(
                self.heights[row]
                    .iter()
                    .zip(water)
                    .zip(near)
                    .flat_map(|((&h, &w), &near)| surface_texel(h, w, near)),
            );
        }
        texels
    }

    /// Mutable elevation samples, laid out as in [`heights`](Self::heights). After changing
    /// some, call [`refresh_region`](Self::refresh_region) for the part that changed. Changes
    /// made this way must not turn land into water or water into land; use
    /// [`set_heights`](Self::set_heights) for that.
    pub fn heights_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    /// Mutable water levels, laid out as in [`water`](Self::water); see
    /// [`heights_mut`](Self::heights_mut).
    pub fn water_mut(&mut self) -> &mut [f32] {
        &mut self.water
    }

    /// Rebuilds the chunks affected by a change to the samples in `region` right away, at the
    /// level of detail they would get with the camera at `cam`. Returns the samples whose
    /// [`surface_texels`](Self::surface_texels) may have changed, which reach a little beyond
    /// `region` where the beaches move.
    pub fn refresh_region(&mut self, region: Region, cam: Vec3) -> Region {
        self.version += 1;
        let texels = self.refresh_near_water(region);
        // A chunk's normals look as far as one stride (up to 8 cells) past its own samples.
        const REACH: usize = 8;
        let cx0 = region.i0.saturating_sub(REACH) / CHUNK_CELLS;
        let cz0 = region.j0.saturating_sub(REACH) / CHUNK_CELLS;
        let cx1 = ((region.i1 + REACH) / CHUNK_CELLS).min(self.chunks_x - 1);
        let cz1 = ((region.j1 + REACH) / CHUNK_CELLS).min(self.cells_z / CHUNK_CELLS - 1);
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let idx = cz * self.chunks_x + cx;
                self.stamps[idx] = self.stamps[idx].wrapping_add(1);
                let lod = lod_for(self.chunk_center(idx).distance(cam));
                self.set_chunk(idx, self.build_chunk(cx, cz, lod));
            }
        }
        texels
    }

    /// Works out again which samples near `region` have water close by, after its heights or
    /// water levels changed. Returns the samples that were looked at.
    fn refresh_near_water(&mut self, region: Region) -> Region {
        let (nx, nz) = self.samples();
        let reach = BEACH_REACH as usize;
        let span = Region {
            i0: region.i0.saturating_sub(reach),
            j0: region.j0.saturating_sub(reach),
            i1: (region.i1 + reach).min(nx - 1),
            j1: (region.j1 + reach).min(nz - 1),
        };
        let flooded = |i: usize, j: usize| self.heights[j * nx + i] < self.water[j * nx + i];
        let mut near = Vec::with_capacity(span.width() * span.height());
        for j in span.j0..=span.j1 {
            for i in span.i0..=span.i1 {
                let found = (j.saturating_sub(reach)..=(j + reach).min(nz - 1)).any(|nj| {
                    (i.saturating_sub(reach)..=(i + reach).min(nx - 1)).any(|ni| {
                        let (di, dj) = (ni.abs_diff(i) as f32, nj.abs_diff(j) as f32);
                        di * di + dj * dj <= BEACH_REACH * BEACH_REACH && flooded(ni, nj)
                    })
                });
                near.push(found);
            }
        }
        let mut k = 0;
        for j in span.j0..=span.j1 {
            for i in span.i0..=span.i1 {
                self.near_water[j * nx + i] = near[k];
                k += 1;
            }
        }
        span
    }

    /// Puts a freshly built chunk in place and marks its block for upload.
    fn set_chunk(&mut self, idx: usize, chunk: Chunk) {
        self.chunks[idx] = Some(chunk);
        let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
        self.blocks[(cz / BLOCK_CHUNKS) * self.blocks_x + cx / BLOCK_CHUNKS].dirty = true;
    }

    /// Highest point of the visible surface anywhere in the world.
    pub fn max_surface_height(&self) -> f32 {
        self.heights.iter().copied().fold(SEA_LEVEL, f32::max)
    }

    fn build_chunk(&self, cx: usize, cz: usize, lod: u8) -> Chunk {
        let stride = 1i32 << lod;
        let n = CHUNK_CELLS >> lod;
        let step = CELL_SIZE * stride as f32;
        let gi0 = (cx * CHUNK_CELLS) as i32;
        let gj0 = (cz * CHUNK_CELLS) as i32;
        let half = self.half_extents();
        let x0 = gi0 as f32 * CELL_SIZE - half.x;
        let z0 = gj0 as f32 * CELL_SIZE - half.y;
        let surf = |gi: i32, gj: i32| self.surface_sample(gi, gj);

        let row = n + 1;
        let mut vertices = Vec::with_capacity(row * row + 4 * row);
        let mut indices: Vec<u16> = Vec::with_capacity(n * n * 6 + 4 * n * 6);
        let (mut y_min, mut y_max) = (f32::MAX, f32::MIN);

        for j in 0..=n {
            for i in 0..=n {
                let gi = gi0 + i as i32 * stride;
                let gj = gj0 + j as i32 * stride;
                let x = x0 + i as f32 * step;
                let z = z0 + j as f32 * step;
                let y = self.surface_sample(gi, gj);

                // Only shape and lighting come from the mesh; whether a pixel is land or water,
                // and its color, come from the surface texture so coastlines stay sharp at
                // every level of detail.
                let normal = vec3(
                    surf(gi - stride, gj) - surf(gi + stride, gj),
                    2.0 * step,
                    surf(gi, gj - stride) - surf(gi, gj + stride),
                )
                .normalize();

                y_min = y_min.min(y);
                y_max = y_max.max(y);
                vertices.push(TerrainVertex {
                    position: [x, y, z],
                    normal: normal.to_array(),
                });
            }
        }

        for j in 0..n {
            for i in 0..n {
                let a = (j * row + i) as u16;
                let b = a + 1;
                let c = a + row as u16;
                let d = c + 1;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }

        // Skirts: a curtain hanging down from every edge hides cracks between chunks of different LOD.
        let skirt = step * 1.5 + 6.0;
        let edges: [Vec<usize>; 4] = [
            (0..=n).collect(),
            (0..=n).map(|i| n * row + i).collect(),
            (0..=n).map(|j| j * row).collect(),
            (0..=n).map(|j| j * row + n).collect(),
        ];
        for edge in &edges {
            let base = vertices.len();
            for &k in edge {
                let mut v = vertices[k];
                v.position[1] -= skirt;
                vertices.push(v);
            }
            for t in 0..n {
                let a = edge[t] as u16;
                let b = edge[t + 1] as u16;
                let sa = (base + t) as u16;
                let sb = sa + 1;
                indices.extend_from_slice(&[a, sa, b, b, sa, sb]);
            }
        }

        Chunk {
            lod,
            vertices,
            indices,
            y_min: y_min - skirt,
            y_max,
        }
    }

    /// Builds missing chunks and swaps LODs as the camera moves, spending at most `budget`.
    /// Missing chunks closest to the camera are built first; LOD swaps come after.
    pub fn update(&mut self, cam: Vec3, budget: Duration) {
        let start = Instant::now();

        let mut work: Vec<(f32, usize, u8)> = Vec::new();
        for (idx, chunk) in self.chunks.iter().enumerate() {
            let d = self.chunk_center(idx).distance(cam);
            let lod = lod_for(d);
            match chunk {
                Some(c) if c.lod == lod => {}
                Some(_) => work.push((d + 1e6, idx, lod)),
                None => work.push((d, idx, lod)),
            }
        }
        work.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (_, idx, lod) in work {
            if start.elapsed() > budget {
                break;
            }
            let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
            self.set_chunk(idx, self.build_chunk(cx, cz, lod));
        }
    }

    /// Center of a chunk at sea level, used to pick its LOD.
    fn chunk_center(&self, idx: usize) -> Vec3 {
        let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
        let half = self.half_extents();
        vec3(
            (cx as f32 + 0.5) * CHUNK_SIZE - half.x,
            SEA_LEVEL,
            (cz as f32 + 0.5) * CHUNK_SIZE - half.y,
        )
    }

    /// Uploads the blocks whose chunks changed since the last call, then returns the blocks
    /// within `max_distance` that intersect the view frustum, nearest first so the depth test
    /// can skip shading what they hide.
    pub fn visible_blocks(&mut self, ctx: &mut dyn RenderingBackend, frustum: &Frustum, cam: Vec3, max_distance: f32) -> Vec<BlockDraw> {
        for b in 0..self.blocks.len() {
            if self.blocks[b].dirty {
                self.upload_block(ctx, b);
            }
        }
        let mut visible: Vec<(f32, BlockDraw)> = self
            .blocks
            .iter()
            .filter_map(|block| {
                let mesh = block.mesh.as_ref()?;
                let distance = block.center.distance(cam) - block.radius;
                (distance < max_distance && frustum.sphere_visible(block.center, block.radius)).then(|| {
                    (distance, BlockDraw { vertices: mesh.vertices, indices: mesh.indices, count: mesh.count })
                })
            })
            .collect();
        visible.sort_by(|a, b| a.0.total_cmp(&b.0));
        visible.into_iter().map(|(_, draw)| draw).collect()
    }

    /// Joins the built chunks of block `b` into one mesh and uploads it in place of the old one.
    fn upload_block(&mut self, ctx: &mut dyn RenderingBackend, b: usize) {
        let (bx, bz) = (b % self.blocks_x, b / self.blocks_x);
        let chunks_z = self.chunks.len() / self.chunks_x;
        let (cx0, cz0) = (bx * BLOCK_CHUNKS, bz * BLOCK_CHUNKS);
        let (cx1, cz1) = ((cx0 + BLOCK_CHUNKS).min(self.chunks_x), (cz0 + BLOCK_CHUNKS).min(chunks_z));

        let mut vertices: Vec<TerrainVertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let (mut y_min, mut y_max) = (f32::MAX, f32::MIN);
        for cz in cz0..cz1 {
            for cx in cx0..cx1 {
                let Some(chunk) = &self.chunks[cz * self.chunks_x + cx] else { continue };
                let base = vertices.len() as u16;
                vertices.extend_from_slice(&chunk.vertices);
                indices.extend(chunk.indices.iter().map(|&i| i + base));
                y_min = y_min.min(chunk.y_min);
                y_max = y_max.max(chunk.y_max);
            }
        }

        let half = self.half_extents();
        let block = &mut self.blocks[b];
        block.dirty = false;
        if let Some(old) = block.mesh.take() {
            old.delete(ctx);
        }
        block.mesh = (!indices.is_empty()).then(|| GpuMesh {
            vertices: ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&vertices)),
            indices: ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&indices)),
            count: indices.len() as i32,
        });
        let min = vec3(cx0 as f32 * CHUNK_SIZE - half.x, y_min, cz0 as f32 * CHUNK_SIZE - half.y);
        let max = vec3(cx1 as f32 * CHUNK_SIZE - half.x, y_max, cz1 as f32 * CHUNK_SIZE - half.y);
        block.center = (min + max) * 0.5;
        block.radius = (max - min).length() * 0.5;
    }
}

impl Drop for World {
    /// Frees the blocks' GPU buffers. A world that was never drawn has none, and never touches
    /// the GPU.
    fn drop(&mut self) {
        if self.blocks.iter().all(|b| b.mesh.is_none()) {
            return;
        }
        let ctx = unsafe { get_internal_gl() }.quad_context;
        for block in &mut self.blocks {
            if let Some(mesh) = block.mesh.take() {
                mesh.delete(ctx);
            }
        }
    }
}

/// Elevations up to this many meters are told apart in the surface texture; higher ground
/// reads as this.
pub const MAX_SURFACE_ELEVATION: f32 = 50.0;

/// How far above the water the shader tells ground apart, in the alpha channel.
const MAX_ABOVE_WATER: f32 = 8.0;

/// How deep the water is told apart, in the alpha channel; deeper water reads as this. Must
/// match `WATER_DEPTH_RANGE` in the terrain shader.
const MAX_WATER_DEPTH: f32 = 64.0;

/// Lakes and rivers read as water no deeper than this, in meters, so they stay shallow water.
const MAX_INLAND_DEPTH: f32 = 4.0;

/// A texel for the terrain shader: the coast in red, elevation in green, whether water is near
/// in blue and the height above the water, or the depth of it, in alpha, as described on
/// [`World::surface_texels`].
/// `w` is the water level at the sample.
fn surface_texel(h: f32, w: f32, near_water: bool) -> [u8; 4] {
    let coast = ((0.5 + (h - w) * 0.5).clamp(0.0, 1.0) * 255.0).round();
    let elevation = (((h - SEA_LEVEL).max(0.0) / MAX_SURFACE_ELEVATION).min(1.0) * 255.0).round();
    // Depth is stored as a square root, which tells shallow depths apart finely and deep ones
    // coarsely.
    let above = if h >= w {
        ((h - w) / MAX_ABOVE_WATER).clamp(0.0, 1.0)
    } else if w <= SEA_LEVEL {
        ((w - h) / MAX_WATER_DEPTH).clamp(0.0, 1.0).sqrt()
    } else {
        ((w - h).min(MAX_INLAND_DEPTH) / MAX_WATER_DEPTH).clamp(0.0, 1.0).sqrt()
    };
    [coast as u8, elevation as u8, if near_water { 255 } else { 0 }, (above * 255.0).round() as u8]
}

/// Samples this close to water (in samples) can have beach; beaches are at most about 12 m
/// wide, three samples, so this leaves a sample to spare.
const BEACH_REACH: f32 = 4.0;

/// Whether each sample of an `nx` by `nz` heightmap lies within [`BEACH_REACH`] of water, which
/// fills the ground lower than its `level`.
fn near_water(heights: &[f32], level: &[f32], nx: usize, nz: usize) -> Vec<bool> {
    // The distance transform measures distance to "land", so mark the water as land.
    let water: Vec<f32> = heights.iter().zip(level).map(|(&h, &w)| if h < w { 0.0 } else { -1.0 }).collect();
    match ExistingLand::new(&water, nx, nz, 1.0) {
        Some(distance) => distance.dist.iter().map(|&d| d <= BEACH_REACH).collect(),
        None => vec![false; heights.len()],
    }
}

/// A rectangle of heightmap samples, both corners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub i0: usize,
    pub j0: usize,
    pub i1: usize,
    pub j1: usize,
}

impl Region {
    pub fn width(&self) -> usize {
        self.i1 - self.i0 + 1
    }

    pub fn height(&self) -> usize {
        self.j1 - self.j0 + 1
    }

    /// The smallest region holding both.
    pub fn union(self, other: Region) -> Region {
        Region {
            i0: self.i0.min(other.i0),
            j0: self.j0.min(other.j0),
            i1: self.i1.max(other.i1),
            j1: self.j1.max(other.j1),
        }
    }
}

fn lod_for(distance: f32) -> u8 {
    LOD_DISTANCES
        .iter()
        .position(|&d| distance < d)
        .unwrap_or(LOD_DISTANCES.len()) as u8
}

/// View frustum side and near planes, extracted from a view-projection matrix.
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    pub fn from_matrix(m: Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2]
            .map(|p| p / p.truncate().length());
        Self { planes }
    }

    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p.truncate().dot(center) + p.w >= -radius)
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[test]
    fn large_maps_snap_to_whole_chunks_instead_of_being_cut_to_a_smaller_cap() {
        assert_eq!(snap_map_size(10000.0), 9984.0);
        assert_eq!(snap_map_size(6000.0), 6016.0);
        assert_eq!(snap_map_size(20000.0), MAX_MAP_SIZE);
        assert!(cells_valid((9984.0 / CELL_SIZE) as usize));
    }

    #[test]
    fn a_10000_by_6000_world_builds_and_generates() {
        use crate::continents::{self, ContinentParams};
        let t = std::time::Instant::now();
        let world = World::new(snap_map_size(10000.0), snap_map_size(6000.0));
        let (nx, nz) = ((9984.0 / CELL_SIZE) as usize, (6016.0 / CELL_SIZE) as usize);
        let heights = continents::generate(&ContinentParams {
            coverage: 0.3, count: 4, roughness: 0.4, regularity: 0.4,
            edge_buffer: 0.05, seed: 7,
        }, nx, nz, &vec![DEEP_OCEAN; (nx + 1) * (nz + 1)]);
        assert_eq!(heights.len(), (nx + 1) * (nz + 1));
        assert!(world.size().x > 9900.0);
        eprintln!("10000x6000 built and generated in {:?}", t.elapsed());
    }
}
