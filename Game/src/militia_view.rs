//! Drawing the militia. Each is a parade of soldiers: a column, four abreast, with an officer and a banner bearer at the
//! head of it, in front of the ranks. The soldiers wear coats in the color of the country they belong to, with white
//! trousers and belts, black hats, and a musket on the shoulder, and one of the first rank beats a drum. A militia with
//! more members has more ranks. The column faces the way it last marched, and while it is walking the soldiers step, each
//! out of time with the next. Like the gatherers it is built bigger than life, so that it can be seen from far off. This is
//! the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::gatherer_view::hit;
use crate::hud;
use crate::ids::MilitiaId;
use crate::militia::{upkeep, Militia};
use crate::movement::DAILY_POINTS;
use crate::walk::Walks;

/// How many times bigger than life it is.
const SCALE: f32 = 4.0;
/// How far from a part of it, as a fraction of how far away it is, the mouse can be and still be on it, so that the
/// thin parts can be hit from far away.
const HOVER_SLOP: f32 = 0.006;
/// How many members each rank-and-file soldier drawn stands for, and the fewest and the most soldiers drawn in the ranks.
const MEMBERS_PER_SOLDIER: u32 = 40;
const FEWEST_SOLDIERS: usize = 2;
const MOST_SOLDIERS: usize = 24;
/// How many soldiers stand abreast, how far apart they are across and along the column, and how far in front of the first
/// rank the officer and the banner bearer stand, in life-size meters.
const ABREAST: usize = 4;
const FILE_GAP: f32 = 1.3;
const RANK_GAP: f32 = 1.7;
const COMMAND_AHEAD: f32 = 2.8;
/// How high a soldier's step lifts him, in life-size meters, and how many times a second he steps.
const STEP_HEIGHT: f32 = 0.12;
const STEP_RATE: f32 = 11.0;

const TROUSERS: Color = Color::new(0.88, 0.88, 0.86, 1.0);
const BELT: Color = Color::new(0.95, 0.95, 0.93, 1.0);
const SKIN: Color = Color::new(0.85, 0.70, 0.55, 1.0);
const HAT: Color = Color::new(0.08, 0.08, 0.10, 1.0);
const MUSKET: Color = Color::new(0.42, 0.30, 0.20, 1.0);
const GOLD: Color = Color::new(0.92, 0.76, 0.22, 1.0);
const POLE: Color = Color::new(0.82, 0.80, 0.74, 1.0);
const DRUM: Color = Color::new(0.62, 0.14, 0.12, 1.0);
/// A country that can't be found is flown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// A box that is part of a parade: where its middle is, how big it is across, and its color.
struct Part {
    middle: Vec3,
    size: Vec3,
    color: Color,
}

/// How many soldiers are drawn in the ranks for a militia of `members`: one for every forty, at least two and at most
/// twenty-four. The officer and the banner bearer are besides.
pub fn soldiers_for(members: u32) -> usize {
    (members.div_ceil(MEMBERS_PER_SOLDIER) as usize).clamp(FEWEST_SOLDIERS, MOST_SOLDIERS)
}

/// Where and how a parade stands, for placing its parts.
struct Stand<'a> {
    world: &'a World,
    /// The middle of the parade on the map, and which way it faces, in quarter turns: 0 is towards +z, 1 towards +x, 2
    /// towards -z and 3 towards -x.
    at: Vec2,
    facing: u8,
    /// The time, if it is marching, for the soldiers to step by.
    marching: Option<f32>,
}

impl Stand<'_> {
    /// A box of `size` (in life-size meters: across the column, high, and along it) standing on the ground, `across` and
    /// `along` (life-size, `along` being towards the front) from the middle, with its bottom `lift` (life-size) over the
    /// ground there, and `step` for how out of time it is with the others when they march.
    fn part(&self, across: f32, along: f32, lift: f32, size: Vec3, color: Color, step: f32) -> Part {
        let offset = match self.facing % 4 {
            0 => vec2(across, along),
            1 => vec2(along, -across),
            2 => vec2(-across, -along),
            _ => vec2(-along, across),
        };
        let at = self.at + offset * SCALE;
        let ground = self.world.surface_height(at.x, at.y);
        // Turned a quarter, the box is turned too.
        let size = if self.facing % 2 == 1 { vec3(size.z, size.y, size.x) } else { size } * SCALE;
        let hop = self.marching.map_or(0.0, |time| (time * STEP_RATE + step).sin().abs() * STEP_HEIGHT);
        Part { middle: vec3(at.x, ground + (lift + hop) * SCALE + size.y * 0.5, at.y), size, color }
    }

    /// A soldier standing across and along from the middle: legs, a coat of `coat`, a white belt over it, a head and a
    /// black hat.
    fn soldier(&self, parts: &mut Vec<Part>, across: f32, along: f32, coat: Color, step: f32) {
        parts.push(self.part(across, along, 0.0, vec3(0.40, 0.55, 0.24), TROUSERS, step));
        parts.push(self.part(across, along, 0.55, vec3(0.50, 0.62, 0.30), coat, step));
        parts.push(self.part(across, along, 0.58, vec3(0.20, 0.52, 0.33), BELT, step));
        parts.push(self.part(across, along, 1.17, vec3(0.26, 0.26, 0.26), SKIN, step));
        parts.push(self.part(across, along, 1.43, vec3(0.30, 0.34, 0.30), HAT, step));
    }
}

