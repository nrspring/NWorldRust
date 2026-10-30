//! Draws the cities in 3D, over the map. This belongs to the game: the map and its generic view know
//! nothing about cities. The shapes are made by [`crate::city_mesh`], and are lit by the same sun
//! and faded by the same fog as the terrain, so they sit in the scene.

use std::collections::HashMap;

use macroquad::models::{draw_mesh, Mesh};
use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::render::{self, AMBIENT, BACKGROUND, SUN_COLOR};

use crate::city::{City, INITIAL_POPULATION};
use crate::ids::{CityId, CountryId};
use crate::city_layout;
use crate::city_mesh;
use crate::city_pick::{self, Bounds};

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

void main() {
    vec3 n = normalize(v_normal.xyz);
    float lit = max(dot(n, normalize(sun_dir)), 0.0);
    vec3 light = ambient + sun_color * lit;
    vec3 col = v_color.rgb * light;

    // What is drawn on a wall, by its style in the fourth part of the normal: 1 for rows of windows,
    // 2 for a timber frame with small windows, 3 for stone with tall narrow windows, 0 for nothing.
    // The detail fades out with distance, where it would only shimmer.
    float dist = length(v_world - cam_pos);
    float style = v_normal.w;
    float near = 1.0 - smoothstep(170.0, 440.0, dist);
    if (style > 0.5 && style < 1.5) {
        vec2 cell = fract(v_uv / vec2(1.55, 1.95));
        float pane = step(0.3, cell.x) * step(cell.x, 0.7) * step(0.3, cell.y) * step(cell.y, 0.78) * step(0.95, v_uv.y);
        col = mix(col, col * vec3(0.38, 0.43, 0.52), pane * near * 0.85);
    } else if (style > 1.5 && style < 2.5) {
        vec2 cell = fract(v_uv / vec2(1.3, 1.2));
        // Posts and rails in dark timber, and a small window in each bay.
        float beam = max(max(step(cell.x, 0.08), step(0.92, cell.x)), step(cell.y, 0.1));
        float pane = step(0.36, cell.x) * step(cell.x, 0.64) * step(0.34, cell.y) * step(cell.y, 0.74) * step(0.5, v_uv.y);
        col = mix(col, vec3(0.27, 0.19, 0.13) * light, beam * near * 0.9);
        col = mix(col, col * vec3(0.3, 0.34, 0.42), pane * near * (1.0 - beam));
    } else if (style > 2.5) {
        vec2 cell = fract(v_uv / vec2(2.0, 3.3));
        float pane = step(0.42, cell.x) * step(cell.x, 0.58) * step(0.3, cell.y) * step(cell.y, 0.82) * step(1.1, v_uv.y);
        col = mix(col, col * vec3(0.3, 0.34, 0.42), pane * near);
    }

    col = mix(col, tint.rgb, tint.a);
    float fog = pow(smoothstep(fog_range.x, fog_range.y, dist), 0.6);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;

/// How much bigger than a city, as a share of how far away it is, it counts as being under the mouse.
/// A far city is a few pixels across, and would be hard to point at otherwise.
const HOVER_SLOP: f32 = 0.012;

/// What is drawn of one city: its meshes, the size they were made for as its bits, and the space it takes.
struct Shape {
    size: u32,
    meshes: Vec<Mesh>,
    bounds: Option<Bounds>,
}

/// The shape of `city` on `world`.
fn shape(city: &City, world: &World) -> Shape {
    let buildings = city_layout::buildings(city, world);
    Shape {
        size: city.size().to_bits(),
        bounds: city_pick::bounds(vec2(city.x, city.z), &buildings),
        meshes: city_mesh::build(&buildings).into_iter().map(|g| Mesh { vertices: g.vertices, indices: g.indices, texture: None }).collect(),
    }
}

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

    /// Draws `cities` as seen by `camera`, which must still be the camera set for the map.
    pub fn draw(&mut self, world: &World, cities: &[City], camera: &Camera3D) {
        self.use_material(camera, Vec4::ZERO);
        for city in cities {
            // The shapes only change when the city's size does: a city with more people than
            // it needs to be full size looks the same.
            let current = self.shapes.get(&city.id).is_some_and(|shape| shape.size == city.size().to_bits());
            if !current {
                self.shapes.insert(city.id, shape(city, world));
            }
            for mesh in &self.shapes[&city.id].meshes {
                draw_mesh(mesh);
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
                let bounds = self.shapes.get(&city.id)?.bounds?;
                let middle = vec3(city.x, (bounds.low + bounds.high) * 0.5, city.z);
                let slop = ray.0.distance(middle) * HOVER_SLOP;
                city_pick::hit(ray.0, ray.1, &bounds, slop).map(|distance| (distance, city.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Shows where a new city would go if it were placed at `at`: the buildings it would start with,
    /// tinted green if it can go there and red if not, inside a ring and over a marker post that stay
    /// easy to see from far away.
    pub fn draw_site(&mut self, world: &World, at: Vec2, can_build: bool, camera: &Camera3D) {
        // The preview is not a real city, so it has ids that are never any city's.
        let city = City { id: CityId::from_u128(0), country: CountryId::from_u128(0), name: String::new(), x: at.x, z: at.y, region: None, population: INITIAL_POPULATION };
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
