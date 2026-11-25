//! Drawing the people of the countries on the map, as one figure each on a round silver base with the standard of their country beside
//! them (see [`crate::miniatures`]). They face the way they last went, and while they walk they step along, as a militia does. The
//! tokens are sized by how far the camera is from them (see [`crate::pieces`]), so that they can be seen from far off. A person who has no
//! place on the map yet is not drawn, except for the figure under the mouse while they are being put there. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::{WaterClass, World};
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::{barracks_view, gatherer_view, shipyard_view};
use crate::hud;
use crate::ids::PersonId;
use crate::miniatures::{Look, Moment};
use crate::movement::DAILY_POINTS;
use crate::person::Person;
use crate::pieces::{self, Token};
use crate::walk::Walks;

/// How high a step lifts a person, as a share of the token's radius, and how many times a second they step.
const STEP_HEIGHT: f32 = 0.18;
const STEP_RATE: f32 = 8.0;

/// How much room is left round a building, whose plot is square, as a share of how far it reaches to the side.
const PLOT_CORNERS: f32 = 1.2;
/// The ways to step out from behind a building, as turns from the way to the camera, in the order they are tried.
const WAYS_OUT: [f32; 5] = [0.0, 0.9, -0.9, 1.6, -1.6];

/// A country that can't be found is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// The token of `person`, standing at `at` for a camera at `eye`, stepping to the time `marching` if they are walking.
pub fn token(world: &World, at: Vec2, marching: Option<f32>, eye: Vec3) -> Token {
    let token = Token::at(world, Look::Person, at.x, at.y, eye);
    let step = marching.map_or(0.0, |time| (time * STEP_RATE).sin().abs() * STEP_HEIGHT * token.radius);
    token.lifted(step)
}

/// Where to show `person`: part of the way along their walk if they are walking, and otherwise where they stand. Nowhere, if they have
/// no place on the map yet.
pub fn position(walks: &Walks, person: &Person) -> Option<Vec2> {
    person.position().map(|at| walks.position_of(person.id, at))
}

/// Where `person` is drawn, and pointed at, for a camera at `eye`: where they are (see [`position`]), except that one standing where a
/// gatherer, a shipyard or a barracks would hide them is drawn just clear of it, on the camera's side. That is only how they are shown:
/// where they stand in the game is not changed. One who is walking is shown on their way.
pub fn shown_at(world: &World, day: &DaySave, walks: &Walks, person: &Person, eye: Vec3) -> Option<Vec2> {
    let at = position(walks, person)?;
    if walks.is_walking(person.id) {
        return Some(at);
    }
    let own = Token::at(world, Look::Person, at.x, at.y, eye);
    let gatherers = day.gatherers.iter().map(|g| gatherer_view::token(world, g.kind, g.x, g.z, eye));
    let shipyards = day.shipyards.iter().map(|s| shipyard_view::token(world, s.kind, s.x, s.z, eye));
    let barracks = day.barracks.iter().map(|b| barracks_view::token(world, b.kind, b.x, b.z, eye));
    // Each building as its middle, and how far from it a person has to be to be clear of it.
    let buildings: Vec<(Vec2, f32)> = gatherers
        .chain(shipyards)
        .chain(barracks)
        .map(|token| (vec2(token.foot.x, token.foot.z), token.look.reach() * token.radius * PLOT_CORNERS + own.look.reach() * own.radius))
        .collect();
    Some(step_out(at, &buildings, vec2(eye.x, eye.z), |spot| world.water_class(spot.x, spot.y) == WaterClass::Dry))
}

/// `at`, moved out of the way of `buildings` (each its middle and how far to keep from it) towards `camera`, which is where it is as
/// seen from above, until it is clear of them all. Dry ground (see `dry`) is chosen over water where there is a way to it.
fn step_out(mut at: Vec2, buildings: &[(Vec2, f32)], camera: Vec2, dry: impl Fn(Vec2) -> bool) -> Vec2 {
    // (Stepping clear of one building may step into another, so it is done a few times over.)
    for _ in 0..4 {
        let Some(&(middle, clear)) = buildings.iter().find(|(middle, clear)| at.distance(*middle) < clear - 0.01) else { break };
        // Looking straight down, the way to the camera is no way at all, and the bottom of the screen will do.
        let toward = (camera - middle).try_normalize().unwrap_or(Vec2::Y);
        let from = at - middle;
        let spot = |turn: f32| {
            let way = Vec2::from_angle(turn).rotate(toward);
            // How far along `way` it is until `clear` of the middle.
            let along = from.dot(way);
            at + way * ((along * along - from.length_squared() + clear * clear).sqrt() - along)
        };
        at = WAYS_OUT.iter().map(|turn| spot(*turn)).find(|s| dry(*s)).unwrap_or_else(|| spot(0.0));
    }
    at
}

/// Draws the people of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    let models = day.people.iter().filter_map(|person| {
        let at = shown_at(world, day, walks, person, eye).filter(|at| pieces::within_view(eye, at.x, at.y))?;
        let country = day.country(person.country).map_or(NO_COUNTRY, |c| c.color.color());
        let marching = walks.is_walking(person.id).then_some(time);
        let turn = Mat3::from_rotation_y(walks.facing(person.id));
        Some(token(world, at, marching, eye).model(turn, country, Moment { time, busy: true }))
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// Draws a figure at `at` where a person is about to be put, in `color`.
pub fn draw_ghost(world: &World, at: Vec2, color: Color, time: f32, eye: Vec3) {
    pieces::draw_models(eye, Vec4::ZERO, std::iter::once(token(world, at, None, eye).model(Mat3::IDENTITY, color, Moment { time, busy: false })));
}

