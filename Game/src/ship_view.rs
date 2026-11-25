//! Drawing the ships, as hulls with sails (see [`crate::miniatures`]) in a ring of gold and of the color of the country they
//! belong to, which floats on the water round them, so that a ship is known at a glance for a unit that moves. A small warship
//! has one mast with a striped sail, and a large one is longer, with three masts and a row of gun ports. A ship faces the way
//! it last sailed, and rocks on the water, more while it is under way. The pieces are sized by how far the camera is from them
//! (see [`crate::pieces`]), so that they can be seen on the map from far off. This is the game's own drawing, and not part of
//! drawing the map.

use std::hash::{DefaultHasher, Hash, Hasher};

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::money;
use crate::hud;
use crate::ids::ShipId;
use crate::miniatures::{Look, Moment};
use crate::movement::DAILY_POINTS;
use crate::pieces::{self, Token};
use crate::ship::{Ship, ShipKind};
use crate::walk::Walks;

/// How far a ship rolls from side to side and pitches, in radians, at rest and under way, and how high a wave lifts it, as a
/// share of the token's radius.
const ROLL: (f32, f32) = (0.035, 0.07);
const PITCH: (f32, f32) = (0.015, 0.035);
const BOB: (f32, f32) = (0.02, 0.06);
/// The color of a ship whose country can't be found.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// What a ship of `kind` looks like.
pub fn look(kind: ShipKind) -> Look {
    Look::Ship { large: kind == ShipKind::Large }
}

/// How many masts a ship has.
#[cfg(test)]
pub fn masts_of(kind: ShipKind) -> usize {
    match kind {
        ShipKind::Small => 1,
        ShipKind::Large => 3,
    }
}

/// How far into its rocking a ship is at `time`, so that ships don't all rock together: 0 to a full turn.
fn phase(id: ShipId) -> f32 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    (hasher.finish() % 1_000) as f32 / 1_000.0 * std::f32::consts::TAU
}

/// The token of `ship`, floating at `at` for a camera at `eye`.
fn token(world: &World, ship: &Ship, at: Vec2, eye: Vec3) -> Token {
    Token::at(world, look(ship.kind), at.x, at.y, eye)
}

/// The way a ship heading `yaw` leans on the water at `time`, and how high the water lifts it, as a share of its radius.
fn rocking(yaw: f32, time: f32, phase: f32, sailing: bool) -> (Mat3, f32) {
    let pick = |(rest, way): (f32, f32)| if sailing { way } else { rest };
    let t = time + phase;
    let roll = (t * 1.3).sin() * pick(ROLL);
    let pitch = (t * 0.9 + 1.0).sin() * pick(PITCH);
    let bob = (t * 1.1).sin() * pick(BOB);
    (Mat3::from_rotation_y(yaw) * Mat3::from_rotation_x(roll) * Mat3::from_rotation_z(pitch), bob)
}

/// Draws the ships of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    let models = day.ships.iter().filter_map(|ship| {
        let at = walks.position_of(ship.id, vec2(ship.x, ship.z));
        if !pieces::within_view(eye, at.x, at.y) {
            return None;
        }
        let country = day.country(ship.country).map_or(NO_COUNTRY, |c| c.color.color());
        let (turn, bob) = rocking(walks.facing(ship.id), time, phase(ship.id), walks.is_walking(ship.id));
        let token = token(world, ship, at, eye);
        Some(token.lifted(bob * token.radius).model(turn, country, Moment { time, busy: true }))
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// The ship the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, ships: &[Ship], walks: &Walks) -> Option<ShipId> {
    ships
        .iter()
        .filter_map(|ship| Some((token(world, ship, walks.position_of(ship.id, vec2(ship.x, ship.z)), ray.0).hit(ray)?, ship.id)))
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
        Ship { id: ShipId::new(), name: "Test Ship".to_owned(), kind, country: CountryId::new(), built: Date::FIRST, x: at.x, z: at.y, points: 100, pay_first: false, garrison: false }
    }

    fn eye_over(at: Vec2, world: &World, distance: f32) -> Vec3 {
        vec3(at.x, world.surface_height(at.x, at.y) + distance, at.y + 1.0)
    }

    /// A point out in the water, if the test continent has any.
    fn water() -> (World, Vec2) {
        let world = test_support::small_continent();
        let (_, sea) = test_support::land_and_sea(&world);
        (world, sea)
    }

    #[test]
    fn a_large_ship_is_bigger_and_has_more_masts() {
        assert_eq!((masts_of(ShipKind::Small), masts_of(ShipKind::Large)), (1, 3));
        assert!(look(ShipKind::Large).reach() > look(ShipKind::Small).reach());
        let (world, at) = water();
        let eye = eye_over(at, &world, 300.0);
        assert!(token(&world, &ship(ShipKind::Large, at), at, eye).bounds().1.x > token(&world, &ship(ShipKind::Small, at), at, eye).bounds().1.x);
    }

    #[test]
    fn it_rocks_more_while_sailing_and_always_faces_the_way_it_goes() {
        let lean = |sailing: bool| (0..60).map(|k| (rocking(0.0, k as f32 * 0.1, 0.0, sailing).0 * Vec3::Y).y).fold(1.0f32, f32::min);
        assert!(lean(true) < lean(false) && lean(false) < 1.0, "it leans, and leans more under way");
        let (turn, _) = rocking(1.0, 0.0, 0.0, false);
        let front = turn * Vec3::X;
        assert!((vec2(front.x, front.z).normalize() - vec2(1.0f32.cos(), -1.0f32.sin())).length() < 0.05, "facing the way of its yaw");
    }

    #[test]
    fn it_can_be_picked_with_the_mouse_where_it_floats() {
        let (world, at) = water();
        let boat = ship(ShipKind::Large, at);
        let down = (eye_over(at, &world, 300.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(down, &world, std::slice::from_ref(&boat), &Walks::default()), Some(boat.id));
        let aside = (down.0 + vec3(400.0, 0.0, 400.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, std::slice::from_ref(&boat), &Walks::default()), None);
        assert!(token(&world, &boat, at, down.0).foot.y >= world.surface_height(at.x, at.y) - 1e-3, "it floats on the water");
    }

    #[test]
    fn it_is_big_enough_to_see_from_far_away() {
        let (world, at) = water();
        let across = |distance: f32| token(&world, &ship(ShipKind::Small, at), at, eye_over(at, &world, distance)).bounds().1.x;
        assert!(across(50.0) >= 16.0 && across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
    }
}
