//! Drawing the militia, as a low pawn: a board game piece in the color of the country it belongs to, standing on a stack of gold
//! coins, with a coin for every hundred members (up to ten), so that a bigger militia has the taller stack. While it is
//! walking it hops along, as a piece does that is being moved. The pieces are sized by how far the camera is from them (see
//! [`crate::pieces`]), so that they can be seen on the map from far off. This is the game's own drawing, and not part of
//! drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::hud;
use crate::ids::MilitiaId;
use crate::militia::Militia;
use crate::movement::DAILY_POINTS;
use crate::pieces::{self, Piece, GOLD};
use crate::walk::Walks;

/// How many members each coin in the stack stands for, and the most coins in it.
const MEMBERS_PER_COIN: u32 = 100;
const MOST_COINS: usize = 10;
/// How thick a coin is, as a share of the token's radius, and how high a step lifts the piece, and how many times a second it
/// steps.
const COIN_THICKNESS: f32 = 0.09;
/// How big the pawn is, as a share of the coins under it.
const PAWN_SCALE: f32 = 1.3;
const HOP_HEIGHT: f32 = 0.5;
const HOP_RATE: f32 = 8.0;

/// A country that can't be found is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);
/// The steel of a soldier's helmet, which a militia that has been trained wears and one that hasn't does not.
const STEEL: Color = Color::new(0.55, 0.58, 0.64, 1.0);

/// How many coins are in the stack for a militia of `members`: one for every hundred, at least one and at most ten.
pub fn coins_for(members: u32) -> usize {
    (members.div_ceil(MEMBERS_PER_COIN) as usize).clamp(1, MOST_COINS)
}

/// The pieces a militia is drawn as, standing at `at` for a camera at `eye`: the stack of coins, and the pawn of `country`'s
/// color on it, hopping to the time `marching` if it is walking.
fn parts(world: &World, militia: &Militia, at: Vec2, marching: Option<f32>, country: Color, eye: Vec3) -> Vec<Piece> {
    let r = pieces::radius_at(eye, vec3(at.x, world.surface_height(at.x, at.y), at.y));
    if pieces::is_far(eye, vec3(at.x, world.surface_height(at.x, at.y), at.y)) {
        return far_marker(world, at, r, marching, country, militia.trained);
    }
    let hop = marching.map_or(0.0, |time| (time * HOP_RATE).sin().abs() * HOP_HEIGHT * r);
    let foot = vec3(at.x, pieces::ground_under(world, at.x, at.y, r), at.y) + vec3(0.0, hop, 0.0);
    let mut parts = Vec::new();

    // The coins, one on another, each rim a little different so that they can be told apart, and the top one with its face set
    // in it, so that from above it is a coin.
    let coins = coins_for(militia.members);
    for k in 0..coins {
        let rim = if k % 2 == 0 { GOLD } else { pieces::shade(GOLD, 0.82) };
        parts.push(Piece::disc(foot + vec3(0.0, k as f32 * COIN_THICKNESS * r, 0.0), r, COIN_THICKNESS * r, rim));
    }
    let top = foot + vec3(0.0, coins as f32 * COIN_THICKNESS * r, 0.0);
    parts.push(Piece::disc(top, 0.9 * r, 0.05 * r, pieces::shade(GOLD, 1.1)));

    // The pawn, in rings from above: a dark foot, a pale collar, the body in the country's color, and a light round head.
    let top = top + vec3(0.0, 0.05 * r, 0.0);
    let q = r * PAWN_SCALE;
    let pawn = vec![
        Piece::disc(top, 0.62 * q, 0.16 * q, pieces::shade(country, 0.6)),
        Piece::cone(top + vec3(0.0, 0.16 * q, 0.0), 0.46 * q, 1.25 * q, country),
        Piece::disc(top + vec3(0.0, 1.30 * q, 0.0), 0.50 * q, 0.1 * q, pieces::MEDALLION),
        Piece::ball(top + vec3(0.0, 1.72 * q, 0.0), 0.36 * q, pieces::shade(country, 1.3)),
    ];
    let mut pawn = pawn;
    if militia.trained {
        // A helmet over the head: a round cap with a brim.
        pawn.push(Piece::disc(top + vec3(0.0, 1.80 * q, 0.0), 0.44 * q, 0.26 * q, STEEL));
        pawn.push(Piece::disc(top + vec3(0.0, 1.76 * q, 0.0), 0.56 * q, 0.07 * q, pieces::shade(STEEL, 0.8)));
    }
    parts.extend(pieces::flatten(pawn, top.y, pieces::FLAT));
    parts
}

