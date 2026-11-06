//! Drawing the barracks, as a building on a square plot like the gatherers' and the shipyards' (see [`crate::shipyard_view`] and
//! [`crate::miniatures`]): a dark slate plot with a band of the color of the country it belongs to, and on it a small fort, with
//! walls and battlements, a gatehouse, a round tower at each corner with a pointed roof of the country's color, a hall inside, straw
//! men to train on and a great banner. The pieces are sized by how far the camera is from them (see [`crate::pieces`]), so that
//! they can be seen from far off. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::barracks::Barracks;
use crate::day::DaySave;
use crate::ids::BarracksId;
use crate::miniatures::{Look, Moment};
use crate::pieces::{self, Token};

/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

fn token(world: &World, x: f32, z: f32, eye: Vec3) -> Token {
    Token::at(world, Look::Barracks, x, z, eye)
}

/// Draws the barracks of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at
/// `eye`, at `time`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3, time: f32) {
    let models = day.barracks.iter().filter(|b| pieces::within_view(eye, b.x, b.z)).map(|barracks| {
        let country = day.country(barracks.country).map_or(NO_COUNTRY, |c| c.color.color());
        token(world, barracks.x, barracks.z, eye).model(Mat3::IDENTITY, country, Moment { time, busy: true })
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// Draws `barracks` over again in `color`, with a frame round it, to mark it out, as seen by the camera that is set, at `eye`, at
/// `time`.
pub fn outline(world: &World, barracks: &Barracks, eye: Vec3, time: f32, color: Color) {
    let token = token(world, barracks.x, barracks.z, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    token.draw_frame(color);
}

/// The barracks the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, barracks: &[Barracks]) -> Option<BarracksId> {
    barracks
        .iter()
        .filter_map(|b| Some((token(world, b.x, b.z, ray.0).hit(ray)?, b.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a barracks would stand if it were built at `at`: the barracks in `color`, and a square on the ground round it. As
/// seen by the camera that is set, at `eye`, at `time`.
pub fn preview(world: &World, at: Vec2, eye: Vec3, time: f32, color: Color) {
    let token = token(world, at.x, at.y, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    pieces::draw_square(world, at.x, at.y, Look::Barracks.reach() * token.radius, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use crate::test_support;

    #[test]
    fn a_ray_straight_down_through_a_barracks_hits_it_and_one_beside_it_does_not() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 40.0);
        let b = Barracks { id: BarracksId::new(), country: CountryId::new(), x: at.x, z: at.y, region: Some(1), ready_on: None };
        let top = vec3(b.x, world.surface_height(b.x, b.z) + 300.0, b.z + 1.0);
        let list = [b];
        assert_eq!(under_ray((vec3(b.x, top.y, b.z), vec3(0.0, -1.0, 0.0)), &world, &list), Some(b.id));
        assert_eq!(under_ray((vec3(b.x + 500.0, top.y, b.z), vec3(0.0, -1.0, 0.0)), &world, &list), None);
    }
}
