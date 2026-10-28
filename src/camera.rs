//! Free-moving editor camera: holds still until you move it, with an animated top-down overview.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::world::World;

pub const FOV_DEGREES: f32 = 65.0;
const MIN_CLEARANCE: f32 = 2.0;
const LOOK_RATE: f32 = 1.4;
const MOUSE_LOOK: f32 = 0.004;
const FAST_MULTIPLIER: f32 = 4.0;
const TRANSITION_SECONDS: f32 = 0.9;
/// The angled view looks this far below the horizon.
const ANGLED_PITCH_DEGREES: f32 = 20.0;
/// The angled view keeps at least this far from what it looks at.
const MIN_ANGLED_DISTANCE: f32 = 40.0;
/// Extra room around the world when framing it from above.
const OVERVIEW_MARGIN: f32 = 1.08;

#[derive(Clone, Copy)]
struct Pose {
    pos: Vec3,
    /// Radians; 0 faces -Z ("north"), increasing turns right.
    yaw: f32,
    pitch: f32,
}

struct Transition {
    from: Pose,
    to: Pose,
    t: f32,
}

pub struct EditorCamera {
    pose: Pose,
    transition: Option<Transition>,
    /// Where to return when leaving the overview; cleared once the camera is moved away from it.
    before_overview: Option<Pose>,
    last_mouse: Vec2,
    /// A left-button drag that pans the view is under way.
    panning: bool,
}

fn axis(positive: &[KeyCode], negative: &[KeyCode]) -> f32 {
    let held = |keys: &[KeyCode]| keys.iter().any(|&k| is_key_down(k));
    held(positive) as i32 as f32 - held(negative) as i32 as f32
}

/// Signed shortest angular difference from `a` to `b`.
fn angle_delta(a: f32, b: f32) -> f32 {
    (b - a + PI).rem_euclid(TAU) - PI
}

/// The unit vector a camera with this yaw and pitch looks along.
fn look_direction(yaw: f32, pitch: f32) -> Vec3 {
    vec3(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos())
}

/// Where the angled view goes from `from`: looking at the same ground, from the same distance,
/// but 20 degrees below the horizon and kept inside the map's limits.
fn angled_pose(from: Pose, world: &World) -> Pose {
    let forward = look_direction(from.yaw, from.pitch);
    // The ground point at the center of the view; with the camera looking level or up there
    // isn't one, so pivot around a spot ahead of it.
    let mut focus = from.pos + forward * 500.0;
    if forward.y < -0.01 {
        let mut ground = 0.0;
        for _ in 0..4 {
            let t = ((from.pos.y - ground) / -forward.y).max(0.0);
            focus = from.pos + forward * t;
            ground = world.surface_height(focus.x, focus.z);
        }
    }
    let distance = from.pos.distance(focus).max(MIN_ANGLED_DISTANCE);

    let pitch = -ANGLED_PITCH_DEGREES.to_radians();
    let mut pos = focus - look_direction(from.yaw, pitch) * distance;
    let limit = world.half_extents() * 1.5;
    pos.x = pos.x.clamp(-limit.x, limit.x);
    pos.z = pos.z.clamp(-limit.y, limit.y);
    let floor = world.surface_height(pos.x, pos.z) + MIN_CLEARANCE;
    pos.y = pos.y.clamp(floor, world.size().max_element() * 2.0);
    Pose { pos, yaw: from.yaw, pitch }
}

impl EditorCamera {
    /// A camera for a freshly opened map: already in the top-down overview, so the whole map is
    /// in view, with the free-camera starting pose to return to. `side_inset` is as for
    /// [`toggle_overview`](Self::toggle_overview).
    pub fn new(world: &World, side_inset: f32) -> Self {
        Self {
            pose: Self::overview_pose(world, side_inset),
            transition: None,
            before_overview: Some(Self::start_pose(world)),
            last_mouse: mouse_position().into(),
            panning: false,
        }
    }

    /// Low over the southern part of the world, looking north.
    fn start_pose(world: &World) -> Pose {
        Pose {
            pos: vec3(0.0, 150.0, world.half_extents().y * 0.6),
            yaw: 0.0,
            pitch: -0.25,
        }
    }

    pub fn position(&self) -> Vec3 {
        self.pose.pos
    }

    pub fn in_overview(&self) -> bool {
        self.before_overview.is_some()
    }

    pub fn forward(&self) -> Vec3 {
        let Pose { yaw, pitch, .. } = self.pose;
        vec3(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos())
    }

    pub fn reset(&mut self, world: &World) {
        self.fly_to(Self::start_pose(world));
        self.before_overview = None;
    }

    fn fly_to(&mut self, to: Pose) {
        self.transition = Some(Transition {
            from: self.pose,
            to,
            t: 0.0,
        });
    }