/// A militia seen from far off: a flat gold round with a disc of its country's color on it, and nothing else.
fn far_marker(world: &World, at: Vec2, r: f32, marching: Option<f32>, country: Color, trained: bool) -> Vec<Piece> {
    let hop = marching.map_or(0.0, |time| (time * HOP_RATE).sin().abs() * HOP_HEIGHT * r);
    let foot = vec3(at.x, pieces::ground_under(world, at.x, at.y, r), at.y) + vec3(0.0, hop, 0.0);
    let mut marker = vec![Piece::disc(foot, r, 0.1 * r, GOLD), Piece::disc(foot + vec3(0.0, 0.1 * r, 0.0), 0.62 * r, 0.06 * r, country)];
    if trained {
        // A small steel dot on the country's disc, which soldiers have and militia do not.
        marker.push(Piece::disc(foot + vec3(0.0, 0.16 * r, 0.0), 0.28 * r, 0.06 * r, STEEL));
    }
    marker
}

/// Draws the militia of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and
/// is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    for militia in &day.militias {
        let at = walks.position(militia);
        if !pieces::within_view(eye, at.x, at.y) {
            continue;
        }
        let country = day.country(militia.country).map_or(NO_COUNTRY, |c| c.color.color());
        let marching = walks.is_walking(militia.id).then_some(time);
        for piece in parts(world, militia, at, marching, country, eye) {
            piece.draw();
        }
    }
}

