//! Elevation labels for the topographic view: numbers set on the contour lines.

use std::collections::HashSet;

use macroquad::prelude::*;

use crate::style::{self, Style, Weight};
use crate::world::{World, CELL_SIZE};

/// Meters between contour lines.
const CONTOUR_STEP: f32 = 5.0;
/// Every this many meters the contour is an index line, labeled even from afar.
const INDEX_STEP: i32 = 25;
/// Candidate labels are found about this far apart, in meters, so every stretch of line that
/// is long enough has somewhere to put one. Which of them are drawn is up to the screen.
const CANDIDATE_SPACING: f32 = 80.0;
/// Pixels of clear space kept around every label.
const LABEL_GAP: f32 = 6.0;
/// Two labels for the same elevation stay at least this many pixels apart, so a line gets a
/// label every so often rather than a string of them, but every separate line still gets one.
const SAME_LEVEL_APART: f32 = 150.0;
/// Seen from far away the lines that aren't index lines are labeled more sparsely: the distance
/// between labels of one elevation grows by up to this many pixels.
const FAR_EXTRA_APART: f32 = 60.0;
const FONT_SIZE: u16 = 11;
/// How often the labels are re-found while the map is being painted, in seconds.
const REFRESH_SECONDS: f64 = 0.3;

/// Where a contour line is, to hang a number on.
struct Candidate {
    /// World position: x, the contour's elevation, z.
    pos: Vec3,
    level: i32,
}

/// The elevation labels of the map, found again when the terrain changes.
#[derive(Default)]
pub struct ContourLabels {
    /// Which terrain the candidates were found on.
    revision: Option<(u64, u64)>,
    found_at: f64,
    candidates: Vec<Candidate>,
}

impl ContourLabels {
    /// Finds the candidate label spots again if the terrain changed since they were found. While
    /// painting that is every frame, so it waits a moment between searches.
    pub fn refresh(&mut self, world: &World, now: f64) {
        let revision = world.revision();
        if self.revision == Some(revision) || (self.revision.is_some() && now - self.found_at < REFRESH_SECONDS) {
            return;
        }
        let (nx, nz) = world.samples();
        self.candidates = find_candidates(world.heights(), world.water(), nx, nz, world.half_extents());
        self.revision = Some(revision);
        self.found_at = now;
    }

    /// Draws the labels that fit on screen without crowding each other or hiding behind hills.
    /// `camera` is the one the world was just drawn with.
    pub fn draw(&self, style: &Style, world: &World, camera: &Camera3D, left_edge: f32) {
        let view = camera.matrix();
        let cam = camera.position;
        let (width, height) = (screen_width(), screen_height());

        // Where each candidate lands on screen; the ones not in view are dropped.
        let mut visible: Vec<(bool, f32, Vec2, &Candidate)> = Vec::new();
        for c in &self.candidates {
            let distance = c.pos.distance(cam);
            let index = c.level % INDEX_STEP == 0;
            let p = view * c.pos.extend(1.0);
            if p.w <= 0.0 {
                continue;
            }
            let ndc = p.truncate() / p.w;
            if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z.abs() > 1.0 {
                continue;
            }
            let screen = vec2((ndc.x * 0.5 + 0.5) * width, (0.5 - ndc.y * 0.5) * height);
            if screen.x < left_edge {
                continue;
            }
            visible.push((index, distance, screen, c));
        }
        // Index lines first, then nearest first, so the labels that stay are the useful ones.
        visible.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)));

        let mut placed: Vec<(Rect, i32)> = Vec::new();
        for (index, distance, screen, candidate) in visible {
            let text = candidate.level.to_string();
            let w = style.measure(&text, FONT_SIZE, Weight::Semibold).width + 8.0;
            let rect = Rect::new(screen.x - w * 0.5, screen.y - 8.0, w, 16.0);
            let padded = Rect::new(rect.x - LABEL_GAP, rect.y - LABEL_GAP, rect.w + 2.0 * LABEL_GAP, rect.h + 2.0 * LABEL_GAP);
            let far = if index { 0.0 } else { ((distance - 1500.0) / 20.0).clamp(0.0, FAR_EXTRA_APART) };
            let apart = SAME_LEVEL_APART + far;
            let crowded = placed.iter().any(|(r, level)| {
                r.overlaps(&padded) || (*level == candidate.level && (r.center() - rect.center()).length() < apart)
            });
            if crowded || hidden(world, cam, candidate.pos) {
                continue;
            }
            style::rounded_rect(rect, 4.0, Color::new(0.96, 0.94, 0.82, 0.82));
            style.text_centered(&text, rect, FONT_SIZE, Weight::Semibold, Color::new(0.33, 0.20, 0.11, 1.0));
            placed.push((rect, candidate.level));
        }
    }
}