/// The person the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, day: &DaySave, walks: &Walks) -> Option<PersonId> {
    day.people
        .iter()
        .filter_map(|person| Some((token(world, shown_at(world, day, walks, person, ray.0)?, None, ray.0).hit(ray)?, person.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws the tooltip for `person`, beside the mouse.
pub fn tooltip(style: &Style, day: &DaySave, person: &Person) {
    let rows = [
        ("Country", day.country_name(person.country).to_owned()),
        ("Title", person.title.clone()),
        ("Movement", format!("{} of {} points", person.points, DAILY_POINTS)),
    ];
    let label_w = rows.iter().map(|(label, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let title = person.name.as_str();
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
    use crate::ids::{CountryId, ShipyardId, TitleTypeId};
    use crate::shipyard::{Shipyard, ShipyardKind};
    use crate::test_support;

    fn person_at(at: Option<(f32, f32)>) -> Person {
        Person { id: PersonId::new(), name: "Mara Voss".to_owned(), title: "President".to_owned(), title_type: TitleTypeId::new(), country: CountryId::new(), at, points: 200 }
    }

    #[test]
    fn a_person_with_no_place_has_no_position_and_one_with_a_place_is_there_until_they_walk() {
        let walks = Walks::default();
        assert_eq!(position(&walks, &person_at(None)), None);
        assert_eq!(position(&walks, &person_at(Some((3.0, 4.0)))), Some(vec2(3.0, 4.0)));
    }

    #[test]
    fn a_ray_straight_down_through_a_person_hits_them_and_one_beside_does_not_and_the_unplaced_are_never_hit() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 40.0);
        let person = person_at(Some((at.x, at.y)));
        let top = world.surface_height(at.x, at.y) + 300.0;
        let walks = Walks::default();
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.people = vec![person.clone(), person_at(None)];
        assert_eq!(under_ray((vec3(at.x, top, at.y), vec3(0.0, -1.0, 0.0)), &world, &day, &walks), Some(person.id));
        assert_eq!(under_ray((vec3(at.x + 500.0, top, at.y), vec3(0.0, -1.0, 0.0)), &world, &day, &walks), None);
    }

    #[test]
    fn a_person_behind_a_building_is_shown_clear_of_it_towards_the_camera() {
        let buildings = [(vec2(0.0, 0.0), 10.0)];
        let camera = vec2(0.0, 100.0);
        // One who is clear already stays where they are.
        assert_eq!(step_out(vec2(30.0, 0.0), &buildings, camera, |_| true), vec2(30.0, 0.0));
        // One in the middle of it comes out on the camera's side, and one off to a side of it keeps to that side.
        assert!(step_out(vec2(0.0, 0.0), &buildings, camera, |_| true).distance(vec2(0.0, 10.0)) < 0.01);
        let beside = step_out(vec2(6.0, 0.0), &buildings, camera, |_| true);
        assert!(beside.distance(vec2(6.0, 8.0)) < 0.01, "{beside}");
        // Looking straight down, they come out towards the bottom of the screen.
        assert!(step_out(vec2(0.0, 0.0), &buildings, vec2(0.0, 0.0), |_| true).distance(vec2(0.0, 10.0)) < 0.01);
        // Water on the camera's side sends them another way, and with water all round they come out towards the camera anyway.
        let ashore = step_out(vec2(0.0, 0.0), &buildings, camera, |spot| spot.x < -5.0);
        assert!(ashore.x < -5.0 && (ashore.length() - 10.0).abs() < 0.01, "{ashore}");
        assert!(step_out(vec2(0.0, 0.0), &buildings, camera, |_| false).distance(vec2(0.0, 10.0)) < 0.01);
        // Out of one and into the next, they come out of that too.
        let two = [(vec2(0.0, 0.0), 10.0), (vec2(0.0, 12.0), 10.0)];
        let out = step_out(vec2(0.0, 0.0), &two, camera, |_| true);
        assert!(two.iter().all(|(middle, clear)| out.distance(*middle) > clear - 0.02), "{out}");
    }

    #[test]
    fn a_person_in_a_shipyard_is_pointed_at_beside_it_and_still_stands_where_they_did() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 40.0);
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.people = vec![person_at(Some((at.x, at.y)))];
        let walks = Walks::default();
        let eye = vec3(at.x, world.surface_height(at.x, at.y) + 200.0, at.y + 200.0);
        assert_eq!(shown_at(&world, &day, &walks, &day.people[0], eye), position(&walks, &day.people[0]), "nothing in the way");
        day.shipyards = vec![Shipyard { id: ShipyardId::new(), country: CountryId::new(), kind: ShipyardKind::Large, x: at.x + 3.0, z: at.y + 2.0, region: Some(1), ready_on: None }];
        let shown = shown_at(&world, &day, &walks, &day.people[0], eye).unwrap();
        assert!(shown.distance(vec2(at.x + 3.0, at.y + 2.0)) > 8.0 && shown.y > at.y, "in front of it: {shown}");
        assert_eq!(day.people[0].position(), Some(at));
    }
}
