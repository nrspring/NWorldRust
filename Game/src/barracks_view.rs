//! Drawing the barracks, as tiles like the gatherers' and the shipyards' (see [`crate::shipyard_view`]): a square tile in the color of
//! the country it belongs to, with a stone hall on it, a dark roof, a tower at each end and a banner on a pole. The pieces are sized by
//! how far the camera is from them (see [`crate::pieces`]), so that they can be seen from far off, and are low, like the pieces of a
//! flat game. This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::barracks::Barracks;
use crate::day::DaySave;
use crate::ids::BarracksId;
use crate::pieces::{self, Piece};

/// The color of the mark that stands for a barracks when it is seen from far off: a soldier's red.
const MARK: Color = Color::new(0.78, 0.25, 0.20, 1.0);
/// How big the figure on a tile is, as a share of the tile, so that it can be made out from far off.
const FIGURE_SCALE: f32 = 1.15;
/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

const WALL: Color = Color::new(0.66, 0.64, 0.60, 1.0);
const ROOF: Color = Color::new(0.34, 0.24, 0.20, 1.0);
const POLE: Color = Color::new(0.52, 0.55, 0.62, 1.0);

/// How wide a half of the tile is, as a share of the token's radius, and how thick it is.
const TILE: (f32, f32) = (1.3, 0.18);

/// The hall, the two towers and the banner that stand on the tile of a barracks, with the middle of their foot at `top`, `r` being the
/// radius of the token, in the color `country`.
fn figure(top: Vec3, r: f32, country: Color) -> Vec<Piece> {
    let at = |dx: f32, dz: f32, up: f32| top + vec3(dx * r, up * r, dz * r);
    vec![
        Piece::block_on(at(0.0, 0.0, 0.0), vec3(1.5, 0.5, 0.8) * r, WALL),
        Piece::block_on(at(0.0, 0.0, 0.5), vec3(1.7, 0.14, 0.95) * r, ROOF),
        Piece::block_on(at(-0.75, 0.0, 0.0), vec3(0.34, 0.9, 0.34) * r, WALL),
        Piece::block_on(at(0.75, 0.0, 0.0), vec3(0.34, 0.9, 0.34) * r, WALL),
        Piece::block_on(at(-0.75, 0.0, 0.9), vec3(0.44, 0.1, 0.44) * r, ROOF),
        Piece::block_on(at(0.75, 0.0, 0.9), vec3(0.44, 0.1, 0.44) * r, ROOF),
        Piece::block_on(at(0.0, 0.0, 0.64), vec3(0.05, 0.7, 0.05) * r, POLE),
        Piece::block_on(at(0.17, 0.0, 1.0), vec3(0.3, 0.2, 0.04) * r, country),
    ]
}

/// The pieces of a barracks standing at (`x`, `z`) in the color `country`, for a camera at `eye`: the tile, and the hall on it.
fn parts_at(world: &World, x: f32, z: f32, eye: Vec3, country: Color) -> Vec<Piece> {
    let ground = world.surface_height(x, z);
    let r = pieces::radius_at(eye, vec3(x, ground, z));
    let foot = vec3(x, pieces::ground_under(world, x, z, r), z);
    let (half, thickness) = TILE;
    let mut parts: Vec<Piece> = pieces::tile(foot, half * r, thickness * r, country).into();
    let top = pieces::tile_top(foot, half * r, thickness * r);
    if pieces::is_far(eye, vec3(x, ground, z)) {
        // From far off, only a mark of what it is: a small red square on the tile.
        parts.push(Piece::block_on(top, vec3(0.6 * r, 0.05 * r, 0.6 * r), MARK));
    } else {
        parts.extend(pieces::flatten(figure(top, r * FIGURE_SCALE, country), top.y, pieces::FLAT));
    }
    parts
}

fn parts(world: &World, barracks: &Barracks, eye: Vec3, country: Color) -> Vec<Piece> {
    parts_at(world, barracks.x, barracks.z, eye, country)
}

/// Draws the barracks of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and is at `eye`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3) {
    for barracks in day.barracks.iter().filter(|b| pieces::within_view(eye, b.x, b.z)) {
        let country = day.country(barracks.country).map_or(NO_COUNTRY, |c| c.color.color());
        for piece in parts(world, barracks, eye, country) {
            piece.draw();
        }
    }
}

/// Draws a frame of `color` round each piece of `barracks`, to mark it out, as seen by the camera that is set, at `eye`.
pub fn outline(world: &World, barracks: &Barracks, eye: Vec3, color: Color) {
    for piece in parts(world, barracks, eye, WHITE) {
        piece.draw_frame(color);
    }
}

/// The barracks the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, barracks: &[Barracks]) -> Option<BarracksId> {
    barracks
        .iter()
        .filter_map(|b| Some((pieces::first_hit(ray, &parts(world, b, ray.0, WHITE))?, b.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a barracks would stand if it were built at `at`: a frame round each piece of it, and a ring on the ground round it, in
/// `color`. As seen by the camera that is set, at `eye`.
pub fn preview(world: &World, at: Vec2, eye: Vec3, color: Color) {
    for piece in parts_at(world, at.x, at.y, eye, WHITE) {
        piece.draw_frame(color);
    }
    let r = pieces::radius_at(eye, vec3(at.x, world.surface_height(at.x, at.y), at.y));
    pieces::draw_square(world, at.x, at.y, TILE.0 * r, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::CountryId;
    use crate::test_support;

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);

    fn barracks(world: &World) -> Barracks {
        let at = test_support::inland_spot(world, 40.0);
        Barracks { id: BarracksId::new(), country: CountryId::new(), x: at.x, z: at.y, region: Some(1), ready_on: None }
    }

    fn eye_over(b: &Barracks, world: &World, distance: f32) -> Vec3 {
        vec3(b.x, world.surface_height(b.x, b.z) + distance, b.z + 1.0)
    }

    #[test]
    fn a_barracks_is_a_square_tile_in_its_countrys_color_with_a_hall_towers_and_a_banner_on_it() {
        let world = test_support::small_continent();
        let b = barracks(&world);
        let parts = parts(&world, &b, eye_over(&b, &world, 300.0), COUNTRY);
        assert_eq!(parts.len(), 3 + 8, "a tile of three squares, and the hall, its roof, two towers with roofs, a pole and a banner");
        assert!(parts[..3].iter().all(|p| p.shape == pieces::Shape::Block));
        assert_eq!(parts[1].color, COUNTRY, "the frame is the country's color");
        assert_eq!(parts[0].color, pieces::TILE_EDGE);
        assert!(parts.last().unwrap().color == COUNTRY, "the banner is too");
        assert!(parts[3..].iter().all(|p| p.middle.y >= parts[2].top() - 0.5), "the hall stands on the tile");
    }

    #[test]
    fn from_far_off_it_is_only_a_tile_with_a_red_mark() {
        let world = test_support::small_continent();
        let b = barracks(&world);
        let parts = parts(&world, &b, eye_over(&b, &world, pieces::FAR + 500.0), COUNTRY);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3].color, MARK);
    }

    #[test]
    fn a_ray_straight_down_through_a_barracks_hits_it_and_one_beside_it_does_not() {
        let world = test_support::small_continent();
        let b = barracks(&world);
        let top = eye_over(&b, &world, 300.0);
        let list = [b];
        assert_eq!(under_ray((vec3(b.x, top.y, b.z), vec3(0.0, -1.0, 0.0)), &world, &list), Some(b.id));
        assert_eq!(under_ray((vec3(b.x + 500.0, top.y, b.z), vec3(0.0, -1.0, 0.0)), &world, &list), None);
    }
}
