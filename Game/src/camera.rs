//! The game's camera: looks down at a point on the ground from a distance, like a strategy game.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::MapView;

/// Closest and farthest the camera gets from the point it looks at, in meters.
const MIN_DISTANCE: f32 = 25.0;
/// How steeply it looks down: nearly along the ground to straight down, in radians.
const MIN_PITCH: f32 = 0.2;
const MAX_PITCH: f32 = 1.5;
/// Keep the camera at least this far above the ground.
const MIN_CLEARANCE: f32 = 3.0;

pub struct GameCamera {
    /// The point on the ground being looked at, in world x and z.
    focus: Vec2,
    /// Its height, which follows the ground smoothly so the camera doesn't jerk over bumps.
    focus_height: f32,
    distance: f32,
    /// The compass direction being looked in, in radians. Zero looks towards +z.
    yaw: f32,
    pitch: f32,
    max_distance: f32,
    last_mouse: Vec2,
}

impl GameCamera {
    /// A camera over the middle of `world`, high enough to see all of it.
    pub fn new(world: &World) -> Self {
        let size = world.size();
        Self {
            focus: Vec2::ZERO,
            focus_height: 0.0,
            distance: size.max_element() * 0.55,
            yaw: 0.0,
            pitch: 1.15,
            max_distance: size.max_element() * 1.2,
            last_mouse: Vec2::ZERO,
        }
    }


    /// Reacts to the keyboard and mouse, `dt` seconds after the last frame.
    pub fn update(&mut self, world: &World, dt: f32) {
        let key = |code| is_key_down(code) as i32 as f32;
        let pan = vec2(
            key(KeyCode::D) + key(KeyCode::Right) - key(KeyCode::A) - key(KeyCode::Left),
            key(KeyCode::W) + key(KeyCode::Up) - key(KeyCode::S) - key(KeyCode::Down),
        );
        let fast = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) { 3.0 } else { 1.0 };
        self.pan(pan * self.distance * 0.8 * fast * dt);

        let turn = key(KeyCode::E) - key(KeyCode::Q);
        self.yaw += turn * 1.4 * dt;

        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            self.distance *= 0.88f32.powf(wheel.clamp(-3.0, 3.0));
        }

        let mouse = Vec2::from(mouse_position());
        let drag = mouse - self.last_mouse;
        self.last_mouse = mouse;
        if is_mouse_button_down(MouseButton::Right) {
            self.yaw += drag.x * 0.006;
            self.pitch = (self.pitch + drag.y * 0.006).clamp(MIN_PITCH, MAX_PITCH);
        }
        if is_mouse_button_down(MouseButton::Left) {
            // Grab the ground: it follows the mouse.
            self.pan(vec2(-drag.x, drag.y) * self.distance * 0.0016);
        }

        self.distance = self.distance.clamp(MIN_DISTANCE, self.max_distance);
        let half = world.half_extents();
        self.focus = self.focus.clamp(-half, half);
        let ground = world.surface_height(self.focus.x, self.focus.y);
        self.focus_height += (ground - self.focus_height) * (1.0 - (-8.0 * dt).exp());
    }

    /// Moves the focus by `amount`, where x is to the right and y is forwards on screen.
    fn pan(&mut self, amount: Vec2) {
        let forward = vec2(self.yaw.sin(), self.yaw.cos());
        let right = vec2(-forward.y, forward.x);
        self.focus += right * amount.x + forward * amount.y;
    }

    /// The camera to draw the map with.
    pub fn camera(&self, world: &World) -> Camera3D {
        let target = vec3(self.focus.x, self.focus_height, self.focus.y);
        let forward = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
        let mut position = target - forward * (self.pitch.cos() * self.distance) + Vec3::Y * (self.pitch.sin() * self.distance);
        // Never dip into the ground or the sea.
        position.y = position.y.max(world.surface_height(position.x, position.z) + MIN_CLEARANCE);
        let z_near = (position.y * 0.02).clamp(0.5, 20.0);
        MapView::camera(position, target, Vec3::Y, z_near)
    }
}