/// The boxes a militia is drawn as, standing at `at` and facing `facing` quarter turns, with the soldiers' coats and its flag
/// in `flag`, and stepping to the time `marching` if it is marching.
fn parts(world: &World, militia: &Militia, at: Vec2, facing: u8, marching: Option<f32>, flag: Color) -> Vec<Part> {
    let stand = Stand { world, at, facing, marching };
    let mut parts = Vec::new();

    // At the head of the column: the officer, with a gold epaulette and a tall hat, and the banner bearer, with the flag.
    let ahead = COMMAND_AHEAD;
    stand.soldier(&mut parts, -FILE_GAP * 0.5, ahead, flag, 0.0);
    parts.push(stand.part(-FILE_GAP * 0.5, ahead, 1.30, vec3(0.62, 0.10, 0.34), GOLD, 0.0));
    stand.soldier(&mut parts, FILE_GAP * 0.5, ahead, flag, 1.5);
    parts.push(stand.part(FILE_GAP * 0.5 + 0.28, ahead, 0.0, vec3(0.09, 3.8, 0.09), POLE, 1.5));
    parts.push(stand.part(FILE_GAP * 0.5 + 0.28, ahead, 2.6, vec3(1.7, 1.1, 0.06), flag, 1.5));

    // The ranks behind them, four abreast. The second man of the first rank has a drum, and the others a musket.
    for k in 0..soldiers_for(militia.members) {
        let (rank, file) = (k / ABREAST, k % ABREAST);
        let across = (file as f32 - (ABREAST as f32 - 1.0) * 0.5) * FILE_GAP;
        let along = -(rank as f32) * RANK_GAP;
        let step = k as f32 * 1.7;
        stand.soldier(&mut parts, across, along, flag, step);
        if rank == 0 && file == 1 {
            parts.push(stand.part(across, along + 0.3, 0.45, vec3(0.50, 0.40, 0.38), DRUM, step));
        } else {
            parts.push(stand.part(across + 0.3, along, 0.30, vec3(0.05, 1.5, 0.05), MUSKET, step));
        }
    }
    parts
}

/// Draws the militia of `day`, as seen by the camera that is set, which must still be the one the map was drawn with.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32) {
    for militia in &day.militias {
        let flag = day.country(militia.country).map_or(NO_COUNTRY, |c| c.color.color());
        let marching = walks.is_walking(militia.id).then_some(time);
        for part in parts(world, militia, walks.position(militia), walks.facing(militia.id), marching, flag) {
            draw_cube(part.middle, part.size, None, part.color);
        }
    }
}

