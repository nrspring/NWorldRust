//! Finding what is under the mouse.

use macroquad::prelude::*;
use nworld_map::world::{World, CELL_SIZE};

/// The world x and z under the mouse, found by following the view ray down to the surface
/// (the sea counts as surface). `None` if the ray never reaches it.
pub fn pick_terrain(camera: &Camera3D, world: &World, mouse: Vec2) -> Option<Vec2> {
    const MAX_DISTANCE: f32 = 40_000.0;
    let ndc = vec2(2.0 * mouse.x / screen_width() - 1.0, 1.0 - 2.0 * mouse.y / screen_height());
    let inverse = camera.matrix().inverse();
    let unproject = |z: f32| {
        let p = inverse * vec4(ndc.x, ndc.y, z, 1.0);
        p.truncate() / p.w
    };
    let origin = camera.position;
    let dir = (unproject(1.0) - unproject(-1.0)).normalize();

    let above = |t: f32| {
        let p = origin + dir * t;
        p.y - world.surface_height(p.x, p.z)
    };
    // Take big steps while high above the ground and small ones close to it.
    let (mut t, mut prev) = (0.0f32, 0.0f32);
    while t < MAX_DISTANCE {
        if above(t) <= 0.0 {
            // Crossed the surface between `prev` and `t`; narrow it down.
            let (mut lo, mut hi) = (prev, t);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                if above(mid) > 0.0 { lo = mid } else { hi = mid }
            }
            let p = origin + dir * hi;
            return Some(vec2(p.x, p.z));
        }
        prev = t;
        t += (above(t) * 0.4).max(CELL_SIZE * 0.5);
    }
    None
}
