//! Drawing the shipyards, as tiles like the gatherers' (see [`crate::gatherer_view`]): a square tile in the color of the country
//! it belongs to, with a ship on slipways standing on it, and a crane beside it. A large shipyard has the wider tile, a longer
//! hull and a second crane. The pieces are sized by how far the camera is from them (see [`crate::pieces`]), so that they can be
//! seen from far off, and are low, like the pieces of a flat game. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::ids::ShipyardId;
use crate::pieces::{self, Piece};
use crate::shipyard::{Shipyard, ShipyardKind};

/// The color of the mark that stands for a shipyard when it is seen from far off: the blue of water.
const MARK: Color = Color::new(0.24, 0.52, 0.80, 1.0);
/// How big the figure on a tile is, as a share of the tile, so that it can be made out from far off.
const FIGURE_SCALE: f32 = 1.15;
/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

const HULL: Color = Color::new(0.45, 0.30, 0.18, 1.0);
const DECK: Color = Color::new(0.80, 0.64, 0.40, 1.0);
const SLIPWAY: Color = Color::new(0.64, 0.64, 0.68, 1.0);
const CRANE: Color = Color::new(0.52, 0.55, 0.62, 1.0);

/// The tile a kind of shipyard stands on: how wide a half of it is, as a share of the token's radius, and how thick.
fn tile_size(kind: ShipyardKind) -> (f32, f32) {
    match kind {
        ShipyardKind::Small => (1.2, 0.16),
        ShipyardKind::Large => (1.7, 0.22),
    }
}

/// How high the top of a shipyard of `kind` stands over the ground, in meters, for holding things over it.
#[allow(dead_code)]
pub fn height(kind: ShipyardKind) -> f32 {
    match kind {
        ShipyardKind::Small => 16.0,
        ShipyardKind::Large => 30.0,
    }
}

/// A crane with its foot at `foot`, `r` being the radius of the token and `scale` its size: a column and an arm over the hull.
fn crane(pieces: &mut Vec<Piece>, foot: Vec3, r: f32, scale: f32) {
    let s = r * scale;
    pieces.push(Piece::block_on(foot, vec3(0.1, 0.9, 0.1) * s, CRANE));
    pieces.push(Piece::block_on(foot + vec3(0.0, 0.9 * s, 0.0), vec3(0.1, 0.08, 0.7) * s, CRANE));
}

/// The ship and its cranes that stand on the tile of a shipyard of `kind`, with the middle of their foot at `top`, `r` being the
/// radius of the token.
fn figure(kind: ShipyardKind, top: Vec3, r: f32) -> Vec<Piece> {
    let at = |dx: f32, dz: f32, up: f32| top + vec3(dx * r, up * r, dz * r);
    let mut pieces = Vec::new();
    match kind {
        ShipyardKind::Small => {
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.0), vec3(1.5, 0.08, 0.8) * r, SLIPWAY));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.08), vec3(1.3, 0.24, 0.42) * r, HULL));
            pieces.push(Piece::block_on(at(0.1, 0.0, 0.32), vec3(0.5, 0.18, 0.3) * r, DECK));
            crane(&mut pieces, at(-0.62, 0.34, 0.0), r, 0.9);
        }
        ShipyardKind::Large => {
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.0), vec3(2.2, 0.1, 1.2) * r, SLIPWAY));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.1), vec3(2.0, 0.3, 0.56) * r, HULL));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.4), vec3(1.5, 0.2, 0.42) * r, DECK));
            pieces.push(Piece::block_on(at(-0.2, 0.0, 0.6), vec3(0.6, 0.2, 0.3) * r, HULL));
            crane(&mut pieces, at(-0.95, 0.5, 0.0), r, 1.2);
            crane(&mut pieces, at(0.95, -0.5, 0.0), r, 1.2);
        }
    }
    pieces
}

/// The pieces of a shipyard of `kind` standing at (`x`, `z`) in the color `country`, for a camera at `eye`: the tile, and the
/// ship on it.
fn parts_at(world: &World, kind: ShipyardKind, x: f32, z: f32, eye: Vec3, country: Color) -> Vec<Piece> {
    let ground = world.surface_height(x, z);
    let r = pieces::radius_at(eye, vec3(x, ground, z));
    let foot = vec3(x, pieces::ground_under(world, x, z, r), z);
    let (half, thickness) = tile_size(kind);
    let mut parts: Vec<Piece> = pieces::tile(foot, half * r, thickness * r, country).into();
    let top = pieces::tile_top(foot, half * r, thickness * r);
    if pieces::is_far(eye, vec3(x, ground, z)) {
        // From far off, only a mark of what it is: a small square of the color of water on the tile.
        parts.push(Piece::block_on(top, vec3(0.6 * r, 0.05 * r, 0.6 * r), MARK));
    } else {
        parts.extend(pieces::flatten(figure(kind, top, r * FIGURE_SCALE), top.y, pieces::FLAT));
    }
    parts
}

