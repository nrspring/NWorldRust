//! Forests: a density layer painted with a brush, and the 3D trees drawn from it.
//!
//! The map only stores how dense the forest is at each sample, from 0 to 255. The trees
//! themselves are worked out from that and the terrain when they are needed, on a fixed grid of
//! spots across the world, so they stay where they are however the map is edited and cost
//! nothing in a saved map. A spot gets a tree with a chance equal to the density there, unless
//! the ground is wrong for one: water, beaches, steep slopes, the high rock above the treeline.
//!
//! Trees are drawn as low-poly meshes, thousands at a time with one draw call per kind of tree
//! (instancing), out to a few hundred meters. They thin out and shrink away with distance, and
//! beyond that the terrain shader paints the forest's canopy on the ground.

use std::collections::HashMap;

use macroquad::miniquad::{
    self, Bindings, BufferId, BufferLayout, BufferSource, BufferType, BufferUsage, Pipeline, RenderingBackend,
    ShaderMeta, UniformBlockLayout, UniformsSource, VertexAttribute, VertexFormat, VertexStep,
};
use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::world::{Region, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a tree can stand on.
const TREE_SPACING: f32 = 4.0;
/// Trees only stand on ground up to about this high, and below this slope (rise over run).
const TREELINE: f32 = 36.0;
const MAX_SLOPE: f32 = 0.7;
/// Beaches, and the ground this close above water near it, stay clear of trees.
const SHORE_CLEARANCE: f32 = 3.0;
/// Trees are drawn in the chunks within this many chunks of the camera's, and fade away between
/// these two distances in meters from it.
const DRAW_CHUNKS: i32 = 11;
const FADE: (f32, f32) = (260.0, 620.0);
/// Most chunks whose trees are worked out in one frame, so the first view of a big forest
/// doesn't stall.
const CHUNKS_PER_FRAME: usize = 48;
/// Most trees drawn at once.
const MAX_TREES: usize = 400_000;

// ---------------------------------------------------------------------------------- painting

/// How fast the brush works: a sample under it goes from its starting density to the painted
/// one in about this long, in seconds, with the brush held still.
const BRUSH_SECONDS: f32 = 0.35;
/// The brush is at full strength inside this share of its radius, then eases off to nothing.
const BRUSH_CORE: f32 = 0.55;

/// One press-drag-release of the forest brush: paints forest of a chosen density on dry land, or
/// with `erase` clears it.
pub struct ForestStroke {
    nx: usize,
    nz: usize,
    erase: bool,
    /// The density painted, from 0 to 255.
    target: f32,
    /// The densities when the stroke began.
    base: Vec<u8>,
    /// Which samples are dry land.
    land: Vec<bool>,
    /// How far each sample has come from its starting density toward the painted one, 0 to 1.
    progress: Vec<f32>,
    last: Option<Vec2>,
}

impl ForestStroke {
    /// Starts a stroke on `forest` (`nx` by `nz` samples) that paints `density` (0 to 1), or
    /// with `erase` clears forest.
    pub fn begin(forest: &[u8], heights: &[f32], water: &[f32], nx: usize, nz: usize, erase: bool, density: f32) -> Self {
        Self {
            nx,
            nz,
            erase,
            target: (density.clamp(0.0, 1.0) * 255.0).round(),
            base: forest.to_vec(),
            land: heights.iter().zip(water).map(|(&h, &w)| h >= w).collect(),
            progress: vec![0.0; forest.len()],
            last: None,
        }
    }

    /// Whether the stroke clears forest.
    pub fn erases(&self) -> bool {
        self.erase
    }

    /// The densities the stroke started from.
    pub fn base(&self) -> &[u8] {
        &self.base
    }

    /// Paints with a brush of `radius` meters at `center` (world x and z, with the map centered on
    /// the origin), along the line from wherever the last call painted, and returns the part of
    /// the map that changed, if any. `dt` is the seconds since the last call.
    pub fn paint(&mut self, forest: &mut [u8], half: Vec2, center: Vec2, radius: f32, dt: f32) -> Option<Region> {
        let from = self.last.unwrap_or(center);
        self.last = Some(center);

        let reach = |lo: f32, hi: f32, n: usize, offset: f32| {
            let first = ((lo - radius + offset) / CELL_SIZE).floor().max(0.0) as usize;
            let last = (((hi + radius + offset) / CELL_SIZE).ceil().max(0.0) as usize).min(n - 1);
            (first, last)
        };
        let (i0, i1) = reach(from.x.min(center.x), from.x.max(center.x), self.nx, half.x);
        let (j0, j1) = reach(from.y.min(center.y), from.y.max(center.y), self.nz, half.y);
        let step = dt / BRUSH_SECONDS;
        let target = if self.erase { 0.0 } else { self.target };

        let mut changed: Option<Region> = None;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let idx = j * self.nx + i;
                if !self.land[idx] {
                    continue;
                }
                let at = vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                let d = distance_to_segment(at, from, center);
                if d > radius {
                    continue;
                }
                let weight = 1.0 - smoothstep(BRUSH_CORE, 1.0, d / radius);
                let now = self.progress[idx];
                let next = now.max((now + step).min(weight));
                if next == now {
                    continue;
                }
                self.progress[idx] = next;
                let base = self.base[idx] as f32;
                let moved = base + (target - base) * smoothstep(0.0, 1.0, next);
                // Painting never undoes itself: denser forest stays as dense as it was where it
                // was denser than painted, and clearing never plants.
                let value = if self.erase { base.min(moved) } else { base.max(moved) };
                let value = value.round() as u8;
                if value != forest[idx] {
                    forest[idx] = value;
                    let sample = Region { i0: i, j0: j, i1: i, j1: j };
                    changed = Some(changed.map_or(sample, |c| c.union(sample)));
                }
            }
        }
        changed
    }
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

