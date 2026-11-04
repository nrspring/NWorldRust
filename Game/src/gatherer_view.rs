//! Drawing the gatherers, as tiles: a square tile with a dark edge, a frame in the color of the country it belongs to and a pale
//! square in the middle with a little figure of what it gathers standing on it, in that resource's own color: a pine for a woodcutter, a pair of pines and a plank for a
//! sawmill, a boulder for a stonecutter, steps of cut stone for a quarry, an ingot for an iron mine, a stack of coins for a
//! copper mine, a yellow heap for a sulphur pit and a black drop for an oil well. The lesser gatherers, the woodcutter and
//! the stonecutter, have the smaller, thinner tile. A gatherer is fixed, and so stands on a square, and a unit that can move
//! stands on a round gold base (see [`crate::militia_view`]), so that they are told apart at a glance. The tokens are sized by how far the camera is from them (see
//! [`crate::pieces`]), so that they can be seen on the map from far off. They are low, like the pieces of a flat game: from above
//! they are what they are, and only from a low angle are they seen to have a body. This is the game's own drawing, and not part of
//! drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::gatherer::{Gatherer, GathererKind};
use crate::ids::GathererId;
use crate::pieces::{self, Piece};

/// How big the figure on a coin is, as a share of the coin, so that it can be made out from far off.
const FIGURE_SCALE: f32 = 1.15;
/// A country that can't be found (which can't happen while it owns one) is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// How high the top of a gatherer of `kind` stands over the ground, in meters, for holding things over it: about as high as a
/// token is close up.
pub fn height(kind: GathererKind) -> f32 {
    if kind.greater() { 27.0 } else { 14.0 }
}

const fn c(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

const PINE: Color = c(0.16, 0.48, 0.22);
const TRUNK: Color = c(0.45, 0.30, 0.18);
const PLANK: Color = c(0.80, 0.64, 0.40);
const STONE: Color = c(0.64, 0.64, 0.68);
const STONE_LIGHT: Color = c(0.82, 0.82, 0.85);
const IRON: Color = c(0.52, 0.55, 0.62);
const RUST: Color = c(0.66, 0.30, 0.20);
const COPPER: Color = c(0.84, 0.48, 0.26);
const SULPHUR: Color = c(0.95, 0.86, 0.28);
const OIL: Color = c(0.10, 0.10, 0.13);
const OIL_PAD: Color = c(0.34, 0.36, 0.42);
const OIL_SHINE: Color = c(0.55, 0.60, 0.70);

/// The tile a kind of gatherer stands on: how wide a half of it is, as a share of the token's radius, and how thick.
fn tile_size(kind: GathererKind) -> (f32, f32) {
    if kind.greater() { (1.2, 0.2) } else { (1.0, 0.13) }
}

/// A pine of `scale` (1 is a tall one) with its foot at `foot`, `r` being the radius of the token: a trunk and three tiers of
/// branches, each a little lighter or darker than the next, so that from above it is rings.
fn pine(pieces: &mut Vec<Piece>, foot: Vec3, r: f32, scale: f32) {
    let s = r * scale;
    pieces.push(Piece::disc(foot, 0.11 * s, 0.24 * s, TRUNK));
    for (k, (radius, shade)) in [(0.46, 0.8), (0.34, 1.0), (0.22, 1.25)].into_iter().enumerate() {
        pieces.push(Piece::cone(foot + vec3(0.0, (0.18 + 0.36 * k as f32) * s, 0.0), radius * s, 0.62 * s, pieces::shade(PINE, shade)));
    }
}

/// The figure that stands on the coin of a gatherer of `kind`, with the middle of its foot at `top`, `r` being the radius of
/// the token.
fn figure(kind: GathererKind, top: Vec3, r: f32) -> Vec<Piece> {
    let at = |dx: f32, dz: f32, up: f32| top + vec3(dx * r, up * r, dz * r);
    let mut pieces = Vec::new();
    match kind {
        GathererKind::Woodcutter => pine(&mut pieces, at(0.0, 0.0, 0.0), r, 1.0),
        GathererKind::Sawmill => {
            pine(&mut pieces, at(-0.42, 0.1, 0.0), r, 0.85);
            pine(&mut pieces, at(0.42, -0.1, 0.0), r, 0.85);
            pieces.push(Piece::block_on(at(0.0, 0.5, 0.0), vec3(1.0, 0.14, 0.34) * r, PLANK));
            pieces.push(Piece::block_on(at(0.0, 0.5, 0.14), vec3(0.9, 0.14, 0.34) * r, PLANK));
        }
        GathererKind::Stonecutter => {
            pieces.push(Piece::block_on(at(-0.12, 0.0, 0.0), vec3(0.7, 0.5, 0.58) * r, STONE));
            pieces.push(Piece::block_on(at(0.4, 0.2, 0.0), vec3(0.36, 0.3, 0.34) * r, STONE_LIGHT));
        }
        GathererKind::Quarry => {
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.0), vec3(1.3, 0.3, 0.9) * r, STONE));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.3), vec3(0.95, 0.3, 0.66) * r, STONE_LIGHT));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.6), vec3(0.6, 0.3, 0.42) * r, STONE));
        }
        GathererKind::IronMine => {
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.0), vec3(1.0, 0.28, 0.56) * r, IRON));
            pieces.push(Piece::block_on(at(0.0, 0.0, 0.28), vec3(0.76, 0.26, 0.42) * r, RUST));
        }
        GathererKind::CopperMine => {
            // A stack of coins that narrows, so that from above it is rings.
            for k in 0..4 {
                let rim = if k % 2 == 0 { COPPER } else { pieces::shade(COPPER, 0.78) };
                pieces.push(Piece::disc(at(0.0, 0.0, 0.14 * k as f32), (0.56 - 0.07 * k as f32) * r, 0.14 * r, rim));
            }
        }
        GathererKind::SulphurPit => {
            pieces.push(Piece::cone(at(-0.12, 0.0, 0.0), 0.58 * r, 0.5 * r, pieces::shade(SULPHUR, 0.85)));
            pieces.push(Piece::cone(at(-0.12, 0.0, 0.3), 0.36 * r, 0.5 * r, SULPHUR));
            pieces.push(Piece::cone(at(-0.12, 0.0, 0.6), 0.18 * r, 0.3 * r, pieces::shade(SULPHUR, 1.12)));
            pieces.push(Piece::cone(at(0.46, 0.22, 0.0), 0.28 * r, 0.4 * r, pieces::shade(SULPHUR, 0.7)));
        }
        GathererKind::OilWell => {
            pieces.push(Piece::disc(at(0.0, 0.0, 0.0), 0.62 * r, 0.12 * r, OIL_PAD));
            pieces.push(Piece::ball(at(0.0, 0.0, 0.56), 0.46 * r, OIL));
            pieces.push(Piece::ball(at(-0.16, -0.16, 0.8), 0.13 * r, OIL_SHINE));
        }
    }
    pieces
}