    /// A straight-down view, north up, that frames the whole world. `side_inset` is the width in
    /// pixels covered by UI along a side of the screen, which the framing keeps the world clear
    /// of.
    fn overview_pose(world: &World, side_inset: f32) -> Pose {
        // Looking straight down with north up, the world's height fills the screen vertically
        // and its width horizontally; climb until both fit.
        let tan_half = (FOV_DEGREES.to_radians() * 0.5).tan();
        // The view stays centered, so the inset is kept clear on both sides.
        let aspect = (screen_width() - 2.0 * side_inset) / screen_height();
        let half = world.half_extents() * OVERVIEW_MARGIN;
        let height = (half.y / tan_half).max(half.x / (tan_half * aspect))
            + world.max_surface_height();
        Pose {
            pos: vec3(0.0, height, 0.0),
            yaw: 0.0,
            pitch: -FRAC_PI_2,
        }
    }

    /// Flies to the top-down overview, or back to where the camera was if it is still sitting
    /// in that overview.
    pub fn toggle_overview(&mut self, world: &World, side_inset: f32) {
        if let Some(previous) = self.before_overview.take() {
            self.fly_to(previous);
            return;
        }
        self.before_overview = Some(self.transition.as_ref().map_or(self.pose, |t| t.to));
        self.fly_to(Self::overview_pose(world, side_inset));
    }

    /// Flies to a low angled view: the camera drops back and down so it looks 20 degrees below
    /// the horizon at the same spot as now, from the same distance as far as the map's limits
    /// allow. The next [`toggle_overview`](Self::toggle_overview) goes to the overview, and the
    /// one after returns here.
    pub fn angled_view(&mut self, world: &World) {
        let from = self.transition.as_ref().map_or(self.pose, |t| t.to);
        self.fly_to(angled_pose(from, world));
        self.before_overview = None;
    }

    /// `keyboard` and `mouse` say whether those devices drive the camera this frame; they are
    /// off while the editor UI is using them (typing into a field, pointer over a panel).
    /// `mouse_look` says whether dragging with the right button turns the camera; a tool can
    /// claim that button, and the arrow keys still look around. `pan` says whether a drag with
    /// the left button, started now, pans the view: it grabs the ground and moves it with the
    /// pointer.
    pub fn update(&mut self, dt: f32, world: &World, keyboard: bool, mouse: bool, mouse_look: bool, pan: bool) {
        let keys = |positive: &[KeyCode], negative: &[KeyCode]| {
            if keyboard { axis(positive, negative) } else { 0.0 }
        };
        let mouse_pos: Vec2 = mouse_position().into();
        let mouse_delta = mouse_pos - self.last_mouse;
        self.last_mouse = mouse_pos;

        if is_mouse_button_pressed(MouseButton::Left) {
            self.panning = pan && mouse && self.transition.is_none();
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.panning = false;
        }

        if let Some(tr) = &mut self.transition {
            tr.t = (tr.t + dt / TRANSITION_SECONDS).min(1.0);
            let s = smoothstep(0.0, 1.0, tr.t);
            self.pose = Pose {
                pos: tr.from.pos.lerp(tr.to.pos, s),
                yaw: tr.from.yaw + angle_delta(tr.from.yaw, tr.to.yaw) * s,
                pitch: tr.from.pitch + (tr.to.pitch - tr.from.pitch) * s,
            };
            if tr.t >= 1.0 {
                self.transition = None;
            }
            return;
        }

        let mut turn = keys(&[KeyCode::Right], &[KeyCode::Left]) * LOOK_RATE * dt;
        let mut tilt = keys(&[KeyCode::Up], &[KeyCode::Down]) * LOOK_RATE * dt;
        if mouse && mouse_look && is_mouse_button_down(MouseButton::Right) {
            turn += mouse_delta.x * MOUSE_LOOK;
            tilt -= mouse_delta.y * MOUSE_LOOK;
        }

        let forward = keys(&[KeyCode::W], &[KeyCode::S]);
        let strafe = keys(&[KeyCode::D], &[KeyCode::A]);
        let lift = keys(
            &[KeyCode::E, KeyCode::PageUp],
            &[KeyCode::Q, KeyCode::PageDown],
        );
        // Windows reports the wheel in 120-unit notches, other platforms in single steps.
        let wheel = if mouse { mouse_wheel().1 } else { 0.0 };
        let notches = if wheel.abs() >= 60.0 { wheel / 120.0 } else { wheel };

        let moving = forward != 0.0 || strafe != 0.0 || lift != 0.0 || notches != 0.0;
        if moving || turn != 0.0 || tilt != 0.0 {
            // Any manual move leaves the overview, so the next toggle frames the world again.
            self.before_overview = None;
        }

        self.pose.yaw += turn;
        self.pose.pitch = (self.pose.pitch + tilt).clamp(-FRAC_PI_2, 1.45);
        if self.panning && mouse_delta != Vec2::ZERO {
            // Slide the camera so the ground that was under the pointer stays under it.
            let level = world.surface_height(self.pose.pos.x, self.pose.pos.z);
            if let (Some(before), Some(now)) =
                (self.ground_point(world, mouse_pos - mouse_delta, level), self.ground_point(world, mouse_pos, level))
            {
                self.pose.pos.x += before.x - now.x;
                self.pose.pos.z += before.y - now.y;
                self.before_overview = None;
            }
        }
        let view_dir = self.forward();

        let p = &mut self.pose;

        let ground = world.surface_height(p.pos.x, p.pos.z);
        let altitude = (p.pos.y - ground).max(0.0);
        // Move faster the higher up you are, so both close-up work and crossing the map feel right.
        let mut speed = 40.0 + altitude * 0.9;
        if keyboard && (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)) {
            speed *= FAST_MULTIPLIER;
        }

