//! Lighting, the backdrop around the world, and the GLSL shaders that draw them.

use macroquad::miniquad::{
    self, BufferLayout, Pipeline, RenderingBackend, ShaderMeta, UniformBlockLayout, UniformsSource,
    VertexAttribute, VertexFormat,
};
use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

use crate::camera::FOV_DEGREES;
use crate::world::{Frustum, Region, World, CELL_SIZE, SEA_LEVEL};

/// The pale haze of the sky at the horizon. Distant terrain and sea fog to it, so they fade
/// into the sky.
pub const BACKGROUND: Vec3 = Vec3::new(0.64, 0.76, 0.88);
const SUN_ELEVATION: f32 = 0.6;
const SUN_AZIMUTH: f32 = 2.4;
const SUN_COLOR: Vec3 = Vec3::new(1.0, 0.95, 0.85);
const AMBIENT: Vec3 = Vec3::new(0.34, 0.39, 0.47);
/// How far the backdrop plane extends past the world's edge.
const BACKDROP_EXTENT: f32 = 40_000.0;
/// Texels along each side of the repeating random texture the terrain shader's noise reads.
/// Must match `NOISE_SIZE` in the shader.
const NOISE_SIZE: u16 = 512;

pub fn background_color() -> Color {
    Color::new(BACKGROUND.x, BACKGROUND.y, BACKGROUND.z, 1.0)
}

fn sun_dir() -> Vec3 {
    vec3(
        SUN_ELEVATION.cos() * SUN_AZIMUTH.sin(),
        SUN_ELEVATION.sin(),
        -SUN_ELEVATION.cos() * SUN_AZIMUTH.cos(),
    )
}

/// Fog starts and ends at these fractions of the view distance.
fn fog_range(view_distance: f32) -> Vec2 {
    vec2(view_distance * 0.4, view_distance)
}

/// A terrain mesh vertex as the GPU reads it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// The terrain shaders' uniforms, in the order [`terrain_uniform_layout`] lists them.
#[repr(C)]
struct TerrainUniforms {
    view_projection: Mat4,
    cam_pos: Vec3,
    sun_dir: Vec3,
    sun_color: Vec3,
    ambient: Vec3,
    fog_color: Vec3,
    fog_range: Vec2,
    time: f32,
    surface_scale: Vec2,
    surface_offset: Vec2,
    pixel_cells: f32,
    /// 1 to draw a topographic map instead of the lit terrain, 0 for the regular view.
    topo: f32,
}

fn terrain_uniform_layout() -> UniformBlockLayout {
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
            U::new("surface_scale", Float2),
            U::new("surface_offset", Float2),
            U::new("pixel_cells", Float1),
            U::new("topo", Float1),
        ],
    }
}

/// Draws the world's terrain. The terrain lives on the GPU in blocks the world uploads only when
/// they change, so a frame costs one draw call per visible block instead of copying every
/// vertex in view, as drawing it through macroquad's batches would.
pub struct TerrainRenderer {
    pipeline: Pipeline,
    noise: Texture2D,
}

