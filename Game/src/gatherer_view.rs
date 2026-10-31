//! Drawing the gatherers. A woodcutter and a stonecutter are each a cabin with a dark roof and a pile of what they
//! gather beside it, in logs or in gray stone. A sawmill and a quarry, the greater ones, are bigger and have more to
//! them: a sawmill is a long hall with a tall chimney, a lean-to, stacks of logs and of planks, and a quarry is a
//! stepped pit of stone with a crane over it, a hut and stacks of cut blocks. Each flies a flag in the color of the
//! country it belongs to on a pole. All are built bigger than life, so that they can be seen over the trees. This is the
//! game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::gatherer::{Gatherer, GathererKind};
use crate::ids::GathererId;

/// How far from a part of it, as a fraction of how far away it is, the mouse can be and still be on it, so that the
/// thin pole can be hit from far away.
const HOVER_SLOP: f32 = 0.006;
/// How far apart the points are along the edge of the ground a preview covers, and how high above the ground it is held.
const FOOTPRINT_EVERY: f32 = 4.0;
const FOOTPRINT_HOLD: f32 = 0.6;
const POLE: Color = Color::new(0.82, 0.80, 0.74, 1.0);
/// A country that can't be found (which can't happen while it owns one) is flown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// How many times bigger than life it is, so that it stands out over the forest it is in: the greater ones more.
fn scale(kind: GathererKind) -> f32 {
    if kind.greater() {
        3.0
    } else {
        2.0
    }
}

/// How tall its flag pole is, in life-size meters, which is the tallest part of it.
fn pole_height(kind: GathererKind) -> f32 {
    if kind.greater() {
        9.0
    } else {
        7.0
    }
}

/// How high the top of a gatherer of `kind` stands over the ground, in meters, which is for holding things over it.
pub fn height(kind: GathererKind) -> f32 {
    pole_height(kind) * scale(kind)
}

/// The colors a kind of gatherer is made of: its walls, its roof, the pile of what it has gathered beside it, and
/// timber or planks for what else it has.
struct Palette {
    wall: Color,
    roof: Color,
    pile: Color,
    trim: Color,
}

impl Palette {
    fn of(kind: GathererKind) -> Self {
        let c = |r: f32, g: f32, b: f32| Color::new(r, g, b, 1.0);
        match kind {
            // Log walls and a dark roof, with a pile of logs.
            GathererKind::Woodcutter => Self { wall: c(0.50, 0.34, 0.20), roof: c(0.27, 0.19, 0.14), pile: c(0.68, 0.50, 0.30), trim: c(0.68, 0.50, 0.30) },
            // Gray stone walls and a slate roof, with a pile of cut blocks.
            GathererKind::Stonecutter => Self { wall: c(0.58, 0.58, 0.60), roof: c(0.22, 0.25, 0.30), pile: c(0.80, 0.80, 0.82), trim: c(0.45, 0.33, 0.20) },
            // Dark timber walls under a red roof, with logs, and stacks of pale planks.
            GathererKind::Sawmill => Self { wall: c(0.47, 0.31, 0.19), roof: c(0.52, 0.21, 0.16), pile: c(0.68, 0.50, 0.30), trim: c(0.86, 0.72, 0.50) },
            // Pale stone in steps, with a slate roof, and a timber crane.
            GathererKind::Quarry => Self { wall: c(0.62, 0.59, 0.54), roof: c(0.22, 0.25, 0.30), pile: c(0.82, 0.82, 0.84), trim: c(0.45, 0.33, 0.20) },
        }
    }
}

/// `color` made lighter (over 1) or darker (under 1).
fn shade(color: Color, by: f32) -> Color {
    Color::new((color.r * by).min(1.0), (color.g * by).min(1.0), (color.b * by).min(1.0), color.a)
}

/// A box that is part of a gatherer: where its middle is, how big it is across, and its color.
struct Part {
    middle: Vec3,
    size: Vec3,
    color: Color,
}

/// Draws the gatherers of `day`, as seen by the camera that is set, which must still be the one the map was drawn
/// with.
pub fn draw(world: &World, day: &DaySave) {
    for gatherer in &day.gatherers {
        let flag = day.country(gatherer.country).map_or(NO_COUNTRY, |c| c.color.color());
        for part in parts(world, gatherer, flag) {
            draw_cube(part.middle, part.size, None, part.color);
        }
    }
}

