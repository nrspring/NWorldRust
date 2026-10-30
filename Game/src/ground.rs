//! Questions about the ground and water at a point or along a line, for the things the game lays over the map:
//! paths, which keep to dry ground, and bridges, which cross water.

use macroquad::prelude::*;
use nworld_map::world::{WaterClass, World};

/// How far from water, in meters, the middle of a path keeps, so that its width stays off the water.
pub const SHORE_CLEARANCE: f32 = 3.0;
/// How far apart the points are checked along a line for water, in meters.
pub const CHECK_EVERY: f32 = 2.0;
/// How far from a point a clear place is looked for, in meters.
const SNAP_REACH: f32 = 10.0;

pub fn dry(world: &World, at: Vec2) -> bool {
    world.water_class(at.x, at.y) == WaterClass::Dry
}

/// Whether the ground at `at` is dry, and so is some way round it.
pub fn clear(world: &World, at: Vec2) -> bool {
    dry(world, at) && [vec2(SHORE_CLEARANCE, 0.0), vec2(-SHORE_CLEARANCE, 0.0), vec2(0.0, SHORE_CLEARANCE), vec2(0.0, -SHORE_CLEARANCE)].iter().all(|o| dry(world, at + *o))
}

/// Whether the straight line from `a` to `b` is clear all along.
pub fn line_clear(world: &World, a: Vec2, b: Vec2) -> bool {
    let steps = (a.distance(b) / CHECK_EVERY).ceil().max(1.0) as usize;
    (0..=steps).all(|k| clear(world, a.lerp(b, k as f32 / steps as f32)))
}

/// The point nearest `at` that is clear (see [`clear`]), looked for out to a few meters, if there is one: `at`
/// itself if it is, and otherwise somewhere a little way from the water.
pub fn nearest_clear(world: &World, at: Vec2) -> Option<Vec2> {
    if clear(world, at) {
        return Some(at);
    }
    let mut radius = 1.0;
    while radius <= SNAP_REACH {
        let found = (0..24)
            .map(|k| {
                let angle = std::f32::consts::TAU * k as f32 / 24.0;
                at + vec2(angle.cos(), angle.sin()) * radius
            })
            .find(|p| clear(world, *p));
        if found.is_some() {
            return found;
        }
        radius += 1.0;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nworld_map::world::CELL_SIZE;

    fn dry_world() -> World {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        world
    }

    fn flood(world: &mut World, inside: impl Fn(Vec2) -> bool) {
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        for j in 0..nz {
            for i in 0..nx {
                if inside(vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y)) {
                    world.water_mut()[j * nx + i] = 10.0;
                }
            }
        }
    }

    #[test]
    fn clear_ground_is_dry_and_a_few_meters_from_the_water() {
        let mut world = dry_world();
        flood(&mut world, |p| p.x > 0.0);
        assert!(clear(&world, vec2(-50.0, 0.0)));
        assert!(!clear(&world, vec2(50.0, 0.0)), "in the water");
        assert!(dry(&world, vec2(1.0, 0.0)) && !clear(&world, vec2(1.0, 0.0)), "dry but on the bank");
    }

    #[test]
    fn a_point_on_the_bank_is_moved_a_little_way_inland() {
        let mut world = dry_world();
        flood(&mut world, |p| p.x > 0.0);
        let bank = vec2(-1.0, 40.0);
        let moved = nearest_clear(&world, bank).unwrap();
        assert!(clear(&world, moved) && moved.distance(bank) <= SNAP_REACH, "{moved}");
        assert_eq!(nearest_clear(&world, vec2(-80.0, 0.0)), Some(vec2(-80.0, 0.0)), "a clear point stays");
        // In the middle of the water there is nowhere near that is clear.
        assert_eq!(nearest_clear(&world, vec2(100.0, 0.0)), None);
    }

    #[test]
    fn a_line_is_clear_only_if_all_of_it_is() {
        let mut world = dry_world();
        flood(&mut world, |p| p.x.abs() < 10.0);
        assert!(line_clear(&world, vec2(-100.0, 0.0), vec2(-30.0, 0.0)));
        assert!(!line_clear(&world, vec2(-100.0, 0.0), vec2(100.0, 0.0)));
    }
}