// ---------------------------------------------------------------------------------- trees

/// A tree to draw.
#[derive(Clone, Copy)]
struct Tree {
    pos: Vec3,
    /// Size relative to the mesh.
    scale: f32,
    yaw: f32,
    /// Varies the color a little from tree to tree, 0 to 1.
    tint: f32,
    /// A random number in 0..1 that decides which trees drop out first as distance grows.
    rank: f32,
    conifer: bool,
}

/// Four independent random numbers in 0..1 for the spot at grid position (`a`, `b`).
fn randoms(a: i32, b: i32) -> [f32; 4] {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA6B) ^ 0x2C1B_3C6D;
    let mut next = || {
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        h = h.wrapping_mul(0x297A_2D39);
        h ^= h >> 15;
        (h >> 8) as f32 / (1u32 << 24) as f32
    };
    [next(), next(), next(), next()]
}

/// The forest density at a world position, 0 to 1, between samples.
fn density_at(world: &World, x: f32, z: f32) -> f32 {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, (nx - 1) as f32);
    let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, (nz - 1) as f32);
    let (i, j) = (fx.floor() as usize, fz.floor() as usize);
    let (i1, j1) = ((i + 1).min(nx - 1), (j + 1).min(nz - 1));
    let (tx, tz) = (fx - i as f32, fz - j as f32);
    let f = world.forest();
    let at = |i: usize, j: usize| f[j * nx + i] as f32;
    let top = at(i, j) + (at(i1, j) - at(i, j)) * tx;
    let bottom = at(i, j1) + (at(i1, j1) - at(i, j1)) * tx;
    (top + (bottom - top) * tz) / 255.0
}

/// The trees of chunk (`cx`, `cz`).
fn trees_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Tree> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / TREE_SPACING) as i32;
    let mut trees = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            // Spots sit on a grid over the whole world, each jittered within its own cell.
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * TREE_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * TREE_SPACING - half.y;
            if accept >= density_at(world, x, z) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            let idx = j * nx + i;
            let (ground, level) = (heights[idx], water[idx]);
            // Not in water, and not on a beach or the bank of a river or lake.
            if ground < level || ground < 0.75 || (world.beach_nearby(idx) && ground - level < SHORE_CLEARANCE) {
                continue;
            }
            // Not up the high rock, and not up a cliff.
            let treeline = TREELINE + 4.0 * (variety - 0.5);
            let (i0, i1) = (i.saturating_sub(1), (i + 1).min(nx - 1));
            let (j0, j1) = (j.saturating_sub(1), (j + 1).min(nz - 1));
            let slope = vec2(
                (heights[j * nx + i1] - heights[j * nx + i0]) / ((i1 - i0) as f32 * CELL_SIZE),
                (heights[j1 * nx + i] - heights[j0 * nx + i]) / ((j1 - j0) as f32 * CELL_SIZE),
            )
            .length();
            if ground > treeline || slope > MAX_SLOPE {
                continue;
            }

            // Conifers take over higher up, and trees shrink toward the treeline.
            let conifer = variety < 0.15 + 0.85 * smoothstep(5.0, 26.0, ground);
            let size = (0.8 + 0.45 * randoms(gb, ga)[0]) * (1.0 - 0.4 * smoothstep(24.0, TREELINE, ground));
            trees.push(Tree {
                pos: vec3(x, world.surface_height(x, z), z),
                scale: size,
                yaw: variety * 97.0,
                tint: randoms(ga ^ 0x55, gb)[1],
                rank: randoms(ga, gb ^ 0x2B)[2],
                conifer,
            });
        }
    }
    trees
}

