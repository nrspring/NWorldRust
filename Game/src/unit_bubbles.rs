//! Bubbles over the units: when the camera looks at the map at an angle, rather than straight down, each militia and ship has
//! a small round bubble held over it, in the color of the country it belongs to, so that whose it is can be told from the
//! side, where its base can't be seen. Round the bubble is a green ring for the movement points it has left today, full when
//! it has not moved and gone when it can move no more, and a militia under siege has a red badge with a tower on it. The
//! bubbles are drawn on the screen, over the map and under the interface, the same size however far off the unit is. This
//! is the game's own drawing, and not part of drawing the map.

use macroquad::prelude::*;
use nworld_map::world::World;

use crate::day::DaySave;
use crate::movement::DAILY_POINTS;
use crate::pieces::{self, Token};
use crate::region_labels::to_screen;
use crate::walk::Walks;
use crate::{militia_view, ship_view};

/// How steeply the camera may look down and still show the bubbles: they are all there below the first pitch, fade out
/// between the two, and are gone above the second, where the map is seen from straight above and a unit's base shows its
/// country, in radians.
const FULL_BELOW: f32 = 1.15;
const GONE_ABOVE: f32 = 1.35;
/// How far over the top of the unit the bubble is held, as a share of the token's radius.
const CLEARANCE: f32 = 0.15;
/// The least a bubble shrinks to, against its full size, from far out: it shrinks as the camera goes out, as the pieces do.
const LEAST: f32 = 0.55;
/// The bubble's radius, the length of the tail that points down to the unit, and how thick the ring of movement points is,
/// in pixels, at its full size.
const RADIUS: f32 = 9.0;
const TAIL: f32 = 7.0;
const RING: f32 = 3.0;
/// The radius of the badge of a unit under siege, in pixels.
const BADGE: f32 = 6.5;

const MOVES: Color = Color::new(0.42, 0.95, 0.45, 1.0);
const TRACK: Color = Color::new(0.06, 0.07, 0.10, 0.75);
const SIEGE: Color = Color::new(0.88, 0.12, 0.10, 1.0);
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// The bubble over one unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bubble {
    /// The point over the unit that the tail of the bubble points to, on the map.
    pub over: Vec3,
    pub country: Color,
    /// How much of the day's movement points the unit has left, from 0 to 1.
    pub moves: f32,
    pub under_siege: bool,
}

