//! The game's camera: looks down at a point on the ground from a distance, like a strategy game.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::MapView;

/// Closest and farthest the camera gets from the point it looks at, in meters.
const MIN_DISTANCE: f32 = 25.0;
/// How steeply it looks down: nearly along the ground to straight down, in radians.
const MIN_PITCH: f32 = 0.2;
const MAX_PITCH: f32 = 1.5;
/// The compass direction that looks north. The map's z runs from north to south, so north is towards -z.
pub const NORTH: f32 = std::f32::consts::PI;
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
    /// Whether the mouse button held down began on the map, and so is a drag of the map. A press that
    /// began on the interface, or on a popup, is not.
    grabbing: bool,
    looking: bool,
    /// A place and a distance the camera is flying to, until the player steers it somewhere else.
    goal: Option<(Vec2, f32)>,
    /// The compass direction and the tilt the camera is turning to, until the player steers it somewhere else.
    facing_goal: Option<(f32, f32)>,
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
            grabbing: false,
            looking: false,
            goal: None,
            facing_goal: None,
        }
    }

    /// For a frame the camera isn't updated in, such as while a popup is up: notes where the mouse is,
    /// so that moving it then isn't taken for a drag of the map afterwards.
    pub fn skip_frame(&mut self) {
        self.last_mouse = Vec2::from(mouse_position());
    }

    /// The compass direction being looked in, in radians. Zero looks towards +z, which is south on the map, and
    /// [`NORTH`] looks north.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Turns the camera, smoothly, to look straight down with north at the top of the screen, from where it is and
    /// as far away as it is. Any steering by the player stops it.
    pub fn reset_view(&mut self) {
        self.facing_goal = Some((NORTH, MAX_PITCH));
    }

    /// Flies the camera, smoothly, to look at the ground at `at` from `distance` meters away. Any
    /// steering by the player stops it.
    pub fn fly_to(&mut self, at: Vec2, distance: f32) {
        self.goal = Some((at, distance.clamp(MIN_DISTANCE, self.max_distance)));
    }

    /// Reacts to the keyboard and mouse, `dt` seconds after the last frame. The mouse is ignored
    /// while it is not `mouse_free`, as when it is over the interface.
    pub fn update(&mut self, world: &World, dt: f32, mouse_free: bool) {
        let key = |code| is_key_down(code) as i32 as f32;
        let pan = vec2(
            key(KeyCode::D) + key(KeyCode::Right) - key(KeyCode::A) - key(KeyCode::Left),
            key(KeyCode::W) + key(KeyCode::Up) - key(KeyCode::S) - key(KeyCode::Down),
        );
        let fast = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) { 3.0 } else { 1.0 };
        self.pan(pan * self.distance * 0.8 * fast * dt);

        let turn = key(KeyCode::E) - key(KeyCode::Q);
        self.yaw += turn * 1.4 * dt;

        let wheel = if mouse_free { mouse_wheel().1 } else { 0.0 };
        if wheel != 0.0 {
            self.distance *= 0.88f32.powf(wheel.clamp(-3.0, 3.0));
        }

        let mouse = Vec2::from(mouse_position());
        let drag = mouse - self.last_mouse;
        self.last_mouse = mouse;
        // A drag is of the map only if the button went down on it, and lasts until the button is let go.
        self.looking = is_mouse_button_down(MouseButton::Right) && (self.looking || (mouse_free && is_mouse_button_pressed(MouseButton::Right)));
        self.grabbing = is_mouse_button_down(MouseButton::Left) && (self.grabbing || (mouse_free && is_mouse_button_pressed(MouseButton::Left)));
        if mouse_free && self.looking {
            self.yaw += drag.x * 0.006;
            self.pitch = (self.pitch + drag.y * 0.006).clamp(MIN_PITCH, MAX_PITCH);
        }
        if mouse_free && self.grabbing {
            // Grab the ground: it follows the mouse.
            self.pan(vec2(-drag.x, drag.y) * self.distance * 0.0016);
        }

        if is_key_pressed(KeyCode::N) {
            self.reset_view();
        }
        // Turning or tilting by hand stops it turning to north on its own.
        if turn != 0.0 || (mouse_free && self.looking) {
            self.facing_goal = None;
        }
        if let Some(goal) = self.facing_goal {
            self.facing_goal = self.turn_towards(goal, dt);
        }

        let steering = pan != Vec2::ZERO || turn != 0.0 || wheel != 0.0 || (mouse_free && (self.grabbing || self.looking));
        if steering {
            self.goal = None;
        }
        if let Some((at, distance)) = self.goal {
            let ease = 1.0 - (-3.0 * dt).exp();
            self.focus += (at - self.focus) * ease;
            self.distance += (distance - self.distance) * ease;
            if at.distance(self.focus) < 1.0 && (distance - self.distance).abs() < 1.0 {
                self.goal = None;
            }
        }

        self.distance = self.distance.clamp(MIN_DISTANCE, self.max_distance);
        let half = world.half_extents();
        self.focus = self.focus.clamp(-half, half);
        let ground = world.surface_height(self.focus.x, self.focus.y);
        self.focus_height += (ground - self.focus_height) * (1.0 - (-8.0 * dt).exp());
    }

    /// Turns the camera a step, `dt` seconds' worth, towards looking in `goal` (a direction and a tilt), the short
    /// way round. Returns the goal while it hasn't got there, and `None` once it has.
    fn turn_towards(&mut self, goal: (f32, f32), dt: f32) -> Option<(f32, f32)> {
        let ease = 1.0 - (-8.0 * dt).exp();
        let turn = (goal.0 - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        self.yaw += turn * ease;
        self.pitch += (goal.1 - self.pitch) * ease;
        if turn.abs() < 0.002 && (goal.1 - self.pitch).abs() < 0.002 {
            self.yaw = goal.0;
            self.pitch = goal.1;
            None
        } else {
            Some(goal)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    fn camera() -> GameCamera {
        GameCamera::new(&test_support::small_continent())
    }

    /// Steps the camera to north up for a few seconds at 60 frames a second.
    fn settle(camera: &mut GameCamera) {
        for _ in 0..300 {
            if let Some(goal) = camera.facing_goal {
                camera.facing_goal = camera.turn_towards(goal, 1.0 / 60.0);
            }
        }
    }

    #[test]
    fn resetting_the_view_looks_straight_down_with_north_up_and_keeps_the_place_and_distance() {
        let mut camera = camera();
        camera.yaw = 1.0;
        camera.pitch = 0.4;
        camera.focus = vec2(120.0, -80.0);
        let distance = camera.distance;
        camera.reset_view();
        settle(&mut camera);
        assert_eq!((camera.yaw, camera.pitch), (NORTH, MAX_PITCH));
        assert_eq!((camera.focus, camera.distance), (vec2(120.0, -80.0), distance));
        assert!(camera.facing_goal.is_none(), "it is there, so it stops");
    }

    #[test]
    fn it_turns_the_short_way_round_whatever_the_turns_the_player_has_made() {
        // Just past north on one side, and a lot of turns round from it on the other: both are a short way.
        for start in [NORTH + 0.5, NORTH - 0.5, NORTH + 7.0 * std::f32::consts::TAU + 0.5, NORTH - 5.0 * std::f32::consts::TAU - 0.5] {
            let mut camera = camera();
            camera.yaw = start;
            camera.reset_view();
            let mut steps = 0;
            while let Some(goal) = camera.facing_goal {
                let before = camera.yaw;
                camera.facing_goal = camera.turn_towards(goal, 1.0 / 60.0);
                // Compared as directions: coming to rest at exactly north may be a whole number of turns from where
                // the player's turns had left the number.
                let moved = (camera.yaw - before + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                assert!(moved.abs() < 0.2, "it never spins the long way: {before} to {}", camera.yaw);
                steps += 1;
                assert!(steps < 1_000, "it gets there");
            }
            assert_eq!(camera.yaw, NORTH);
        }
    }

    #[test]
    fn it_turns_gradually_and_not_all_at_once() {
        let mut camera = camera();
        camera.yaw = 2.0;
        camera.pitch = 0.5;
        camera.reset_view();
        let goal = camera.facing_goal.unwrap();
        camera.facing_goal = camera.turn_towards(goal, 1.0 / 60.0);
        assert!(camera.yaw > 2.0 && camera.yaw < NORTH && camera.pitch > 0.5 && camera.pitch < MAX_PITCH, "{} {}", camera.yaw, camera.pitch);
        assert!(camera.facing_goal.is_some());
    }

    #[test]
    fn north_is_up_on_the_screen_when_looking_north() {
        // Looking in the direction NORTH, the camera's forward is -z, so a point to the north is ahead (up the screen).
        let forward = vec2(NORTH.sin(), NORTH.cos());
        assert!((forward - vec2(0.0, -1.0)).length() < 1e-5, "{forward}");
        // And the view straight down at it is the highest the camera is let tilt.
        assert!(MAX_PITCH > 1.4 && MAX_PITCH < std::f32::consts::FRAC_PI_2);
    }
}
