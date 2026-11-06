//! Drawing the shipyards, as buildings on square plots like the gatherers' (see [`crate::gatherer_view`] and
//! [`crate::miniatures`]): a dark slate plot with a band of the color of the country it belongs to, a slip of blue water along
//! one side, and a ship on the stocks, ribbed from end to end and planked from the stern, with scaffolding along it and a crane
//! over it. A large shipyard has the bigger plot, a longer ship further on, a second crane and a storehouse. The pieces are sized
//! by how far the camera is from them (see [`crate::pieces`]), so that they can be seen from far off. This is the game's own
//! drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::ids::ShipyardId;
use crate::miniatures::{Look, Moment};
use crate::pieces::{self, Token};
use crate::shipyard::{Shipyard, ShipyardKind};

/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// What a shipyard of `kind` looks like.
pub fn look(kind: ShipyardKind) -> Look {
    Look::Shipyard { large: kind == ShipyardKind::Large }
}

fn token(world: &World, kind: ShipyardKind, x: f32, z: f32, eye: Vec3) -> Token {
    Token::at(world, look(kind), x, z, eye)
}

/// Draws the shipyards of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at
/// `eye`, at `time`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3, time: f32) {
    let models = day.shipyards.iter().filter(|s| pieces::within_view(eye, s.x, s.z)).map(|shipyard| {
        let country = day.country(shipyard.country).map_or(NO_COUNTRY, |c| c.color.color());
        token(world, shipyard.kind, shipyard.x, shipyard.z, eye).model(Mat3::IDENTITY, country, Moment { time, busy: true })
    });
    pieces::draw_models(eye, Vec4::ZERO, models);
}

/// Draws `shipyard` over again in `color`, with a frame round it, to mark it out, as seen by the camera that is set, at `eye`, at
/// `time`.
pub fn outline(world: &World, shipyard: &Shipyard, eye: Vec3, time: f32, color: Color) {
    let token = token(world, shipyard.kind, shipyard.x, shipyard.z, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    token.draw_frame(color);
}

/// The shipyard the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through
/// any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, shipyards: &[Shipyard]) -> Option<ShipyardId> {
    shipyards
        .iter()
        .filter_map(|shipyard| Some((token(world, shipyard.kind, shipyard.x, shipyard.z, ray.0).hit(ray)?, shipyard.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a shipyard of `kind` would stand if it were built at `at`, so that it can be seen against the coast: the shipyard
/// in `color`, and a square on the ground round it. As seen by the camera that is set, at `eye`, at `time`.
pub fn preview(world: &World, kind: ShipyardKind, at: Vec2, eye: Vec3, time: f32, color: Color) {
    let token = token(world, kind, at.x, at.y, eye);
    pieces::draw_models(eye, pieces::tint_of(color), [token.model(Mat3::IDENTITY, color, Moment { time, busy: true })]);
    pieces::draw_square(world, at.x, at.y, look(kind).reach() * token.radius, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use crate::test_support;

    fn of_kind(world: &World, kind: ShipyardKind) -> Shipyard {
        let at = test_support::inland_spot(world, 40.0);
        Shipyard { id: ShipyardId::new(), kind, country: CountryId::new(), x: at.x, z: at.y, region: Some(1), ready_on: None }
    }

    #[test]
    fn a_large_shipyard_has_the_bigger_plot() {
        assert!(look(ShipyardKind::Large).reach() > look(ShipyardKind::Small).reach());
        assert_ne!(look(ShipyardKind::Large), look(ShipyardKind::Small));
    }

    #[test]
    fn a_ray_straight_down_through_a_shipyard_hits_it_and_one_beside_it_does_not() {
        let world = test_support::small_continent();
        let yard = of_kind(&world, ShipyardKind::Small);
        let top = vec3(yard.x, world.surface_height(yard.x, yard.z) + 300.0, yard.z + 1.0);
        let yards = [yard];
        assert_eq!(under_ray((vec3(yard.x, top.y, yard.z), vec3(0.0, -1.0, 0.0)), &world, &yards), Some(yard.id));
        assert_eq!(under_ray((vec3(yard.x + 500.0, top.y, yard.z), vec3(0.0, -1.0, 0.0)), &world, &yards), None);
    }
}