// ---------------------------------------------------------------------------------- meshes

#[repr(C)]
#[derive(Clone, Copy)]
struct TreeVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

/// Builds a mesh of flat-shaded triangles.
#[derive(Default)]
struct MeshBuilder {
    vertices: Vec<TreeVertex>,
}

impl MeshBuilder {
    /// A triangle of (position, color) corners, shaded to face away from `inside`.
    fn tri(&mut self, inside: Vec3, a: (Vec3, Vec3), b: (Vec3, Vec3), c: (Vec3, Vec3)) {
        let mut normal = (b.0 - a.0).cross(c.0 - a.0).normalize_or_zero();
        if normal.dot((a.0 + b.0 + c.0) / 3.0 - inside) < 0.0 {
            normal = -normal;
        }
        for (p, col) in [a, b, c] {
            self.vertices.push(TreeVertex { position: p.to_array(), normal: normal.to_array(), color: col.to_array() });
        }
    }

    /// A cone of `sides` sides from a ring of `radius` at height `base` to a point at `top`.
    fn cone(&mut self, base: f32, top: f32, radius: f32, sides: usize, dark: Vec3, light: Vec3) {
        let inside = vec3(0.0, (base + top) * 0.5, 0.0);
        let apex = (vec3(0.0, top, 0.0), light);
        let ring = |k: usize| {
            let a = std::f32::consts::TAU * k as f32 / sides as f32;
            (vec3(a.cos() * radius, base, a.sin() * radius), dark)
        };
        for k in 0..sides {
            self.tri(inside, apex, ring(k), ring(k + 1));
        }
    }

    /// A tapering trunk of `sides` sides from `y0` to `y1`.
    fn trunk(&mut self, y0: f32, y1: f32, radius: f32, sides: usize) {
        let inside = vec3(0.0, (y0 + y1) * 0.5, 0.0);
        let (low, high) = (vec3(0.20, 0.13, 0.08), vec3(0.30, 0.20, 0.12));
        let ring = |k: usize, y: f32, r: f32, col: Vec3| {
            let a = std::f32::consts::TAU * k as f32 / sides as f32;
            (vec3(a.cos() * r, y, a.sin() * r), col)
        };
        for k in 0..sides {
            let (a, b) = (ring(k, y0, radius, low), ring(k + 1, y0, radius, low));
            let (c, d) = (ring(k, y1, radius * 0.7, high), ring(k + 1, y1, radius * 0.7, high));
            self.tri(inside, a, b, c);
            self.tri(inside, b, d, c);
        }
    }

