//! Drawing the woodcutters: a log cabin with a dark roof, a pile of logs beside it, and a flag in the color of the
//! country it belongs to on a pole. It is built twice as big as a real one would be, so that it can be seen over the
//! trees. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::ids::WoodcutterId;
use crate::woodcutter::Woodcutter;

/// How many times bigger than life it is, so that it stands out over the forest it is in.
const SCALE: f32 = 2.0;
/// How far from a part of it, as a fraction of how far away it is, the mouse can be and still be on it, so that the
/// thin pole can be hit from far away.
const HOVER_SLOP: f32 = 0.006;
const WALL: Color = Color::new(0.50, 0.34, 0.20, 1.0);
const ROOF: Color = Color::new(0.27, 0.19, 0.14, 1.0);
const LOGS: Color = Color::new(0.68, 0.50, 0.30, 1.0);
const POLE: Color = Color::new(0.82, 0.80, 0.74, 1.0);
/// A country that can't be found (which can't happen while it owns one) is flown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// A box that is part of a woodcutter: where its middle is, how big it is across, and its color.
struct Part {
    middle: Vec3,
    size: Vec3,
    color: Color,
}

/// Draws the woodcutters of `day`, as seen by the camera that is set, which must still be the one the map was drawn
/// with.
pub fn draw(world: &World, day: &DaySave) {
    for woodcutter in &day.woodcutters {
        let flag = day.country(woodcutter.country).map_or(NO_COUNTRY, |c| c.color.color());
        for part in parts(world, woodcutter, flag) {
            draw_cube(part.middle, part.size, None, part.color);
        }
    }
}

/// Draws a frame of `color` round each part of `woodcutter`, to mark it out, as seen by the camera that is set.
pub fn outline(world: &World, woodcutter: &Woodcutter, color: Color) {
    for part in parts(world, woodcutter, WHITE) {
        draw_cube_wires(part.middle, part.size * 1.04, color);
    }
}