/// The pieces of a gatherer of `kind` standing at (`x`, `z`) in the color `country`, for a camera at `eye`: the coin, and the
/// figure on it.
fn parts_at(world: &World, kind: GathererKind, x: f32, z: f32, eye: Vec3, country: Color) -> Vec<Piece> {
    let ground = world.surface_height(x, z);
    let r = pieces::radius_at(eye, vec3(x, ground, z));
    let foot = vec3(x, pieces::ground_under(world, x, z, r), z);
    let (half, thickness) = tile_size(kind);
    let mut parts: Vec<Piece> = pieces::tile(foot, half * r, thickness * r, country).into();
    let top = pieces::tile_top(foot, half * r, thickness * r);
    parts.extend(pieces::flatten(figure(kind, top, r * FIGURE_SCALE), top.y, pieces::FLAT));
    parts
}

fn parts(world: &World, gatherer: &Gatherer, eye: Vec3, country: Color) -> Vec<Piece> {
    parts_at(world, gatherer.kind, gatherer.x, gatherer.z, eye, country)
}

/// Draws the gatherers of `day`, as seen by the camera that is set, which must still be the one the map was drawn with, and
/// is at `eye`.
pub fn draw(world: &World, day: &DaySave, eye: Vec3) {
    for gatherer in &day.gatherers {
        let country = day.country(gatherer.country).map_or(NO_COUNTRY, |c| c.color.color());
        for piece in parts(world, gatherer, eye, country) {
            piece.draw();
        }
    }
}

/// Draws a frame of `color` round each piece of `gatherer`, to mark it out, as seen by the camera that is set, at `eye`.
pub fn outline(world: &World, gatherer: &Gatherer, eye: Vec3, color: Color) {
    for piece in parts(world, gatherer, eye, WHITE) {
        piece.draw_frame(color);
    }
}

