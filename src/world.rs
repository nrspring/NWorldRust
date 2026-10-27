//! The editable world: a finite heightmap rendered as a grid of level-of-detail chunks.

use std::time::{Duration, Instant};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

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
}

/// Heightmap cells along a chunk side at the finest LOD; each coarser LOD halves the vertex count.
const CHUNK_CELLS: usize = 16;
/// Map sides are whole numbers of chunks.
pub const CHUNK_SIZE: f32 = CELL_SIZE * CHUNK_CELLS as f32;
pub const MIN_MAP_SIZE: f32 = 256.0;
pub const MAX_MAP_SIZE: f32 = 8192.0;
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
    mesh: Mesh,
    center: Vec3,
    radius: f32,
}

pub struct World {
    /// Heightmap cells along x (west to east) and z (north to south).
    cells_x: usize,
    cells_z: usize,
    /// (cells_x + 1) × (cells_z + 1) elevation samples, row-major from the north-west corner.
    heights: Vec<f32>,
    chunks_x: usize,
    chunks: Vec<Option<Chunk>>,
}

impl World {
    /// A new world of the given size in meters (snapped with [`snap_map_size`]): deep ocean
    /// everywhere, centered on the origin.
    pub fn new(width: f32, height: f32) -> Self {
        let cells = |meters: f32| (snap_map_size(meters) / CELL_SIZE) as usize;
        let (cells_x, cells_z) = (cells(width), cells(height));
        let (chunks_x, chunks_z) = (cells_x / CHUNK_CELLS, cells_z / CHUNK_CELLS);
        Self {
            cells_x,
            cells_z,
            heights: vec![DEEP_OCEAN; (cells_x + 1) * (cells_z + 1)],
            chunks_x,
            chunks: (0..chunks_x * chunks_z).map(|_| None).collect(),
        }
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

    /// Replaces every elevation sample (laid out as in `heights`) and drops the built chunks
    /// so they are rebuilt from the new terrain.
    pub fn set_heights(&mut self, heights: Vec<f32>) {
        assert_eq!(heights.len(), self.heights.len(), "heightmap size mismatch");
        self.heights = heights;
        self.chunks.iter_mut().for_each(|c| *c = None);
    }

    /// Elevation at a heightmap sample; coordinates outside the world are clamped to its edge.
    fn sample(&self, gi: i32, gj: i32) -> f32 {
        let i = gi.clamp(0, self.cells_x as i32) as usize;
        let j = gj.clamp(0, self.cells_z as i32) as usize;
        self.heights[j * (self.cells_x + 1) + i]
    }

    /// Height of the visible surface (land or water) at a world position.
    pub fn surface_height(&self, x: f32, z: f32) -> f32 {
        self.elevation(x, z).max(SEA_LEVEL)
    }

    /// Elevation of the ground at a world position in meters: negative under water, where it is
    /// how deep the seabed lies.
    pub fn elevation(&self, x: f32, z: f32) -> f32 {
        let half = self.half_extents();
        let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, self.cells_x as f32);
        let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, self.cells_z as f32);
        let (i, j) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let top = self.sample(i, j) + (self.sample(i + 1, j) - self.sample(i, j)) * tx;
        let bottom = self.sample(i, j + 1) + (self.sample(i + 1, j + 1) - self.sample(i, j + 1)) * tx;
        top + (bottom - top) * tz
    }

    /// Heightmap samples along x and z: one more than the cells.
    pub fn samples(&self) -> (usize, usize) {
        (self.cells_x + 1, self.cells_z + 1)
    }

    /// The heightmap as RGBA texels for the terrain shader, one per sample in the same order.
    /// Red is height around sea level, from 0 (1 m or more under water) through 128 (sea
    /// level) to 255 (1 m or more above); the shader traces the coast where it crosses the
    /// middle, which follows a smooth curve between samples. Green encodes
    /// elevation from 0 to 50 m, which the shader colors: 25 shades of green up to 25 m, grey
    /// rock from 26 m and snow from 40 m.
    pub fn surface_texels(&self) -> Vec<u8> {
        self.heights.iter().flat_map(|&h| surface_texel(h)).collect()
    }

    /// The part of [`surface_texels`](Self::surface_texels) inside `region`, row by row.
    pub fn surface_texels_in(&self, region: Region) -> Vec<u8> {
        let nx = self.cells_x + 1;
        let mut texels = Vec::with_capacity(region.width() * region.height() * 4);
        for j in region.j0..=region.j1 {
            let row = &self.heights[j * nx + region.i0..=j * nx + region.i1];
            texels.extend(row.iter().flat_map(|&h| surface_texel(h)));
        }
        texels
    }

    /// Mutable elevation samples, laid out as in [`heights`](Self::heights). After changing
    /// some, call [`refresh_region`](Self::refresh_region) for the part that changed.
    pub fn heights_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    /// Rebuilds the chunks affected by a change to the samples in `region` right away, at the
    /// level of detail they would get with the camera at `cam`.
    pub fn refresh_region(&mut self, region: Region, cam: Vec3) {
        // A chunk's normals look as far as one stride (up to 8 cells) past its own samples.
        const REACH: usize = 8;
        let cx0 = region.i0.saturating_sub(REACH) / CHUNK_CELLS;
        let cz0 = region.j0.saturating_sub(REACH) / CHUNK_CELLS;
        let cx1 = ((region.i1 + REACH) / CHUNK_CELLS).min(self.chunks_x - 1);
        let cz1 = ((region.j1 + REACH) / CHUNK_CELLS).min(self.cells_z / CHUNK_CELLS - 1);
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let idx = cz * self.chunks_x + cx;
                let lod = lod_for(self.chunk_center(idx).distance(cam));
                self.chunks[idx] = Some(self.build_chunk(cx, cz, lod));
            }
        }
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
        let surf = |gi: i32, gj: i32| self.sample(gi, gj).max(SEA_LEVEL);

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
                let r = self.sample(gi, gj);
                let y = r.max(SEA_LEVEL);

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
                vertices.push(Vertex {
                    position: vec3(x, y, z),
                    uv: Vec2::ZERO,
                    color: [255; 4],
                    normal: normal.extend(0.0),
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
                let mut v = vertices[k].clone();
                v.position.y -= skirt;
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

        let half = CHUNK_SIZE * 0.5;
        let half_h = (y_max - (y_min - skirt)) * 0.5;
        Chunk {
            lod,
            mesh: Mesh {
                vertices,
                indices,
                texture: None,
            },
            center: vec3(x0 + half, y_min - skirt + half_h, z0 + half),
            radius: (half * half * 2.0 + half_h * half_h).sqrt(),
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
            self.chunks[idx] = Some(self.build_chunk(cx, cz, lod));
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

    /// Draws every chunk within `max_distance` that intersects the view frustum.
    pub fn draw(&self, frustum: &Frustum, cam: Vec3, max_distance: f32) {
        for chunk in self.chunks.iter().flatten() {
            if chunk.center.distance(cam) - chunk.radius < max_distance
                && frustum.sphere_visible(chunk.center, chunk.radius)
            {
                draw_mesh(&chunk.mesh);
            }
        }
    }
}

/// Elevations up to this many meters are told apart in the surface texture; higher ground
/// reads as this.
pub const MAX_SURFACE_ELEVATION: f32 = 50.0;

/// A texel for the terrain shader: the coast in red and elevation in green, as described on
/// [`World::surface_texels`].
fn surface_texel(h: f32) -> [u8; 4] {
    let coast = ((0.5 + (h - SEA_LEVEL) * 0.5).clamp(0.0, 1.0) * 255.0).round();
    let elevation = (((h - SEA_LEVEL).max(0.0) / MAX_SURFACE_ELEVATION).min(1.0) * 255.0).round();
    [coast as u8, elevation as u8, 0, 255]
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
