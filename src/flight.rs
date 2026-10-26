//! Arcade flight model: keyboard-driven yaw/pitch, throttle, boost, and a terrain-following autopilot.

use macroquad::prelude::*;

use crate::terrain::HeightField;

const MIN_SPEED: f32 = 10.0;
const MAX_SPEED: f32 = 600.0;
const BOOST: f32 = 3.0;
const GROUND_CLEARANCE: f32 = 6.0;
const CEILING: f32 = 2500.0;
const MAX_PITCH: f32 = 1.4;
const TURN_RATE: f32 = 1.1;
const PITCH_RATE: f32 = 0.9;

pub struct Flight {
    pub pos: Vec3,
    /// Radians; 0 faces -Z ("north"), increasing turns right.
    pub yaw: f32,
    pub pitch: f32,
    bank: f32,
    yaw_rate: f32,
    pitch_rate: f32,
    pub speed: f32,
    pub boosting: bool,
    pub autopilot: bool,
    pub target_agl: f32,
    pub invert_pitch: bool,
    fov: f32,
}

/// Frame-rate independent exponential approach of `current` toward `target`.
fn approach(current: f32, target: f32, sharpness: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-sharpness * dt).exp())
}

fn axis(positive: &[KeyCode], negative: &[KeyCode]) -> f32 {
    let held = |keys: &[KeyCode]| keys.iter().any(|&k| is_key_down(k));
    held(positive) as i32 as f32 - held(negative) as i32 as f32
}

impl Flight {
    pub fn new(field: &HeightField) -> Self {
        let pos = vec3(0.0, field.height(0.0, 0.0) + 120.0, 0.0);
        Self {
            pos,
            yaw: 0.0,
            pitch: -0.1,
            bank: 0.0,
            yaw_rate: 0.0,
            pitch_rate: 0.0,
            speed: 80.0,
            boosting: false,
            autopilot: false,
            target_agl: 80.0,
            invert_pitch: false,
            fov: 65.0,
        }
    }

    pub fn forward(&self) -> Vec3 {
        vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    pub fn heading_degrees(&self) -> f32 {
        self.yaw.to_degrees().rem_euclid(360.0)
    }

    pub fn agl(&self, field: &HeightField) -> f32 {
        self.pos.y - field.height(self.pos.x, self.pos.z)
    }

    pub fn toggle_autopilot(&mut self, field: &HeightField) {
        self.autopilot = !self.autopilot;
        if self.autopilot {
            self.target_agl = self.agl(field).clamp(20.0, 400.0);
        } else {
            self.pitch_rate = 0.0;
        }
    }

    pub fn update(&mut self, dt: f32, field: &HeightField) {
        let turn = axis(
            &[KeyCode::D, KeyCode::Right],
            &[KeyCode::A, KeyCode::Left],
        );
        let mut climb = axis(&[KeyCode::W, KeyCode::Up], &[KeyCode::S, KeyCode::Down]);
        if self.invert_pitch {
            climb = -climb;
        }

        self.yaw_rate = approach(self.yaw_rate, turn * TURN_RATE, 4.0, dt);
        self.yaw += self.yaw_rate * dt;
        self.bank = approach(self.bank, self.yaw_rate * 0.5, 3.0, dt);

        if self.autopilot {
            // In autopilot, W/S adjust the height held above the ground instead of pitching directly.
            self.target_agl = (self.target_agl + climb * 60.0 * dt).clamp(15.0, 600.0);

            // Look ahead along the flight path so we start climbing before a ridge arrives.
            let flat = vec3(self.yaw.sin(), 0.0, -self.yaw.cos());
            let look = self.current_speed().max(40.0);
            let mut ground = field.height(self.pos.x, self.pos.z);
            for k in 1..=6 {
                let p = self.pos + flat * look * k as f32 * 0.4;
                ground = ground.max(field.height(p.x, p.z));
            }
            let error = ground + self.target_agl - self.pos.y;
            let desired = (error / (look * 1.5)).atan().clamp(-0.5, 0.7);
            self.pitch = approach(self.pitch, desired, 1.8, dt);
        } else {
            self.pitch_rate = approach(self.pitch_rate, climb * PITCH_RATE, 4.0, dt);
            self.pitch = (self.pitch + self.pitch_rate * dt).clamp(-MAX_PITCH, MAX_PITCH);
        }

        let throttle = axis(
            &[KeyCode::LeftShift, KeyCode::RightShift, KeyCode::Equal, KeyCode::KpAdd],
            &[KeyCode::LeftControl, KeyCode::RightControl, KeyCode::Minus, KeyCode::KpSubtract],
        );
        // Multiplicative throttle feels even across the whole speed range.
        self.speed = (self.speed * (1.0 + throttle * 0.9 * dt)).clamp(MIN_SPEED, MAX_SPEED);
        self.boosting = is_key_down(KeyCode::Space);

        self.pos += self.forward() * self.current_speed() * dt;

        let floor = field.height(self.pos.x, self.pos.z) + GROUND_CLEARANCE;
        if self.pos.y < floor {
            self.pos.y = floor;
            if self.pitch < 0.0 {
                self.pitch = approach(self.pitch, 0.05, 8.0, dt);
                self.pitch_rate = self.pitch_rate.max(0.0);
            }
        }
        self.pos.y = self.pos.y.min(CEILING);

        let target_fov = if self.boosting { 78.0 } else { 65.0 };
        self.fov = approach(self.fov, target_fov, 4.0, dt);
    }

    pub fn current_speed(&self) -> f32 {
        if self.boosting {
            self.speed * BOOST
        } else {
            self.speed
        }
    }

    pub fn camera(&self, z_far: f32) -> Camera3D {
        let fwd = self.forward();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let banked_up = up * self.bank.cos() + right * self.bank.sin();
        Camera3D {
            position: self.pos,
            target: self.pos + fwd,
            up: banked_up,
            fovy: self.fov.to_radians(),
            z_near: 0.5,
            z_far,
            ..Default::default()
        }
    }
}