impl TerrainRenderer {
    pub fn new() -> Self {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let shader = ctx
            .new_shader(
                miniquad::ShaderSource::Glsl {
                    vertex: TERRAIN_VERTEX,
                    fragment: TERRAIN_FRAGMENT,
                },
                ShaderMeta {
                    uniforms: terrain_uniform_layout(),
                    images: vec!["surface".to_owned(), "noise".to_owned()],
                },
            )
            .expect("terrain shader failed to compile");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("position", VertexFormat::Float3),
                VertexAttribute::new("normal", VertexFormat::Float3),
            ],
            shader,
            opaque_pipeline(),
        );
        Self {
            pipeline,
            noise: noise_texture(),
        }
    }

    /// Draws the terrain of `world`, colored from its `surface` texture, as seen by `camera`
    /// from `cam`, out to `view_distance`. Whatever macroquad has queued is drawn first, so the
    /// terrain lands on top of it.
    pub fn draw(&self, world: &mut World, surface: &Texture2D, camera: &Camera3D, cam: Vec3, view_distance: f32, time: f32, topo: bool) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx: &mut dyn RenderingBackend = gl.quad_context;

        let view_projection = camera.matrix();
        let blocks = world.visible_blocks(ctx, &Frustum::from_matrix(view_projection), cam, view_distance);

        // Map world x/z to texel centers of the surface texture (one texel per heightmap sample).
        let (nx, nz) = world.samples();
        let texels = vec2(nx as f32, nz as f32);
        // Heightmap cells spanned by one screen pixel per meter of distance from the camera.
        let pixel_angle = 2.0 * (FOV_DEGREES.to_radians() * 0.5).tan() / screen_height();
        let uniforms = TerrainUniforms {
            view_projection,
            cam_pos: cam,
            sun_dir: sun_dir(),
            sun_color: SUN_COLOR,
            ambient: AMBIENT,
            fog_color: BACKGROUND,
            fog_range: fog_range(view_distance),
            time,
            surface_scale: Vec2::ONE / (texels * CELL_SIZE),
            surface_offset: (world.half_extents() / CELL_SIZE + 0.5) / texels,
            pixel_cells: pixel_angle / CELL_SIZE,
            topo: if topo { 1.0 } else { 0.0 },
        };

        let (width, height) = miniquad::window::screen_size();
        ctx.begin_default_pass(miniquad::PassAction::Nothing);
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_viewport(0, 0, width as i32, height as i32);
        ctx.apply_scissor_rect(0, 0, width as i32, height as i32);
        let images = vec![surface.raw_miniquad_id(), self.noise.raw_miniquad_id()];
        for block in blocks {
            ctx.apply_bindings(&miniquad::Bindings {
                vertex_buffers: vec![block.vertices],
                index_buffer: block.indices,
                images: images.clone(),
            });
            ctx.apply_uniforms(UniformsSource::table(&uniforms));
            ctx.draw(0, block.count, 1);
        }
        ctx.end_render_pass();
    }
}

/// Uploads the world's coast and elevation for the terrain shader.
pub fn surface_texture(world: &World) -> Texture2D {
    let (nx, nz) = world.samples();
    let texture = Texture2D::from_rgba8(nx as u16, nz as u16, &world.surface_texels());
    texture.set_filter(FilterMode::Linear);
    texture
}

/// Re-uploads the part of the surface texture inside `region` after its heights changed.
pub fn update_surface(texture: &Texture2D, world: &World, region: Region) {
    let image = Image {
        bytes: world.surface_texels_in(region),
        width: region.width() as u16,
        height: region.height() as u16,
    };
    texture.update_part(&image, region.i0 as i32, region.j0 as i32, image.width as i32, image.height as i32);
}

/// A repeating texture of random values for the terrain shader's noise. Reading noise from it
/// takes one texture lookup instead of hashing four lattice points, which is most of what the
/// grass and rock detail used to cost.
pub fn noise_texture() -> Texture2D {
    // xorshift: any fixed sequence will do, as long as it is the same every run.
    let mut state: u32 = 0x9E37_79B9;
    let bytes: Vec<u8> = (0..NOISE_SIZE as usize * NOISE_SIZE as usize * 4)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect();
    let texture = Texture2D::from_rgba8(NOISE_SIZE, NOISE_SIZE, &bytes);
    texture.set_filter(FilterMode::Linear);
    let id = texture.raw_miniquad_id();
    unsafe { get_internal_gl() }
        .quad_context
        .texture_set_wrap(id, miniquad::TextureWrap::Repeat, miniquad::TextureWrap::Repeat);
    texture
}

/// `world_half` is the world's half extents in x and z.
pub fn apply_backdrop_uniforms(mat: &Material, cam: Vec3, view_distance: f32, world_half: Vec2) {
    mat.set_uniform("cam_pos", cam);
    mat.set_uniform("fog_color", BACKGROUND);
    mat.set_uniform("fog_range", fog_range(view_distance));
    mat.set_uniform("world_half", world_half);
    mat.set_uniform("sun_dir", sun_dir());
    mat.set_uniform("sun_color", SUN_COLOR);
    mat.set_uniform("ambient", AMBIENT);
}