        let flat_forward = vec3(p.yaw.sin(), 0.0, -p.yaw.cos());
        let right = vec3(p.yaw.cos(), 0.0, p.yaw.sin());
        p.pos += (flat_forward * forward + right * strafe + Vec3::Y * lift) * speed * dt;

        if notches != 0.0 {
            // Wheel zooms along the view direction by a fraction of the current altitude.
            p.pos += view_dir * notches * (altitude * 0.15).max(4.0);
        }

        let limit = world.half_extents() * 1.5;
        p.pos.x = p.pos.x.clamp(-limit.x, limit.x);
        p.pos.z = p.pos.z.clamp(-limit.y, limit.y);
        let floor = world.surface_height(p.pos.x, p.pos.z) + MIN_CLEARANCE;
        p.pos.y = p.pos.y.clamp(floor, world.size().max_element() * 2.0);
    }

    /// Where the ray through the screen position `at` meets the horizontal plane `height` meters
    /// up, as world x and z, or `None` if the ray doesn't point down at it.
    fn ground_point(&self, world: &World, at: Vec2, height: f32) -> Option<Vec2> {
        let inverse = self.camera(world, 10_000.0).matrix().inverse();
        let ndc = vec2(at.x / screen_width() * 2.0 - 1.0, 1.0 - at.y / screen_height() * 2.0);
        let far = inverse * vec4(ndc.x, ndc.y, 1.0, 1.0);
        let dir = (far.truncate() / far.w - self.pose.pos).normalize();
        (dir.y < -0.02).then(|| {
            let p = self.pose.pos + dir * ((height - self.pose.pos.y) / dir.y);
            vec2(p.x, p.z)
        })
    }

    /// Height above the surface directly below the camera.
    pub fn altitude(&self, world: &World) -> f32 {
        self.pose.pos.y - world.surface_height(self.pose.pos.x, self.pose.pos.z)
    }

    pub fn camera(&self, world: &World, z_far: f32) -> Camera3D {
        let fwd = self.forward();
        // Derive "right" from yaw rather than forward x up, so looking straight down stays stable.
        let right = vec3(self.pose.yaw.cos(), 0.0, self.pose.yaw.sin());
        Camera3D {
            position: self.pose.pos,
            target: self.pose.pos + fwd,
            up: right.cross(fwd),
            fovy: FOV_DEGREES.to_radians(),
            // Push the near plane out when high up to keep depth precision for the distant ground.
            z_near: (self.altitude(world) * 0.02).clamp(0.5, 20.0),
            z_far,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angled_view_looks_20_degrees_down_at_the_same_ground() {
        let world = World::new(1024.0, 1024.0);
        let overview = Pose {
            pos: vec3(100.0, 300.0, -50.0),
            yaw: 0.0,
            pitch: -FRAC_PI_2,
        };
        let angled = angled_pose(overview, &world);
        assert!((angled.pitch + 20f32.to_radians()).abs() < 1e-5);
        assert_eq!(angled.yaw, overview.yaw);
        // Still looking at the ground the overview was centered on.
        let forward = look_direction(angled.yaw, angled.pitch);
        let t = angled.pos.y / -forward.y;
        let hit = angled.pos + forward * t;
        assert!((hit.x - 100.0).abs() < 1.0 && (hit.z + 50.0).abs() < 1.0, "looking at {hit}");
        assert!(angled.pos.y > 0.0);
    }

    #[test]
    fn angled_view_stays_inside_the_map_limits() {
        let world = World::new(1024.0, 1024.0);
        let far = Pose {
            pos: vec3(0.0, 5000.0, 0.0),
            yaw: 0.0,
            pitch: -FRAC_PI_2,
        };
        let angled = angled_pose(far, &world);
        let limit = world.half_extents() * 1.5;
        assert!(angled.pos.z.abs() <= limit.y && angled.pos.x.abs() <= limit.x);
        assert!(angled.pos.y > 0.0 && angled.pos.y <= world.size().max_element() * 2.0);
    }
}
