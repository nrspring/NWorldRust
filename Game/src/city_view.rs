//! Draws the cities in 3D, over the map. This belongs to the game: the map and its generic view know
//! nothing about cities. The shapes are made by [`crate::city_mesh`], and are lit by the same sun
//! and faded by the same fog as the terrain, so they sit in the scene.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::render::{self, AMBIENT, BACKGROUND, SUN_COLOR};

use crate::city::{City, INITIAL_POPULATION};
use crate::ids::{CityId, CountryId};
use crate::city_layout;
use crate::city_mesh;
use crate::city_pick::{self, Bounds};
use crate::greyscale;

const VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;

varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    vec4 world = Model * vec4(position, 1.0);
    v_world = world.xyz;
    v_uv = texcoord;
    v_color = color0 / 255.0;
    v_normal = normal;
    gl_Position = Projection * world;
}
"#;

const FRAGMENT: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec2 v_uv;
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 fog_color;
uniform vec2 fog_range;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;
// A colour to mix over everything, and how much of it, for showing where a city would go.
uniform vec4 tint;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
               mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x), f.y);
}

// 1 where p is inside the rectangle from lo to hi, and 0 where it is not.
float box(vec2 p, vec2 lo, vec2 hi) {
    vec2 s = step(lo, p) * step(p, hi);
    return s.x * s.y;
}

// Masonry courses on a wall: bricks if the wall is a reddish colour, and blocks of dressed stone if not.
// `at` is meters along and up the wall, and the result is what to multiply the wall colour by.
vec3 courses(vec2 at, bool brick, vec3 base, float fine) {
    float course_h = brick ? 0.24 : 0.52;
    float unit_w = brick ? 0.5 : 0.95;
    float cy = at.y / course_h;
    float row = floor(cy);
    float x = at.x / unit_w + 0.5 * mod(row, 2.0);
    float tone = 0.9 + 0.2 * hash21(vec2(floor(x), row));
    float gap = max(step(fract(cy), brick ? 0.14 : 0.07), step(fract(x), brick ? 0.05 : 0.025));
    vec3 mortar = brick ? vec3(0.66, 0.62, 0.55) : base * 0.74;
    vec3 col = base * mix(1.0, tone, fine);
    return mix(col, mortar, gap * fine * (brick ? 0.85 : 0.7));
}

