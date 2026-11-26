//! Lit models: the meshes that the units and the buildings are drawn as. A [`Model`] is built from shapes (boxes, columns and
//! cones, rods, balls and domes, roofs, and bent sheets for flags and sails) laid out in the model's own space, where 1 is the
//! radius of a token, the ground is at height 0 and the front is +x. It is placed on the map as it is built: moved to where it
//! stands, made as big as the token is, and turned. It is drawn with its own shader, lit by the same sun and sky as the terrain
//! and faded by the same haze, so that it has light and shade and sits in the scene. This is the game's own drawing, and not
//! part of drawing the map.

use std::cell::OnceCell;

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_view::render::{self, AMBIENT, BACKGROUND, SUN_COLOR};

/// How a surface takes the light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Wood, stone, cloth: no shine.
    Matte,
    /// Steel, gold, copper: a bright highlight where the sun glints off it, and darker away from it.
    Metal,
    /// Fire and lamps: not lit, but as bright as their color.
    Glow,
    /// Water, oil and paint: a small sharp highlight.
    Gloss,
}

impl Finish {
    fn code(self) -> f32 {
        match self {
            Finish::Matte => 0.0,
            Finish::Metal => 1.0,
            Finish::Glow => 2.0,
            Finish::Gloss => 3.0,
        }
    }
}

/// How high over the floor (in the model's own units) the dimming of the shapes nearest it fades out, and how dark it is right
/// at the floor: the shade that things get where they meet the ground.
const SHADE_REACH: f32 = 0.4;
const SHADE_LOW: f32 = 0.72;

/// A model being built: the shapes put in it so far, as triangles, already placed on the map.
pub struct Model {
    origin: Vec3,
    scale: f32,
    turn: Mat3,
    floor: f32,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl Model {
    /// A model whose own (0, 0, 0) is at `origin` on the map, `scale` meters to each of its units, turned by `yaw` about the
    /// upright (0 has its front, +x, to the east).
    #[cfg(test)]
    pub fn new(origin: Vec3, scale: f32, yaw: f32) -> Self {
        Self::turned(origin, scale, Mat3::from_rotation_y(yaw))
    }

    /// A model as [`Model::new`], turned by `turn`, which may lean it as well.
    pub fn turned(origin: Vec3, scale: f32, turn: Mat3) -> Self {
        Self { origin, scale, turn, floor: 0.0, vertices: Vec::new(), indices: Vec::new() }
    }

    /// Sets the height of the floor that the shapes put in from now on stand on, for the shade where they meet it.
    pub fn set_floor(&mut self, floor: f32) {
        self.floor = floor;
    }

    /// How many corners it has.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    /// The box that holds it, on the map: its lowest and its highest corner.
    #[cfg(test)]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.vertices.iter().fold((Vec3::MAX, Vec3::MIN), |(low, high), v| (low.min(v.position), high.max(v.position)))
    }

    /// Builds, with `build`, a part of it that is `factor` times as big, with its own (0, 0, 0) at `at`: a figure drawn to its
    /// own measure, for one.
    pub fn scaled(&mut self, at: Vec3, factor: f32, build: impl FnOnce(&mut Model)) {
        let (origin, scale, floor) = (self.origin, self.scale, self.floor);
        self.origin = self.place(at);
        self.scale *= factor;
        self.floor = (floor - at.y) / factor;
        build(self);
        (self.origin, self.scale, self.floor) = (origin, scale, floor);
    }

    /// Where a point of the model's own space is on the map.
    pub fn place(&self, p: Vec3) -> Vec3 {
        self.origin + self.turn * (p * self.scale)
    }