/// The woodcutter the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it
/// goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, woodcutters: &[Woodcutter]) -> Option<WoodcutterId> {
    woodcutters
        .iter()
        .filter_map(|woodcutter| {
            let nearest = parts(world, woodcutter, WHITE)
                .iter()
                .filter_map(|part| hit(ray, part.middle, part.size, ray.0.distance(part.middle) * HOVER_SLOP))
                .min_by(f32::total_cmp)?;
            Some((nearest, woodcutter.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// How far along the ray a box is first hit, if it is, with the box grown by `slop` all round.
fn hit((origin, direction): (Vec3, Vec3), middle: Vec3, size: Vec3, slop: f32) -> Option<f32> {
    let half = size * 0.5 + Vec3::splat(slop);
    let (mut near, mut far) = (0.0f32, f32::MAX);
    for axis in 0..3 {
        let (low, high) = (middle[axis] - half[axis], middle[axis] + half[axis]);
        if direction[axis].abs() < 1e-8 {
            if origin[axis] < low || origin[axis] > high {
                return None;
            }
        } else {
            let (a, b) = ((low - origin[axis]) / direction[axis], (high - origin[axis]) / direction[axis]);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}

/// The parts of one woodcutter, flying `flag`. Each stands on the ground at its own place, so that the whole follows a
/// slope.
fn parts(world: &World, woodcutter: &Woodcutter, flag: Color) -> Vec<Part> {
    let ground = |dx: f32, dz: f32| world.surface_height(woodcutter.x + dx * SCALE, woodcutter.z + dz * SCALE);
    // A part standing `rise` above the ground at (`dx`, `dz`) from the woodcutter, in life-size meters.
    let part = |dx: f32, dz: f32, rise: f32, size: Vec3, color: Color| {
        let size = size * SCALE;
        Part { middle: vec3(woodcutter.x + dx * SCALE, ground(dx, dz) + rise * SCALE + size.y * 0.5, woodcutter.z + dz * SCALE), size, color }
    };
    // The cabin stands on the highest ground under it, so that none of it is buried.
    let base = [(-2.5, -2.0), (2.5, -2.0), (-2.5, 2.0), (2.5, 2.0), (0.0, 0.0)].iter().map(|(dx, dz)| ground(*dx, *dz)).fold(f32::MIN, f32::max);
    let lift = (base - ground(0.0, 0.0)) / SCALE;
    let mut parts = vec![
        part(0.0, 0.0, lift, vec3(5.0, 2.6, 4.0), WALL),
        part(0.0, 0.0, lift + 2.6, vec3(5.8, 0.5, 4.8), ROOF),
        part(0.0, 0.0, lift + 3.1, vec3(4.0, 0.5, 3.0), ROOF),
        part(4.2, 0.5, 0.0, vec3(2.4, 0.9, 1.0), LOGS),
        part(4.2, 0.5, 0.9, vec3(2.0, 0.8, 0.9), LOGS),
        part(-3.8, -2.4, 0.0, vec3(0.22, 7.0, 0.22), POLE),
    ];
    // The flag, hung from the top of the pole.
    let pole = &parts[5];
    parts.push(Part { middle: pole.middle + vec3(1.0 * SCALE, 2.7 * SCALE, 0.0), size: vec3(1.8, 1.1, 0.12) * SCALE, color: flag });
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use nworld_map::world::SEA_LEVEL;

    fn world() -> World {
        let mut world = World::new(512.0, 512.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        world
    }

    fn woodcutter() -> Woodcutter {
        Woodcutter { id: WoodcutterId::new(), country: CountryId::new(), x: 0.0, z: 0.0, region: 1, operating: false }
    }

    #[test]
    fn it_is_twice_the_size_of_a_cabin_of_life_size() {
        assert_eq!(SCALE, 2.0);
        let parts = parts(&world(), &woodcutter(), WHITE);
        assert_eq!(parts[0].size, vec3(10.0, 5.2, 8.0), "the walls");
        assert_eq!(parts.len(), 7);
        // Everything stands on or above the ground at 10 meters, none of it buried.
        assert!(parts.iter().all(|p| p.middle.y - p.size.y * 0.5 >= 10.0 - 1e-3));
        // The flag, which is in the country's color, flies high on the pole.
        let flag = parts.last().unwrap();
        assert_eq!(flag.color, WHITE);
        assert!(flag.middle.y > 10.0 + 12.0, "it flies over 12 meters up");
        let tallest = parts.iter().map(|p| p.middle.y + p.size.y * 0.5).fold(f32::MIN, f32::max);
        assert!(tallest >= 10.0 + 14.0, "the pole is 14 meters tall");
    }

    #[test]
    fn a_ray_at_the_cabin_hits_it_and_one_that_misses_does_not() {
        let world = world();
        let cutter = woodcutter();
        let down_at = |x: f32, z: f32| (vec3(x, 100.0, z), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down_at(0.0, 0.0), &world, &[cutter]), Some(cutter.id));
        assert_eq!(under_ray(down_at(30.0, 0.0), &world, &[cutter]), None);
        // The pole, which is thin, is hit too.
        assert_eq!(under_ray(down_at(-7.6, -4.8), &world, &[cutter]), Some(cutter.id));
        // From the side, at the height of the walls.
        assert_eq!(under_ray((vec3(-60.0, 12.0, 0.0), vec3(1.0, 0.0, 0.0)), &world, &[cutter]), Some(cutter.id));
        // Looking away from it, it is not there.
        assert_eq!(under_ray((vec3(-60.0, 12.0, 0.0), vec3(-1.0, 0.0, 0.0)), &world, &[cutter]), None);
    }

    #[test]
    fn of_two_in_line_the_nearer_is_the_one_pointed_at() {
        let world = world();
        let (near, far) = (Woodcutter { x: -40.0, ..woodcutter() }, Woodcutter { x: 40.0, ..woodcutter() });
        let ray = (vec3(-200.0, 12.0, 0.0), vec3(1.0, 0.0, 0.0));
        assert_eq!(under_ray(ray, &world, &[far, near]), Some(near.id));
    }

    #[test]
    fn a_box_is_hit_from_outside_and_from_inside() {
        let (middle, size) = (vec3(0.0, 0.0, 0.0), vec3(2.0, 2.0, 2.0));
        assert_eq!(hit((vec3(-5.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0)), middle, size, 0.0), Some(4.0));
        assert_eq!(hit((vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0)), middle, size, 0.0), Some(0.0));
        assert_eq!(hit((vec3(-5.0, 3.0, 0.0), vec3(1.0, 0.0, 0.0)), middle, size, 0.0), None);
        assert_eq!(hit((vec3(-5.0, 1.5, 0.0), vec3(1.0, 0.0, 0.0)), middle, size, 1.0), Some(3.0), "slop widens the box");
    }
}