void main() {
    vec3 n = normalize(v_normal.xyz);
    // 10 added to the style says to draw it in shades of grey.
    float grey = step(9.5, v_normal.w);
    float style = v_normal.w - 10.0 * grey;

    // Light from the sun, wrapped a little past the line where it stops, and from the sky above and the
    // ground around, which is a warmer, darker light that comes up from below.
    float lit = clamp((dot(n, normalize(sun_dir)) + 0.15) / 1.15, 0.0, 1.0);
    vec3 sky = ambient * 1.2;
    vec3 ground_light = vec3(0.27, 0.243, 0.198);
    vec3 light = mix(ground_light, sky, n.y * 0.5 + 0.5) + sun_color * lit;

    float dist = length(v_world - cam_pos);
    // Windows and the like fade out with distance, where they would only shimmer, and the finer detail of
    // bricks and tiles sooner.
    float near = 1.0 - smoothstep(170.0, 440.0, dist);
    float fine = 1.0 - smoothstep(55.0, 200.0, dist);

    // Patchy ageing, in large and small patches.
    float wear = vnoise(v_world.xz * 0.45 + v_world.y * 0.9);
    float wear_fine = vnoise(v_world.xz * 2.3 + v_world.y * 3.1);
    vec3 base = v_color.rgb * (0.9 + 0.2 * wear + 0.07 * (wear_fine - 0.5) * fine);
    vec3 col = base * light;

    if (style > 0.5 && style < 1.5) {
        // Brick, or dressed stone, in storeys of 1.95 m with a window in each bay.
        bool brick = v_color.r > v_color.g * 1.3;
        float grime = mix(0.8, 1.0, smoothstep(0.6, 2.6, v_uv.y));
        col = courses(v_uv, brick, base, fine) * light * grime;

        vec2 cell = fract(vec2(v_uv.x / 1.55, (v_uv.y - 0.15) / 1.95));
        float win = box(cell, vec2(0.3, 0.26), vec2(0.7, 0.7));
        float frame = box(cell, vec2(0.26, 0.22), vec2(0.74, 0.74)) - win;
        float sill = box(cell, vec2(0.24, 0.2), vec2(0.76, 0.25));
        vec2 local = (cell - vec2(0.3, 0.26)) / vec2(0.4, 0.44);
        float bars = max(step(abs(local.x - 0.5), 0.035), step(abs(local.y - 0.58), 0.03));
        // Glass shows the sky, brighter towards the top, and is shaded under the head of the window.
        vec3 glass = mix(vec3(0.1, 0.15, 0.22), vec3(0.5, 0.64, 0.8), smoothstep(0.1, 1.0, local.y) * 0.75);
        glass *= (0.82 + 0.3 * hash21(floor(vec2(v_uv.x / 1.55, (v_uv.y - 0.15) / 1.95)))) * (1.0 - 0.35 * smoothstep(0.82, 1.0, local.y));
        vec3 trim = vec3(0.87, 0.84, 0.77) * light;
        col = mix(col, trim, max(frame, sill) * near);
        col = mix(col, glass, win * near);
        col = mix(col, trim * 0.92, win * bars * near);
    } else if (style > 1.5 && style < 2.5) {
        // Plaster between a frame of dark timber, with a window in each bay and shutters beside it.
        float grime = mix(0.82, 1.0, smoothstep(0.6, 2.4, v_uv.y));
        col = base * grime * light * (0.97 + 0.06 * wear_fine);
        vec2 cell = fract(v_uv / vec2(1.3, 1.2));
        float beam = max(max(step(cell.x, 0.08), step(0.92, cell.x)), step(cell.y, 0.1));
        // Braces, leaning one way or the other from bay to bay.
        vec2 bay = floor(v_uv / vec2(1.3, 1.2));
        float lean = hash21(bay) > 0.5 ? cell.x : 1.0 - cell.x;
        float brace = step(abs(cell.y - lean), 0.075) * step(0.3, abs(cell.x - 0.5)) ;
        vec3 timber = vec3(0.27, 0.19, 0.13) * (0.85 + 0.3 * hash21(bay + 3.7)) * light;
        float win = box(cell, vec2(0.38, 0.34), vec2(0.62, 0.74)) * step(0.5, v_uv.y);
        float shutter = (box(cell, vec2(0.28, 0.34), vec2(0.37, 0.74)) + box(cell, vec2(0.63, 0.34), vec2(0.72, 0.74))) * step(0.5, v_uv.y);
        vec2 local = (cell - vec2(0.38, 0.34)) / vec2(0.24, 0.4);
        float bars = max(step(abs(local.x - 0.5), 0.05), step(abs(local.y - 0.5), 0.04));
        vec3 glass = mix(vec3(0.12, 0.16, 0.22), vec3(0.45, 0.58, 0.72), smoothstep(0.0, 1.0, local.y) * 0.7);
        vec3 shutter_col = mix(vec3(0.3, 0.42, 0.3), vec3(0.45, 0.3, 0.25), step(0.5, hash21(bay + 9.1))) * light;
        col = mix(col, timber, max(beam, brace * 0.9) * near * 0.92);
        col = mix(col, shutter_col, shutter * near * (1.0 - beam));
        col = mix(col, glass, win * near * (1.0 - beam));
        col = mix(col, timber, win * bars * near * 0.9);
    } else if (style > 2.5 && style < 3.5) {
        // Large blocks of stone, with tall narrow windows, pointed at the top and of coloured glass.
        float grime = mix(0.78, 1.0, smoothstep(0.8, 3.0, v_uv.y));
        col = courses(v_uv * vec2(0.8, 0.9), false, base, fine) * light * grime;
        vec2 cell = fract(v_uv / vec2(2.0, 3.3));
        float ax = abs(cell.x - 0.5);
        float top = 0.62 + (0.08 - ax) * 3.4;
        float win = step(ax, 0.08) * step(0.3, cell.y) * step(cell.y, top);
        float frame = step(ax, 0.115) * step(0.28, cell.y) * step(cell.y, top + 0.04) - win;
        vec2 id = floor(v_uv / vec2(2.0, 3.3));
        float h = hash21(id);
        vec3 stained = h < 0.33 ? vec3(0.2, 0.28, 0.6) : (h < 0.66 ? vec3(0.62, 0.2, 0.22) : vec3(0.78, 0.6, 0.2));
        vec3 glass = mix(stained * 0.55, stained, smoothstep(0.3, top, cell.y)) * (0.8 + 0.4 * sin(cell.y * 50.0 + h * 9.0) * 0.25);
        col = mix(col, vec3(0.85, 0.82, 0.74) * light, frame * near);
        col = mix(col, glass, win * near);
    } else if (style > 3.5 && style < 4.5) {
        // Roof: courses of tiles or slates, each a different tone, with a dark line under each course.
        float cy = v_uv.y / 0.3;
        float row = floor(cy);
        float fr = fract(cy);
        float x = v_uv.x / 0.24 + 0.5 * mod(row, 2.0);
        float tile = hash21(vec2(floor(x), row));
        float line = smoothstep(0.0, 0.16, fr);
        float joint = smoothstep(0.0, 0.1, fract(x));
        float shade = mix(0.64, 1.0, line) * mix(0.86, 1.0, joint) * (0.88 + 0.16 * fr);
        float roughness = mix(0.9, (0.84 + 0.3 * tile) * shade, fine);
        col = base * roughness * light;
    } else if (style > 4.5) {
        // The front of a dormer: a small window, with a frame around it.
        float win = box(v_uv, vec2(0.28, 0.3), vec2(0.72, 0.74));
        float frame = box(v_uv, vec2(0.23, 0.25), vec2(0.77, 0.79)) - win;
        float bars = step(abs(v_uv.x - 0.5), 0.03);
        vec3 glass = mix(vec3(0.1, 0.15, 0.22), vec3(0.48, 0.62, 0.78), smoothstep(0.3, 0.74, v_uv.y) * 0.75);
        vec3 trim = vec3(0.87, 0.84, 0.77) * light;
        col = base * light * (0.97 + 0.06 * wear_fine);
        col = mix(col, trim, frame * near);
        col = mix(col, glass, win * near);
        col = mix(col, trim * 0.9, win * bars * near);
    }

    col = mix(col, tint.rgb, tint.a);
    float fog = pow(smoothstep(fog_range.x, fog_range.y, dist), 0.6);
    vec3 shown = mix(col, fog_color, fog);
    gl_FragColor = vec4(mix(shown, vec3(dot(shown, vec3(0.299, 0.587, 0.114))), grey), 1.0);
}
"#;

