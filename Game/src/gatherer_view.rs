//! Drawing the gatherers, as buildings on square plots (see [`crate::miniatures`]): a dark slate plot with a band of the color of
//! the country it belongs to, and on it a model of the place, each with a shape and a color of its own so that it can be told
//! from the others from the side and from above: pines and a hut for a woodcutter, a mill with a red roof and a water wheel for a
//! sawmill, boulders and cut blocks for a stonecutter, a stepped face and a derrick for a quarry, a headframe and rust red ore for
//! an iron mine, a smelter's chimney with green ore and copper ingots for a copper mine, a yellow crater for a sulphur pit and a
//! derrick, a red pump and a black tank for an oil well. The lesser gatherers, the woodcutter and the stonecutter, have the
//! smaller plot. A gatherer that is working has its wheels turning and its fires smoking. A gatherer is fixed, and so stands on a
//! square, and a unit that can move stands on a round base (see [`crate::militia_view`]), so that they are told apart at a
//! glance. The tokens are sized by how far the camera is from them (see [`crate::pieces`]), so that they can be seen on the map
//! from far off. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::gatherer::{Gatherer, GathererKind};
use crate::ids::GathererId;
use crate::miniatures::{Look, Moment};
use crate::pieces::{self, Token};

/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);
/// How many meters a token's radius is taken to be, for holding things over a gatherer at a height that doesn't change with the
/// camera.
const HOLD_RADIUS: f32 = 16.0;

/// What a gatherer of `kind` looks like.
pub fn look(kind: GathererKind) -> Look {
    match kind {
        GathererKind::Woodcutter => Look::Woodcutter,
        GathererKind::Sawmill => Look::Sawmill,
        GathererKind::Stonecutter => Look::Stonecutter,
        GathererKind::Quarry => Look::Quarry,
        GathererKind::IronMine => Look::IronMine,
        GathererKind::CopperMine => Look::CopperMine,
        GathererKind::SulphurPit => Look::SulphurPit,
        GathererKind::OilWell => Look::OilWell,
    }
}

/// How high the top of a gatherer of `kind` stands over the ground, in meters, for holding things over it: about as high as it
/// is close up.
pub fn height(kind: GathererKind) -> f32 {
    look(kind).height() * HOLD_RADIUS
}

fn token(world: &World, kind: GathererKind, x: f32, z: f32, eye: Vec3) -> Token {
    Token::at(world, look(kind), x, z, eye)
}

/// Draws the gatherers of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and
/// is at `eye`, at `time`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3, time: f32) {
    let models = day.gatherers.iter().filter(|g| pieces::within_view(eye, g.x, g.z)).map(|gatherer| {
        let country = day.country(gatherer.country).map_or(NO_COUNTRY, |c| c.color.color());
        token(world, gatherer.kind, gatherer.x, gatherer.z, eye).model(Mat3::IDENTITY, country, Moment { time, busy: gatherer.operating })
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// Draws `gatherer` over again in `color`, with a frame round it, to mark it out, as seen by the camera that is set, at `eye`, at
/// `time`.
pub fn outline(world: &World, gatherer: &Gatherer, eye: Vec3, time: f32, color: Color) {
    let token = token(world, gatherer.kind, gatherer.x, gatherer.z, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: gatherer.operating })]);
    token.draw_frame(color);
}

