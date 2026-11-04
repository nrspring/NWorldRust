//! Board game pieces: the shapes that the gatherers and the militia are made of, and drawn as. A piece is a flat-colored
//! box, disc, cone or ball, and each is kept as the box that holds it, which is what the mouse points at. A set of them
//! makes a token, a coin or a pawn, that stands on the map in the color of its country.
//!
//! The tokens are not to the scale of the map. A token is sized by how far the camera is from it, so that it takes up
//! about the same room on the screen however far away it is, and never shrinks below a size that can be seen from close by.
//! This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

/// How much of the distance to the camera a token's radius is, so that a token is a few percent of the screen across, and
/// the least and the most it is, in meters: it is not so small close up that it can't be seen, nor so big far off that it
/// covers a whole region.
const SCREEN_SHARE: f32 = 0.012;
const MIN_RADIUS: f32 = 8.0;
const MAX_RADIUS: f32 = 48.0;
/// How far from the camera a token is drawn as a plain marker, with none of the detail of a figure or a stack: from this far,
/// the rings and the figures would only be clutter, and what is wanted is a mark of whose it is and what kind.
pub const FAR: f32 = 2_500.0;
/// How far from the mouse's ray, as a fraction of how far away it is, a piece still counts as pointed at, so that a thin one
/// can be hit from far away.
const HOVER_SLOP: f32 = 0.006;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A box.
    Block,
    /// A round slab or column, standing flat on its foot.
    Disc,
    /// A cone, standing on its base.
    Cone,
    /// A ball.
    Ball,
}

/// One piece of a token: its shape, the box that holds it (its middle and its size across, high and deep), and its color.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub middle: Vec3,
    pub size: Vec3,
    pub color: Color,
}

impl Piece {
    /// A box with its middle at `middle`.
    pub fn block(middle: Vec3, size: Vec3, color: Color) -> Self {
        Self { shape: Shape::Block, middle, size, color }
    }

    /// A box standing on the ground, with the middle of its foot at `foot`.
    pub fn block_on(foot: Vec3, size: Vec3, color: Color) -> Self {
        Self::block(foot + vec3(0.0, size.y * 0.5, 0.0), size, color)
    }

    /// A disc of `radius` and `height`, with the middle of its foot at `foot`.
    pub fn disc(foot: Vec3, radius: f32, height: f32, color: Color) -> Self {
        Self { shape: Shape::Disc, middle: foot + vec3(0.0, height * 0.5, 0.0), size: vec3(radius * 2.0, height, radius * 2.0), color }
    }

    /// A cone of `radius` at its foot and `height`, with the middle of its foot at `foot`.
    pub fn cone(foot: Vec3, radius: f32, height: f32, color: Color) -> Self {
        Self { shape: Shape::Cone, middle: foot + vec3(0.0, height * 0.5, 0.0), size: vec3(radius * 2.0, height, radius * 2.0), color }
    }

    /// A ball of `radius` with its middle at `middle`.
    pub fn ball(middle: Vec3, radius: f32, color: Color) -> Self {
        Self { shape: Shape::Ball, middle, size: Vec3::splat(radius * 2.0), color }
    }

    /// The top of it: the height of the highest point.
    #[cfg(test)]
    pub fn top(&self) -> f32 {
        self.middle.y + self.size.y * 0.5
    }

    /// The same piece lifted `by` meters.
    #[cfg(test)]
    pub fn lifted(mut self, by: f32) -> Self {
        self.middle.y += by;
        self
    }

    /// Draws it, as seen by the camera that is set.
    pub fn draw(&self) {
        let radius = self.size.x * 0.5;
        let foot = vec3(self.middle.x, self.middle.y - self.size.y * 0.5, self.middle.z);
        match self.shape {
            Shape::Block => draw_cube(self.middle, self.size, None, self.color),
            Shape::Disc => draw_cylinder(foot, radius, radius, self.size.y, None, self.color),
            Shape::Cone => draw_cylinder_ex(foot, 0.0, radius, self.size.y, None, self.color, DrawCylinderParams { sides: 12, ..Default::default() }),
            Shape::Ball => draw_sphere_ex(self.middle, radius, None, self.color, DrawSphereParams { rings: 8, slices: 12, ..Default::default() }),
        }
    }

    /// Draws a frame of `color` round the box that holds it.
    pub fn draw_frame(&self, color: Color) {
        draw_cube_wires(self.middle, self.size * 1.04, color);
    }

    /// How far along the ray the box that holds it is first hit, if it is, with a little give for the distance.
    pub fn hit(&self, ray: (Vec3, Vec3)) -> Option<f32> {
        hit(ray, self.middle, self.size, ray.0.distance(self.middle) * HOVER_SLOP)
    }
}