/// Fills the screen with the sky, as seen by a camera at `cam` with this view-projection matrix.
/// Draws behind everything, whatever is drawn next.
pub fn draw_sky(mat: &Material, view_projection: Mat4, cam: Vec3, time: f32) {
    let (width, height) = miniquad::window::screen_size();
    mat.set_uniform("inv_view_projection", view_projection.inverse());
    mat.set_uniform("cam_pos", cam);
    mat.set_uniform("sun_dir", sun_dir());
    mat.set_uniform("fog_color", BACKGROUND);
    mat.set_uniform("time", time);
    mat.set_uniform("resolution", vec2(width, height));
    gl_use_material(mat);
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), WHITE);
    gl_use_default_material();
}

pub fn load_sky_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: SKY_FRAGMENT,
        },
        MaterialParams {
            // Drawn first, under everything: it neither tests nor writes depth.
            pipeline_params: PipelineParams {
                depth_write: false,
                depth_test: Comparison::Always,
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("inv_view_projection", UniformType::Mat4),
                UniformDesc::new("cam_pos", UniformType::Float3),
                UniformDesc::new("sun_dir", UniformType::Float3),
                UniformDesc::new("fog_color", UniformType::Float3),
                UniformDesc::new("time", UniformType::Float1),
                UniformDesc::new("resolution", UniformType::Float2),
            ],
            ..Default::default()
        },
    )
    .expect("sky shader failed to compile")
}

fn opaque_pipeline() -> PipelineParams {
    PipelineParams {
        depth_write: true,
        depth_test: Comparison::LessOrEqual,
        ..Default::default()
    }
}

pub fn load_backdrop_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: BACKDROP_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: opaque_pipeline(),
            uniforms: vec![
                UniformDesc::new("cam_pos", UniformType::Float3),
                UniformDesc::new("fog_color", UniformType::Float3),
                UniformDesc::new("fog_range", UniformType::Float2),
                UniformDesc::new("world_half", UniformType::Float2),
                UniformDesc::new("sun_dir", UniformType::Float3),
                UniformDesc::new("sun_color", UniformType::Float3),
                UniformDesc::new("ambient", UniformType::Float3),
            ],
            ..Default::default()
        },
    )
    .expect("backdrop shader failed to compile")
}