/// Whether terrain stands between the camera and `point`, so a label there would float in front
/// of a hill instead of sitting on the ground behind it.
fn hidden(world: &World, cam: Vec3, point: Vec3) -> bool {
    const STEPS: usize = 24;
    (1..STEPS).any(|k| {
        let p = cam.lerp(point, k as f32 / STEPS as f32 * 0.98);
        p.y < world.surface_height(p.x, p.z) - 0.5
    })
}

/// One candidate spot per contour level in every `CANDIDATE_SPACING` square of the map, at the
/// first place the line is found there. `water` holds the water levels, since lines only mean
/// something on dry ground.
fn find_candidates(heights: &[f32], water: &[f32], nx: usize, nz: usize, half: Vec2) -> Vec<Candidate> {
    let bucket = (CANDIDATE_SPACING / CELL_SIZE) as usize;
    let mut taken: HashSet<(i32, usize, usize)> = HashSet::new();
    let mut found = Vec::new();
    for j in 0..nz {
        for i in 0..nx {
            let h = heights[j * nx + i];
            for (di, dj) in [(1usize, 0usize), (0, 1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni >= nx || nj >= nz {
                    continue;
                }
                let g = heights[nj * nx + ni];
                let (lo, hi) = (h.min(g), h.max(g));
                // Lines only mean something on land.
                if lo < 1.0 || lo == hi || h < water[j * nx + i] || g < water[nj * nx + ni] {
                    continue;
                }
                let mut level = ((lo / CONTOUR_STEP).floor() + 1.0) * CONTOUR_STEP;
                while level <= hi {
                    let key = (level as i32, i / bucket, j / bucket);
                    if taken.insert(key) {
                        let t = (level - h) / (g - h);
                        found.push(Candidate {
                            pos: vec3(
                                (i as f32 + di as f32 * t) * CELL_SIZE - half.x,
                                level,
                                (j as f32 + dj as f32 * t) * CELL_SIZE - half.y,
                            ),
                            level: level as i32,
                        });
                    }
                    level += CONTOUR_STEP;
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_sit_on_the_contours_of_a_ramp() {
        // A ramp climbing 1 m every cell, up to 40 m.
        let (nx, nz) = (40, 4);
        let heights: Vec<f32> = (0..nz).flat_map(|_| (0..nx).map(|i| 1.0 + i as f32)).collect();
        let found = find_candidates(&heights, &vec![0.0; nx * nz], nx, nz, Vec2::ZERO);
        let mut levels: Vec<i32> = found.iter().map(|c| c.level).collect();
        levels.sort_unstable();
        levels.dedup();
        assert_eq!(levels, vec![5, 10, 15, 20, 25, 30, 35, 40]);
        for c in &found {
            // The label's elevation is the level, and it lies where the ramp reaches it.
            assert_eq!(c.pos.y, c.level as f32);
            assert!(((c.pos.x / CELL_SIZE) + 1.0 - c.level as f32).abs() < 1e-3);
        }
    }

    #[test]
    fn water_and_flat_ground_get_no_labels() {
        let heights = vec![-5.0; 100];
        assert!(find_candidates(&heights, &[0.0; 100], 10, 10, Vec2::ZERO).is_empty());
        let heights = vec![12.0; 100];
        assert!(find_candidates(&heights, &[0.0; 100], 10, 10, Vec2::ZERO).is_empty());
    }

    #[test]
    fn the_bed_of_a_lake_gets_no_labels() {
        // A ramp that climbs 1 m a cell, flooded up to 20 m: only the dry part is labeled.
        let (nx, nz) = (40, 4);
        let heights: Vec<f32> = (0..nz).flat_map(|_| (0..nx).map(|i| 1.0 + i as f32)).collect();
        let water = vec![20.0; nx * nz];
        let mut levels: Vec<i32> = find_candidates(&heights, &water, nx, nz, Vec2::ZERO).iter().map(|c| c.level).collect();
        levels.sort_unstable();
        levels.dedup();
        // The 20 m line is the shore itself.
        assert_eq!(levels, vec![25, 30, 35, 40]);
    }
}
