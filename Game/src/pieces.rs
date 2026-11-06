//! Tokens: the units and the buildings as they stand on the map. Each is a model (see [`crate::miniatures`]) on a base, a round
//! one for a unit that moves and a square plot for a building, and is kept, for the mouse to point at, as the box that holds it.
//!
//! The tokens are not to the scale of the map. A token is sized by how far the camera is from it, so that it takes up
//! about the same room on the screen however far away it is, and never shrinks below a size that can be seen from close by.
//! This is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::miniatures::{self, Look, Moment};
use crate::model::{self, Model};

/// How much of the distance to the camera a token's radius is, so that a token is a few percent of the screen across, and
/// the least and the most it is, in meters: it is not so small close up that it can't be seen, nor so big far off that it
/// covers a whole region.
const SCREEN_SHARE: f32 = 0.02;
const MIN_RADIUS: f32 = 8.0;
const MAX_RADIUS: f32 = 48.0;
/// How far from the mouse's ray, as a fraction of how far away it is, a token still counts as pointed at, so that a small one
/// can be hit from far away.
const HOVER_SLOP: f32 = 0.006;

/// A token of some look, standing on the map: the middle of its foot, and the radius it is made with.
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub look: Look,
    pub foot: Vec3,
    pub radius: f32,
}

impl Token {
    /// A token of `look` standing at (`x`, `z`), for a camera at `eye`: on the highest ground under it, so that none of it is
    /// buried.
    pub fn at(world: &World, look: Look, x: f32, z: f32, eye: Vec3) -> Self {
        let radius = radius_at(eye, vec3(x, world.surface_height(x, z), z));
        Self { look, foot: vec3(x, ground_under(world, x, z, radius * look.reach()), z), radius }
    }

    /// The same token, lifted `by` meters (as a unit is, when it steps along).
    pub fn lifted(mut self, by: f32) -> Self {
        self.foot.y += by;
        self
    }

    /// The box that holds it: its middle and its size.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (reach, height) = (self.look.reach() * self.radius, self.look.height() * self.radius);
        (self.foot + Vec3::Y * height * 0.5, vec3(reach * 2.0, height, reach * 2.0))
    }

    /// How far along the ray the box that holds it is first hit, if it is, with a little give for the distance.
    pub fn hit(&self, ray: (Vec3, Vec3)) -> Option<f32> {
        let (middle, size) = self.bounds();
        hit(ray, middle, size, ray.0.distance(middle) * HOVER_SLOP)
    }

    /// Its model, facing `turn`, in the color of `country`, as it is at `moment`.
    pub fn model(&self, turn: Mat3, country: Color, moment: Moment) -> Model {
        let mut m = Model::turned(self.foot, self.radius, turn);
        miniatures::build(&mut m, self.look, country, moment);
        m
    }

    /// Draws a frame of `color` round the box that holds it.
    pub fn draw_frame(&self, color: Color) {
        let (middle, size) = self.bounds();
        draw_cube_wires(middle, size * 1.03, color);
    }
}

/// Draws `models`, as seen by the camera that is set, which is at `eye`, with `tint` mixed over them.
pub fn draw_models(eye: Vec3, tint: Vec4, models: impl IntoIterator<Item = Model>) {
    model::begin(eye, tint);
    for m in models {
        m.draw();
    }
    model::end();
}

/// The tint of a token drawn to show where something would be built, or what would be pulled down: `color`, over most of it.
pub fn tint_of(color: Color) -> Vec4 {
    vec4(color.r, color.g, color.b, 0.55)
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

/// The height of the ground a thing reaching `radius` round (`x`, `z`) stands on (`x`, `z`) stands on: the highest ground under it, so that none
/// of it is buried.
pub fn ground_under(world: &World, x: f32, z: f32, radius: f32) -> f32 {
    let reach = radius * 0.8;
    [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)].iter().map(|(dx, dz)| world.surface_height(x + dx, z + dz)).fold(f32::MIN, f32::max)
}

/// The frame of the ground a plot covers: a square of `half` round (`x`, `z`), held just over the ground, as lines.
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
    use crate::test_support;

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
    fn a_token_is_as_big_on_the_screen_wherever_it_is_but_never_tiny_or_huge() {
        let at = vec3(0.0, 0.0, 0.0);
        let radius = |distance: f32| radius_at(vec3(0.0, distance, 0.0), at);
        assert_eq!(radius(100.0), MIN_RADIUS, "close up it keeps a size that can be seen");
        assert_eq!(radius(100_000.0), MAX_RADIUS, "far off it does not cover regions");
        assert!(radius(3_000.0) > radius(2_000.0) && radius(2_000.0) > radius(1_000.0), "it grows with the distance");
        assert!((radius(2_000.0) - 40.0).abs() < 1e-3);
        assert!(MAX_RADIUS * 2.0 <= 100.0, "and is never more than about 100 m across");
    }

    #[test]
    fn a_token_stands_on_the_highest_ground_under_it_and_is_held_by_its_box() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let eye = vec3(at.x, world.surface_height(at.x, at.y) + 300.0, at.y + 1.0);
        let token = Token::at(&world, Look::Quarry, at.x, at.y, eye);
        assert!(token.foot.y >= world.surface_height(at.x, at.y) - 1e-3);
        let (middle, size) = token.bounds();
        assert!((size.x - Look::Quarry.reach() * 2.0 * token.radius).abs() < 1e-3 && (middle.y - token.foot.y - size.y * 0.5).abs() < 1e-3);
        let down = (eye, vec3(0.0, -1.0, 0.0));
        assert!(token.hit(down).is_some());
        assert!(token.hit((eye + vec3(500.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0))).is_none());
        assert!(token.lifted(2.0).foot.y - token.foot.y > 1.99);
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
}
