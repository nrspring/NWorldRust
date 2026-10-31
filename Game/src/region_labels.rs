//! Drawing the names of the regions, rivers, lakes and places on the map, over everything else: a region's at its
//! middle, a river's or lake's at a point of its water near its middle, and a place's at the point it was put.
//! Anything with no name has no label.

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
use nworld_map::water_bodies::{BodyId, WaterBodies};
use nworld_map::world::World;
use nworld_view::style::{Style, Weight};

use crate::day::DaySave;

const TEXT_SIZE: u16 = 15;
/// How high above the ground the label is held, in meters.
const HOLD: f32 = 4.0;

/// Where the world point `at` is on the screen, if it is in front of the camera: in pixels from the top left.
pub fn to_screen(camera: &Camera3D, at: Vec3) -> Option<Vec2> {
    let clip = camera.matrix() * at.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(vec2((ndc.x + 1.0) * 0.5 * screen_width(), (1.0 - ndc.y) * 0.5 * screen_height()))
}

/// Draws the name of each region of `day` that has one, centered on the region as `camera` sees it.
pub fn draw(style: &Style, day: &DaySave, map: &RegionMap, world: &World, camera: &Camera3D) {
    for named in &day.region_names {
        let Some(center) = RegionId::from_raw(named.region).and_then(|r| map.center(r)) else { continue };
        draw_label(style, world, camera, center, &named.name, WHITE);
    }
}

/// Draws the name of each river of `day` that has one, on its water as `camera` sees it, in a pale blue.
pub fn draw_rivers(style: &Style, day: &DaySave, bodies: &WaterBodies, world: &World, camera: &Camera3D) {
    draw_waters(style, day.river_names.iter().map(|n| (n.river, n.name.as_str())), bodies, world, camera, Color::new(0.72, 0.90, 1.0, 1.0));
}

/// The same for the lakes, in a paler blue-green so that the two can be told apart.
pub fn draw_lakes(style: &Style, day: &DaySave, bodies: &WaterBodies, world: &World, camera: &Camera3D) {
    draw_waters(style, day.lake_names.iter().map(|n| (n.lake, n.name.as_str())), bodies, world, camera, Color::new(0.80, 1.0, 0.92, 1.0));
}

/// Draws the name of each place of `day`, in a pale gold.
pub fn draw_places(style: &Style, day: &DaySave, world: &World, camera: &Camera3D) {
    for place in &day.places {
        draw_label(style, world, camera, vec2(place.x, place.z), &place.name, Color::new(1.0, 0.91, 0.68, 1.0));
    }
}

/// The place whose label is under `mouse` (in pixels), if there is one; the one drawn last if labels overlap.
pub fn place_under(style: &Style, day: &DaySave, world: &World, camera: &Camera3D, mouse: Vec2) -> Option<u32> {
    day.places.iter().rev().find(|place| {
        let Some(at) = to_screen(camera, vec3(place.x, world.surface_height(place.x, place.z) + HOLD, place.z)) else { return false };
        let width = style.measure(&place.name, TEXT_SIZE, Weight::Semibold).width;
        Rect::new(at.x - width * 0.5 - 4.0, at.y - 11.0, width + 8.0, 22.0).contains(mouse)
    }).map(|place| place.id)
}

/// Draws the names of the bodies of water given as their numbers and names.
fn draw_waters<'a>(style: &Style, names: impl Iterator<Item = (u32, &'a str)>, bodies: &WaterBodies, world: &World, camera: &Camera3D, color: Color) {
    for (id, name) in names {
        let Some(body) = bodies.body(BodyId(id)) else { continue };
        draw_label(style, world, camera, body.label, name, color);
    }
}

/// Draws `text` centered at the point `at` of the map, as `camera` sees it, if that is on the screen.
fn draw_label(style: &Style, world: &World, camera: &Camera3D, at: Vec2, text: &str, color: Color) {
    let Some(at) = to_screen(camera, vec3(at.x, world.surface_height(at.x, at.y) + HOLD, at.y)) else { return };
    let width = style.measure(text, TEXT_SIZE, Weight::Semibold).width;
    // Off the screen, there is nothing to draw.
    if at.x < -width || at.x > screen_width() + width || at.y < -20.0 || at.y > screen_height() + 20.0 {
        return;
    }
    // A dark edge first, so that the name can be read over any ground.
    for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5), (1.0, 1.0), (-1.0, -1.0)] {
        style.text_mid(text, at.x - width * 0.5 + dx, at.y + dy, TEXT_SIZE, Weight::Semibold, Color::new(0.04, 0.05, 0.08, 0.85));
    }
    style.text_mid(text, at.x - width * 0.5, at.y, TEXT_SIZE, Weight::Semibold, color);
}
