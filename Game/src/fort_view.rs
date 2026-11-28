//! Drawing the forts, as buildings on square plots like the barracks' (see [`crate::barracks_view`] and [`crate::miniatures`]): a
//! small one is a palisade of pointed stakes round a wooden watchtower, a medium one has stone walls, square towers and a keep, and
//! a large one has a ring of outer walls with bastions, an inner wall with round towers and a tall keep with turrets. This is the
//! game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::fort::{Fort, FortSize};
use crate::ids::FortId;
use crate::miniatures::{Look, Moment};
use crate::pieces::{self, Token};

/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// What a fort of `size` looks like.
pub fn look(size: FortSize) -> Look {
    Look::Fort { size }
}

pub fn token(world: &World, size: FortSize, x: f32, z: f32, eye: Vec3) -> Token {
    Token::at(world, look(size), x, z, eye)
}

/// Draws the forts of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at
/// `eye`, at `time`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3, time: f32) {
    let models = day.forts.iter().filter(|f| pieces::within_view(eye, f.x, f.z)).map(|fort| {
        let country = day.country(fort.country).map_or(NO_COUNTRY, |c| c.color.color());
        token(world, fort.size, fort.x, fort.z, eye).model(Mat3::IDENTITY, country, Moment { time, busy: true })
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// Draws `fort` over again in `color`, with a frame round it, to mark it out, as seen by the camera that is set, at `eye`, at
/// `time`.
pub fn outline(world: &World, fort: &Fort, eye: Vec3, time: f32, color: Color) {
    let token = token(world, fort.size, fort.x, fort.z, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    token.draw_frame(color);
}

/// The fort the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, forts: &[Fort]) -> Option<FortId> {
    forts
        .iter()
        .filter_map(|f| Some((token(world, f.size, f.x, f.z, ray.0).hit(ray)?, f.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a small fort would stand if it were built at `at`: the fort in `color`, and a square on the ground round it. As
/// seen by the camera that is set, at `eye`, at `time`.
pub fn preview(world: &World, at: Vec2, eye: Vec3, time: f32, color: Color) {
    let size = FortSize::Small;
    let token = token(world, size, at.x, at.y, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    pieces::draw_square(world, at.x, at.y, look(size).reach() * token.radius, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use crate::test_support;

    #[test]
    fn each_size_of_fort_has_a_bigger_plot_and_stands_higher() {
        for pair in FortSize::ALL.windows(2) {
            assert!(look(pair[1]).reach() > look(pair[0]).reach() && look(pair[1]).height() > look(pair[0]).height());
        }
    }

    #[test]
    fn a_ray_straight_down_through_a_fort_hits_it_and_one_beside_it_does_not() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 40.0);
        let fort = Fort { id: FortId::new(), name: String::new(), size: FortSize::Medium, country: CountryId::new(), x: at.x, z: at.y, region: Some(1), under_siege: false, supplies_used: 0 };
        let top = vec3(fort.x, world.surface_height(fort.x, fort.z) + 300.0, fort.z + 1.0);
        let id = fort.id;
        let list = [fort.clone()];
        assert_eq!(under_ray((vec3(fort.x, top.y, fort.z), vec3(0.0, -1.0, 0.0)), &world, &list), Some(id));
        assert_eq!(under_ray((vec3(fort.x + 500.0, top.y, fort.z), vec3(0.0, -1.0, 0.0)), &world, &list), None);
    }
}