/// How far along the ray (where it starts and which way it points) a box is first hit, if it is, with the box grown by `slop`
/// all round.
pub fn hit((origin, direction): (Vec3, Vec3), middle: Vec3, size: Vec3, slop: f32) -> Option<f32> {
    let half = size * 0.5 + Vec3::splat(slop);
    let (mut near, mut far) = (0.0f32, f32::MAX);
    for axis in 0..3 {
        let (low, high) = (middle[axis] - half[axis], middle[axis] + half[axis]);
        if direction[axis].abs() < 1e-8 {
            if origin[axis] < low || origin[axis] > high {
                return None;
            }
        } else {
            let (a, b) = ((low - origin[axis]) / direction[axis], (high - origin[axis]) / direction[axis]);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}

/// How far along the ray the first of `pieces` is hit, if any is.
pub fn first_hit(ray: (Vec3, Vec3), pieces: &[Piece]) -> Option<f32> {
    pieces.iter().filter_map(|piece| piece.hit(ray)).min_by(f32::total_cmp)
}

/// The radius a token at `at` is made with, for a camera at `eye`, in meters.
pub fn radius_at(eye: Vec3, at: Vec3) -> f32 {
    (eye.distance(at) * SCREEN_SHARE).clamp(MIN_RADIUS, MAX_RADIUS)
}

/// How far past the distance that can be seen (beyond which everything is lost in the haze) a thing may stand and still be
/// drawn, for the part of it that reaches back towards the camera, in meters.
const VIEW_MARGIN: f32 = 200.0;

/// Whether a thing standing at (`x`, `z`) can be seen from a camera at `eye` at all, or is too far off, in the haze, to be worth
/// drawing.
pub fn within_view(eye: Vec3, x: f32, z: f32) -> bool {
    vec2(x - eye.x, z - eye.z).length() <= nworld_view::view_distance(eye.y) + VIEW_MARGIN
}

/// Whether a token at `at` is far enough from a camera at `eye` to be drawn as a plain marker.
pub fn is_far(eye: Vec3, at: Vec3) -> bool {
    eye.distance(at) > FAR
}

/// The height of the ground a token of `radius` standing at (`x`, `z`) stands on: the highest ground under it, so that none
/// of it is buried.
pub fn ground_under(world: &World, x: f32, z: f32, radius: f32) -> f32 {
    let reach = radius * 0.8;
    [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)].iter().map(|(dx, dz)| world.surface_height(x + dx, z + dz)).fold(f32::MIN, f32::max)
}

/// `color` made lighter (over 1) or darker (under 1).
pub fn shade(color: Color, by: f32) -> Color {
    Color::new((color.r * by).min(1.0), (color.g * by).min(1.0), (color.b * by).min(1.0), color.a)
}

/// How much of its height a figure on a token keeps: pieces are squashed to this share of their height, so that from above
/// they are what they would be and from the side they are low, like the pieces of a flat game, and are only seen to have a body
/// at a low angle.
pub const FLAT: f32 = 0.4;

/// `pieces` squashed to `flat` of their height, measured up from the height `base`, which is where they stand. A ball can't be
/// squashed, so it becomes a low round dome of a disc, of the same width.
pub fn flatten(pieces: Vec<Piece>, base: f32, flat: f32) -> Vec<Piece> {
    pieces
        .into_iter()
        .map(|piece| {
            let foot = piece.middle.y - piece.size.y * 0.5;
            let height = piece.size.y * flat;
            let foot = base + (foot - base) * flat;
            let shape = if piece.shape == Shape::Ball { Shape::Disc } else { piece.shape };
            Piece { shape, middle: vec3(piece.middle.x, foot + height * 0.5, piece.middle.z), size: vec3(piece.size.x, height, piece.size.z), color: piece.color }
        })
        .collect()
}

/// The color of the edge of a tile, which a fixed thing stands on: dark slate, so that it is nothing like the gold of the round
/// base a unit that can move stands on.
pub const TILE_EDGE: Color = Color::new(0.20, 0.22, 0.27, 1.0);

/// A square tile for a fixed thing to stand on, with a half side of `half` and a `height`, the middle of its foot at `foot`, in the
/// color of `country`. Seen from above it is squares one in another: a dark slate edge, a thick frame in the country's color and a
/// pale square in the middle for a figure to stand on. A unit that can move stands on a round, gold base instead, so that the two
/// are told apart at a glance by their shape and their color. The pieces come lowest first, and the last is the pale square; a
/// figure stands on top of it, at [`tile_top`].
pub fn tile(foot: Vec3, half: f32, height: f32, country: Color) -> [Piece; 3] {
    let layer = half * 0.06;
    let at = |n: f32| foot + vec3(0.0, height + layer * n, 0.0);
    let block = |foot: Vec3, share: f32, thick: f32, color: Color| Piece::block_on(foot, vec3(half * 2.0 * share, thick, half * 2.0 * share), color);
    [block(foot, 1.0, height, TILE_EDGE), block(at(0.0), 0.88, layer, country), block(at(1.0), 0.62, layer, MEDALLION)]
}

/// The height of the top of the pale square of a tile made by [`tile`] with this `foot`, `half` and `height`.
pub fn tile_top(foot: Vec3, half: f32, height: f32) -> Vec3 {
    foot + vec3(0.0, height + half * 0.12, 0.0)
}

/// The color of the middle of a token's coin, or of a tile.
pub const MEDALLION: Color = Color::new(0.96, 0.93, 0.84, 1.0);
/// The gold of a unit's base.
pub const GOLD: Color = Color::new(0.93, 0.77, 0.22, 1.0);

/// The frame of the ground a tile covers: a square of `half` round (`x`, `z`), held just over the ground, as lines.
pub fn draw_square(world: &World, x: f32, z: f32, half: f32, color: Color) {
    let corners = [(-half, -half), (half, -half), (half, half), (-half, half), (-half, -half)];
    for pair in corners.windows(2) {
        let steps = ((pair[0].0 - pair[1].0).abs() + (pair[0].1 - pair[1].1).abs()).div_euclid(4.0).max(1.0) as usize;
        let at = |k: usize| {
            let t = k as f32 / steps as f32;
            let (px, pz) = (x + pair[0].0 + (pair[1].0 - pair[0].0) * t, z + pair[0].1 + (pair[1].1 - pair[0].1) * t);
            vec3(px, world.surface_height(px, pz) + 0.6, pz)
        };
        for k in 0..steps {
            draw_line_3d(at(k), at(k + 1), color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_piece_is_kept_as_the_box_that_holds_it() {
        let foot = vec3(10.0, 5.0, -3.0);
        let disc = Piece::disc(foot, 2.0, 1.0, WHITE);
        assert_eq!((disc.middle, disc.size), (vec3(10.0, 5.5, -3.0), vec3(4.0, 1.0, 4.0)));
        let cone = Piece::cone(foot, 1.0, 3.0, WHITE);
        assert_eq!((cone.middle.y, cone.size), (6.5, vec3(2.0, 3.0, 2.0)));
        let ball = Piece::ball(vec3(0.0, 4.0, 0.0), 1.5, WHITE);
        assert_eq!((ball.top(), ball.size), (5.5, Vec3::splat(3.0)));
        let block = Piece::block_on(foot, vec3(2.0, 2.0, 2.0), WHITE);
        assert_eq!((block.middle.y, block.top()), (6.0, 7.0));
        assert_eq!(disc.lifted(2.0).middle.y, 7.5);
    }

    #[test]
    fn a_box_is_hit_from_outside_and_from_inside_and_missed_from_the_side() {
        let (middle, size) = (vec3(0.0, 5.0, 0.0), vec3(4.0, 4.0, 4.0));
        let down = (vec3(0.0, 50.0, 0.0), vec3(0.0, -1.0, 0.0));
        let near = hit(down, middle, size, 0.0).unwrap();
        assert!((near - 43.0).abs() < 1e-4, "the top is 43 below");
        assert_eq!(hit((vec3(0.0, 5.0, 0.0), vec3(0.0, -1.0, 0.0)), middle, size, 0.0), Some(0.0), "from inside");
        assert_eq!(hit((vec3(9.0, 50.0, 0.0), vec3(0.0, -1.0, 0.0)), middle, size, 0.0), None);
        assert!(hit((vec3(9.0, 50.0, 0.0), vec3(0.0, -1.0, 0.0)), middle, size, 8.0).is_some(), "slop gives it room");
    }

    #[test]
    fn the_nearest_piece_is_the_one_hit_first() {
        let low = Piece::block(vec3(0.0, 1.0, 0.0), vec3(2.0, 2.0, 2.0), WHITE);
        let high = Piece::block(vec3(0.0, 6.0, 0.0), vec3(2.0, 2.0, 2.0), WHITE);
        let ray = (vec3(0.0, 30.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert!((first_hit(ray, &[low, high]).unwrap() - 23.0).abs() < 0.2);
        assert_eq!(first_hit(ray, &[]), None);
    }

    #[test]
    fn a_token_is_as_big_on_the_screen_wherever_it_is_but_never_tiny_or_huge() {
        let at = vec3(0.0, 0.0, 0.0);
        let radius = |distance: f32| radius_at(vec3(0.0, distance, 0.0), at);
        assert_eq!(radius(100.0), MIN_RADIUS, "close up it keeps a size that can be seen");
        assert_eq!(radius(100_000.0), MAX_RADIUS, "far off it does not cover regions");
        assert!(radius(3_000.0) > radius(2_000.0) && radius(2_000.0) > radius(1_000.0), "it grows with the distance");
        assert!((radius(2_000.0) - 24.0).abs() < 1e-3);
        assert!(MAX_RADIUS * 2.0 <= 100.0, "and is never more than about 100 m across");
        let (near, far) = (vec3(0.0, FAR - 1.0, 0.0), vec3(0.0, FAR + 1.0, 0.0));
        assert!(!is_far(near, at) && is_far(far, at));
    }

    #[test]
    fn flattening_squashes_pieces_down_towards_where_they_stand_and_turns_balls_into_domes() {
        let base = 10.0;
        let tall = Piece::cone(vec3(1.0, 12.0, 2.0), 3.0, 5.0, WHITE);
        let ball = Piece::ball(vec3(0.0, 14.0, 0.0), 2.0, WHITE);
        let flat = flatten(vec![tall, ball], base, 0.5);
        // The cone stood 2 over the base and was 5 high: now 1 over it and 2.5 high.
        assert_eq!((flat[0].middle.y - flat[0].size.y * 0.5, flat[0].size.y), (11.0, 2.5));
        assert_eq!(flat[0].size.x, 6.0, "as wide as it was");
        assert_eq!(flat[0].shape, Shape::Cone);
        // The ball was 4 across, from 12 up: now a disc 4 across and 2 high from 11.
        assert_eq!((flat[1].shape, flat[1].size, flat[1].middle.y - flat[1].size.y * 0.5), (Shape::Disc, vec3(4.0, 2.0, 4.0), 11.0));
        assert_eq!(flatten(vec![tall], base, 1.0)[0].size, tall.size, "a flatness of 1 changes nothing");
    }

    #[test]
    fn a_tile_is_squares_from_above_edge_frame_and_middle_each_a_little_higher() {
        let country = Color::new(0.2, 0.4, 0.9, 1.0);
        let foot = vec3(5.0, 2.0, 5.0);
        let layers = tile(foot, 10.0, 3.0, country);
        assert!(layers.iter().all(|p| p.shape == Shape::Block), "squares, not rounds");
        let sides: Vec<f32> = layers.iter().map(|p| p.size.x).collect();
        assert!(sides.windows(2).all(|w| w[0] > w[1]), "each smaller than the one under it: {sides:?}");
        assert!(layers.iter().all(|p| p.size.x == p.size.z), "square");
        let tops: Vec<f32> = layers.iter().map(Piece::top).collect();
        assert!(tops.windows(2).all(|w| w[0] < w[1]), "and each higher: {tops:?}");
        assert_eq!(layers[0].color, TILE_EDGE);
        assert_eq!(layers[1].color, country);
        assert_eq!(layers[2].color, MEDALLION);
        assert!((tile_top(foot, 10.0, 3.0).y - layers[2].top()).abs() < 1e-4, "a figure stands on the middle");
        assert!(tops.iter().zip(&tops[1..]).all(|(a, b)| b - a >= 0.4), "far enough apart for the depth buffer to tell them apart");
    }

    #[test]
    fn what_is_fixed_and_what_moves_stand_on_bases_that_are_nothing_alike() {
        // A fixed thing stands on a dark square, and a unit on a gold round.
        assert!(TILE_EDGE.r + TILE_EDGE.g + TILE_EDGE.b < 0.8, "the tile is dark");
        assert!(GOLD.r + GOLD.g + GOLD.b > 1.8, "the unit's base is bright");
        assert!((GOLD.r - TILE_EDGE.r) > 0.5, "and they are far apart in color");
    }

    #[test]
    fn only_what_is_within_the_distance_that_can_be_seen_is_drawn() {
        let eye = vec3(0.0, 300.0, 0.0);
        let seen = nworld_view::view_distance(300.0);
        assert!(within_view(eye, 0.0, 0.0) && within_view(eye, seen, 0.0));
        assert!(within_view(eye, 0.0, -(seen + VIEW_MARGIN * 0.5)), "a little past, for what reaches back");
        assert!(!within_view(eye, seen + VIEW_MARGIN + 1.0, 0.0));
        assert!(within_view(vec3(0.0, 5_000.0, 0.0), seen + VIEW_MARGIN + 1.0, 0.0), "from higher up, more can be seen");
    }

    #[test]
    fn colors_are_made_lighter_and_darker_without_going_over() {
        let c = shade(Color::new(0.5, 0.8, 1.0, 1.0), 1.5);
        assert_eq!((c.r, c.g, c.b), (0.75, 1.0, 1.0));
        assert!(shade(Color::new(0.5, 0.5, 0.5, 1.0), 0.5).r < 0.3);
    }
}