/// The gatherer the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it
/// goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, gatherers: &[Gatherer]) -> Option<GathererId> {
    gatherers
        .iter()
        .filter_map(|gatherer| Some((token(world, gatherer.kind, gatherer.x, gatherer.z, ray.0).hit(ray)?, gatherer.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a gatherer of `kind` would stand if it were built at `at`, so that it can be seen against the region: the gatherer
/// in `color`, and a square on the ground round it. As seen by the camera that is set, at `eye`, at `time`.
pub fn preview(world: &World, kind: GathererKind, at: Vec2, eye: Vec3, time: f32, color: Color) {
    let token = token(world, kind, at.x, at.y, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: false })]);
    pieces::draw_square(world, at.x, at.y, look(kind).reach() * token.radius, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::CountryId;
    use crate::test_support;

    fn world() -> World {
        test_support::small_continent()
    }

    fn of_kind(world: &World, kind: GathererKind) -> Gatherer {
        let at = test_support::inland_spot(world, 40.0);
        Gatherer { id: GathererId::new(), kind, country: CountryId::new(), x: at.x, z: at.y, region: 1, operating: false }
    }

    fn eye_over(gatherer: &Gatherer, world: &World, distance: f32) -> Vec3 {
        vec3(gatherer.x, world.surface_height(gatherer.x, gatherer.z) + distance, gatherer.z + 1.0)
    }

    #[test]
    fn every_kind_has_a_look_of_its_own() {
        let looks: Vec<Look> = GathererKind::ALL.iter().map(|k| look(*k)).collect();
        for (n, a) in looks.iter().enumerate() {
            assert!(!looks[n + 1..].contains(a), "two kinds look the same: {a:?}");
        }
    }

    #[test]
    fn the_greater_ones_have_the_bigger_plot() {
        for (lesser, greater) in [(GathererKind::Woodcutter, GathererKind::Sawmill), (GathererKind::Stonecutter, GathererKind::Quarry)] {
            assert!(look(greater).reach() > look(lesser).reach(), "{greater:?} over {lesser:?}");
        }
        assert_eq!(look(GathererKind::IronMine).reach(), look(GathererKind::Sawmill).reach(), "the mines are greater ones too");
        assert!(height(GathererKind::Quarry) > height(GathererKind::Woodcutter));
    }

    #[test]
    fn a_token_is_big_enough_to_see_from_far_away_and_grows_with_the_distance() {
        let world = world();
        let gatherer = of_kind(&world, GathererKind::Woodcutter);
        let across = |distance: f32| token(&world, gatherer.kind, gatherer.x, gatherer.z, eye_over(&gatherer, &world, distance)).bounds().1.x;
        assert!(across(50.0) >= 16.0, "close up it is still a token: {}", across(50.0));
        assert!(across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
        assert!(across(3_000.0) > 60.0, "from 3 km it is over 60 m across: {}", across(3_000.0));
        assert!(across(1_000_000.0) <= 120.0, "but not so big that it covers a region: {}", across(1_000_000.0));
    }

    #[test]
    fn it_stands_on_the_highest_ground_under_it_so_none_of_it_is_buried() {
        let world = world();
        let gatherer = of_kind(&world, GathererKind::Quarry);
        let token = token(&world, gatherer.kind, gatherer.x, gatherer.z, eye_over(&gatherer, &world, 400.0));
        assert!(token.foot.y >= world.surface_height(gatherer.x, gatherer.z) - 1e-3);
    }

    #[test]
    fn a_ray_at_the_token_hits_it_and_one_that_misses_does_not_and_the_nearer_is_the_one_pointed_at() {
        let world = world();
        let near = of_kind(&world, GathererKind::Woodcutter);
        let mut far = near.clone();
        far.id = GathererId::new();
        far.x += 400.0;
        let over = |g: &Gatherer| (vec3(g.x, world.surface_height(g.x, g.z) + 200.0, g.z), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(over(&near), &world, &[near.clone(), far.clone()]), Some(near.id));
        assert_eq!(under_ray(over(&far), &world, &[near.clone(), far.clone()]), Some(far.id));
        let aside = (vec3(near.x + 200.0, 300.0, near.z), vec3(0.0, -1.0, 0.0));
        assert_eq!(under_ray(aside, &world, &[near]), None);
    }

    #[test]
    fn only_the_gatherers_of_the_day_are_drawn_from_its_countries() {
        // (Drawing needs the window, so this is only that the day and the pieces fit together.)
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        assert!(day.gatherers.is_empty());
    }
}
