//! Lighting, the backdrop around the world, and the GLSL shaders that draw them.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

use crate::camera::FOV_DEGREES;
use crate::world::{Region, World, CELL_SIZE, SEA_LEVEL};

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

pub fn apply_terrain_uniforms(mat: &Material, world: &World, cam: Vec3, view_distance: f32, time: f32) {
    mat.set_uniform("cam_pos", cam);
    mat.set_uniform("sun_dir", sun_dir());
    mat.set_uniform("sun_color", SUN_COLOR);
    mat.set_uniform("ambient", AMBIENT);
    mat.set_uniform("fog_color", BACKGROUND);
    mat.set_uniform("fog_range", fog_range(view_distance));
    mat.set_uniform("time", time);

    // Map world x/z to texel centers of the surface texture (one texel per heightmap sample).
    let (nx, nz) = world.samples();
    let texels = vec2(nx as f32, nz as f32);
    let scale = Vec2::ONE / (texels * CELL_SIZE);
    let offset = (world.half_extents() / CELL_SIZE + 0.5) / texels;
    mat.set_uniform("surface_scale", scale);
    mat.set_uniform("surface_offset", offset);
    // Heightmap cells spanned by one screen pixel per meter of distance from the camera.
    let pixel_angle = 2.0 * (FOV_DEGREES.to_radians() * 0.5).tan() / screen_height();
    mat.set_uniform("pixel_cells", pixel_angle / CELL_SIZE);
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
                UniformDesc::new("surface_scale", UniformType::Float2),
                UniformDesc::new("surface_offset", UniformType::Float2),
                UniformDesc::new("pixel_cells", UniformType::Float1),
            ],
            textures: vec!["surface".to_owned()],
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
// Height around sea level in red, and land elevation (0-50 m) in green.
uniform sampler2D surface;
uniform vec2 surface_scale;
uniform vec2 surface_offset;
uniform float pixel_cells;

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

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
               mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x), f.y);
}

// `up` is how level the ground is (1 is flat).
vec3 grass_detail(vec3 grass, vec2 p, float dist, float elevation, float up) {
    float far_fade = 1.0 - smoothstep(600.0, 2500.0, dist);
    float mid_fade = 1.0 - smoothstep(120.0, 700.0, dist);
    float near_fade = 1.0 - smoothstep(25.0, 140.0, dist);
    float speck_fade = 1.0 - smoothstep(15.0, 60.0, dist);
    float grain_fade = 1.0 - smoothstep(4.0, 30.0, dist);

    // Big patches of sun-dried and of lush grass.
    float dry = smoothstep(0.52, 0.78, vnoise(p / 70.0 + 5.0)) * 0.6 * far_fade;
    float lush = smoothstep(0.55, 0.80, vnoise(p / 45.0 - 21.0)) * 0.5 * far_fade;
    vec3 col = mix(grass, DRY_GRASS, dry);
    col = mix(col, grass * 0.62, lush);

    // Mottling at a few sizes, down to grain you can see on the blades.
    col *= 1.0 + (vnoise(p / 12.0 + 3.0) - 0.5) * 0.28 * mid_fade;
    col *= 1.0 + (vnoise(p / 2.5 - 8.0) - 0.5) * 0.22 * near_fade;
    col *= 1.0 + (mix(vnoise(p * 7.0), vnoise(p * 19.0 + 5.0), 0.5) - 0.5) * 0.20 * grain_fade;

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
    // The small offset absorbs 8-bit rounding so a whole-metre height lands in its own shade.
    float shade = clamp(floor(elevation + 0.2), 1.0, GREEN_SHADES);
    vec3 grass = mix(GRASS, HIGH_GRASS, (shade - 1.0) / (GREEN_SHADES - 1.0));
    grass = grass_detail(grass, v_world.xz, dist, elevation, normalize(v_normal.xyz).y);
    // Rock is fully in by 26 m and snow by 40 m, each blended in over the metre below; the rock
    // lightens toward the snow line.
    vec3 rock = mix(ROCK, HIGH_ROCK, smoothstep(ROCK_START, SNOW_START, elevation));
    // Bare rock is blotchy and grainy up close.
    rock *= 1.0 + (vnoise(v_world.xz / 7.0) - 0.5) * 0.35 * (1.0 - smoothstep(120.0, 700.0, dist))
        + (vnoise(v_world.xz / 1.8) - 0.5) * 0.25 * (1.0 - smoothstep(25.0, 140.0, dist));
    vec3 ground = mix(grass, rock, smoothstep(ROCK_START - 1.0, ROCK_START, elevation));
    ground = mix(ground, SNOW, smoothstep(SNOW_START - 1.0, SNOW_START, elevation));
    // A narrow sandy beach along every shore, damp where it meets the water. Far away it would
    // only be a hair-thin line, so it fades out with distance.
    float beach_fade = (1.0 - smoothstep(500.0, 1500.0, dist)) * (1.0 - smoothstep(3.0, 6.0, elevation));
    if (land > 0.001 && beach_fade > 0.001) {
        float width = BEACH_WIDTH * (0.65 + 0.7 * vnoise(v_world.xz / 23.0 + 40.0));
        float beach = smoothstep(0.15, 0.6, beach_amount(v_world.xz, coast, width)) * beach_fade;
        vec3 sand = mix(SAND, WET_SAND, 1.0 - smoothstep(0.5, 0.62, coast));
        sand *= 1.0 + (vnoise(v_world.xz / 1.7) - 0.5) * 0.12 * (1.0 - smoothstep(20.0, 150.0, dist));
        ground = mix(ground, sand, beach);
    }
    vec3 base = mix(WATER, ground, land);

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

    float fog = smoothstep(fog_range.x, fog_range.y, dist);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;
