//! Drawing the militia. Each is a camp on the ground outside the city it was raised in: a tent, a pole flying a flag in
//! the color of the country it belongs to, and a line of soldiers with spears in front of the tent, more of them for a
//! bigger militia. Like the gatherers it is built bigger than life, so that it can be seen from far off. This is the
//! game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::gatherer_view::hit;
use crate::hud;
use crate::ids::MilitiaId;
use crate::militia::{upkeep, Militia};

/// How many times bigger than life it is.
const SCALE: f32 = 2.0;
/// How far from a part of it, as a fraction of how far away it is, the mouse can be and still be on it, so that the
/// thin pole can be hit from far away.
const HOVER_SLOP: f32 = 0.006;
/// How many members each soldier drawn stands for, and the most soldiers drawn.
const MEMBERS_PER_SOLDIER: u32 = 100;
const MOST_SOLDIERS: usize = 10;
/// How many soldiers stand in a rank.
const RANK: usize = 5;

const CANVAS: Color = Color::new(0.84, 0.80, 0.68, 1.0);
const CANVAS_DARK: Color = Color::new(0.62, 0.58, 0.46, 1.0);
const POLE: Color = Color::new(0.82, 0.80, 0.74, 1.0);
const COAT: Color = Color::new(0.30, 0.31, 0.36, 1.0);
const SKIN: Color = Color::new(0.85, 0.70, 0.55, 1.0);
const SPEAR: Color = Color::new(0.55, 0.40, 0.25, 1.0);
/// A country that can't be found is flown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// A box that is part of a camp: where its middle is, how big it is across, and its color.
struct Part {
    middle: Vec3,
    size: Vec3,
    color: Color,
}

/// How many soldiers are drawn for a militia of `members`: one for every hundred, at least one and at most ten.
pub fn soldiers_for(members: u32) -> usize {
    (members.div_ceil(MEMBERS_PER_SOLDIER) as usize).clamp(1, MOST_SOLDIERS)
}

/// A box of `size` (in life-size meters) standing on the ground, `across` and `along` (life-size) from the camp's middle
/// at `camp`, with its bottom `lift` (life-size) over the ground there.
fn standing(world: &World, camp: Vec2, across: f32, along: f32, lift: f32, size: Vec3, color: Color) -> Part {
    let at = camp + vec2(across, along) * SCALE;
    let ground = world.surface_height(at.x, at.y);
    let size = size * SCALE;
    Part { middle: vec3(at.x, ground + lift * SCALE + size.y * 0.5, at.y), size, color }
}

/// The boxes a militia is drawn as, with its flag in `flag`.
fn parts(world: &World, militia: &Militia, flag: Color) -> Vec<Part> {
    let camp = vec2(militia.x, militia.z);
    let mut parts = vec![
        // The tent: a low body under a ridge.
        standing(world, camp, 0.0, -1.0, 0.0, vec3(3.0, 1.5, 2.4), CANVAS),
        standing(world, camp, 0.0, -1.0, 1.5, vec3(3.0, 0.5, 1.0), CANVAS_DARK),
        // The pole beside it, with the flag at the top.
        standing(world, camp, 2.4, -1.0, 0.0, vec3(0.14, 6.0, 0.14), POLE),
        standing(world, camp, 3.3, -1.0, 4.9, vec3(1.7, 1.0, 0.07), flag),
    ];
    // The soldiers, in ranks in front of the tent, each a coat, a head and a spear.
    for k in 0..soldiers_for(militia.members) {
        let (rank, file) = (k / RANK, k % RANK);
        let across = (file as f32 - (RANK as f32 - 1.0) * 0.5) * 1.3;
        let along = 2.2 + rank as f32 * 1.4;
        parts.push(standing(world, camp, across, along, 0.0, vec3(0.5, 1.1, 0.32), COAT));
        parts.push(standing(world, camp, across, along, 1.1, vec3(0.3, 0.3, 0.3), SKIN));
        parts.push(standing(world, camp, across + 0.3, along, 0.0, vec3(0.06, 2.4, 0.06), SPEAR));
    }
    parts
}

/// Draws the militia of `day`, as seen by the camera that is set, which must still be the one the map was drawn with.
pub fn draw(world: &World, day: &DaySave) {
    for militia in &day.militias {
        let flag = day.country(militia.country).map_or(NO_COUNTRY, |c| c.color.color());
        for part in parts(world, militia, flag) {
            draw_cube(part.middle, part.size, None, part.color);
        }
    }
}

/// The militia the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes
/// through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, militias: &[Militia]) -> Option<MilitiaId> {
    militias
        .iter()
        .filter_map(|militia| {
            let nearest = parts(world, militia, WHITE)
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
        Militia { id: MilitiaId::new(), name: "Test Militia".to_owned(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members, x: at.x, z: at.y }
    }

    #[test]
    fn a_soldier_is_drawn_for_every_hundred_members_up_to_ten() {
        let drawn = |members| soldiers_for(members);
        assert_eq!((drawn(1), drawn(100), drawn(101), drawn(250), drawn(950), drawn(1_000)), (1, 1, 2, 3, 10, 10));
        assert_eq!(drawn(0), 1, "a camp always has someone in it");
    }

    #[test]
    fn a_camp_has_a_tent_a_flag_and_its_soldiers() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let flag = Color::new(1.0, 0.0, 0.0, 1.0);
        let few = parts(&world, &militia(100, at), flag);
        let many = parts(&world, &militia(1_000, at), flag);
        assert_eq!(few.len(), 4 + 3, "the tent, its ridge, the pole and the flag, and one soldier");
        assert_eq!(many.len(), 4 + 3 * 10);
        assert!(few.iter().any(|p| p.color == flag), "the flag is the color of the country");
    }

    #[test]
    fn the_camp_stands_on_the_ground_and_can_be_picked_with_the_mouse() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let camp = militia(300, at);
        let ground = world.surface_height(at.x, at.y);
        for part in parts(&world, &camp, WHITE) {
            assert!(part.middle.y - part.size.y * 0.5 > ground - 40.0, "not buried");
        }
        // A ray straight down at the tent hits it, and one far off to the side doesn't.
        let tent = parts(&world, &camp, WHITE)[0].middle;
        let down = (tent + vec3(0.0, 100.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down, &world, std::slice::from_ref(&camp)), Some(camp.id));
        let aside = (tent + vec3(300.0, 100.0, 300.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, std::slice::from_ref(&camp)), None);
    }
}