    /// A rough ball: an icosahedron stretched to `radii` around `center`, darker below.
    fn blob(&mut self, center: Vec3, radii: Vec3, dark: Vec3, light: Vec3) {
        const T: f32 = 1.618_034;
        let v = |x: f32, y: f32, z: f32| vec3(x, y, z).normalize();
        let corners = [
            v(-1.0, T, 0.0), v(1.0, T, 0.0), v(-1.0, -T, 0.0), v(1.0, -T, 0.0),
            v(0.0, -1.0, T), v(0.0, 1.0, T), v(0.0, -1.0, -T), v(0.0, 1.0, -T),
            v(T, 0.0, -1.0), v(T, 0.0, 1.0), v(-T, 0.0, -1.0), v(-T, 0.0, 1.0),
        ];
        const FACES: [[usize; 3]; 20] = [
            [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
            [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
            [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
            [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
        ];
        let corner = |k: usize| {
            let u = corners[k];
            (center + u * radii, dark.lerp(light, (u.y * 0.5 + 0.5).clamp(0.0, 1.0)))
        };
        for [a, b, c] in FACES {
            self.tri(center, corner(a), corner(b), corner(c));
        }
    }
}

fn conifer_mesh() -> Vec<TreeVertex> {
    let mut m = MeshBuilder::default();
    m.trunk(0.0, 2.0, 0.35, 6);
    let (dark, light) = (vec3(0.05, 0.17, 0.09), vec3(0.14, 0.33, 0.15));
    // Three tiers of foliage, each narrower than the one below.
    m.cone(1.4, 5.0, 2.7, 7, dark, light);
    m.cone(3.2, 7.8, 2.1, 7, dark, light);
    m.cone(5.5, 10.5, 1.4, 7, dark, light);
    m.vertices
}

fn broadleaf_mesh() -> Vec<TreeVertex> {
    let mut m = MeshBuilder::default();
    m.trunk(0.0, 3.4, 0.42, 6);
    let (dark, light) = (vec3(0.07, 0.22, 0.08), vec3(0.22, 0.42, 0.14));
    m.blob(vec3(0.0, 5.4, 0.0), vec3(3.4, 2.8, 3.4), dark, light);
    m.blob(vec3(1.4, 4.3, 0.9), vec3(2.2, 1.9, 2.2), dark, light);
    m.blob(vec3(-1.3, 4.6, -0.9), vec3(2.0, 1.8, 2.0), dark, light);
    m.vertices
}

// ---------------------------------------------------------------------------------- drawing

/// One tree as the GPU reads it: where it stands and how big, then how it is turned, tinted and
/// ranked.
#[repr(C)]
#[derive(Clone, Copy)]
struct Instance {
    at: [f32; 4],
    look: [f32; 4],
}

#[repr(C)]
struct TreeUniforms {
    view_projection: Mat4,
    cam_pos: Vec3,
    sun_dir: Vec3,
    sun_color: Vec3,
    ambient: Vec3,
    fog_color: Vec3,
    fog_range: Vec2,
    time: f32,
    fade: Vec2,
}

struct GpuMesh {
    vertices: BufferId,
    count: i32,
}

/// The instance data of one kind of tree on the GPU.
struct GpuInstances {
    buffer: Option<BufferId>,
    capacity: usize,
    count: usize,
}

/// Draws the trees of a world.
pub struct TreeRenderer {
    pipeline: Pipeline,
    meshes: [GpuMesh; 2],
    indices: BufferId,
    instances: [GpuInstances; 2],
    /// Each chunk's trees, and the state of the chunk when they were worked out.
    cache: HashMap<(u64, usize), (u32, Vec<Tree>)>,
    /// The world and chunks the instance buffers were last filled for.
    filled: Option<(u64, i32, i32)>,
    /// Some chunk's trees changed since the instance buffers were filled.
    stale: bool,
}

impl TreeRenderer {
    pub fn new() -> Self {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let shader = ctx
            .new_shader(
                miniquad::ShaderSource::Glsl { vertex: TREE_VERTEX, fragment: TREE_FRAGMENT },
                ShaderMeta { uniforms: uniform_layout(), images: vec![] },
            )
            .expect("tree shader failed to compile");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default(), BufferLayout { step_func: VertexStep::PerInstance, ..Default::default() }],
            &[
                VertexAttribute::with_buffer("position", VertexFormat::Float3, 0),
                VertexAttribute::with_buffer("normal", VertexFormat::Float3, 0),
                VertexAttribute::with_buffer("color", VertexFormat::Float3, 0),
                VertexAttribute::with_buffer("inst_at", VertexFormat::Float4, 1),
                VertexAttribute::with_buffer("inst_look", VertexFormat::Float4, 1),
            ],
            shader,
            crate::render::opaque_pipeline(),
        );
        let (conifer, broadleaf) = (conifer_mesh(), broadleaf_mesh());
        // The meshes are lists of separate triangles, so the indices just count up.
        let longest = conifer.len().max(broadleaf.len());
        let counting: Vec<u16> = (0..longest as u16).collect();
        let upload = |ctx: &mut dyn RenderingBackend, mesh: &[TreeVertex]| GpuMesh {
            vertices: ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(mesh)),
            count: mesh.len() as i32,
        };
        let meshes = [upload(ctx, &conifer), upload(ctx, &broadleaf)];
        let indices = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&counting));
        Self {
            pipeline,
            meshes,
            indices,
            instances: [
                GpuInstances { buffer: None, capacity: 0, count: 0 },
                GpuInstances { buffer: None, capacity: 0, count: 0 },
            ],
            cache: HashMap::new(),
            filled: None,
            stale: true,
        }
    }

    /// Draws the trees of `world` around the camera at `cam`, as seen by `camera`.
    pub fn draw(&mut self, world: &World, camera: &Camera3D, cam: Vec3, view_distance: f32, time: f32) {
        self.refresh(world, cam);
        if self.instances.iter().all(|i| i.count == 0) {
            return;
        }

        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx: &mut dyn RenderingBackend = gl.quad_context;
        let uniforms = TreeUniforms {
            view_projection: camera.matrix(),
            cam_pos: cam,
            sun_dir: crate::render::sun_direction(),
            sun_color: crate::render::SUN_COLOR,
            ambient: crate::render::AMBIENT,
            fog_color: crate::render::BACKGROUND,
            fog_range: crate::render::fog_range(view_distance),
            time,
            fade: vec2(FADE.0, FADE.1),
        };
        let (width, height) = miniquad::window::screen_size();
        ctx.begin_default_pass(miniquad::PassAction::Nothing);
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_viewport(0, 0, width as i32, height as i32);
        ctx.apply_scissor_rect(0, 0, width as i32, height as i32);
        for (mesh, instances) in self.meshes.iter().zip(&self.instances) {
            let Some(buffer) = instances.buffer.filter(|_| instances.count > 0) else {
                continue;
            };
            ctx.apply_bindings(&Bindings {
                vertex_buffers: vec![mesh.vertices, buffer],
                index_buffer: self.indices,
                images: vec![],
            });
            ctx.apply_uniforms(UniformsSource::table(&uniforms));
            ctx.draw(0, mesh.count, instances.count as i32);
        }
        ctx.end_render_pass();
    }

    /// Brings the trees around `cam` up to date, and refills the instance buffers if anything
    /// changed.
    fn refresh(&mut self, world: &World, cam: Vec3) {
        let (id, _) = world.revision();
        let (chunks_x, chunks_z) = world.chunk_counts();
        let half = world.half_extents();
        let (ccx, ccz) = (
            (((cam.x + half.x) / CHUNK_SIZE).floor() as i32).clamp(0, chunks_x as i32 - 1),
            (((cam.z + half.y) / CHUNK_SIZE).floor() as i32).clamp(0, chunks_z as i32 - 1),
        );
        if self.filled.is_some_and(|(world_id, _, _)| world_id != id) {
            self.cache.clear();
            self.stale = true;
        }

        // The chunks to draw, nearest first.
        let mut wanted: Vec<(i32, usize, usize)> = Vec::new();
        for dz in -DRAW_CHUNKS..=DRAW_CHUNKS {
            for dx in -DRAW_CHUNKS..=DRAW_CHUNKS {
                let (cx, cz) = (ccx + dx, ccz + dz);
                if dx * dx + dz * dz <= DRAW_CHUNKS * DRAW_CHUNKS
                    && (0..chunks_x as i32).contains(&cx)
                    && (0..chunks_z as i32).contains(&cz)
                {
                    wanted.push((dx * dx + dz * dz, cx as usize, cz as usize));
                }
            }
        }
        wanted.sort_unstable();

        let mut made = 0;
        for &(_, cx, cz) in &wanted {
            let stamp = world.chunk_stamp(cx, cz);
            let key = (id, cz * chunks_x + cx);
            if self.cache.get(&key).is_some_and(|(s, _)| *s == stamp) {
                continue;
            }
            if made == CHUNKS_PER_FRAME {
                // The rest come next frame.
                self.stale = true;
                break;
            }
            self.cache.insert(key, (stamp, trees_in_chunk(world, cx, cz)));
            made += 1;
            self.stale = true;
        }
        if self.filled != Some((id, ccx, ccz)) {
            self.stale = true;
        }
        if !self.stale {
            return;
        }
        self.stale = made == CHUNKS_PER_FRAME;
        self.filled = Some((id, ccx, ccz));

        let mut conifers: Vec<Instance> = Vec::new();
        let mut broadleaves: Vec<Instance> = Vec::new();
        for &(_, cx, cz) in &wanted {
            let Some((_, trees)) = self.cache.get(&(id, cz * chunks_x + cx)) else {
                continue;
            };
            for t in trees {
                let instance = Instance {
                    at: [t.pos.x, t.pos.y, t.pos.z, t.scale],
                    look: [t.yaw, t.tint, t.rank, 0.0],
                };
                if t.conifer { conifers.push(instance) } else { broadleaves.push(instance) }
            }
        }
        conifers.truncate(MAX_TREES / 2);
        broadleaves.truncate(MAX_TREES / 2);
        // Forget chunks that are no longer around the camera.
        if self.cache.len() > wanted.len() * 3 {
            let keep: std::collections::HashSet<usize> = wanted.iter().map(|&(_, cx, cz)| cz * chunks_x + cx).collect();
            self.cache.retain(|(world_id, idx), _| *world_id == id && keep.contains(idx));
        }

        let ctx = unsafe { get_internal_gl() }.quad_context;
        for (gpu, data) in self.instances.iter_mut().zip([&conifers, &broadleaves]) {
            gpu.count = data.len();
            if data.is_empty() {
                continue;
            }
            if gpu.capacity < data.len() {
                if let Some(old) = gpu.buffer.take() {
                    ctx.delete_buffer(old);
                }
                gpu.capacity = (data.len() * 3 / 2).max(4096);
                gpu.buffer = Some(ctx.new_buffer(
                    BufferType::VertexBuffer,
                    BufferUsage::Stream,
                    BufferSource::empty::<Instance>(gpu.capacity),
                ));
            }
            ctx.buffer_update(gpu.buffer.unwrap(), BufferSource::slice(data));
        }
    }
}