/// The gatherer the ray (where it starts and which way it points, as `mouse_ray` gives it) goes through first, if it
/// goes through any.
pub fn under_ray(ray: (Vec3, Vec3), world: &World, gatherers: &[Gatherer]) -> Option<GathererId> {
    gatherers
        .iter()
        .filter_map(|gatherer| Some((pieces::first_hit(ray, &parts(world, gatherer, ray.0, WHITE))?, gatherer.id)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Draws where a gatherer of `kind` would stand if it were built at `at`, so that it can be seen against the region: a frame
/// round each piece of it, and a ring on the ground round it, in `color`. As seen by the camera that is set, at `eye`.
pub fn preview(world: &World, kind: GathererKind, at: Vec2, eye: Vec3, color: Color) {
    for piece in parts_at(world, kind, at.x, at.y, eye, WHITE) {
        piece.draw_frame(color);
    }
    let r = pieces::radius_at(eye, vec3(at.x, world.surface_height(at.x, at.y), at.y));
    pieces::draw_square(world, at.x, at.y, tile_size(kind).0 * r, color);
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

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);

    #[test]
    fn every_gatherer_is_a_square_tile_in_its_countrys_color_with_a_figure_on_it() {
        let world = world();
        for kind in GathererKind::ALL {
            let gatherer = of_kind(&world, kind);
            let parts = parts(&world, &gatherer, eye_over(&gatherer, &world, 300.0), COUNTRY);
            assert!(parts.len() >= 5, "{kind:?}: a tile of three squares and a figure");
            assert!(parts[..3].iter().all(|p| p.shape == pieces::Shape::Block), "{kind:?}: a square, not a round");
            assert_eq!(parts[1].color, COUNTRY, "the frame is the country's color");
            assert_eq!(parts[0].color, pieces::TILE_EDGE, "with a dark edge");
            assert_eq!(parts[2].color, pieces::MEDALLION, "and a pale square for the figure");
            assert!(parts[3..].iter().all(|p| p.middle.y >= parts[2].top() - 0.5), "the figure stands on it");
        }
    }

    #[test]
    fn each_kind_has_a_figure_of_its_own() {
        let world = world();
        let signature = |kind: GathererKind| {
            let gatherer = of_kind(&world, kind);
            let parts = parts(&world, &gatherer, eye_over(&gatherer, &world, 300.0), COUNTRY);
            let mut colors: Vec<[u8; 3]> = parts[3..].iter().map(|p| [(p.color.r * 255.0) as u8, (p.color.g * 255.0) as u8, (p.color.b * 255.0) as u8]).collect();
            colors.sort_unstable();
            colors.dedup();
            (parts[3..].iter().map(|p| p.shape as u8).collect::<Vec<_>>(), colors)
        };
        let all: Vec<_> = GathererKind::ALL.iter().map(|k| signature(*k)).collect();
        for (n, a) in all.iter().enumerate() {
            for b in &all[n + 1..] {
                assert_ne!(a, b, "two kinds look the same");
            }
        }
    }

    #[test]
    fn the_greater_ones_have_the_bigger_and_thicker_tile() {
        let world = world();
        let width = |kind| {
            let gatherer = of_kind(&world, kind);
            let parts = parts(&world, &gatherer, eye_over(&gatherer, &world, 300.0), COUNTRY);
            (parts[0].size.x, parts[0].size.y)
        };
        for (lesser, greater) in [(GathererKind::Woodcutter, GathererKind::Sawmill), (GathererKind::Stonecutter, GathererKind::Quarry)] {
            let (a, b) = (width(lesser), width(greater));
            assert!(b.0 > a.0 && b.1 > a.1, "{greater:?} {b:?} over {lesser:?} {a:?}");
        }
        assert_eq!(width(GathererKind::IronMine), width(GathererKind::Sawmill), "the mines are greater ones too");
    }

    #[test]
    fn a_token_is_big_enough_to_see_from_far_away_and_grows_with_the_distance() {
        let world = world();
        let gatherer = of_kind(&world, GathererKind::Woodcutter);
        let across = |distance: f32| parts(&world, &gatherer, eye_over(&gatherer, &world, distance), COUNTRY)[0].size.x;
        assert!(across(50.0) >= 16.0, "close up it is still a token: {}", across(50.0));
        assert!(across(3_000.0) > across(1_000.0) && across(1_000.0) > across(300.0));
        assert!(across(3_000.0) > 100.0, "from 3 km it is over 100 m across: {}", across(3_000.0));
        assert!(across(1_000_000.0) <= 290.0, "but not so big that it covers a region");
    }

    #[test]
    fn it_stands_on_the_highest_ground_under_it_so_none_of_it_is_buried() {
        let world = world();
        let gatherer = of_kind(&world, GathererKind::Quarry);
        let parts = parts(&world, &gatherer, eye_over(&gatherer, &world, 400.0), COUNTRY);
        let ground = world.surface_height(gatherer.x, gatherer.z);
        assert!(parts[0].middle.y - parts[0].size.y * 0.5 >= ground - 1e-3);
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
        assert!(height(GathererKind::Quarry) > height(GathererKind::Woodcutter));
    }
}
