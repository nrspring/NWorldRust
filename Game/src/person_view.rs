//! Drawing the people of the countries on the map, as one figure each on a round silver base with the standard of their country beside
//! them (see [`crate::miniatures`]). They face the way they last went, and while they walk they step along, as a militia does. The
//! tokens are sized by how far the camera is from them (see [`crate::pieces`]), so that they can be seen from far off. A person who has no
//! place on the map yet is not drawn. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
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

/// Draws the people of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    let models = day.people.iter().filter_map(|person| {
        let at = position(walks, person).filter(|at| pieces::within_view(eye, at.x, at.y))?;
        let country = day.country(person.country).map_or(NO_COUNTRY, |c| c.color.color());
        let marching = walks.is_walking(person.id).then_some(time);
        let turn = Mat3::from_rotation_y(walks.facing(person.id));
        Some(token(world, at, marching, eye).model(turn, country, Moment { time, busy: true }))
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// The person the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, people: &[Person], walks: &Walks) -> Option<PersonId> {
    people
        .iter()
        .filter_map(|person| Some((token(world, position(walks, person)?, None, ray.0).hit(ray)?, person.id)))
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
    use crate::ids::{CountryId, TitleTypeId};
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
        let list = [person.clone(), person_at(None)];
        assert_eq!(under_ray((vec3(at.x, top, at.y), vec3(0.0, -1.0, 0.0)), &world, &list, &walks), Some(person.id));
        assert_eq!(under_ray((vec3(at.x + 500.0, top, at.y), vec3(0.0, -1.0, 0.0)), &world, &list, &walks), None);
    }
}