/// How much bigger than a building, as a share of how far away it is, it counts as being under the mouse.
/// A far city is a few pixels across, and would be hard to point at otherwise, though much more than this
/// makes the open ground round a city part of it.
const HOVER_SLOP: f32 = 0.004;

/// What is drawn of one city: its meshes, the size they were made for as its bits, and the space it takes.
struct Shape {
    size: u32,
    meshes: Vec<Mesh>,
    /// The same meshes to be drawn grey, made when they are first wanted.
    grey: Vec<Mesh>,
    bounds: Option<Bounds>,
    buildings: Vec<Bounds>,
}

/// The shape of `city` on `world`.
fn shape(city: &City, world: &World) -> Shape {
    let buildings = city_layout::buildings(city, world);
    Shape {
        size: city.size().to_bits(),
        grey: Vec::new(),
        bounds: city_pick::bounds(vec2(city.x, city.z), &buildings),
        buildings: city_pick::each(&buildings),
        meshes: city_mesh::build(&buildings).into_iter().map(|g| Mesh { vertices: g.vertices, indices: g.indices, texture: None }).collect(),
    }
}

/// A copy of a city's mesh that the shader draws grey, which it is told by 10 added to the style of each corner.
fn greyed(mesh: &Mesh) -> Mesh {
    let vertices = mesh.vertices.iter().map(|v| Vertex { normal: v.normal + vec4(0.0, 0.0, 0.0, 10.0), ..*v }).collect();
    Mesh { vertices, indices: mesh.indices.clone(), texture: None }
}

/// How long may be spent in a frame building the shapes of cities whose shapes are out of date.
const REBUILD_BUDGET: Duration = Duration::from_millis(4);

pub struct CityRenderer {
    material: Material,
    /// The shapes of each city, by its id.
    shapes: HashMap<CityId, Shape>,
}

