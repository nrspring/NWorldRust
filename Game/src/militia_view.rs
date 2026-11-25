//! Drawing the militia, as a band of figures on a round base (see [`crate::miniatures`]): the base is a rim round a disc of the
//! color of the country it belongs to, and there is a figure for every two hundred members, up to five, so that a bigger
//! militia is the bigger band, with a flag of its country flying behind them. Militia are in brown, with straw hats and spears,
//! on a gold rim, and soldiers, a militia that has been trained, in their country's color, with steel helmets, pikes and
//! shields, on a steel rim, and archers, in green hoods with a bow and a quiver, on a green rim. They face the way they last went, and while they walk they step along. The tokens are sized by how
//! far the camera is from them (see [`crate::pieces`]), so that they can be seen on the map from far off. This is the game's own
//! drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::hud;
use crate::ids::MilitiaId;
use crate::militia::{Class, Militia};
use crate::miniatures::{Look, Moment};
use crate::pieces::{self, Token};
use crate::walk::Walks;

/// How many members each figure stands for, and the most figures there are.
const MEMBERS_PER_FIGURE: u32 = 200;
const MOST_FIGURES: usize = 5;
/// The most horsemen a band of cavalry has: a horse is bigger than a man on foot.
const MOST_HORSEMEN: usize = 3;
/// How high a step lifts the band, as a share of the token's radius, and how many times a second it steps.
const STEP_HEIGHT: f32 = 0.18;
const STEP_RATE: f32 = 8.0;

/// A country that can't be found is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// How many figures there are for a militia of `members`: one for every two hundred, at least one and at most five.
pub fn figures_for(members: u32) -> usize {
    (members.div_ceil(MEMBERS_PER_FIGURE) as usize).clamp(1, MOST_FIGURES)
}

/// What `militia` looks like.
pub fn look(militia: &Militia) -> Look {
    let figures = figures_for(militia.members);
    match militia.class() {
        Class::Cavalry => Look::Cavalry { figures: figures.min(MOST_HORSEMEN) },
        Class::Archers => Look::Archers { figures },
        class => Look::Militia { figures, trained: class != Class::Militia },
    }
}

/// The token of `militia`, standing at `at` for a camera at `eye`, stepping to the time `marching` if it is walking.
fn token(world: &World, militia: &Militia, at: Vec2, marching: Option<f32>, eye: Vec3) -> Token {
    let token = Token::at(world, look(militia), at.x, at.y, eye);
    let step = marching.map_or(0.0, |time| (time * STEP_RATE).sin().abs() * STEP_HEIGHT * token.radius);
    token.lifted(step)
}

/// Draws the militia of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and
/// is at `eye`.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, time: f32, eye: Vec3) {
    let models = day.militias.iter().filter_map(|militia| {
        let at = walks.position(militia);
        if !pieces::within_view(eye, at.x, at.y) {
            return None;
        }
        let country = day.country(militia.country).map_or(NO_COUNTRY, |c| c.color.color());
        let marching = walks.is_walking(militia.id).then_some(time);
        let turn = Mat3::from_rotation_y(walks.facing(militia.id));
        Some(token(world, militia, at, marching, eye).model(turn, country, Moment { time, busy: true }))
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// The militia the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes
/// through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, militias: &[Militia], walks: &Walks) -> Option<MilitiaId> {
    militias
        .iter()
        .filter_map(|militia| Some((token(world, militia, walks.position(militia), None, ray.0).hit(ray)?, militia.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws the tooltip for `militia`, beside the mouse.
pub fn tooltip(style: &Style, day: &DaySave, militia: &Militia) {
    let origin = day.city(militia.city).map_or("a city", |city| city.name.as_str());
    let rows = [
        ("Country", day.country_name(militia.country).to_owned()),
        (militia.people_label(), thousands(militia.members)),
        ("Raised in", origin.to_owned()),
        ("Upkeep", format!("{} coins a day", money(militia.upkeep()))),
        ("Movement", format!("{} of {} points", militia.points, militia.daily_points())),
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
        Militia { id: MilitiaId::new(), name: "Test Militia".to_owned(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members, x: at.x, z: at.y, points: 100, under_siege: false, trained: false, archers: false, cavalry: false, pay_first: false, garrison: false }
    }

    fn eye_over(at: Vec2, world: &World, distance: f32) -> Vec3 {
        vec3(at.x, world.surface_height(at.x, at.y) + distance, at.y + 1.0)
    }

    #[test]
    fn a_figure_is_in_the_band_for_every_two_hundred_members_between_one_and_five() {
        let figures = |members| figures_for(members);
        assert_eq!((figures(1), figures(200), figures(201), figures(500), figures(999), figures(1_000), figures(5_000)), (1, 1, 2, 3, 5, 5, 5));
        assert_eq!(figures(0), 1, "there is always a figure");
    }

    #[test]
    fn soldiers_look_like_soldiers_and_militia_like_militia() {
        let at = Vec2::ZERO;
        let soldiers = Militia { trained: true, ..militia(300, at) };
        assert_eq!(look(&militia(300, at)), Look::Militia { figures: 2, trained: false });
        assert_eq!(look(&soldiers), Look::Militia { figures: 2, trained: true });
        let archers = Militia { trained: true, archers: true, ..militia(300, at) };
        assert_eq!(look(&archers), Look::Archers { figures: 2 });
        let mut cavalry = militia(900, at);
        cavalry.set_class(Class::Cavalry);
        assert_eq!(look(&cavalry), Look::Cavalry { figures: 3 }, "five figures' worth of members, but never more than three horsemen");
        assert_eq!(look(&Militia { members: 100, ..cavalry }), Look::Cavalry { figures: 1 });
    }

    #[test]
    fn it_is_big_enough_to_see_from_far_away() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let across = |distance: f32| token(&world, &militia(300, at), at, None, eye_over(at, &world, distance)).bounds().1.x;
        assert!(across(50.0) >= 16.0, "close up: {}", across(50.0));
        assert!(across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
        assert!(across(3_000.0) > 40.0, "{}", across(3_000.0));
    }

    #[test]
    fn it_steps_only_while_walking() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let eye = eye_over(at, &world, 300.0);
        let standing = token(&world, &militia(300, at), at, None, eye).foot.y;
        let heights: Vec<f32> = (0..40).map(|k| token(&world, &militia(300, at), at, Some(k as f32 * 0.05), eye).foot.y - standing).collect();
        assert!(heights.iter().all(|h| *h >= -1e-4), "never into the ground");
        assert!(heights.iter().any(|h| *h > 0.5), "it steps");
    }

    #[test]
    fn it_stands_on_the_ground_and_can_be_picked_with_the_mouse_where_it_is_shown() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let camp = militia(300, at);
        let ground = world.surface_height(at.x, at.y);
        assert!(token(&world, &camp, at, None, eye_over(at, &world, 300.0)).foot.y >= ground - 1e-3, "the base is not buried");
        // A ray straight down at it hits, and one far off to the side doesn't.
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
