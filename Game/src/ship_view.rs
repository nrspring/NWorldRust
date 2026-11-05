//! Drawing the ships, as a low boat: a board game piece with a hull, a stripe of its country's color along the deck and white sails,
//! standing on the same round, gold base as a militia, so that it is known at a glance for a unit that moves. A small warship has one
//! mast and a large one is bigger, with two. While it is sailing it bobs along. The pieces are sized by how far the camera is from
//! them (see [`crate::pieces`]), so that they can be seen on the map from far off. This is the game's own drawing, and not part of
//! drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::money;
use crate::hud;
use crate::ids::ShipId;
use crate::movement::DAILY_POINTS;
use crate::pieces::{self, Piece, GOLD, MEDALLION};
use crate::ship::{Ship, ShipKind};
use crate::walk::Walks;

/// How thick the gold base is, as a share of the token's radius, and how high a wave lifts the boat, and how many times a second.
const BASE_THICKNESS: f32 = 0.09;
const BOB_HEIGHT: f32 = 0.3;
const BOB_RATE: f32 = 5.0;
/// The brown of the hull, and the color of a ship whose country can't be found.
const HULL: Color = Color::new(0.38, 0.25, 0.15, 1.0);
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// How big the boat is, as a share of the base under it: a large one is bigger than a small one.
fn scale(kind: ShipKind) -> f32 {
    match kind {
        ShipKind::Small => 1.0,
        ShipKind::Large => 1.3,
    }
}

/// How many masts a ship has.
pub fn masts_of(kind: ShipKind) -> usize {
    match kind {
        ShipKind::Small => 1,
        ShipKind::Large => 2,
    }
}

/// The pieces a ship is drawn as, floating at `at` for a camera at `eye`: the gold base, and the boat on it in the color of `country`,
/// bobbing to the time `sailing` if it is under way.
fn parts(world: &World, ship: &Ship, at: Vec2, sailing: Option<f32>, country: Color, eye: Vec3) -> Vec<Piece> {
    let water = vec3(at.x, world.surface_height(at.x, at.y), at.y);
    let r = pieces::radius_at(eye, water);
    let bob = sailing.map_or(0.0, |time| (time * BOB_RATE).sin().abs() * BOB_HEIGHT * r);
    let foot = vec3(at.x, pieces::ground_under(world, at.x, at.y, r), at.y) + vec3(0.0, bob, 0.0);
    if pieces::is_far(eye, water) {
        return far_marker(foot, r, country);
    }
    let s = scale(ship.kind);
    let mut parts = vec![Piece::disc(foot, r * s.min(1.15), BASE_THICKNESS * r, GOLD)];
    let top = foot + vec3(0.0, BASE_THICKNESS * r, 0.0);

    // The boat, from the hull up: the hull itself, a stripe of the country's color on its deck, and a mast with a sail for each.
    let (length, beam) = (1.5 * r * s, 0.62 * r * s);
    let mut boat = vec![
        Piece::block_on(top, vec3(length, 0.34 * r * s, beam), HULL),
        Piece::block_on(top + vec3(0.0, 0.34 * r * s, 0.0), vec3(length * 0.86, 0.1 * r * s, beam * 0.76), country),
    ];
    let deck = top + vec3(0.0, 0.44 * r * s, 0.0);
    let masts = masts_of(ship.kind);
    for k in 0..masts {
        let x = if masts == 1 { 0.0 } else { (k as f32 - 0.5) * 0.7 * r * s };
        boat.push(Piece::block_on(deck + vec3(x, 0.0, 0.0), vec3(0.07 * r * s, 1.15 * r * s, 0.07 * r * s), HULL));
        boat.push(Piece::block_on(deck + vec3(x, 0.25 * r * s, 0.0), vec3(0.05 * r * s, 0.8 * r * s, 0.62 * r * s), MEDALLION));
    }
    parts.extend(pieces::flatten(boat, top.y, pieces::FLAT));
    parts
}

/// A ship seen from far off: a flat gold round with a disc of its country's color on it, and a small pale mark for the sail, which is
/// what tells it from a militia.
fn far_marker(foot: Vec3, r: f32, country: Color) -> Vec<Piece> {
    let on = foot + vec3(0.0, 0.1 * r, 0.0);
    vec![
        Piece::disc(foot, r, 0.1 * r, GOLD),
        Piece::disc(on, 0.62 * r, 0.06 * r, country),
        Piece::block_on(on + vec3(0.0, 0.06 * r, 0.0), vec3(0.14 * r, 0.1 * r, 0.5 * r), MEDALLION),
    ]
}

/// Draws the ships of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    for ship in &day.ships {
        let at = walks.position_of(ship.id, vec2(ship.x, ship.z));
        if !pieces::within_view(eye, at.x, at.y) {
            continue;
        }
        let country = day.country(ship.country).map_or(NO_COUNTRY, |c| c.color.color());
        let sailing = walks.is_walking(ship.id).then_some(time);
        for piece in parts(world, ship, at, sailing, country, eye) {
            piece.draw();
        }
    }
}

