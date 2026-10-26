//! Lighting, the backdrop around the world, and the GLSL shaders that draw them.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

use crate::world::SEA_LEVEL;

/// The neutral grey the backdrop fades to away from the world; distant terrain fogs to it too.
pub const BACKGROUND: Vec3 = Vec3::new(0.30, 0.30, 0.31);
const SUN_ELEVATION: f32 = 0.6;
const SUN_AZIMUTH: f32 = 2.4;
const SUN_COLOR: Vec3 = Vec3::new(1.0, 0.95, 0.85);
const AMBIENT: Vec3 = Vec3::new(0.34, 0.39, 0.47);
/// How far the backdrop plane extends past the world's edge.
const BACKDROP_EXTENT: f32 = 40_000.0;

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
    vec2(view_distance * 0.6, view_distance)
}

pub fn apply_terrain_uniforms(mat: &Material, cam: Vec3, view_distance: f32, time: f32) {
    mat.set_uniform("cam_pos", cam);
    mat.set_uniform("sun_dir", sun_dir());
    mat.set_uniform("sun_color", SUN_COLOR);
    mat.set_uniform("ambient", AMBIENT);
    mat.set_uniform("fog_color", BACKGROUND);
    mat.set_uniform("fog_range", fog_range(view_distance));
    mat.set_uniform("time", time);
}

/// `world_half` is the world's half extents in x and z.
pub fn apply_backdrop_uniforms(mat: &Material, cam: Vec3, view_distance: f32, world_half: Vec2) {
    mat.set_uniform("cam_pos", cam);
    mat.set_uniform("fog_color", BACKGROUND);
    mat.set_uniform("fog_range", fog_range(view_distance));
    mat.set_uniform("world_half", world_half);
}

fn opaque_pipeline() -> PipelineParams {
    PipelineParams {
        depth_write: true,
        depth_test: Comparison::LessOrEqual,
        ..Default::default()
    }
}

pub fn load_terrain_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: TERRAIN_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: opaque_pipeline(),
            uniforms: vec![
                UniformDesc::new("cam_pos", UniformType::Float3),
                UniformDesc::new("sun_dir", UniformType::Float3),
                UniformDesc::new("sun_color", UniformType::Float3),
                UniformDesc::new("ambient", UniformType::Float3),
                UniformDesc::new("fog_color", UniformType::Float3),
                UniformDesc::new("fog_range", UniformType::Float2),
                UniformDesc::new("time", UniformType::Float1),
            ],
            ..Default::default()
        },
    )
    .expect("terrain shader failed to compile")
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

const BACKDROP_FRAGMENT: &str = r#"#version 100
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

void main() {
    // Black right at the world's edge, easing to the background grey farther out.
    float edge_dist = length(max(abs(v_world.xz) - world_half, 0.0));
    vec3 col = fog_color * smoothstep(150.0, 4500.0, edge_dist);

    float fog = smoothstep(fog_range.x, fog_range.y, length(v_world - cam_pos));
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;

const TERRAIN_FRAGMENT: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;
uniform vec3 fog_color;
uniform vec2 fog_range;
uniform float time;

// Water reflects a blue sky even though the backdrop is neutral, so it still reads as water.
const vec3 SKY_REFLECTION = vec3(0.55, 0.66, 0.80);

void main() {
    vec3 view = v_world - cam_pos;
    float dist = length(view);
    vec3 vdir = view / max(dist, 0.001);
    vec3 sun = normalize(sun_dir);

    float water = smoothstep(0.3, 0.7, v_normal.w);
    vec3 n = normalize(v_normal.xyz);

    // Animated ripples perturb the water normal so the sun glints and the surface shimmers.
    vec2 p = mod(v_world.xz, 2048.0);
    vec3 wn = normalize(vec3(
        0.07 * sin(p.x * 0.31 + time * 1.7) + 0.05 * sin((p.x + p.y) * 0.57 - time * 2.3),
        1.0,
        0.07 * cos(p.y * 0.37 + time * 1.3) + 0.05 * cos((p.x - p.y) * 0.49 + time * 1.9)));
    // Fade ripples out with distance; seen from high above they would only shimmer.
    n = normalize(mix(n, wn, water * (1.0 - smoothstep(120.0, 700.0, dist))));

    float diffuse = max(dot(n, sun), 0.0);
    vec3 col = v_color.rgb * (ambient + sun_color * diffuse);

    vec3 r = reflect(vdir, n);
    float spec = pow(max(dot(r, sun), 0.0), 90.0);
    float fresnel = pow(1.0 - max(dot(-vdir, n), 0.0), 4.0);
    vec3 water_col = mix(col, SKY_REFLECTION, fresnel * 0.65) + sun_color * spec * 1.4;
    col = mix(col, water_col, water);

    float fog = smoothstep(fog_range.x, fog_range.y, dist);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;