fn parts(world: &World, shipyard: &Shipyard, eye: Vec3, country: Color) -> Vec<Piece> {
    parts_at(world, shipyard.kind, shipyard.x, shipyard.z, eye, country)
}

/// Draws the shipyards of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at
/// `eye`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3) {
    for shipyard in day.shipyards.iter().filter(|s| pieces::within_view(eye, s.x, s.z)) {
        let country = day.country(shipyard.country).map_or(NO_COUNTRY, |c| c.color.color());
        for piece in parts(world, shipyard, eye, country) {
            piece.draw();
        }
    }
}

/// Draws a frame of `color` round each piece of `shipyard`, to mark it out, as seen by the camera that is set, at `eye`.
pub fn outline(world: &World, shipyard: &Shipyard, eye: Vec3, color: Color) {
    for piece in parts(world, shipyard, eye, WHITE) {
        piece.draw_frame(color);
    }
}

/// The shipyard the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through
/// any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, shipyards: &[Shipyard]) -> Option<ShipyardId> {
    shipyards
        .iter()
        .filter_map(|shipyard| Some((pieces::first_hit(ray, &parts(world, shipyard, ray.0, WHITE))?, shipyard.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a shipyard of `kind` would stand if it were built at `at`, so that it can be seen against the coast: a frame round
/// each piece of it, and a ring on the ground round it, in `color`. As seen by the camera that is set, at `eye`.
pub fn preview(world: &World, kind: ShipyardKind, at: Vec2, eye: Vec3, color: Color) {
    for piece in parts_at(world, kind, at.x, at.y, eye, WHITE) {
        piece.draw_frame(color);
    }
    let r = pieces::radius_at(eye, vec3(at.x, world.surface_height(at.x, at.y), at.y));
    pieces::draw_square(world, at.x, at.y, tile_size(kind).0 * r, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use crate::test_support;

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);

    fn of_kind(world: &World, kind: ShipyardKind) -> Shipyard {
        let at = test_support::inland_spot(world, 40.0);
        Shipyard { id: ShipyardId::new(), kind, country: CountryId::new(), x: at.x, z: at.y, region: Some(1) }
    }

    fn eye_over(yard: &Shipyard, world: &World, distance: f32) -> Vec3 {
        vec3(yard.x, world.surface_height(yard.x, yard.z) + distance, yard.z + 1.0)
    }

    #[test]
    fn a_shipyard_is_a_square_tile_in_its_countrys_color_with_a_ship_on_it() {
        let world = test_support::small_continent();
        for kind in ShipyardKind::ALL {
            let yard = of_kind(&world, kind);
            let parts = parts(&world, &yard, eye_over(&yard, &world, 300.0), COUNTRY);
            assert!(parts.len() >= 6, "{kind:?}: a tile of three squares and a ship with a crane");
            assert!(parts[..3].iter().all(|p| p.shape == pieces::Shape::Block));
            assert_eq!(parts[1].color, COUNTRY, "the frame is the country's color");
            assert_eq!(parts[0].color, pieces::TILE_EDGE);
            assert!(parts[3..].iter().all(|p| p.middle.y >= parts[2].top() - 0.5), "the ship stands on the tile");
        }
    }

    #[test]
    fn a_large_shipyard_has_the_wider_tile_and_more_on_it() {
        let world = test_support::small_continent();
        let (small, large) = (of_kind(&world, ShipyardKind::Small), of_kind(&world, ShipyardKind::Large));
        let eye = eye_over(&small, &world, 300.0);
        let (a, b) = (parts(&world, &small, eye, COUNTRY), parts(&world, &Shipyard { x: small.x, z: small.z, ..large }, eye, COUNTRY));
        assert!(b[0].size.x > a[0].size.x, "a wider tile");
        assert!(b.len() > a.len(), "a longer ship and a second crane");
    }

    #[test]
    fn from_far_off_it_is_only_a_tile_with_a_mark() {
        let world = test_support::small_continent();
        let yard = of_kind(&world, ShipyardKind::Large);
        let parts = parts(&world, &yard, eye_over(&yard, &world, pieces::FAR + 500.0), COUNTRY);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3].color, MARK);
    }

    #[test]
    fn a_ray_straight_down_through_a_shipyard_hits_it_and_one_beside_it_does_not() {
        let world = test_support::small_continent();
        let yard = of_kind(&world, ShipyardKind::Small);
        let top = eye_over(&yard, &world, 300.0);
        let yards = [yard];
        assert_eq!(under_ray((vec3(yard.x, top.y, yard.z), vec3(0.0, -1.0, 0.0)), &world, &yards), Some(yard.id));
        assert_eq!(under_ray((vec3(yard.x + 500.0, top.y, yard.z), vec3(0.0, -1.0, 0.0)), &world, &yards), None);
    }
}