/// Draws a frame of `color` round each part of `gatherer`, to mark it out, as seen by the camera that is set.
pub fn outline(world: &World, gatherer: &Gatherer, color: Color) {
    for part in parts(world, gatherer, WHITE) {
        draw_cube_wires(part.middle, part.size * 1.04, color);
    }
}

/// The gatherer the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it
/// goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, gatherers: &[Gatherer]) -> Option<GathererId> {
    gatherers
        .iter()
        .filter_map(|gatherer| {
            let nearest = parts(world, gatherer, WHITE)
                .iter()
                .filter_map(|part| hit(ray, part.middle, part.size, ray.0.distance(part.middle) * HOVER_SLOP))
                .min_by(f32::total_cmp)?;
            Some((nearest, gatherer.id))
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

/// Draws where a gatherer of `kind` would stand if it were built at `at`, so that its size can be seen against the
/// region: a frame round each part of it, and the ground it covers marked out, in `color`. As seen by the camera that
/// is set.
pub fn preview(world: &World, kind: GathererKind, at: Vec2, color: Color) {
    let parts = parts_at(world, kind, at.x, at.y, WHITE);
    for part in &parts {
        draw_cube_wires(part.middle, part.size * 1.04, color);
    }
    // The ground it covers, from the farthest its parts reach, held just over the ground along each side.
    let (mut low, mut high) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for part in &parts {
        let half = vec2(part.size.x, part.size.z) * 0.5;
        low = low.min(vec2(part.middle.x, part.middle.z) - half);
        high = high.max(vec2(part.middle.x, part.middle.z) + half);
    }
    let corners = [low, vec2(high.x, low.y), high, vec2(low.x, high.y), low];
    for pair in corners.windows(2) {
        let steps = (pair[0].distance(pair[1]) / FOOTPRINT_EVERY).ceil().max(1.0) as usize;
        let point = |k: usize| {
            let p = pair[0].lerp(pair[1], k as f32 / steps as f32);
            vec3(p.x, world.surface_height(p.x, p.y) + FOOTPRINT_HOLD, p.y)
        };
        for k in 0..steps {
            draw_line_3d(point(k), point(k + 1), color);
        }
    }
}

/// The parts of one gatherer, flying `flag`. Each stands on the ground at its own place, so that the whole follows a
/// slope. The last two are its flag pole and its flag.
fn parts(world: &World, gatherer: &Gatherer, flag: Color) -> Vec<Part> {
    parts_at(world, gatherer.kind, gatherer.x, gatherer.z, flag)
}

/// The parts of a gatherer of `kind` standing at (`x`, `z`), flying `flag`.
fn parts_at(world: &World, kind: GathererKind, x: f32, z: f32, flag: Color) -> Vec<Part> {
    let (look, s) = (Palette::of(kind), scale(kind));
    let ground = |dx: f32, dz: f32| world.surface_height(x + dx * s, z + dz * s);
    // A part standing `rise` above the ground at (`dx`, `dz`) from the gatherer, in life-size meters.
    let part = |dx: f32, dz: f32, rise: f32, size: Vec3, color: Color| {
        let size = size * s;
        Part { middle: vec3(x + dx * s, ground(dx, dz) + rise * s + size.y * 0.5, z + dz * s), size, color }
    };
    // The main building stands on the highest ground under it, so that none of it is buried.
    let (reach_x, reach_z) = if kind.greater() { (6.0, 4.5) } else { (2.5, 2.0) };
    let base = [(-reach_x, -reach_z), (reach_x, -reach_z), (-reach_x, reach_z), (reach_x, reach_z), (0.0, 0.0)].iter().map(|(dx, dz)| ground(*dx, *dz)).fold(f32::MIN, f32::max);
    let lift = (base - ground(0.0, 0.0)) / s;
    let mut parts = match kind {
        GathererKind::Woodcutter | GathererKind::Stonecutter => vec![
            part(0.0, 0.0, lift, vec3(5.0, 2.6, 4.0), look.wall),
            part(0.0, 0.0, lift + 2.6, vec3(5.8, 0.5, 4.8), look.roof),
            part(0.0, 0.0, lift + 3.1, vec3(4.0, 0.5, 3.0), look.roof),
            part(4.2, 0.5, 0.0, vec3(2.4, 0.9, 1.0), look.pile),
            part(4.2, 0.5, 0.9, vec3(2.0, 0.8, 0.9), look.pile),
        ],
        GathererKind::Sawmill => vec![
            // The hall, under a roof of three tiers.
            part(0.0, 0.0, lift, vec3(9.0, 3.4, 6.0), look.wall),
            part(0.0, 0.0, lift + 3.4, vec3(10.4, 0.6, 7.4), look.roof),
            part(0.0, 0.0, lift + 4.0, vec3(7.6, 0.8, 4.8), look.roof),
            part(0.0, 0.0, lift + 4.8, vec3(9.0, 0.5, 1.2), look.roof),
            // A tall chimney.
            part(3.2, 1.5, lift + 4.0, vec3(1.2, 4.5, 1.2), shade(look.roof, 0.55)),
            // A lean-to at the side.
            part(-6.4, 0.5, 0.0, vec3(3.4, 2.2, 4.4), look.wall),
            part(-6.4, 0.5, 2.2, vec3(4.0, 0.4, 5.0), look.roof),
            // Logs in three layers, and planks in two.
            part(6.8, -1.6, 0.0, vec3(2.4, 1.0, 5.0), look.pile),
            part(6.8, -1.6, 1.0, vec3(2.4, 1.0, 4.2), look.pile),
            part(6.8, -1.6, 2.0, vec3(2.4, 1.0, 3.4), look.pile),
            part(0.0, 4.6, 0.0, vec3(5.0, 1.2, 1.4), look.trim),
            part(0.0, 4.6, 1.2, vec3(4.2, 0.8, 1.2), look.trim),
        ],
        GathererKind::Quarry => vec![
            // The pit, in three steps going up.
            part(0.0, 0.0, lift, vec3(12.0, 1.0, 9.0), shade(look.wall, 0.8)),
            part(0.0, 0.0, lift + 1.0, vec3(9.6, 1.0, 7.0), look.wall),
            part(0.0, 0.0, lift + 2.0, vec3(7.2, 1.0, 5.0), shade(look.wall, 1.15)),
            // A hut for the workers.
            part(-6.6, -1.5, 0.0, vec3(3.4, 2.4, 3.6), shade(look.wall, 0.9)),
            part(-6.6, -1.5, 2.4, vec3(4.0, 0.5, 4.2), look.roof),
            // A crane: a mast, a boom, and a block on a cable.
            part(4.6, -3.2, 0.0, vec3(0.8, 9.0, 0.8), look.trim),
            part(2.4, -3.2, 8.2, vec3(5.6, 0.5, 0.5), look.trim),
            part(0.2, -3.2, 6.4, vec3(0.15, 1.8, 0.15), POLE),
            part(0.2, -3.2, 5.2, vec3(1.2, 1.2, 1.2), look.pile),
            // Cut blocks stacked about.
            part(5.4, 2.4, 0.0, vec3(2.4, 1.2, 2.4), look.pile),
            part(5.4, 2.4, 1.2, vec3(1.8, 1.0, 1.8), look.pile),
            part(-2.4, 4.2, 0.0, vec3(3.0, 1.0, 1.6), look.pile),
        ],
    };
    // The flag pole, away from the rest, and the flag hung from the top of it.
    let (pole_x, pole_z) = if kind.greater() { (-6.8, -5.2) } else { (-3.8, -2.4) };
    let pole_height = pole_height(kind);
    let pole = part(pole_x, pole_z, 0.0, vec3(0.22, pole_height, 0.22), POLE);
    let flag_at = pole.middle + vec3(1.0 * s, (pole_height * 0.5 - 0.8) * s, 0.0);
    parts.push(pole);
    parts.push(Part { middle: flag_at, size: vec3(1.8, 1.1, 0.12) * s, color: flag });
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

    fn gatherer() -> Gatherer {
        Gatherer { id: GathererId::new(), kind: GathererKind::Woodcutter, country: CountryId::new(), x: 0.0, z: 0.0, region: 1, operating: false }
    }

    fn of_kind(kind: GathererKind) -> Gatherer {
        Gatherer { kind, ..gatherer() }
    }

    #[test]
    fn it_is_twice_the_size_of_a_cabin_of_life_size() {
        assert_eq!(scale(GathererKind::Woodcutter), 2.0);
        let parts = parts(&world(), &gatherer(), WHITE);
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
    fn a_sawmill_and_a_quarry_are_bigger_and_have_more_to_them_than_the_lesser_ones() {
        let world = world();
        let reach = |parts: &[Part]| {
            let (mut low, mut high) = (vec3(f32::MAX, f32::MAX, f32::MAX), vec3(f32::MIN, f32::MIN, f32::MIN));
            for p in parts {
                low = low.min(p.middle - p.size * 0.5);
                high = high.max(p.middle + p.size * 0.5);
            }
            high - low
        };
        for (lesser, greater) in [(GathererKind::Woodcutter, GathererKind::Sawmill), (GathererKind::Stonecutter, GathererKind::Quarry)] {
            let (small, big) = (parts(&world, &of_kind(lesser), WHITE), parts(&world, &of_kind(greater), WHITE));
            assert_eq!(scale(greater), 3.0);
            assert!(big.len() >= small.len() + 5, "{greater:?} has {} parts to {}", big.len(), small.len());
            let (a, b) = (reach(&small), reach(&big));
            assert!(b.x > a.x * 2.0 && b.z > a.z * 1.5 && b.y > a.y, "{a:?} against {b:?}");
            // Everything stands on the ground, none of it buried, and the flag is last, in its country's color.
            assert!(big.iter().all(|p| p.middle.y - p.size.y * 0.5 >= 10.0 - 1e-3));
            assert_eq!(big.last().unwrap().color, WHITE);
            // Nothing is taller than its pole, which the icons over it are held above.
            let tallest = big.iter().map(|p| p.middle.y + p.size.y * 0.5 - 10.0).fold(f32::MIN, f32::max);
            assert!((tallest - height(greater)).abs() < 1e-3, "{tallest} against {}", height(greater));
        }
        assert_eq!(height(GathererKind::Sawmill), 27.0);
        assert!(height(GathererKind::Quarry) > height(GathererKind::Stonecutter));
        // The hall of the sawmill, which stands in the middle.
        let hall = parts(&world, &of_kind(GathererKind::Sawmill), WHITE)[0].size;
        assert!((hall - vec3(27.0, 10.2, 18.0)).abs().max_element() < 1e-3, "{hall:?}");
    }

    #[test]
    fn a_greater_one_is_pointed_at_by_its_hall_its_crane_or_its_pole() {
        let world = world();
        let down_at = |x: f32, z: f32| (vec3(x, 100.0, z), vec3(0.0, -1.0, 0.0));
        let sawmill = of_kind(GathererKind::Sawmill);
        assert_eq!(under_ray(down_at(0.0, 0.0), &world, &[sawmill]), Some(sawmill.id));
        assert_eq!(under_ray(down_at(-20.4, -15.6), &world, &[sawmill]), Some(sawmill.id), "the pole");
        assert_eq!(under_ray(down_at(0.0, 40.0), &world, &[sawmill]), None);
        let quarry = of_kind(GathererKind::Quarry);
        assert_eq!(under_ray(down_at(13.8, -9.6), &world, &[quarry]), Some(quarry.id), "the crane mast");
    }

    #[test]
    fn a_ray_at_the_cabin_hits_it_and_one_that_misses_does_not() {
        let world = world();
        let cutter = gatherer();
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
    fn each_kind_has_a_look_of_its_own_and_the_same_shape() {
        let world = world();
        let (wood, stone) = (gatherer(), Gatherer { kind: GathererKind::Stonecutter, ..gatherer() });
        let (a, b) = (parts(&world, &wood, WHITE), parts(&world, &stone, WHITE));
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| p.middle == q.middle && p.size == q.size), "the same shape");
        assert_ne!(a[0].color, b[0].color, "the walls differ");
        assert_ne!(a[3].color, b[3].color, "and so does the pile");
        assert_eq!(a[5].color, b[5].color, "the pole is the same");
        // Stone walls are gray, and not brown like logs.
        let c = b[0].color;
        assert!((c.r - c.g).abs() < 0.05 && (c.g - c.b).abs() < 0.1);
        assert!(a[0].color.r > a[0].color.b + 0.2);
        // The greater ones are of the lesser one's materials: a sawmill is timber and a quarry is stone.
        let (sawmill, quarry) = (parts(&world, &of_kind(GathererKind::Sawmill), WHITE), parts(&world, &of_kind(GathererKind::Quarry), WHITE));
        assert!(sawmill[0].color.r > sawmill[0].color.b + 0.2, "timber is brown");
        assert!((quarry[1].color.r - quarry[1].color.b).abs() < 0.12, "stone is gray");
    }

    #[test]
    fn of_two_in_line_the_nearer_is_the_one_pointed_at() {
        let world = world();
        let (near, far) = (Gatherer { x: -40.0, ..gatherer() }, Gatherer { x: 40.0, ..gatherer() });
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
