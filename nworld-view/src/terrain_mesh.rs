//! The terrain as meshes the GPU can draw: the map cut into chunks of several levels of detail,
//! grouped into blocks that are uploaded and drawn together. The map knows nothing of this; the mesh
//! reads it, and rebuilds whatever chunks the map's stamps say have changed.

use std::time::{Duration, Instant};

use macroquad::miniquad::{BufferId, BufferSource, BufferType, BufferUsage, RenderingBackend};
use macroquad::prelude::*;

use crate::render::TerrainVertex;
use nworld_map::world::{World, CELL_SIZE, CHUNK_CELLS, CHUNK_SIZE, SEA_LEVEL};

/// Chunks along a side of a block, the group of chunks that is kept on the GPU and drawn together.
/// Bigger blocks mean fewer draw calls; smaller ones less to re-upload when a chunk changes. An
/// 8 x 8 block has at most 64 x 357 vertices, well inside 16-bit indices.
const BLOCK_CHUNKS: usize = 8;
/// Distance thresholds (world units) for LOD 0, 1 and 2; anything farther is LOD 3.
/// Generous enough that coastlines stay crisp in the top-down view of a 4 km map.
const LOD_DISTANCES: [f32; 3] = [500.0, 1300.0, 4000.0];

struct Chunk {
    lod: u8,
    /// The map's stamp for this chunk when the mesh was built.
    stamp: u32,
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

/// The terrain of one world as meshes.
pub struct TerrainMesh {
    chunks_x: usize,
    chunks: Vec<Option<Chunk>>,
    blocks_x: usize,
    blocks: Vec<Block>,
    half: Vec2,
}

impl TerrainMesh {
    /// An empty mesh for `world`, which [`update`](Self::update) fills in.
    pub fn new(world: &World) -> Self {
        let (chunks_x, chunks_z) = world.chunk_counts();
        let (blocks_x, blocks_z) = (chunks_x.div_ceil(BLOCK_CHUNKS), chunks_z.div_ceil(BLOCK_CHUNKS));
        Self {
            chunks_x,
            chunks: (0..chunks_x * chunks_z).map(|_| None).collect(),
            blocks_x,
            blocks: (0..blocks_x * blocks_z)
                .map(|_| Block { dirty: true, mesh: None, center: Vec3::ZERO, radius: 0.0 })
                .collect(),
            half: world.half_extents(),
        }
    }

    /// Builds missing and out-of-date chunks and swaps LODs as the camera moves, spending at
    /// most `budget`. Chunks the map has changed, and missing ones, closest to the camera are
    /// built first; LOD swaps come after.
    pub fn update(&mut self, world: &World, cam: Vec3, budget: Duration) {
        let start = Instant::now();

        let mut work: Vec<(f32, usize, u8)> = Vec::new();
        for (idx, chunk) in self.chunks.iter().enumerate() {
            let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
            let d = self.chunk_center(idx).distance(cam);
            let lod = lod_for(d);
            match chunk {
                Some(c) if c.stamp != world.terrain_stamp(cx, cz) => work.push((d, idx, lod)),
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
            self.set_chunk(idx, Self::build_chunk(world, cx, cz, lod));
        }
    }

    /// Puts a freshly built chunk in place and marks its block for upload.
    fn set_chunk(&mut self, idx: usize, chunk: Chunk) {
        self.chunks[idx] = Some(chunk);
        let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
        self.blocks[(cz / BLOCK_CHUNKS) * self.blocks_x + cx / BLOCK_CHUNKS].dirty = true;
    }

    /// Center of a chunk at sea level, used to pick its LOD.
    fn chunk_center(&self, idx: usize) -> Vec3 {
        let (cx, cz) = (idx % self.chunks_x, idx / self.chunks_x);
        vec3(
            (cx as f32 + 0.5) * CHUNK_SIZE - self.half.x,
            SEA_LEVEL,
            (cz as f32 + 0.5) * CHUNK_SIZE - self.half.y,
        )
    }

    fn build_chunk(world: &World, cx: usize, cz: usize, lod: u8) -> Chunk {
        let stride = 1i32 << lod;
        let n = CHUNK_CELLS >> lod;
        let step = CELL_SIZE * stride as f32;
        let gi0 = (cx * CHUNK_CELLS) as i32;
        let gj0 = (cz * CHUNK_CELLS) as i32;
        let half = world.half_extents();
        let x0 = gi0 as f32 * CELL_SIZE - half.x;
        let z0 = gj0 as f32 * CELL_SIZE - half.y;
        let surf = |gi: i32, gj: i32| world.surface_sample(gi, gj);

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
                let y = world.surface_sample(gi, gj);

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
            stamp: world.terrain_stamp(cx, cz),
            vertices,
            indices,
            y_min: y_min - skirt,
            y_max,
        }
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

        let half = self.half;
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

impl Drop for TerrainMesh {
    /// Frees the blocks' GPU buffers. A mesh that was never drawn has none, and never touches
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