/// Four large quads framing a world with half extents `half` at sea level; the backdrop
/// shader colors them.
pub fn backdrop_mesh(half: Vec2) -> Mesh {
    let (hx, hz, o) = (half.x, half.y, BACKDROP_EXTENT);
    let rects = [
        (-o, -o, o, -hz),   // north
        (-o, hz, o, o),     // south
        (-o, -hz, -hx, hz), // west
        (hx, -hz, o, hz),   // east
    ];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (x0, z0, x1, z1) in rects {
        let base = vertices.len() as u16;
        for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
            vertices.push(Vertex {
                position: vec3(x, SEA_LEVEL, z),
                uv: Vec2::ZERO,
                color: [0, 0, 0, 255],
                normal: Vec4::Y,
            });
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

/// GLSL for the color of the sky in a direction `d` (a unit vector) with the sun in direction
/// `sun`: pale haze at the horizon deepening to blue overhead, with a glow around the sun. The
/// haze is the color distant land and sea fade to, so they fade into the sky. Shared by the sky,
/// the sea beyond the map and the sky's reflection in water.
macro_rules! sky_glsl {
    () => {
        r#"
const vec3 ZENITH = vec3(0.17, 0.38, 0.76);
const vec3 SUN_GLOW = vec3(1.0, 0.82, 0.55);
vec3 sky_gradient(vec3 d, vec3 sun, vec3 haze) {
    vec3 col = mix(haze, ZENITH, pow(clamp(d.y, 0.0, 1.0), 0.55));
    float s = max(dot(d, sun), 0.0);
    return col + SUN_GLOW * (pow(s, 6.0) * 0.18 + pow(s, 48.0) * 0.35);
}
"#
    };
}

const VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec4 color0;
attribute vec4 normal;

varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    vec4 world = Model * vec4(position, 1.0);
    v_world = world.xyz;
    v_color = color0 / 255.0;
    v_normal = normal;
    gl_Position = Projection * world;
}
"#;

const TERRAIN_VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec3 normal;

varying vec4 v_normal;
varying vec3 v_world;

uniform mat4 view_projection;

void main() {
    v_world = position;
    v_normal = vec4(normal, 0.0);
    gl_Position = view_projection * vec4(position, 1.0);
}
"#;

const BACKDROP_FRAGMENT: &str = concat!(
    r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 fog_color;
uniform vec2 fog_range;
uniform vec2 world_half;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;

const vec3 WATER = vec3(0.02, 0.10, 0.26);
"#,
    sky_glsl!(),
    r#"
void main() {
    vec3 sun = normalize(sun_dir);
    // The open sea, lit like the water inside the world, and a little darker right at the
    // world's edge so the extent of the map can be seen.
    float edge_dist = length(max(abs(v_world.xz) - world_half, 0.0));
    vec3 base = WATER * (ambient + sun_color * max(sun.y, 0.0));
    base *= mix(0.7, 1.0, smoothstep(0.0, 1200.0, edge_dist));

    // The sky shows in the water more the lower the camera looks across it.
    vec3 v = normalize(cam_pos - v_world);
    float fresnel = 0.04 + 0.96 * pow(1.0 - max(v.y, 0.0), 5.0);
    vec3 reflection = sky_gradient(vec3(-v.x, max(v.y, 0.0), -v.z), sun, fog_color);
    vec3 col = mix(base, reflection, fresnel);

    float fog = pow(smoothstep(fog_range.x, fog_range.y, length(v_world - cam_pos)), 0.6);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#
);

const SKY_FRAGMENT: &str = concat!(
    r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec3 v_world;

uniform mat4 inv_view_projection;
uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform vec3 fog_color;
uniform float time;
uniform vec2 resolution;

// Clouds drift along a layer this high, in meters, and thin out as the camera climbs to it.
const float CLOUD_HEIGHT = 2200.0;
"#,
    sky_glsl!(),
    r#"
float hash21(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
               mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    for (int k = 0; k < 5; k++) {
        sum += amp * vnoise(p);
        p = p * 2.03 + vec2(17.1, 5.3);
        amp *= 0.5;
    }
    return sum;
}

void main() {
    // The direction this pixel looks in, from where its position on the far plane is.
    vec2 ndc = gl_FragCoord.xy / resolution * 2.0 - 1.0;
    vec4 far_point = inv_view_projection * vec4(ndc, 1.0, 1.0);
    vec3 d = normalize(far_point.xyz / far_point.w - cam_pos);
    vec3 sun = normalize(sun_dir);

    vec3 col = sky_gradient(d, sun, fog_color);
    float s = dot(d, sun);
    // The sun itself, with a soft rim.
    col += vec3(1.0, 0.95, 0.80) * smoothstep(0.99950, 0.99985, s) * 2.5 * step(0.0, d.y);

    // Clouds on a flat layer, thinning out toward the horizon where they'd crowd together, and
    // gone for a camera above them.
    float height = CLOUD_HEIGHT - cam_pos.y;
    float visible = smoothstep(0.02, 0.22, d.y) * (1.0 - smoothstep(1300.0, 1900.0, cam_pos.y));
    if (visible > 0.0 && height > 0.0) {
        vec2 p = (cam_pos.xz + d.xz * (height / d.y)) / 2400.0 + vec2(time * 0.0035, time * 0.0012);
        float n = fbm(p);
        float cover = smoothstep(0.46, 0.72, n);
        float thick = smoothstep(0.55, 0.85, n);
        // Bright on top and toward the sun, bluish grey where they are thick.
        vec3 cloud = mix(vec3(0.98, 0.98, 1.0), vec3(0.60, 0.66, 0.77), thick * 0.75);
        cloud += vec3(1.0, 0.85, 0.6) * pow(max(s, 0.0), 6.0) * 0.18 * (1.0 - thick);
        col = mix(col, cloud, cover * visible * 0.92);
    }
    gl_FragColor = vec4(col, 1.0);
}
"#
);

const TERRAIN_FRAGMENT: &str = concat!(
    r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;
uniform vec3 fog_color;
uniform vec2 fog_range;
uniform float time;
// Height around the water level in red, land elevation (0-50 m) in green, in blue whether
// water is close enough for a beach and in alpha how high the ground stands above the water
// (0-8 m).
uniform sampler2D surface;
// Repeating random values, for noise.
uniform sampler2D noise;
const float NOISE_SIZE = 512.0;
uniform vec2 surface_scale;
uniform vec2 surface_offset;
uniform float pixel_cells;
// 1 draws a topographic map instead of the lit terrain.
uniform float topo;

"#,
    sky_glsl!(),
    r#"
// Hills are drawn in 25 shades of green, one per metre: shade 1 (up to 2 m) is the lightest and
// shade 25 (25 m) the darkest. Mountains are grey rock from 26 m and snow from 40 m.
const float GREEN_SHADES = 25.0;
const float ROCK_START = 26.0;
const float SNOW_START = 40.0;
const float ELEVATION_RANGE = 50.0;
const vec3 ROCK = vec3(0.42, 0.42, 0.43);
const vec3 HIGH_ROCK = vec3(0.60, 0.60, 0.62);
const vec3 SNOW = vec3(0.96, 0.97, 1.0);
const vec3 GRASS = vec3(0.62, 0.80, 0.30);
const vec3 HIGH_GRASS = vec3(0.03, 0.14, 0.05);
const vec3 WATER = vec3(0.02, 0.10, 0.26);
// Deep water is dark blue. Shallow water, which is all of the lakes and rivers and the sea where
// its floor is within a few meters, is a lighter blue-green, palest at the shore, giving way to
// the dark blue as the floor drops away. How deep the water is, in meters, is in the surface
// texture's alpha; must match `MAX_WATER_DEPTH` in world.rs.
const float WATER_DEPTH_RANGE = 64.0;
const vec3 SHALLOW_EDGE = vec3(0.30, 0.68, 0.66);
const vec3 SHALLOW = vec3(0.09, 0.42, 0.56);
const vec3 SAND = vec3(0.86, 0.77, 0.53);
const vec3 WET_SAND = vec3(0.60, 0.52, 0.34);
// Beaches reach about this far from the water, in meters (varied along the shore).
const float BEACH_WIDTH = 9.0;

// Small-scale detail so grassland isn't one flat color up close. Everything is a function of
// world position, so it stays put as the camera moves, and each layer fades out with distance
// before it would shimmer.
const vec3 DRY_GRASS = vec3(0.58, 0.52, 0.26);

float hash21(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Smooth value noise in one lookup: the texture's linear filter blends the four lattice values,
// and easing the position within the texel turns that blend into a smooth curve.
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return texture2D(noise, (mod(i, NOISE_SIZE) + f + 0.5) / NOISE_SIZE).r;
}

// `up` is how level the ground is (1 is flat).
vec3 grass_detail(vec3 grass, vec2 p, float dist, float elevation, float up) {
    float far_fade = 1.0 - smoothstep(600.0, 2500.0, dist);
    float mid_fade = 1.0 - smoothstep(120.0, 700.0, dist);
    float near_fade = 1.0 - smoothstep(25.0, 140.0, dist);
    float speck_fade = 1.0 - smoothstep(15.0, 60.0, dist);
    float grain_fade = 1.0 - smoothstep(4.0, 30.0, dist);

    // Each layer is skipped once it has faded out, which is most of the screen from afar.
    // Big patches of sun-dried and of lush grass.
    vec3 col = grass;
    if (far_fade > 0.0) {
        float dry = smoothstep(0.52, 0.78, vnoise(p / 70.0 + 5.0)) * 0.6 * far_fade;
        float lush = smoothstep(0.55, 0.80, vnoise(p / 45.0 - 21.0)) * 0.5 * far_fade;
        col = mix(grass, DRY_GRASS, dry);
        col = mix(col, grass * 0.62, lush);
    }

    // Mottling at a few sizes, down to grain you can see on the blades.
    if (mid_fade > 0.0) {
        col *= 1.0 + (vnoise(p / 12.0 + 3.0) - 0.5) * 0.28 * mid_fade;
    }
    if (near_fade > 0.0) {
        col *= 1.0 + (vnoise(p / 2.5 - 8.0) - 0.5) * 0.22 * near_fade;
    }
    if (grain_fade > 0.0) {
        col *= 1.0 + (mix(vnoise(p * 7.0), vnoise(p * 19.0 + 5.0), 0.5) - 0.5) * 0.20 * grain_fade;
    }

    // Only on level ground well above the shore and below the rock.
    float spot = (1.0 - smoothstep(ROCK_START - 6.0, ROCK_START - 1.0, elevation))
        * smoothstep(1.5, 4.0, elevation) * smoothstep(0.80, 0.93, up);

    // Wildflowers: tiny pale and yellow specks, only seen from close up.
    if (speck_fade * spot > 0.001) {
        vec2 g = p / 1.6;
        vec2 cell = floor(g);
        if (hash21(cell + 7.7) > 0.93) {
            vec2 centre = vec2(0.15) + 0.7 * vec2(hash21(cell + 2.3), hash21(cell + 5.9));
            float d = length((fract(g) - centre) * 1.6);
            vec3 flower = hash21(cell + 13.1) < 0.5 ? vec3(0.95, 0.93, 0.85) : vec3(0.95, 0.80, 0.25);
            col = mix(col, flower, (1.0 - smoothstep(0.05, 0.10, d)) * speck_fade * spot);
        }
    }
    return col;
}

// How close the point is to open water, from 0 (further than a beach is wide) to 1 (at the
// water's edge). Looks for water in rings around the point in the surface texture, which
// interpolates smoothly so the beach edge follows the coastline between samples.
float beach_amount(vec2 p, float coast, float width) {
    float closest = 0.0;
    for (int ring = 1; ring <= 3; ring++) {
        float r = width * float(ring) / 3.0;
        float water = 0.0;
        for (int k = 0; k < 8; k++) {
            float a = float(k) * 0.7853982 + float(ring);
            vec2 at = (p + r * vec2(cos(a), sin(a))) * surface_scale + surface_offset;
            water = max(water, 1.0 - smoothstep(0.46, 0.54, texture2D(surface, at).r));
        }
        closest = max(closest, water * (1.0 - (float(ring) - 1.0) / 3.0));
    }
    // Right at the shoreline the ground itself is within a hair of sea level.
    return max(closest, 1.0 - smoothstep(0.5, 0.7, coast));
}

// The topographic map: height bands tinted from green through yellow to brown, contour lines
// every 5 m with a heavier one every 25 m, and a dark line along the coast. `land` is how much
// of the pixel is land, `shore` how close it is to the coast line (1 on it), `dist` the distance
// to the camera.
const float CELL = 4.0;

// How close the point is to the coast line, 1 on it, over about a pixel: whether the coast
// value crosses its midpoint within a pixel's reach.
float shore_line(vec2 p, float dist) {
    float r = max(dist * pixel_cells * CELL * 0.75, 0.5);
    float lo = 1.0;
    float hi = 0.0;
    for (int k = 0; k < 4; k++) {
        vec2 o = k == 0 ? vec2(r, 0.0) : k == 1 ? vec2(-r, 0.0) : k == 2 ? vec2(0.0, r) : vec2(0.0, -r);
        float c = texture2D(surface, (p + o) * surface_scale + surface_offset).r;
        lo = min(lo, c);
        hi = max(hi, c);
    }
    return smoothstep(0.0, 0.05, min(hi - 0.5, 0.5 - lo));
}

vec3 topo_color(float land, float shore, float dist) {
    float h = v_world.y;
    vec3 n = normalize(v_normal.xyz);

    vec3 tint = mix(vec3(0.62, 0.80, 0.52), vec3(0.93, 0.90, 0.62), smoothstep(2.0, 12.0, h));
    tint = mix(tint, vec3(0.82, 0.66, 0.45), smoothstep(12.0, 28.0, h));
    tint = mix(tint, vec3(0.62, 0.48, 0.38), smoothstep(28.0, 45.0, h));
    // A little relief shading so ridges read at a glance.
    tint *= mix(1.0, 0.75 + 0.5 * max(dot(n, normalize(sun_dir)), 0.0), 0.35);
    vec3 col = mix(vec3(0.70, 0.84, 0.93), tint, land);

    // How much height one pixel spans, which sets the width of the contour lines so they stay
    // about a pixel wide whatever the slope, and where they would crowd into a smudge.
    float footprint = dist * pixel_cells * CELL;
    float slope = sqrt(max(1.0 - n.y * n.y, 0.0)) / max(n.y, 0.2);
    float w = max(slope * footprint * 0.75, 0.002);
    float on_land = land * smoothstep(1.0, 2.5, h);

    float d_minor = abs(fract(h / 5.0 + 0.5) - 0.5) * 5.0;
    float spacing_minor = 5.0 / max(slope, 0.001) / max(footprint, 0.001);
    float minor = (1.0 - smoothstep(0.5 * w, 1.5 * w, d_minor)) * smoothstep(2.0, 5.0, spacing_minor);
    col = mix(col, vec3(0.55, 0.38, 0.25), minor * 0.75 * on_land);

    float d_index = abs(fract(h / 25.0 + 0.5) - 0.5) * 25.0;
    float spacing_index = 25.0 / max(slope, 0.001) / max(footprint, 0.001);
    float index = (1.0 - smoothstep(w, 2.5 * w, d_index)) * smoothstep(2.0, 5.0, spacing_index);
    col = mix(col, vec3(0.33, 0.20, 0.11), index * on_land);

    return mix(col, vec3(0.16, 0.34, 0.52), shore);
}

void main() {
    float dist = length(v_world - cam_pos);
    vec3 sun = normalize(sun_dir);

    // Land or water is decided per pixel from the full-resolution surface texture, not per
    // vertex, so coastlines stay sharp however coarse the mesh is. The edge is anti-aliased
    // approximately in proportion to a screen pixel. Keep the minimum tiny: a fixed-width
    // blend becomes a visibly wide, fuzzy strip when the camera is close to the shore.
    vec4 terrain = texture2D(surface, v_world.xz * surface_scale + surface_offset);
    float coast = terrain.r;
    float aa = clamp(dist * pixel_cells * 0.5, 0.0001, 0.5);
    float land = smoothstep(0.5 - aa, 0.5 + aa, coast);
    float elevation = terrain.g * ELEVATION_RANGE;
    if (topo > 0.5) {
        gl_FragColor = vec4(topo_color(land, shore_line(v_world.xz, dist), dist), 1.0);
        return;
    }
    // The small offset absorbs 8-bit rounding so a whole-metre height lands in its own shade.
    float shade = clamp(floor(elevation + 0.2), 1.0, GREEN_SHADES);
    vec3 grass = mix(GRASS, HIGH_GRASS, (shade - 1.0) / (GREEN_SHADES - 1.0));
    // Grass detail only where grass shows: on land, below the rock.
    float rocky = smoothstep(ROCK_START - 1.0, ROCK_START, elevation);
    if (land > 0.0 && rocky < 1.0) {
        grass = grass_detail(grass, v_world.xz, dist, elevation, normalize(v_normal.xyz).y);
    }
    // Rock is fully in by 26 m and snow by 40 m, each blended in over the metre below; the rock
    // lightens toward the snow line.
    vec3 rock = mix(ROCK, HIGH_ROCK, smoothstep(ROCK_START, SNOW_START, elevation));
    // Bare rock is blotchy and grainy up close.
    if (rocky > 0.0 && dist < 700.0) {
        rock *= 1.0 + (vnoise(v_world.xz / 7.0) - 0.5) * 0.35 * (1.0 - smoothstep(120.0, 700.0, dist))
            + (vnoise(v_world.xz / 1.8) - 0.5) * 0.25 * (1.0 - smoothstep(25.0, 140.0, dist));
    }
    vec3 ground = mix(grass, rock, rocky);
    ground = mix(ground, SNOW, smoothstep(SNOW_START - 1.0, SNOW_START, elevation));
    // A narrow sandy beach along every shore, damp where it meets the water. Far away it would
    // only be a hair-thin line, so it fades out with distance.
    float above_water = terrain.a * 8.0;
    float beach_fade = (1.0 - smoothstep(500.0, 1500.0, dist)) * (1.0 - smoothstep(3.0, 6.0, above_water));
    // Blue marks ground with water nearby; the search for it is skipped everywhere else.
    if (land > 0.001 && beach_fade > 0.001 && terrain.b > 0.0) {
        float width = BEACH_WIDTH * (0.65 + 0.7 * vnoise(v_world.xz / 23.0 + 40.0));
        float beach = smoothstep(0.15, 0.6, beach_amount(v_world.xz, coast, width)) * beach_fade;
        vec3 sand = mix(SAND, WET_SAND, 1.0 - smoothstep(0.5, 0.62, coast));
        sand *= 1.0 + (vnoise(v_world.xz / 1.7) - 0.5) * 0.12 * (1.0 - smoothstep(20.0, 150.0, dist));
        ground = mix(ground, sand, beach);
    }
    // Stored as a square root of the depth.
    float depth = terrain.a * terrain.a * WATER_DEPTH_RANGE;
    vec3 water = mix(SHALLOW_EDGE, SHALLOW, smoothstep(0.0, 3.0, depth));
    water = mix(water, WATER, smoothstep(5.0, 30.0, depth));
    vec3 base = mix(water, ground, land);

    vec3 n = mix(vec3(0.0, 1.0, 0.0), normalize(v_normal.xyz), land);

    // Gentle animated ripples shade the water slightly so it doesn't look painted on. They
    // only change the lighting, so the water stays dark blue.
    vec2 p = mod(v_world.xz, 2048.0);
    vec3 wn = normalize(vec3(
        0.07 * sin(p.x * 0.31 + time * 1.7) + 0.05 * sin((p.x + p.y) * 0.57 - time * 2.3),
        1.0,
        0.07 * cos(p.y * 0.37 + time * 1.3) + 0.05 * cos((p.x - p.y) * 0.49 + time * 1.9)));
    // Fade ripples out with distance; seen from high above they would only shimmer.
    n = normalize(mix(n, wn, (1.0 - land) * (1.0 - smoothstep(120.0, 700.0, dist))));

    float diffuse = max(dot(n, sun), 0.0);
    vec3 col = base * (ambient + sun_color * diffuse);

    // Water shows the sky: more of it the lower the camera looks across the water, and the
    // sun glitters in it. Fading out with the ripples, from afar it is a mirror.
    float wet = 1.0 - land;
    if (wet > 0.001) {
        vec3 v = normalize(cam_pos - v_world);
        float fresnel = 0.04 + 0.96 * pow(1.0 - max(dot(v, n), 0.0), 5.0);
        vec3 r = reflect(-v, n);
        vec3 reflection = sky_gradient(vec3(r.x, max(r.y, 0.0), r.z), sun, fog_color);
        float glitter = pow(max(dot(r, sun), 0.0), 300.0) * 2.0;
        col = mix(col, reflection, fresnel * wet) + sun_color * glitter * (0.3 + fresnel) * wet;
    }

    float fog = pow(smoothstep(fog_range.x, fog_range.y, dist), 0.6);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#
);