/// How strongly the bubbles show for a camera looking down at `pitch` radians: 1 at an angle, 0 from straight above.
pub fn strength(pitch: f32) -> f32 {
    let t = ((GONE_ABOVE - pitch) / (GONE_ABOVE - FULL_BELOW)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of the day's movement points `points` is, from 0 to 1.
fn share(points: u32) -> f32 {
    (points as f32 / DAILY_POINTS as f32).clamp(0.0, 1.0)
}

/// The point over `token` that a bubble points to.
fn over(token: &Token) -> Vec3 {
    token.foot + Vec3::Y * (token.look.height() + CLEARANCE) * token.radius
}

/// The bubbles over the militia and the ships of `day` that can be seen from a camera at `eye`, where they are as they walk
/// and sail.
pub fn bubbles(world: &World, day: &DaySave, walks: &Walks, eye: Vec3) -> Vec<Bubble> {
    let country = |id| day.country(id).map_or(NO_COUNTRY, |c| c.color.color());
    let militias = day.militias.iter().filter_map(|militia| {
        let at = walks.position(militia);
        pieces::within_view(eye, at.x, at.y).then(|| Bubble {
            over: over(&Token::at(world, militia_view::look(militia), at.x, at.y, eye)),
            country: country(militia.country),
            moves: share(militia.points),
            under_siege: militia.under_siege,
        })
    });
    let ships = day.ships.iter().filter_map(|ship| {
        let at = walks.position_of(ship.id, vec2(ship.x, ship.z));
        pieces::within_view(eye, at.x, at.y).then(|| Bubble {
            over: over(&Token::at(world, ship_view::look(ship.kind), at.x, at.y, eye)),
            country: country(ship.country),
            moves: share(ship.points),
            under_siege: false,
        })
    });
    militias.chain(ships).collect()
}

/// Draws the bubbles over the units of `day`, as `camera` sees them, which looks down at `pitch` radians. Nothing is drawn
/// while it looks straight down.
pub fn draw(world: &World, day: &DaySave, walks: &Walks, camera: &Camera3D, pitch: f32) {
    let strength = strength(pitch);
    if strength <= 0.0 {
        return;
    }
    let mut bubbles = bubbles(world, day, walks, camera.position);
    // The farthest first, so that the nearer ones are drawn over them.
    bubbles.sort_by(|a, b| b.over.distance_squared(camera.position).total_cmp(&a.over.distance_squared(camera.position)));
    for bubble in &bubbles {
        let Some(tip) = to_screen(camera, bubble.over) else { continue };
        let reach = RADIUS + TAIL + RING + BADGE;
        if tip.x < -reach || tip.x > screen_width() + reach || tip.y < -reach || tip.y > screen_height() + reach * 2.0 {
            continue;
        }
        let scale = pieces::screen_scale(camera.position.distance(bubble.over)).max(LEAST);
        draw_bubble(tip, bubble, strength, scale);
    }
}

/// Draws `bubble` with the point of its tail at `tip`, on the screen, `strength` strong and `scale` times its full size.
fn draw_bubble(tip: Vec2, bubble: &Bubble, strength: f32, scale: f32) {
    let fade = |c: Color| Color::new(c.r, c.g, c.b, c.a * strength);
    let (radius, tail, ring) = (RADIUS * scale, TAIL * scale, RING * scale);
    let middle = tip - vec2(0.0, tail + radius);
    // The tail, and a dark edge round it all, so that it shows over any ground or water.
    let (left, right) = (middle + vec2(-radius * 0.55, radius * 0.6), middle + vec2(radius * 0.55, radius * 0.6));
    draw_triangle(tip + vec2(0.0, 1.5), left + vec2(-1.5, 0.0), right + vec2(1.5, 0.0), fade(TRACK));
    draw_circle(middle.x, middle.y, radius + ring + 1.0, fade(TRACK));
    draw_triangle(tip, left, right, fade(WHITE));
    // The ring of the movement points left, going round from the top as a clock does.
    if bubble.moves > 0.0 {
        draw_arc(middle.x, middle.y, 40, radius, -90.0, ring, 360.0 * bubble.moves, fade(MOVES));
    }
    draw_circle(middle.x, middle.y, radius, fade(WHITE));
    draw_circle(middle.x, middle.y, radius - 1.5 * scale, fade(bubble.country));
    if bubble.under_siege {
        draw_siege_badge(middle + vec2(radius * 0.85, -radius * 0.85), strength, scale);
    }
}

/// A red badge with a white tower on it, at `at`.
fn draw_siege_badge(at: Vec2, strength: f32, scale: f32) {
    let fade = |c: Color| Color::new(c.r, c.g, c.b, c.a * strength);
    let badge = BADGE * scale;
    draw_circle(at.x, at.y, badge + 1.2, fade(TRACK));
    draw_circle(at.x, at.y, badge, fade(SIEGE));
    // The tower is left out when the badge is too small to show it.
    if scale >= 0.8 {
        let white = fade(WHITE);
        draw_rectangle(at.x - 2.6 * scale, at.y - 1.0 * scale, 5.2 * scale, 4.2 * scale, white);
        for dx in [-2.6, -0.8, 1.0] {
            draw_rectangle(at.x + dx * scale, at.y - 3.2 * scale, 1.6 * scale, 2.4 * scale, white);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::{CityId, CountryId, MilitiaId};
    use crate::militia::Militia;
    use crate::test_support;

    #[test]
    fn they_show_at_an_angle_and_not_from_straight_above() {
        assert_eq!(strength(1.5), 0.0, "straight down, as the camera starts and resets");
        assert_eq!(strength(GONE_ABOVE), 0.0);
        assert_eq!(strength(FULL_BELOW), 1.0);
        assert_eq!(strength(0.6), 1.0);
        let between = strength((FULL_BELOW + GONE_ABOVE) * 0.5);
        assert!(between > 0.2 && between < 0.8, "{between}");
    }

    #[test]
    fn they_shrink_as_the_camera_goes_out_but_not_below_a_size_that_can_be_read() {
        let size = |distance: f32| pieces::screen_scale(distance).max(LEAST);
        assert_eq!(size(100.0), 1.0);
        assert!(size(900.0) < 1.0 && size(1_100.0) < size(900.0) && size(1_000_000.0) == LEAST);
    }

    #[test]
    fn the_ring_is_the_share_of_the_points_left() {
        assert_eq!((share(DAILY_POINTS), share(0), share(DAILY_POINTS * 2)), (1.0, 0.0, 1.0));
        assert!((share(DAILY_POINTS / 2) - 0.5).abs() < 0.01);
    }

    #[test]
    fn a_militia_has_a_bubble_over_it_in_its_country_color_with_its_points_and_siege() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 50.0);
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        let country = CountryId::new();
        day.militias.push(Militia { id: MilitiaId::new(), name: "Test Militia".to_owned(), country, city: CityId::new(), raised: Date::FIRST, members: 300, x: at.x, z: at.y, points: 0, under_siege: true, trained: false });
        let eye = vec3(at.x + 200.0, world.surface_height(at.x, at.y) + 150.0, at.y);
        let bubbles = bubbles(&world, &day, &Walks::default(), eye);
        assert_eq!(bubbles.len(), 1);
        let token = Token::at(&world, militia_view::look(&day.militias[0]), at.x, at.y, eye);
        assert!(bubbles[0].over.y > token.bounds().0.y + token.bounds().1.y * 0.5, "over the top of the unit");
        assert_eq!((bubbles[0].moves, bubbles[0].under_siege, bubbles[0].country), (0.0, true, NO_COUNTRY), "a country that can't be found is gray");
    }
}
