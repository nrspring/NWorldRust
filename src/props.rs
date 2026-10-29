//! Resources painted onto the map, like forest, stone, iron and copper: a density layer painted with a brush,
//! and the 3D props drawn from it.
//!
//! The map only stores how dense each resource is at each sample, from 0 to 255. The props
//! themselves, trees, rocks or ore, are worked out from that and the terrain when they are needed, on
//! a fixed grid of spots across the world, so they stay where they are however the map is edited
//! and cost nothing in a saved map. Each kind of resource says where its props may stand.
//!
//! Props are drawn as low-poly meshes, thousands at a time with one draw call per shape
//! (instancing), out to a few hundred meters. They thin out and shrink away with distance, and
//! beyond that the terrain shader tints the ground to show the resource.

use std::collections::HashMap;

use macroquad::miniquad::{
    self, Bindings, BufferId, BufferLayout, BufferSource, BufferType, BufferUsage, Pipeline, RenderingBackend,
    ShaderMeta, UniformBlockLayout, UniformsSource, VertexAttribute, VertexFormat, VertexStep,
};
use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::world::{Region, Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Most chunks whose props are worked out in one frame, so the first view of a big forest
/// doesn't stall.
const CHUNKS_PER_FRAME: usize = 48;
/// Most props of one kind drawn at once.
const MAX_PROPS: usize = 400_000;

// ---------------------------------------------------------------------------------- painting

/// How fast the brush works: a sample under it goes from its starting density to the painted
/// one in about this long, in seconds, with the brush held still.
const BRUSH_SECONDS: f32 = 0.35;
/// The brush is at full strength inside this share of its radius, then eases off to nothing.
const BRUSH_CORE: f32 = 0.55;

/// One press-drag-release of a resource brush: paints a resource of a chosen density on dry
/// land, or with `erase` clears it.
pub struct DensityStroke {
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

impl DensityStroke {
    /// Starts a stroke on `layer` (`nx` by `nz` samples) that paints `density` (0 to 1), or
    /// with `erase` clears it.
    pub fn begin(layer: &[u8], heights: &[f32], water: &[f32], nx: usize, nz: usize, erase: bool, density: f32) -> Self {
        Self {
            nx,
            nz,
            erase,
            target: (density.clamp(0.0, 1.0) * 255.0).round(),
            base: layer.to_vec(),
            land: heights.iter().zip(water).map(|(&h, &w)| h >= w).collect(),
            progress: vec![0.0; layer.len()],
            last: None,
        }
    }

    /// Whether the stroke clears the resource.
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
    pub fn paint(&mut self, layer: &mut [u8], half: Vec2, center: Vec2, radius: f32, dt: f32) -> Option<Region> {
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
                // Painting never undoes itself: denser ground stays as dense as it was where it
                // was denser than painted, and clearing never adds.
                let value = if self.erase { base.min(moved) } else { base.max(moved) };
                let value = value.round() as u8;
                if value != layer[idx] {
                    layer[idx] = value;
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

// ---------------------------------------------------------------------------------- props

/// A prop to draw.
#[derive(Clone, Copy)]
pub struct Prop {
    pub pos: Vec3,
    /// Size relative to the mesh.
    pub scale: f32,
    /// How much taller than wide, relative to the mesh: 1 leaves the shape as it is.
    pub aspect: f32,
    pub yaw: f32,
    /// Varies the color a little from prop to prop, 0 to 1.
    pub tint: f32,
    /// A random number in 0..1 that decides which props drop out first as distance grows.
    pub rank: f32,
    /// Which of the kind's meshes this is.
    pub shape: usize,
}

/// Four independent random numbers in 0..1 for the spot at grid position (`a`, `b`).
pub fn randoms(a: i32, b: i32) -> [f32; 4] {
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

/// Smooth random value noise in 0..1 that varies over about one unit of `x` and `z`; `seed`
/// picks a different pattern.
pub fn value_noise(x: f32, z: f32, seed: i32) -> f32 {
    let (i, j) = (x.floor(), z.floor());
    let (tx, tz) = (x - i, z - j);
    let (tx, tz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let at = |di: i32, dj: i32| randoms(i as i32 + di + seed, j as i32 + dj - seed)[0];
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * tx;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * tx;
    top + (bottom - top) * tz
}

/// The density of `kind` at a world position, 0 to 1, between samples.
pub fn density_at(world: &World, kind: Resource, x: f32, z: f32) -> f32 {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, (nx - 1) as f32);
    let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, (nz - 1) as f32);
    let (i, j) = (fx.floor() as usize, fz.floor() as usize);
    let (i1, j1) = ((i + 1).min(nx - 1), (j + 1).min(nz - 1));
    let (tx, tz) = (fx - i as f32, fz - j as f32);
    let layer = world.resource(kind);
    let at = |i: usize, j: usize| layer[j * nx + i] as f32;
    let top = at(i, j) + (at(i1, j) - at(i, j)) * tx;
    let bottom = at(i, j1) + (at(i1, j1) - at(i, j1)) * tx;
    (top + (bottom - top) * tz) / 255.0
}

// ---------------------------------------------------------------------------------- meshes

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PropVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// Builds a mesh of flat-shaded triangles.
#[derive(Default)]
pub struct MeshBuilder {
    pub vertices: Vec<PropVertex>,
}

impl MeshBuilder {
    /// A triangle of (position, color) corners, shaded to face away from `inside`.
    pub fn tri(&mut self, inside: Vec3, a: (Vec3, Vec3), b: (Vec3, Vec3), c: (Vec3, Vec3)) {
        let mut normal = (b.0 - a.0).cross(c.0 - a.0).normalize_or_zero();
        if normal.dot((a.0 + b.0 + c.0) / 3.0 - inside) < 0.0 {
            normal = -normal;
        }
        for (p, col) in [a, b, c] {
            self.vertices.push(PropVertex { position: p.to_array(), normal: normal.to_array(), color: col.to_array() });
        }
    }

    /// A cone of `sides` sides from a ring of `radius` at height `base` to a point at `top`.
    pub fn cone(&mut self, base: f32, top: f32, radius: f32, sides: usize, dark: Vec3, light: Vec3) {
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

    /// A pyramid of `sides` sides from a ring of `radius` around `base`, perpendicular to the way
    /// it points, to a point at `tip`: a crystal or a thorn.
    pub fn spike(&mut self, base: Vec3, tip: Vec3, radius: f32, sides: usize, dark: Vec3, light: Vec3) {
        let axis = (tip - base).normalize_or_zero();
        let across = axis.cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or_zero();
        let along = axis.cross(across);
        let inside = (base + tip) * 0.5;
        let apex = (tip, light);
        let ring = |k: usize| {
            let a = std::f32::consts::TAU * k as f32 / sides as f32;
            (base + (across * a.cos() + along * a.sin()) * radius, dark)
        };
        for k in 0..sides {
            self.tri(inside, apex, ring(k), ring(k + 1));
        }
    }

    /// A tapering trunk of `sides` sides from `y0` to `y1`.
    pub fn trunk(&mut self, y0: f32, y1: f32, radius: f32, sides: usize) {
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

    /// A rough ball: an icosahedron stretched to `radii` around `center`, darker below. `rough`
    /// (0 for a regular ball) pushes each corner in or out by up to that share of its radius,
    /// differently for each `seed`.
    pub fn blob(&mut self, center: Vec3, radii: Vec3, dark: Vec3, light: Vec3, rough: f32, seed: i32) {
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
            let push = 1.0 + rough * (randoms(k as i32, seed)[0] * 2.0 - 1.0);
            (center + u * radii * push, dark.lerp(light, (u.y * 0.5 + 0.5).clamp(0.0, 1.0)))
        };
        for [a, b, c] in FACES {
            self.tri(center, corner(a), corner(b), corner(c));
        }
    }
}

// ---------------------------------------------------------------------------------- drawing

/// What one kind of prop is: the resource it shows, its shapes, where its props stand, and how
/// it is drawn.
pub struct PropKind {
    pub resource: Resource,
    /// The meshes, as lists of separate triangles. A prop's `shape` picks one.
    pub shapes: Vec<Vec<PropVertex>>,
    /// The props in chunk (`cx`, `cz`).
    pub generate: fn(&World, usize, usize) -> Vec<Prop>,
    /// Props thin out and shrink away between these distances in meters from the camera.
    pub fade: (f32, f32),
    /// Props are drawn in the chunks within this many chunks of the camera's, which should reach
    /// past the fade.
    pub draw_chunks: i32,
    /// How much the props sway in the wind, 1 for trees and 0 for rocks.
    pub wind: f32,
}

/// One prop as the GPU reads it: where it stands and how big, then how it is turned, tinted and
/// ranked, and how tall.
#[repr(C)]
#[derive(Clone, Copy)]
struct Instance {
    at: [f32; 4],
    look: [f32; 4],
}

#[repr(C)]
struct PropUniforms {
    view_projection: Mat4,
    cam_pos: Vec3,
    sun_dir: Vec3,
    sun_color: Vec3,
    ambient: Vec3,
    fog_color: Vec3,
    fog_range: Vec2,
    time: f32,
    fade: Vec2,
    wind: f32,
}

struct GpuMesh {
    vertices: BufferId,
    count: i32,
}

/// The instance data of one shape on the GPU.
struct GpuInstances {
    buffer: Option<BufferId>,
    capacity: usize,
    count: usize,
}

/// Draws the props of one kind in a world.
pub struct PropRenderer {
    kind: PropKind,
    pipeline: Pipeline,
    meshes: Vec<GpuMesh>,
    indices: BufferId,
    instances: Vec<GpuInstances>,
    /// Each chunk's props, and the state of the chunk when they were worked out.
    cache: HashMap<(u64, usize), (u32, Vec<Prop>)>,
    /// The world and chunk the instance buffers were last filled around.
    filled: Option<(u64, i32, i32)>,
    /// Some chunk's props changed since the instance buffers were filled.
    stale: bool,
}

impl PropRenderer {
    pub fn new(kind: PropKind) -> Self {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let shader = ctx
            .new_shader(
                miniquad::ShaderSource::Glsl { vertex: PROP_VERTEX, fragment: PROP_FRAGMENT },
                ShaderMeta { uniforms: uniform_layout(), images: vec![] },
            )
            .expect("prop shader failed to compile");
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
        // The meshes are lists of separate triangles, so the indices just count up.
        let longest = kind.shapes.iter().map(Vec::len).max().unwrap_or(0);
        let counting: Vec<u16> = (0..longest as u16).collect();
        let meshes = kind
            .shapes
            .iter()
            .map(|mesh| GpuMesh {
                vertices: ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(mesh)),
                count: mesh.len() as i32,
            })
            .collect();
        let indices = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&counting));
        let instances = kind.shapes.iter().map(|_| GpuInstances { buffer: None, capacity: 0, count: 0 }).collect();
        Self { kind, pipeline, meshes, indices, instances, cache: HashMap::new(), filled: None, stale: true }
    }

    /// Draws the props of `world` around the camera at `cam`, as seen by `camera`.
    pub fn draw(&mut self, world: &World, camera: &Camera3D, cam: Vec3, view_distance: f32, time: f32) {
        self.refresh(world, cam);
        if self.instances.iter().all(|i| i.count == 0) {
            return;
        }

        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx: &mut dyn RenderingBackend = gl.quad_context;
        let uniforms = PropUniforms {
            view_projection: camera.matrix(),
            cam_pos: cam,
            sun_dir: crate::render::sun_direction(),
            sun_color: crate::render::SUN_COLOR,
            ambient: crate::render::AMBIENT,
            fog_color: crate::render::BACKGROUND,
            fog_range: crate::render::fog_range(view_distance),
            time,
            fade: vec2(self.kind.fade.0, self.kind.fade.1),
            wind: self.kind.wind,
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

    /// Brings the props around `cam` up to date, and refills the instance buffers if anything
    /// changed.
    fn refresh(&mut self, world: &World, cam: Vec3) {
        let (id, _) = world.revision();
        let (chunks_x, chunks_z) = world.chunk_counts();
        let half = world.half_extents();
        let around = self.kind.draw_chunks;
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
        for dz in -around..=around {
            for dx in -around..=around {
                let (cx, cz) = (ccx + dx, ccz + dz);
                if dx * dx + dz * dz <= around * around
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
            let stamp = world.chunk_stamp(self.kind.resource, cx, cz);
            let key = (id, cz * chunks_x + cx);
            if self.cache.get(&key).is_some_and(|(s, _)| *s == stamp) {
                continue;
            }
            if made == CHUNKS_PER_FRAME {
                // The rest come next frame.
                self.stale = true;
                break;
            }
            self.cache.insert(key, (stamp, (self.kind.generate)(world, cx, cz)));
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

        let mut lists: Vec<Vec<Instance>> = vec![Vec::new(); self.meshes.len()];
        for &(_, cx, cz) in &wanted {
            let Some((_, props)) = self.cache.get(&(id, cz * chunks_x + cx)) else {
                continue;
            };
            for p in props {
                lists[p.shape].push(Instance {
                    at: [p.pos.x, p.pos.y, p.pos.z, p.scale],
                    look: [p.yaw, p.tint, p.rank, p.aspect],
                });
            }
        }
        for list in &mut lists {
            list.truncate(MAX_PROPS / self.meshes.len());
        }
        // Forget chunks that are no longer around the camera.
        if self.cache.len() > wanted.len() * 3 {
            let keep: std::collections::HashSet<usize> = wanted.iter().map(|&(_, cx, cz)| cz * chunks_x + cx).collect();
            self.cache.retain(|(world_id, idx), _| *world_id == id && keep.contains(idx));
        }

        let ctx = unsafe { get_internal_gl() }.quad_context;
        for (gpu, data) in self.instances.iter_mut().zip(&lists) {
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
            U::new("wind", Float1),
        ],
    }
}

const PROP_VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec3 normal;
attribute vec3 color;
// Where the prop stands and how big it is; then its turn, its tint, its rank and its height.
attribute vec4 inst_at;
attribute vec4 inst_look;

varying vec3 v_color;
varying vec3 v_normal;
varying vec3 v_world;

uniform mat4 view_projection;
uniform vec3 cam_pos;
uniform float time;
uniform vec2 fade;
uniform float wind;

void main() {
    // Props thin out with distance: the lower a prop's rank, the farther it stays. One that
    // drops out first shrinks away, so they don't pop.
    float dist = length(inst_at.xyz - cam_pos);
    float keep = 1.0 - smoothstep(fade.x, fade.y, dist);
    float size = inst_at.w * clamp((keep - inst_look.z) * 8.0, 0.0, 1.0);

    float c = cos(inst_look.x);
    float s = sin(inst_look.x);
    vec3 p = position * size * vec3(1.0, inst_look.w, 1.0);
    vec3 turned = vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
    // Crowns sway in the wind, more the higher up.
    float sway = position.y * position.y * 0.004 * size * wind;
    turned.x += sin(time * 1.3 + inst_at.x * 0.21 + inst_at.z * 0.13) * sway;
    turned.z += sin(time * 1.1 + inst_at.z * 0.19 - inst_at.x * 0.11) * sway * 0.7;

    // Set a little into the ground so nothing floats on a slope.
    v_world = inst_at.xyz + turned - vec3(0.0, 0.3 * size, 0.0);
    v_normal = vec3(c * normal.x + s * normal.z, normal.y, -s * normal.x + c * normal.z);
    v_color = color * (0.82 + 0.36 * inst_look.y);
    gl_Position = view_projection * vec4(v_world, 1.0);
}
"#;

const PROP_FRAGMENT: &str = r#"#version 100
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

    fn paint(stroke: &mut DensityStroke, layer: &mut [u8], at: Vec2, radius: f32, seconds: f32) {
        for _ in 0..20 {
            stroke.paint(layer, half(), at, radius, seconds / 20.0);
        }
    }

    /// Flat land at 10 m everywhere.
    fn land() -> (Vec<f32>, Vec<f32>) {
        (vec![10.0; N * N], vec![0.0; N * N])
    }

    #[test]
    fn the_brush_paints_with_soft_edges_and_clears() {
        let (h, w) = land();
        let mut layer = vec![0u8; N * N];
        let mut stroke = DensityStroke::begin(&layer, &h, &w, N, N, false, 0.8);
        paint(&mut stroke, &mut layer, Vec2::ZERO, 80.0, 2.0);
        let mid = N / 2;
        assert_eq!(layer[mid * N + mid], 204, "80% density");
        let rim = layer[mid * N + mid + 18];
        assert!(rim > 0 && rim < 100, "rim is {rim}");
        assert_eq!(layer[mid * N + mid + 30], 0);

        let mut clear = DensityStroke::begin(&layer, &h, &w, N, N, true, 0.8);
        assert!(clear.erases());
        paint(&mut clear, &mut layer, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(layer[mid * N + mid], 0);
    }

    #[test]
    fn the_brush_only_works_on_dry_land_and_never_thins_denser_ground() {
        let (mut h, mut w) = land();
        let mid = N / 2;
        // A pond.
        h[mid * N + mid] = 2.0;
        w[mid * N + mid] = 5.0;
        let mut layer = vec![0u8; N * N];
        layer[mid * N + mid + 1] = 255;
        let mut stroke = DensityStroke::begin(&layer, &h, &w, N, N, false, 0.5);
        paint(&mut stroke, &mut layer, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(layer[mid * N + mid], 0, "nothing under water");
        assert_eq!(layer[mid * N + mid + 1], 255, "denser ground stays");
        assert_eq!(layer[(mid + 1) * N + mid], 128);
    }

    #[test]
    fn value_noise_is_smooth_and_varied() {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        let mut last = value_noise(0.0, 0.0, 3);
        for k in 1..400 {
            let v = value_noise(k as f32 * 0.05, 1.3, 3);
            assert!((v - last).abs() < 0.2, "a jump from {last} to {v}");
            last = v;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo < 0.35 && hi > 0.65, "ranges only {lo} to {hi}");
    }

    #[test]
    fn rough_blobs_differ_from_smooth_ones_and_stay_around_their_center() {
        let (mut smooth, mut rough) = (MeshBuilder::default(), MeshBuilder::default());
        smooth.blob(Vec3::ZERO, Vec3::splat(2.0), Vec3::ZERO, Vec3::ONE, 0.0, 1);
        rough.blob(Vec3::ZERO, Vec3::splat(2.0), Vec3::ZERO, Vec3::ONE, 0.3, 1);
        assert_eq!(smooth.vertices.len(), 60);
        assert!(smooth.vertices.iter().zip(&rough.vertices).any(|(a, b)| a.position != b.position));
        assert!(rough.vertices.iter().all(|v| Vec3::from(v.position).length() < 2.0 * 1.31));
    }
}