    fn vertex(&mut self, p: Vec3, n: Vec3, color: Color, finish: Finish) -> u16 {
        let t = ((p.y - self.floor) / SHADE_REACH).clamp(0.0, 1.0);
        let shade = SHADE_LOW + (1.0 - SHADE_LOW) * t * t * (3.0 - 2.0 * t);
        let shade = if finish == Finish::Glow { 1.0 } else { shade };
        let byte = |c: f32| (c * shade * 255.0).clamp(0.0, 255.0) as u8;
        let position = self.place(p);
        let normal = (self.turn * n).normalize_or_zero();
        self.vertices.push(Vertex { position, uv: Vec2::ZERO, color: [byte(color.r), byte(color.g), byte(color.b), 255], normal: normal.extend(finish.code()) });
        (self.vertices.len() - 1) as u16
    }

    /// A flat triangle.
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color, finish: Finish) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let first = self.vertex(a, n, color, finish);
        self.vertex(b, n, color, finish);
        self.vertex(c, n, color, finish);
        self.indices.extend([first, first + 1, first + 2]);
    }

    /// A flat four-sided face, with its corners in order round it.
    pub fn quad(&mut self, corners: [Vec3; 4], color: Color, finish: Finish) {
        let n = (corners[1] - corners[0]).cross(corners[3] - corners[0]);
        let n = if n.length_squared() > 1e-12 { n.normalize() } else { (corners[2] - corners[1]).cross(corners[0] - corners[1]).normalize_or_zero() };
        self.quad_facing(corners, n, color, finish);
    }

    fn quad_facing(&mut self, corners: [Vec3; 4], n: Vec3, color: Color, finish: Finish) {
        let first = self.vertex(corners[0], n, color, finish);
        for corner in &corners[1..] {
            self.vertex(*corner, n, color, finish);
        }
        self.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    /// A box with its middle at `middle`, its sides along `axes` (each of length 1), and `size` along each of them.
    pub fn oriented_box(&mut self, middle: Vec3, axes: Mat3, size: Vec3, color: Color, finish: Finish) {
        let half = [axes.x_axis * size.x * 0.5, axes.y_axis * size.y * 0.5, axes.z_axis * size.z * 0.5];
        for axis in 0..3 {
            let (u, v) = (half[(axis + 1) % 3], half[(axis + 2) % 3]);
            for sign in [-1.0, 1.0] {
                let c = middle + half[axis] * sign;
                let n = half[axis].normalize_or_zero() * sign;
                self.quad_facing([c - u - v, c + u - v, c + u + v, c - u + v], n, color, finish);
            }
        }
    }

    /// A box with its middle at `middle`, square to the model.
    pub fn cuboid(&mut self, middle: Vec3, size: Vec3, color: Color, finish: Finish) {
        self.oriented_box(middle, Mat3::IDENTITY, size, color, finish);
    }

    /// A box standing on `foot`, the middle of its underside.
    pub fn block(&mut self, foot: Vec3, size: Vec3, color: Color) {
        self.cuboid(foot + Vec3::Y * size.y * 0.5, size, color, Finish::Matte);
    }

    /// A box standing on `foot` and turned by `yaw` about the upright.
    pub fn block_turned(&mut self, foot: Vec3, size: Vec3, yaw: f32, color: Color) {
        self.oriented_box(foot + Vec3::Y * size.y * 0.5, Mat3::from_rotation_y(yaw), size, color, Finish::Matte);
    }

    /// A round (or, with few `sides`, many-sided) shape from `a` to `b`, `ra` across at `a` and `rb` at `b`, with its ends closed.
    /// The first corner is `twist` round from the side. With many sides it is shaded smooth.
    pub fn frustum(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, sides: usize, twist: f32, color: Color, finish: Finish) {
        let axis = b - a;
        let length = axis.length();
        if length < 1e-6 {
            return;
        }
        let dir = axis / length;
        let helper = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let u = dir.cross(helper).normalize();
        let w = dir.cross(u);
        let around = |k: usize| {
            let angle = twist + std::f32::consts::TAU * k as f32 / sides as f32;
            u * angle.cos() + w * angle.sin()
        };
        let smooth = sides >= 10;
        let slope = (ra - rb) / length;
        if smooth {
            // The corners of each side are shared with the next, so that it is shaded as one round surface.
            let first = self.vertices.len() as u16;
            for k in 0..=sides {
                let r = around(k);
                let n = (r + dir * slope).normalize();
                self.vertex(a + r * ra, n, color, finish);
                self.vertex(b + r * rb, n, color, finish);
            }
            for k in 0..sides as u16 {
                let i = first + k * 2;
                self.indices.extend([i, i + 2, i + 3, i, i + 3, i + 1]);
            }
        } else {
            for k in 0..sides {
                let (r0, r1) = (around(k), around(k + 1));
                let mid = (r0 + r1).normalize();
                self.quad_facing([a + r0 * ra, a + r1 * ra, b + r1 * rb, b + r0 * rb], (mid + dir * slope).normalize(), color, finish);
            }
        }
        for (centre, radius, n) in [(a, ra, -dir), (b, rb, dir)] {
            if radius > 1e-4 {
                let middle = self.vertex(centre, n, color, finish);
                for k in 0..sides {
                    self.vertex(centre + around(k) * radius, n, color, finish);
                }
                for k in 0..sides as u16 {
                    self.indices.extend([middle, middle + 1 + k, middle + 1 + (k + 1) % sides as u16]);
                }
            }
        }
    }

    /// An upright column (or a cone, with a top radius of 0) standing on `foot`.
    pub fn column(&mut self, foot: Vec3, r0: f32, r1: f32, height: f32, sides: usize, color: Color, finish: Finish) {
        self.frustum(foot, foot + Vec3::Y * height, r0, r1, sides, 0.0, color, finish);
    }

    /// A rod of `radius` from `a` to `b`.
    pub fn rod(&mut self, a: Vec3, b: Vec3, radius: f32, color: Color) {
        self.frustum(a, b, radius, radius, 6, 0.0, color, Finish::Matte);
    }

    /// A ring lying flat, standing on `foot`, from `inner` to `outer` across and `height` high.
    pub fn ring(&mut self, foot: Vec3, inner: f32, outer: f32, height: f32, sides: usize, color: Color, finish: Finish) {
        let at = |k: usize, r: f32, y: f32| {
            let angle = std::f32::consts::TAU * k as f32 / sides as f32;
            foot + vec3(angle.cos() * r, y, angle.sin() * r)
        };
        for k in 0..sides {
            self.quad_facing([at(k, inner, height), at(k, outer, height), at(k + 1, outer, height), at(k + 1, inner, height)], Vec3::Y, color, finish);
            let out = (at(k, 1.0, 0.0) + at(k + 1, 1.0, 0.0) - foot * 2.0).normalize();
            self.quad_facing([at(k, outer, 0.0), at(k + 1, outer, 0.0), at(k + 1, outer, height), at(k, outer, height)], out, color, finish);
            self.quad_facing([at(k, inner, 0.0), at(k + 1, inner, 0.0), at(k + 1, inner, height), at(k, inner, height)], -out, color, finish);
        }
    }

    /// A ball, stretched to `radii` along each axis, with its middle at `middle`. With few `slices` it is a rough, cut stone.
    pub fn ball(&mut self, middle: Vec3, radii: Vec3, slices: usize, color: Color, finish: Finish) {
        self.globe(middle, radii, slices, (slices / 2).max(3), false, color, finish);
    }

    /// The top half of a ball, sitting on `foot`.
    pub fn dome(&mut self, foot: Vec3, radii: Vec3, slices: usize, color: Color, finish: Finish) {
        self.globe(foot, radii, slices, (slices / 4).max(2), true, color, finish);
    }

    fn globe(&mut self, middle: Vec3, radii: Vec3, slices: usize, rings: usize, half: bool, color: Color, finish: Finish) {
        let smooth = slices >= 10;
        let point = |ring: usize, slice: usize| {
            let lat = if half { std::f32::consts::FRAC_PI_2 * (1.0 - ring as f32 / rings as f32) } else { std::f32::consts::PI * (0.5 - ring as f32 / rings as f32) };
            let lon = std::f32::consts::TAU * slice as f32 / slices as f32;
            let unit = vec3(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
            (middle + unit * radii, (unit / radii).normalize())
        };
        if smooth {
            // A grid of corners, each shared by the faces round it.
            let first = self.vertices.len() as u16;
            for ring in 0..=rings {
                for slice in 0..=slices {
                    let (p, n) = point(ring, slice);
                    self.vertex(p, n, color, finish);
                }
            }
            let row = slices as u16 + 1;
            for ring in 0..rings as u16 {
                for slice in 0..slices as u16 {
                    let i = first + ring * row + slice;
                    self.indices.extend([i, i + 1, i + row + 1, i, i + row + 1, i + row]);
                }
            }
            return;
        }
        for ring in 0..rings {
            for slice in 0..slices {
                let c = [point(ring, slice), point(ring, slice + 1), point(ring + 1, slice + 1), point(ring + 1, slice)];
                {
                    let n = (c[0].1 + c[1].1 + c[2].1 + c[3].1).normalize();
                    self.quad_facing([c[0].0, c[1].0, c[2].0, c[3].0], n, color, finish);
                }
            }
        }
    }

    /// A pitched roof on `foot` (the middle of its underside): `length` along its ridge, which runs along x turned by `yaw`,
    /// `width` across and `height` from the eaves to the ridge. The ends are in `gable`, the color of the walls under it.
    pub fn roof(&mut self, foot: Vec3, length: f32, width: f32, height: f32, yaw: f32, color: Color, gable: Color) {
        let turn = Mat3::from_rotation_y(yaw);
        let at = |x: f32, y: f32, z: f32| foot + turn * vec3(x, y, z);
        let (l, w) = (length * 0.5, width * 0.5);
        let (ridge0, ridge1) = (at(-l, height, 0.0), at(l, height, 0.0));
        for side in [-1.0, 1.0] {
            self.quad([at(-l, 0.0, w * side), at(l, 0.0, w * side), ridge1, ridge0], color, Finish::Matte);
            // The thickness of the eaves, as a dark edge.
            self.quad([at(-l, 0.0, w * side), at(l, 0.0, w * side), at(l, -0.03, w * side), at(-l, -0.03, w * side)], shade(color, 0.6), Finish::Matte);
        }
        for end in [-1.0, 1.0] {
            self.tri(at(l * end, 0.0, -w * 0.9), at(l * end, 0.0, w * 0.9), at(l * end * 0.97, height * 0.93, 0.0), gable, Finish::Matte);
        }
    }

    /// A bent sheet, such as a flag or a sail (which, like every face, is lit on the side that is seen): `nu` by `nv` faces, with the point at (`u`, `v`), each from 0
    /// to 1, where `shape` puts it, and the face round it in the color `color` gives for its middle.
    pub fn sheet(&mut self, nu: usize, nv: usize, shape: impl Fn(f32, f32) -> Vec3, color: impl Fn(f32, f32) -> Color, finish: Finish) {
        let at = |i: usize, j: usize| shape(i as f32 / nu as f32, j as f32 / nv as f32);
        for i in 0..nu {
            for j in 0..nv {
                let c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                let n = (c[1] - c[0]).cross(c[3] - c[0]) + (c[2] - c[3]).cross(c[2] - c[1]);
                let n = n.normalize_or_zero();
                let col = color((i as f32 + 0.5) / nu as f32, (j as f32 + 0.5) / nv as f32);
                self.quad_facing(c, n, col, finish);
            }
        }
    }

    /// Has it drawn in shades of grey. (The shader is told by 10 added to the number of each corner's finish.)
    pub fn grey(&mut self) {
        for vertex in &mut self.vertices {
            if vertex.normal.w < GREY_FINISH {
                vertex.normal.w += GREY_FINISH;
            }
        }
    }

    /// Draws it, as seen by the camera that is set, with the material from [`begin`].
    pub fn draw(self) {
        if !self.indices.is_empty() {
            draw_mesh(&Mesh { vertices: self.vertices, indices: self.indices, texture: None });
        }
    }
}

/// `color` made lighter (over 1) or darker (under 1).
pub fn shade(color: Color, by: f32) -> Color {
    Color::new((color.r * by).min(1.0), (color.g * by).min(1.0), (color.b * by).min(1.0), color.a)
}

/// What is added to the number of a corner's finish to have it drawn grey, which is more than any finish's number.
const GREY_FINISH: f32 = 10.0;

const VERTEX: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
attribute vec3 position;
attribute vec2 texcoord;
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

const FRAGMENT: &str = r#"#version 100
#ifdef GL_ES
precision highp float;
#endif
varying vec4 v_color;
varying vec4 v_normal;
varying vec3 v_world;

uniform vec3 cam_pos;
uniform vec3 fog_color;
uniform vec2 fog_range;
uniform vec3 sun_dir;
uniform vec3 sun_color;
uniform vec3 ambient;
// A colour to mix over everything, and how much of it, for showing where something would be built, or what is to be pulled down.
uniform vec4 tint;

void main() {
    vec3 to_eye = normalize(cam_pos - v_world);
    // A face is lit on the side that is seen: a sheet such as a sail has only the one side.
    vec3 n = normalize(v_normal.xyz);
    n = dot(n, to_eye) < 0.0 ? -n : n;
    // 10 added to the finish says to draw it in shades of grey.
    float grey = step(9.5, v_normal.w);
    float finish = v_normal.w - 10.0 * grey;
    vec3 l = normalize(sun_dir);
    vec3 base = v_color.rgb;

    // Light from the sun, wrapped a little past the line where it stops, and from the sky above and the ground around, a warmer,
    // darker light that comes up from below.
    float lit = clamp((dot(n, l) + 0.2) / 1.2, 0.0, 1.0);
    vec3 sky = ambient * 1.3;
    vec3 ground_light = vec3(0.30, 0.27, 0.22);
    vec3 light = mix(ground_light, sky, n.y * 0.5 + 0.5) + sun_color * lit;
    // A rim of sky light along the edges that face away, so that the outline of a piece stands out against the ground.
    float rim = pow(1.0 - max(dot(n, to_eye), 0.0), 3.0);
    vec3 h = normalize(l + to_eye);
    float facing = max(dot(n, h), 0.0);

    vec3 col = base * light + sky * rim * 0.22;
    if (finish > 0.5 && finish < 1.5) {
        // Metal: darker where the sun doesn't catch it, and a broad bright glint where it does.
        col = base * (light * 0.7 + sky * 0.25) + sun_color * pow(facing, 22.0) * 0.75 + base * pow(facing, 6.0) * 0.35 + sky * rim * 0.35;
    } else if (finish > 1.5 && finish < 2.5) {
        col = base * 1.15;
    } else if (finish > 2.5) {
        col += sun_color * pow(facing, 70.0) * 0.6 + sky * rim * 0.2;
    }

    col = mix(col, tint.rgb, tint.a);
    float fog = pow(smoothstep(fog_range.x, fog_range.y, length(v_world - cam_pos)), 0.6);
    vec3 shown = mix(col, fog_color, fog);
    gl_FragColor = vec4(mix(shown, vec3(dot(shown, vec3(0.299, 0.587, 0.114))), grey), 1.0);
}
"#;

thread_local! {
    static MATERIAL: OnceCell<Material> = const { OnceCell::new() };
}

fn with_material(then: impl FnOnce(&Material)) {
    MATERIAL.with(|cell| {
        let material = cell.get_or_init(|| {
            load_material(
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
            .expect("model shader failed to compile")
        });
        then(material);
    });
}

/// Sets the material that models are drawn with, for a camera at `eye`, with `tint` (a color, and how much of it) mixed over
/// them. Call [`end`] after drawing them.
pub fn begin(eye: Vec3, tint: Vec4) {
    with_material(|m| {
        m.set_uniform("cam_pos", eye);
        m.set_uniform("fog_color", BACKGROUND);
        m.set_uniform("fog_range", render::fog_range(nworld_view::view_distance(eye.y)));
        m.set_uniform("sun_dir", render::sun_direction());
        m.set_uniform("sun_color", SUN_COLOR);
        m.set_uniform("ambient", AMBIENT);
        m.set_uniform("tint", tint);
        gl_use_material(m);
    });
}

/// Goes back to the ordinary material, after models have been drawn.
pub fn end() {
    gl_use_default_material();
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn a_model_is_placed_where_it_stands_made_as_big_as_it_is_and_turned() {
        let mut m = Model::new(vec3(100.0, 5.0, -20.0), 10.0, std::f32::consts::FRAC_PI_2);
        m.block(Vec3::ZERO, vec3(2.0, 1.0, 2.0), WHITE);
        assert_eq!(m.len(), 24, "six faces of four corners");
        let (low, high) = m.bounds();
        assert!((low - vec3(90.0, 5.0, -30.0)).length() < 1e-3 && (high - vec3(110.0, 15.0, -10.0)).length() < 1e-3, "{low} {high}");
        // A quarter turn takes the front (+x) to the north (-z).
        let front = m.place(vec3(1.0, 0.0, 0.0));
        assert!((front - vec3(100.0, 5.0, -30.0)).length() < 1e-3, "{front}");
    }

    #[test]
    fn things_are_darker_where_they_meet_the_floor() {
        let mut m = Model::new(Vec3::ZERO, 1.0, 0.0);
        m.column(Vec3::ZERO, 0.5, 0.5, 2.0, 12, WHITE, Finish::Matte);
        let low = m.vertices.iter().filter(|v| v.position.y < 0.01).map(|v| v.color[0]).max().unwrap();
        let high = m.vertices.iter().filter(|v| v.position.y > 1.99).map(|v| v.color[0]).min().unwrap();
        assert!(low < high, "{low} under {high}");
        m.set_floor(2.0);
        m.column(vec3(0.0, 2.0, 0.0), 0.2, 0.0, 1.0, 12, WHITE, Finish::Glow);
        assert!(m.vertices.iter().rev().take(10).all(|v| v.color[0] == 255), "what glows isn't shaded");
    }

    #[test]
    fn a_part_can_be_built_to_its_own_measure() {
        let mut m = Model::new(vec3(10.0, 0.0, 0.0), 2.0, 0.0);
        m.scaled(vec3(1.0, 1.0, 0.0), 3.0, |m| m.block(Vec3::ZERO, Vec3::ONE, WHITE));
        let (low, high) = m.bounds();
        assert!((low - vec3(9.0, 2.0, -3.0)).length() < 1e-4 && (high - vec3(15.0, 8.0, 3.0)).length() < 1e-4, "{low} {high}");
        assert_eq!(m.place(Vec3::X), vec3(12.0, 0.0, 0.0), "and then it is as it was");
    }

    /// A sum of where all its corners are, to tell whether anything in it has moved.
    pub fn sum(m: &Model) -> f32 {
        m.vertices.iter().map(|v| v.position.x + v.position.y * 3.0 + v.position.z * 7.0).sum()
    }

    #[test]
    fn a_sheet_is_a_face_for_each_part_and_a_cone_has_no_top() {
        let mut m = Model::new(Vec3::ZERO, 1.0, 0.0);
        m.sheet(3, 2, |u, v| vec3(u, v, 0.0), |_, _| WHITE, Finish::Matte);
        assert_eq!(m.len(), 3 * 2 * 4);
        let mut cone = Model::new(Vec3::ZERO, 1.0, 0.0);
        cone.column(Vec3::ZERO, 1.0, 0.0, 1.0, 8, WHITE, Finish::Matte);
        assert_eq!(cone.len(), 8 * 4 + 8 + 1, "the sides and the bottom");
        let (low, high) = cone.bounds();
        assert!(low.y.abs() < 1e-5 && (high.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colors_are_made_lighter_and_darker_without_going_over() {
        let c = shade(Color::new(0.5, 0.8, 1.0, 1.0), 1.5);
        assert_eq!((c.r, c.g, c.b), (0.75, 1.0, 1.0));
    }
}