fn uniform_layout() -> UniformBlockLayout {
    use miniquad::UniformDesc as U;
    use miniquad::UniformType::*;
    UniformBlockLayout {
        uniforms: vec![
            U::new("view_projection", Mat4),
            U::new("cam_pos", Float3),
            U::new("sun_dir", Float3),
            U::new("sun_color", Float3),
            U::new("ambient", Float3),
            U::new("fog_color", Float3),
            U::new("fog_range", Float2),
            U::new("time", Float1),
            U::new("fade", Float2),
        ],
    }
}

const TREE_VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec3 normal;
attribute vec3 color;
// Where the tree stands and how big it is; then its turn, its tint and its rank.
attribute vec4 inst_at;
attribute vec4 inst_look;

varying vec3 v_color;
varying vec3 v_normal;
varying vec3 v_world;

uniform mat4 view_projection;
uniform vec3 cam_pos;
uniform float time;
uniform vec2 fade;

void main() {
    // Trees thin out with distance: the lower a tree's rank, the farther it stays. One that
    // drops out first shrinks away, so trees don't pop.
    float dist = length(inst_at.xyz - cam_pos);
    float keep = 1.0 - smoothstep(fade.x, fade.y, dist);
    float size = inst_at.w * clamp((keep - inst_look.z) * 8.0, 0.0, 1.0);

    float c = cos(inst_look.x);
    float s = sin(inst_look.x);
    vec3 p = position * size;
    vec3 turned = vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
    // The crown sways in the wind, more the higher up.
    float sway = position.y * position.y * 0.004 * size;
    turned.x += sin(time * 1.3 + inst_at.x * 0.21 + inst_at.z * 0.13) * sway;
    turned.z += sin(time * 1.1 + inst_at.z * 0.19 - inst_at.x * 0.11) * sway * 0.7;

    // Set a little into the ground so no trunk floats on a slope.
    v_world = inst_at.xyz + turned - vec3(0.0, 0.4 * size, 0.0);
    v_normal = vec3(c * normal.x + s * normal.z, normal.y, -s * normal.x + c * normal.z);
    v_color = color * (0.82 + 0.36 * inst_look.y);
    gl_Position = view_projection * vec4(v_world, 1.0);
}
"#;