impl CityRenderer {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
            MaterialParams {
                pipeline_params: render::opaque_pipeline(),
                uniforms: vec![
                    UniformDesc::new("cam_pos", UniformType::Float3),
                    UniformDesc::new("fog_color", UniformType::Float3),
                    UniformDesc::new("fog_range", UniformType::Float2),
                    UniformDesc::new("sun_dir", UniformType::Float3),
                    UniformDesc::new("sun_color", UniformType::Float3),
                    UniformDesc::new("ambient", UniformType::Float3),
                    UniformDesc::new("tint", UniformType::Float4),
                ],
                ..Default::default()
            },
        )
        .expect("city shader failed to compile");
        Self { material, shapes: HashMap::new() }
    }

    fn use_material(&self, camera: &Camera3D, tint: Vec4) {
        let m = &self.material;
        m.set_uniform("cam_pos", camera.position);
        m.set_uniform("fog_color", BACKGROUND);
        m.set_uniform("fog_range", render::fog_range(nworld_view::view_distance(camera.position.y)));
        m.set_uniform("sun_dir", render::sun_direction());
        m.set_uniform("sun_color", SUN_COLOR);
        m.set_uniform("ambient", AMBIENT);
        m.set_uniform("tint", tint);
        gl_use_material(m);
    }

    /// Draws `cities` as seen by `camera`, which must still be the camera set for the map. A city too far off to be seen, in
    /// the haze, is not drawn.
    pub fn draw(&mut self, world: &World, cities: &[City], camera: &Camera3D) {
        self.use_material(camera, Vec4::ZERO);
        let started = Instant::now();
        for city in cities.iter().filter(|c| crate::pieces::within_view(camera.position, c.x, c.z)) {
            // The shapes only change when the city's size does: a city with more people than it needs to be full size looks
            // the same. A city with none yet gets them at once. One whose shapes are out of date, as every city's are when a
            // new day changes the populations, gets new ones only while there is time for it this frame, and is drawn as it
            // was until then, so that the new day doesn't stall the game while every city is built again.
            let (built, current) = match self.shapes.get(&city.id) {
                Some(shape) => (true, shape.size == city.size().to_bits()),
                None => (false, false),
            };
            if !built || (!current && started.elapsed() < REBUILD_BUDGET) {
                self.shapes.insert(city.id, shape(city, world));
            }
            // A city stands in one region, and so is all grey or all in color.
            let grey = greyscale::greyed(world, city.x, city.z);
            if let Some(shape) = self.shapes.get_mut(&city.id) {
                if grey && shape.grey.is_empty() {
                    shape.grey = shape.meshes.iter().map(greyed).collect();
                }
                for mesh in if grey { &shape.grey } else { &shape.meshes } {
                    draw_mesh(mesh);
                }
            }
        }
        gl_use_default_material();
    }

    /// The city, of `cities`, that the ray from the camera through the mouse (as `nworld_view::mouse_ray`
    /// gives it) meets first, if any. It only knows the cities that have been drawn.
    pub fn city_under(&self, ray: (Vec3, Vec3), cities: &[City]) -> Option<CityId> {
        cities
            .iter()
            .filter_map(|city| {
                let shape = self.shapes.get(&city.id)?;
                let bounds = shape.bounds?;
                let middle = vec3(city.x, (bounds.low + bounds.high) * 0.5, city.z);
                let slop = ray.0.distance(middle) * HOVER_SLOP;
                city_pick::hit_city(ray.0, ray.1, &bounds, &shape.buildings, slop).map(|distance| (distance, city.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Shows where a new city would go if it were placed at `at`: the buildings it would start with,
    /// tinted green if it can go there and red if not, inside a ring and over a marker post that stay
    /// easy to see from far away.
    pub fn draw_site(&mut self, world: &World, at: Vec2, can_build: bool, camera: &Camera3D) {
        // The preview is not a real city, so it has ids that are never any city's.
        let city = City { id: CityId::from_u128(0), country: CountryId::from_u128(0), name: String::new(), x: at.x, z: at.y, region: None, population: INITIAL_POPULATION, under_siege: false };
        let tint = if can_build { vec4(0.3, 0.9, 0.4, 0.45) } else { vec4(0.95, 0.3, 0.3, 0.6) };
        self.use_material(camera, tint);
        for mesh in &shape(&city, world).meshes {
            draw_mesh(mesh);
        }
        gl_use_default_material();

        let ground = vec3(at.x, world.surface_height(at.x, at.y), at.y);
        let distance = camera.position.distance(ground);
        let radius = (distance * 0.015).max(28.0);
        let lift = (distance * 0.003).max(1.0);
        let color = if can_build { Color::new(0.55, 1.0, 0.6, 0.95) } else { Color::new(1.0, 0.4, 0.4, 0.95) };
        const SEGMENTS: usize = 64;
        let point = |k: usize| {
            let angle = std::f32::consts::TAU * k as f32 / SEGMENTS as f32;
            let (x, z) = (at.x + angle.cos() * radius, at.y + angle.sin() * radius);
            vec3(x, world.surface_height(x, z) + lift, z)
        };
        for k in 0..SEGMENTS {
            draw_line_3d(point(k), point(k + 1), color);
        }
        draw_line_3d(ground, ground + Vec3::Y * radius * 1.5, color);
    }
}