/// The ship the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, ships: &[Ship], walks: &Walks) -> Option<ShipId> {
    ships
        .iter()
        .filter_map(|ship| Some((pieces::first_hit(ray, &parts(world, ship, walks.position_of(ship.id, vec2(ship.x, ship.z)), None, WHITE, ray.0))?, ship.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws the tooltip for `ship`, beside the mouse.
pub fn tooltip(style: &Style, day: &DaySave, ship: &Ship) {
    let rows = [
        ("Country", day.country_name(ship.country).to_owned()),
        ("Sails", ship.kind.waters_text().to_owned()),
        ("Built", ship.built.to_string()),
        ("Upkeep", format!("{} coins a day", money(ship.kind.upkeep()))),
        ("Movement", format!("{} of {} points", ship.points, DAILY_POINTS)),
    ];
    let label_w = rows.iter().map(|(label, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let title = ship.title();
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
    use crate::ids::CountryId;
    use crate::test_support;

    fn ship(kind: ShipKind, at: Vec2) -> Ship {
        Ship { id: ShipId::new(), name: "Test Ship".to_owned(), kind, country: CountryId::new(), built: Date::FIRST, x: at.x, z: at.y, points: 100 }
    }

    fn eye_over(at: Vec2, world: &World, distance: f32) -> Vec3 {
        vec3(at.x, world.surface_height(at.x, at.y) + distance, at.y + 1.0)
    }

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);

    /// A point out in the water, if the test continent has any.
    fn water() -> (World, Vec2) {
        let world = test_support::small_continent();
        let (_, sea) = test_support::land_and_sea(&world);
        (world, sea)
    }

    #[test]
    fn a_ship_is_a_boat_in_its_countrys_color_on_a_gold_base_and_a_large_one_has_two_masts() {
        let (world, at) = water();
        let eye = eye_over(at, &world, 300.0);
        let small = parts(&world, &ship(ShipKind::Small, at), at, None, COUNTRY, eye);
        let large = parts(&world, &ship(ShipKind::Large, at), at, None, COUNTRY, eye);
        // The base, the hull, the stripe, and a mast and a sail for each mast.
        assert_eq!(small.len(), 1 + 2 + 2 * masts_of(ShipKind::Small));
        assert_eq!(large.len(), 1 + 2 + 2 * masts_of(ShipKind::Large));
        assert_eq!((masts_of(ShipKind::Small), masts_of(ShipKind::Large)), (1, 2));
        assert_eq!((small[0].shape, small[0].color), (pieces::Shape::Disc, GOLD), "a unit that moves stands on a round gold base");
        assert!(small.iter().any(|p| p.color == COUNTRY) && small.iter().any(|p| p.color == MEDALLION) && small.iter().any(|p| p.color == HULL));
        let hull = |parts: &[Piece]| parts.iter().find(|p| p.color == HULL).unwrap().size.x;
        assert!(hull(&large) > hull(&small), "a large one is bigger");
    }

    #[test]
    fn from_far_off_it_is_a_gold_round_with_its_countrys_color_and_a_pale_sail_mark() {
        let (world, at) = water();
        let near = parts(&world, &ship(ShipKind::Large, at), at, None, COUNTRY, eye_over(at, &world, 300.0));
        let far = parts(&world, &ship(ShipKind::Large, at), at, None, COUNTRY, eye_over(at, &world, pieces::FAR + 500.0));
        assert!(far.len() == 3 && far.len() < near.len());
        assert_eq!((far[0].shape, far[0].color), (pieces::Shape::Disc, GOLD));
        assert_eq!(far[1].color, COUNTRY);
        assert_eq!(far[2].color, MEDALLION, "the sail is what tells it from a militia");
    }

    #[test]
    fn it_bobs_only_while_sailing_and_never_below_the_water() {
        let (world, at) = water();
        let eye = eye_over(at, &world, 300.0);
        let standing = parts(&world, &ship(ShipKind::Small, at), at, None, COUNTRY, eye);
        let heights: Vec<f32> = (0..40).map(|k| parts(&world, &ship(ShipKind::Small, at), at, Some(k as f32 * 0.05), COUNTRY, eye)[0].middle.y - standing[0].middle.y).collect();
        assert!(heights.iter().all(|h| *h >= -1e-4) && heights.iter().any(|h| *h > 0.5));
        assert!(standing[0].middle.y - standing[0].size.y * 0.5 >= world.surface_height(at.x, at.y) - 1e-3, "the base is not under the water");
    }

    #[test]
    fn it_can_be_picked_with_the_mouse_where_it_floats() {
        let (world, at) = water();
        let boat = ship(ShipKind::Large, at);
        let down = (eye_over(at, &world, 300.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down, &world, std::slice::from_ref(&boat), &Walks::default()), Some(boat.id));
        let aside = (down.0 + vec3(400.0, 0.0, 400.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, std::slice::from_ref(&boat), &Walks::default()), None);
    }

    #[test]
    fn it_is_big_enough_to_see_from_far_away() {
        let (world, at) = water();
        let across = |distance: f32| parts(&world, &ship(ShipKind::Small, at), at, None, COUNTRY, eye_over(at, &world, distance))[0].size.x;
        assert!(across(50.0) >= 16.0 && across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
    }
}