const TREE_FRAGMENT: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec3 v_color;
varying vec3 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;
uniform vec3 fog_color;
uniform vec2 fog_range;

void main() {
    vec3 n = normalize(v_normal);
    float diffuse = max(dot(n, normalize(sun_dir)), 0.0);
    vec3 col = v_color * (ambient + sun_color * diffuse);
    float fog = pow(smoothstep(fog_range.x, fog_range.y, length(v_world - cam_pos)), 0.6);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 81;

    fn half() -> Vec2 {
        Vec2::splat((N - 1) as f32 * CELL_SIZE * 0.5)
    }

    fn paint(stroke: &mut ForestStroke, forest: &mut [u8], at: Vec2, radius: f32, seconds: f32) {
        for _ in 0..20 {
            stroke.paint(forest, half(), at, radius, seconds / 20.0);
        }
    }

    /// Flat land at 10 m everywhere.
    fn land() -> (Vec<f32>, Vec<f32>) {
        (vec![10.0; N * N], vec![0.0; N * N])
    }

    #[test]
    fn the_brush_paints_forest_with_soft_edges_and_clears_it() {
        let (h, w) = land();
        let mut forest = vec![0u8; N * N];
        let mut stroke = ForestStroke::begin(&forest, &h, &w, N, N, false, 0.8);
        paint(&mut stroke, &mut forest, Vec2::ZERO, 80.0, 2.0);
        let mid = N / 2;
        assert_eq!(forest[mid * N + mid], 204, "80% density");
        let rim = forest[mid * N + mid + 18];
        assert!(rim > 0 && rim < 100, "rim is {rim}");
        assert_eq!(forest[mid * N + mid + 30], 0);

        let mut clear = ForestStroke::begin(&forest, &h, &w, N, N, true, 0.8);
        assert!(clear.erases());
        paint(&mut clear, &mut forest, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(forest[mid * N + mid], 0);
    }

    #[test]
    fn forest_only_grows_on_dry_land_and_never_thins_denser_forest() {
        let (mut h, mut w) = land();
        let mid = N / 2;
        // A pond.
        h[mid * N + mid] = 2.0;
        w[mid * N + mid] = 5.0;
        let mut forest = vec![0u8; N * N];
        forest[mid * N + mid + 1] = 255;
        let mut stroke = ForestStroke::begin(&forest, &h, &w, N, N, false, 0.5);
        paint(&mut stroke, &mut forest, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(forest[mid * N + mid], 0, "no forest under water");
        assert_eq!(forest[mid * N + mid + 1], 255, "denser forest stays");
        assert_eq!(forest[(mid + 1) * N + mid], 128);
    }

    #[test]
    fn trees_stand_where_the_forest_is_and_nowhere_else() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        // Forest in the west half.
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.forest_mut()[j * nx + i] = 255;
            }
        }
        let mut all = Vec::new();
        for cz in 0..4 {
            for cx in 0..4 {
                all.extend(trees_in_chunk(&world, cx, cz));
            }
        }
        // A full forest has a tree for most spots, about one per 16 square meters.
        assert!(all.len() > 800, "{} trees", all.len());
        assert!(all.iter().all(|t| t.pos.x < 0.0 + CELL_SIZE), "a tree outside the forest");
        assert!(all.iter().all(|t| (t.pos.y - 10.0).abs() < 1e-3));
        // The same world gives the same trees.
        assert_eq!(all.len(), (0..16).map(|k| trees_in_chunk(&world, k % 4, k / 4).len()).sum::<usize>());
    }

    #[test]
    fn trees_keep_off_water_beaches_cliffs_and_the_high_rock() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        let mut water = vec![0.0; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                match i {
                    0..=15 => heights[j * nx + i] = -20.0,
                    // A cliff.
                    16..=19 => heights[j * nx + i] = 10.0 + (i - 16) as f32 * 8.0,
                    // High ground well above the treeline.
                    40..=64 => heights[j * nx + i] = 45.0,
                    _ => {}
                }
            }
        }
        world.set_terrain(std::mem::take(&mut heights), std::mem::take(&mut water));
        world.forest_mut().iter_mut().for_each(|d| *d = 255);
        let mut all = Vec::new();
        for cz in 0..4 {
            for cx in 0..4 {
                all.extend(trees_in_chunk(&world, cx, cz));
            }
        }
        let half_x = world.half_extents().x;
        let column = |t: &Tree| ((t.pos.x + half_x) / CELL_SIZE) as usize;
        assert!(!all.is_empty());
        assert!(all.iter().all(|t| column(t) > 19), "a tree in the sea or on the cliff");
        assert!(all.iter().all(|t| !(40..=64).contains(&column(t))), "a tree on the high rock");
    }

    #[test]
    fn tree_meshes_are_small_and_stand_on_the_ground() {
        for mesh in [conifer_mesh(), broadleaf_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest.abs() < 0.01, "the trunk starts at {lowest}");
            assert!((7.0..14.0).contains(&highest), "a tree {highest} m tall");
        }
    }
}
