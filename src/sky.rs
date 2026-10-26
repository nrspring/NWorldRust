//! Time-of-day lighting, the sky box, and the GLSL shaders that render sky and terrain.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

use crate::noise::smoothstep;

/// Sun position and the colors derived from it, shared by the sky and terrain shaders.
pub struct Environment {
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    pub horizon: Vec3,
    pub zenith: Vec3,
}

struct Palette {
    sun: Vec3,
    ambient: Vec3,
    horizon: Vec3,
    zenith: Vec3,
}

impl Palette {
    fn lerp(&self, other: &Palette, t: f32) -> Palette {
        Palette {
            sun: self.sun.lerp(other.sun, t),
            ambient: self.ambient.lerp(other.ambient, t),
            horizon: self.horizon.lerp(other.horizon, t),
            zenith: self.zenith.lerp(other.zenith, t),
        }
    }
}

impl Environment {
    /// `elevation` is the sun's angle above the horizon in radians.
    pub fn new(elevation: f32, azimuth: f32) -> Self {
        let day = Palette {
            sun: vec3(1.0, 0.95, 0.85),
            ambient: vec3(0.34, 0.39, 0.47),
            horizon: vec3(0.70, 0.80, 0.92),
            zenith: vec3(0.22, 0.42, 0.82),
        };
        let sunset = Palette {
            sun: vec3(1.0, 0.55, 0.25),
            ambient: vec3(0.26, 0.22, 0.28),
            horizon: vec3(0.95, 0.56, 0.32),
            zenith: vec3(0.20, 0.24, 0.50),
        };
        let night = Palette {
            sun: vec3(0.0, 0.0, 0.0),
            ambient: vec3(0.06, 0.08, 0.14),
            horizon: vec3(0.05, 0.07, 0.13),
            zenith: vec3(0.01, 0.02, 0.06),
        };

        let p = sunset
            .lerp(&day, smoothstep(0.02, 0.45, elevation))
            .lerp(&night, smoothstep(0.0, -0.2, elevation));

        Self {
            sun_dir: vec3(
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
                -elevation.cos() * azimuth.cos(),
            ),
            sun_color: p.sun,
            ambient: p.ambient,
            horizon: p.horizon,
            zenith: p.zenith,
        }
    }

    pub fn apply_to_sky(&self, mat: &Material, cam: Vec3) {
        mat.set_uniform("cam_pos", cam);
        mat.set_uniform("sun_dir", self.sun_dir);
        mat.set_uniform("sun_color", self.sun_color);
        mat.set_uniform("fog_color", self.horizon);
        mat.set_uniform("zenith_color", self.zenith);
    }

    pub fn apply_to_terrain(&self, mat: &Material, cam: Vec3, view_distance: f32, time: f32) {
        mat.set_uniform("cam_pos", cam);
        mat.set_uniform("sun_dir", self.sun_dir);
        mat.set_uniform("sun_color", self.sun_color);
        mat.set_uniform("ambient", self.ambient);
        mat.set_uniform("fog_color", self.horizon);
        mat.set_uniform("fog_range", vec2(view_distance * 0.3, view_distance * 0.97));
        mat.set_uniform("time", time);
    }
}

pub fn load_sky_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: SKY_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_write: false,
                depth_test: Comparison::Always,
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("cam_pos", UniformType::Float3),
                UniformDesc::new("sun_dir", UniformType::Float3),
                UniformDesc::new("sun_color", UniformType::Float3),
                UniformDesc::new("fog_color", UniformType::Float3),
                UniformDesc::new("zenith_color", UniformType::Float3),
            ],
            ..Default::default()
        },
    )
    .expect("sky shader failed to compile")
}

pub fn load_terrain_material() -> Material {
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: TERRAIN_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_write: true,
                depth_test: Comparison::LessOrEqual,
                ..Default::default()
            },
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

/// A cube centered on the camera; the sky shader colors it purely by view direction.
pub fn sky_box(center: Vec3) -> Mesh {
    const S: f32 = 50.0;
    let vertices = (0..8)
        .map(|i| Vertex {
            position: center
                + vec3(
                    if i & 1 == 0 { -S } else { S },
                    if i & 2 == 0 { -S } else { S },
                    if i & 4 == 0 { -S } else { S },
                ),
            uv: Vec2::ZERO,
            color: [255; 4],
            normal: Vec4::ZERO,
        })
        .collect();
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 3, 0, 3, 2,  4, 6, 7, 4, 7, 5,
        0, 4, 5, 0, 5, 1,  2, 3, 7, 2, 7, 6,
        0, 2, 6, 0, 6, 4,  1, 5, 7, 1, 7, 3,
    ];
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

const SKY_FRAGMENT: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 fog_color;
uniform vec3 zenith_color;

void main() {
    vec3 dir = normalize(v_world - cam_pos);
    float up = clamp(dir.y, 0.0, 1.0);
    vec3 col = mix(fog_color, zenith_color, pow(up, 0.45));

    float s = max(dot(dir, normalize(sun_dir)), 0.0);
    col += sun_color * (pow(s, 1200.0) * 3.0 + pow(s, 64.0) * 0.35 + pow(s, 6.0) * 0.12);

    gl_FragColor = vec4(col, 1.0);
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
    n = normalize(mix(n, wn, water));

    float diffuse = max(dot(n, sun), 0.0);
    vec3 col = v_color.rgb * (ambient + sun_color * diffuse);

    vec3 r = reflect(vdir, n);
    float spec = pow(max(dot(r, sun), 0.0), 90.0);
    float fresnel = pow(1.0 - max(dot(-vdir, n), 0.0), 4.0);
    vec3 water_col = mix(col, fog_color, fresnel * 0.65) + sun_color * spec * 1.4;
    col = mix(col, water_col, water);

    float fog = smoothstep(fog_range.x, fog_range.y, dist);
    gl_FragColor = vec4(mix(col, fog_color, fog), 1.0);
}
"#;
