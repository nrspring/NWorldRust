//! Drawing the names of the regions on the map, over everything else, at the middle of each region. A region
//! with no name has no label.

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
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
        let Some(at) = to_screen(camera, vec3(center.x, world.surface_height(center.x, center.y) + HOLD, center.y)) else { continue };
        let width = style.measure(&named.name, TEXT_SIZE, Weight::Semibold).width;
        // Off the screen, there is nothing to draw.
        if at.x < -width || at.x > screen_width() + width || at.y < -20.0 || at.y > screen_height() + 20.0 {
            continue;
        }
        // A dark edge first, so that the name can be read over any ground.
        for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5), (1.0, 1.0), (-1.0, -1.0)] {
            style.text_mid(&named.name, at.x - width * 0.5 + dx, at.y + dy, TEXT_SIZE, Weight::Semibold, Color::new(0.04, 0.05, 0.08, 0.85));
        }
        style.text_mid(&named.name, at.x - width * 0.5, at.y, TEXT_SIZE, Weight::Semibold, WHITE);
    }
}