/// The militia the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes
/// through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, militias: &[Militia], walks: &Walks) -> Option<MilitiaId> {
    militias
        .iter()
        .filter_map(|militia| {
            let nearest = parts(world, militia, walks.position(militia), walks.facing(militia.id), None, WHITE)
                .iter()
                .filter_map(|part| hit(ray, part.middle, part.size, ray.0.distance(part.middle) * HOVER_SLOP))
                .min_by(f32::total_cmp)?;
            Some((nearest, militia.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws the tooltip for `militia`, beside the mouse.
pub fn tooltip(style: &Style, day: &DaySave, militia: &Militia) {
    let origin = day.city(militia.city).map_or("a city", |city| city.name.as_str());
    let rows = [
        ("Country", day.country_name(militia.country).to_owned()),
        ("Members", thousands(militia.members)),
        ("Raised in", origin.to_owned()),
        ("Upkeep", format!("{} coins a day", money(upkeep(militia.members)))),
        ("Movement", format!("{} of {} points", militia.points, DAILY_POINTS)),
    ];
    let label_w = rows.iter().map(|(label, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let title = militia.title();
    let width = style.measure(title, 17, Weight::Semibold).width.max(label_w + 18.0 + value_w) + 32.0;
    let height = 16.0 + 24.0 + 12.0 + rows.len() as f32 * 22.0 + 6.0;

    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - 8.0), (mouse.y + 20.0).min(screen_height() - height - hud::FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    style.text_mid(title, panel.x + 16.0, panel.y + 16.0 + 12.0, 17, Weight::Semibold, style::TEXT);
    draw_line(panel.x + 16.0, panel.y + 16.0 + 24.0 + 2.0, panel.right() - 16.0, panel.y + 16.0 + 24.0 + 2.0, 1.0, style::BORDER);
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = panel.y + 16.0 + 24.0 + 12.0 + 11.0 + 22.0 * k as f32;
        style.text_mid(label, panel.x + 16.0, y, 13, Weight::Regular, style::TEXT_MUTED);
        let value_x = panel.right() - 16.0 - style.measure(value, 14, Weight::Semibold).width;
        style.text_mid(value, value_x, y, 14, Weight::Semibold, style::TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::{CityId, CountryId};
    use crate::test_support;

    fn militia(members: u32, at: Vec2) -> Militia {
        Militia { id: MilitiaId::new(), name: "Test Militia".to_owned(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members, x: at.x, z: at.y, points: 100 }
    }

    #[test]
    fn a_soldier_is_drawn_for_every_forty_members_between_two_and_twenty_four() {
        let drawn = |members| soldiers_for(members);
        assert_eq!((drawn(10), drawn(80), drawn(81), drawn(250), drawn(960), drawn(1_000)), (2, 2, 3, 7, 24, 24));
        assert_eq!(drawn(0), 2, "a parade always has some ranks");
    }

    /// The boxes of a parade, with 1 for the flag's color so that it can be found.
    fn stand_at(world: &World, members: u32, at: Vec2, facing: u8, marching: Option<f32>) -> Vec<Part> {
        parts(world, &militia(members, at), at, facing, marching, Color::new(1.0, 0.0, 0.0, 1.0))
    }

    #[test]
    fn a_parade_has_an_officer_a_banner_bearer_a_drummer_and_its_ranks() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let flag = Color::new(1.0, 0.0, 0.0, 1.0);
        // The officer: 5 parts and an epaulette. The bearer: 5, the pole and the flag. Each soldier: 5 and a musket or a drum.
        let few = stand_at(&world, 80, at, 0, None);
        let many = stand_at(&world, 1_000, at, 0, None);
        assert_eq!(few.len(), 6 + 7 + 6 * 2);
        assert_eq!(many.len(), 6 + 7 + 6 * 24);
        assert!(few.iter().any(|p| p.color == DRUM), "a drummer");
        assert!(few.iter().any(|p| p.color == GOLD), "an officer");
        assert!(few.iter().filter(|p| p.color == flag).count() >= 3, "coats and the flag are the color of the country");
        assert!(few.iter().any(|p| p.color == MUSKET));
    }

    #[test]
    fn it_is_about_twice_the_size_it_was() {
        assert_eq!(SCALE, 4.0, "the camp it replaces was drawn at twice life size");
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let top = |parts: &[Part]| parts.iter().map(|p| p.middle.y + p.size.y * 0.5).fold(f32::MIN, f32::max);
        let ground = world.surface_height(at.x, at.y);
        assert!(top(&stand_at(&world, 400, at, 0, None)) - ground > 12.0, "the banner stands taller than 12 m");
    }

    #[test]
    fn the_head_of_the_column_is_the_way_it_faces() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let front = |facing| {
            let parts = stand_at(&world, 400, at, facing, None);
            // The officer's trousers are the first part, and stand ahead of the middle.
            vec2(parts[0].middle.x, parts[0].middle.z) - at
        };
        assert!(front(0).y > 8.0 && front(0).x.abs() < 6.0, "{}", front(0));
        assert!(front(1).x > 8.0 && front(1).y.abs() < 6.0, "{}", front(1));
        assert!(front(2).y < -8.0 && front(2).x.abs() < 6.0, "{}", front(2));
        assert!(front(3).x < -8.0 && front(3).y.abs() < 6.0, "{}", front(3));
        // Turned a quarter, a soldier's slab is turned too: it is wider along x when it faces along x.
        let flat = stand_at(&world, 400, at, 0, None);
        let turned = stand_at(&world, 400, at, 1, None);
        assert_eq!((flat[0].size.x, flat[0].size.z), (turned[0].size.z, turned[0].size.x));
    }

    #[test]
    fn the_soldiers_step_only_while_marching_and_out_of_time_with_each_other() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let standing = stand_at(&world, 400, at, 0, None);
        let marching = stand_at(&world, 400, at, 0, Some(0.37));
        let lifts: Vec<f32> = marching.iter().zip(&standing).map(|(m, s)| m.middle.y - s.middle.y).collect();
        assert!(lifts.iter().all(|l| *l >= -1e-4 && *l <= STEP_HEIGHT * SCALE + 1e-4));
        assert!(lifts.iter().any(|l| *l > 0.01), "some are stepping");
        let distinct = lifts.iter().map(|l| (l * 100.0) as i32).collect::<std::collections::HashSet<_>>().len();
        assert!(distinct > 3, "not all in step: {distinct}");
        assert!(standing.iter().zip(&standing).all(|(a, b)| a.middle == b.middle));
    }

    #[test]
    fn the_parade_stands_on_the_ground_and_can_be_picked_with_the_mouse() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let camp = militia(300, at);
        let ground = world.surface_height(at.x, at.y);
        for part in stand_at(&world, 300, at, 0, None) {
            assert!(part.middle.y - part.size.y * 0.5 > ground - 40.0, "not buried");
        }
        // A ray straight down at the officer hits, and one far off to the side doesn't.
        let officer = stand_at(&world, 300, at, 0, None)[0].middle;
        let walks = Walks::default();
        let down = (officer + vec3(0.0, 100.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down, &world, std::slice::from_ref(&camp), &walks), Some(camp.id));
        let aside = (officer + vec3(300.0, 100.0, 300.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, std::slice::from_ref(&camp), &walks), None);
    }
}