/// The militia the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes
/// through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, militias: &[Militia], walks: &Walks) -> Option<MilitiaId> {
    militias
        .iter()
        .filter_map(|militia| Some((pieces::first_hit(ray, &parts(world, militia, walks.position(militia), None, WHITE, ray.0))?, militia.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws the tooltip for `militia`, beside the mouse.
pub fn tooltip(style: &Style, day: &DaySave, militia: &Militia) {
    let origin = day.city(militia.city).map_or("a city", |city| city.name.as_str());
    let rows = [
        ("Country", day.country_name(militia.country).to_owned()),
        (if militia.trained { "Soldiers" } else { "Members" }, thousands(militia.members)),
        ("Raised in", origin.to_owned()),
        ("Upkeep", format!("{} coins a day", money(militia.upkeep()))),
        ("Movement", format!("{} of {} points", militia.points, DAILY_POINTS)),
    ];
    let mut rows = rows.to_vec();
    if militia.under_siege {
        rows.push(("Status", "Under siege".to_owned()));
    }
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
        Militia { id: MilitiaId::new(), name: "Test Militia".to_owned(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members, x: at.x, z: at.y, points: 100, under_siege: false, trained: false }
    }

    #[test]
    fn a_coin_is_in_the_stack_for_every_hundred_members_between_one_and_ten() {
        let coins = |members| coins_for(members);
        assert_eq!((coins(1), coins(100), coins(101), coins(250), coins(950), coins(1_000), coins(5_000)), (1, 1, 2, 3, 10, 10, 10));
        assert_eq!(coins(0), 1, "there is always a coin");
    }

    fn across_of(parts: &[Piece]) -> f32 {
        parts[0].size.x
    }

    fn eye_over(at: Vec2, world: &World, distance: f32) -> Vec3 {
        vec3(at.x, world.surface_height(at.x, at.y) + distance, at.y + 1.0)
    }

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);

    #[test]
    fn a_militia_is_a_pawn_in_its_countrys_color_on_a_stack_of_gold_coins() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let eye = eye_over(at, &world, 300.0);
        let few = parts(&world, &militia(100, at), at, None, COUNTRY, eye);
        let many = parts(&world, &militia(1_000, at), at, None, COUNTRY, eye);
        assert_eq!(few.len(), 1 + 1 + 4, "a coin, its face, and the four parts of a pawn");
        assert_eq!(many.len(), 10 + 1 + 4);
        assert_eq!(few.iter().filter(|p| p.color == GOLD).count(), 1);
        assert!(few.iter().any(|p| p.color == COUNTRY), "the body is the country's color");
        assert!(few.iter().any(|p| p.color == pieces::MEDALLION), "with a pale collar");
        let top = |parts: &[Piece]| parts.iter().map(Piece::top).fold(f32::MIN, f32::max);
        assert!(top(&many) > top(&few), "a bigger militia is taller");
        assert!((few.last().unwrap().top() - top(&few)).abs() < 1e-3, "the head is on top");
        assert!(top(&few) - few[0].middle.y < 4.0 * across_of(&few), "it is low, not a tall piece");
    }

    #[test]
    fn a_unit_stands_on_a_round_gold_base_and_a_gatherer_on_a_dark_square_tile() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let eye = eye_over(at, &world, 300.0);
        let unit = parts(&world, &militia(100, at), at, None, COUNTRY, eye);
        assert_eq!(unit[0].shape, pieces::Shape::Disc, "round");
        assert_eq!(unit[0].color, GOLD, "gold");
        let tile = pieces::tile(vec3(at.x, 0.0, at.y), 10.0, 1.0, COUNTRY);
        assert_eq!(tile[0].shape, pieces::Shape::Block, "square");
        assert_eq!(tile[0].color, pieces::TILE_EDGE, "dark");
        assert_ne!((unit[0].shape, unit[0].color), (tile[0].shape, tile[0].color));
    }

    #[test]
    fn it_is_big_enough_to_see_from_far_away() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let across = |distance: f32| parts(&world, &militia(300, at), at, None, COUNTRY, eye_over(at, &world, distance))[0].size.x;
        assert!(across(50.0) >= 16.0, "close up: {}", across(50.0));
        assert!(across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
        assert!(across(3_000.0) > 60.0, "{}", across(3_000.0));
    }

    #[test]
    fn from_far_off_it_is_only_a_gold_round_with_the_countrys_color_on_it() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let near = parts(&world, &militia(1_000, at), at, None, COUNTRY, eye_over(at, &world, 300.0));
        let far = parts(&world, &militia(1_000, at), at, None, COUNTRY, eye_over(at, &world, pieces::FAR + 500.0));
        assert!(far.len() == 2 && far.len() < near.len());
        assert_eq!((far[0].shape, far[0].color), (pieces::Shape::Disc, GOLD), "still a gold round, so it is still not mistaken for a fixed thing");
        assert_eq!(far[1].color, COUNTRY);
        assert!(far[1].size.x < far[0].size.x);
        // It still hops when walking, and is picked where it is.
        let hopping = parts(&world, &militia(1_000, at), at, Some(0.05), COUNTRY, eye_over(at, &world, pieces::FAR + 500.0));
        assert!(hopping[0].middle.y >= far[0].middle.y);
    }

    #[test]
    fn soldiers_wear_a_steel_helmet_and_from_far_off_a_steel_dot_and_militia_do_not() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let near = eye_over(at, &world, 300.0);
        let far = eye_over(at, &world, pieces::FAR + 500.0);
        let soldiers = Militia { trained: true, ..militia(100, at) };
        let (plain, trained) = (parts(&world, &militia(100, at), at, None, COUNTRY, near), parts(&world, &soldiers, at, None, COUNTRY, near));
        assert_eq!(trained.len(), plain.len() + 2, "a cap and its brim");
        assert!(trained.iter().filter(|p| p.color == STEEL).count() == 1 && !plain.iter().any(|p| p.color == STEEL));
        let top = |parts: &[Piece]| parts.iter().map(Piece::top).fold(f32::MIN, f32::max);
        assert!(top(&trained) > top(&plain) - 1e-3, "the helmet is on the head");
        let (plain, trained) = (parts(&world, &militia(100, at), at, None, COUNTRY, far), parts(&world, &soldiers, at, None, COUNTRY, far));
        assert_eq!((plain.len(), trained.len()), (2, 3));
        assert_eq!(trained[2].color, STEEL);
    }

    #[test]
    fn it_hops_only_while_walking() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let eye = eye_over(at, &world, 300.0);
        let standing = parts(&world, &militia(300, at), at, None, COUNTRY, eye);
        let heights: Vec<f32> = (0..40).map(|k| parts(&world, &militia(300, at), at, Some(k as f32 * 0.05), COUNTRY, eye)[0].middle.y - standing[0].middle.y).collect();
        assert!(heights.iter().all(|h| *h >= -1e-4), "never into the ground");
        assert!(heights.iter().any(|h| *h > 1.0), "it hops");
        assert!(standing.iter().zip(&standing).all(|(a, b)| a.middle == b.middle));
    }

    #[test]
    fn it_stands_on_the_ground_and_can_be_picked_with_the_mouse_where_it_is_shown() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let camp = militia(300, at);
        let ground = world.surface_height(at.x, at.y);
        let shown = parts(&world, &camp, at, None, COUNTRY, eye_over(at, &world, 300.0));
        assert!(shown[0].middle.y - shown[0].size.y * 0.5 >= ground - 1e-3, "the bottom coin is not buried");
        // A ray straight down at the pawn hits, and one far off to the side doesn't.
        let walks = Walks::default();
        let down = (eye_over(at, &world, 300.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down, &world, std::slice::from_ref(&camp), &walks), Some(camp.id));
        let aside = (down.0 + vec3(400.0, 0.0, 400.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, std::slice::from_ref(&camp), &walks), None);
        // While it walks, it is picked where it has got to.
        let mut walks = Walks::default();
        walks.start(camp.id, &[at, at + vec2(60.0, 0.0)]);
        walks.update(10.0);
        assert_eq!(walks.position(&camp), at, "once the walk is over it is where the day says");
    }
}
