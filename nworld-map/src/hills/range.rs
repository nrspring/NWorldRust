//! Mountain ranges: a winding main ridge with branches off it, placed at random.

use glam::*;

use super::{HillStroke, Peak};
use crate::gen_common::Rng;
use crate::noise::smoothstep;
use crate::world::CELL_SIZE;

/// Meters between the points a mountain range's spine is traced through.
const RANGE_STEP: f32 = 20.0;
/// A range stops this far from the water, so it never runs into the coast.
const RANGE_COAST_MARGIN: f32 = 60.0;
/// Spots tried as the start of a range; the one with the most room to run is used.
const RANGE_TRIES: usize = 24;
/// Times the brush walks along a range's spine, back and forth, to grow it fully.
const RANGE_PASSES: usize = 3;
/// Seconds of brush time at each point of the spine on each pass.
const RANGE_DT: f32 = 0.5;

impl HillStroke {
    /// Points along a winding line from `start`, heading `heading` radians, up to `length` meters
    /// long, stopping early where it would leave the map or come close to the water.
    fn trace_spine(&self, start: Vec2, mut heading: f32, length: f32, half: Vec2) -> Vec<Vec2> {
        let mut points = vec![start];
        let mut at = start;
        while points.len() as f32 * RANGE_STEP <= length {
            // Wind gently, more in some stretches than others.
            heading += 0.15 * self.noise.fbm(at.x / 300.0 + 61.0, at.y / 300.0 - 19.0, 2, 0.5);
            let next = at + vec2(heading.cos(), heading.sin()) * RANGE_STEP;
            let i = ((next.x + half.x) / CELL_SIZE).round();
            let j = ((next.y + half.y) / CELL_SIZE).round();
            if i < 0.0 || j < 0.0 || i > (self.nx - 1) as f32 || j > (self.nz - 1) as f32 {
                break;
            }
            if self.inland(j as usize * self.nx + i as usize) < RANGE_COAST_MARGIN {
                break;
            }
            points.push(next);
            at = next;
        }
        points
    }
}

/// One arm of a mountain range: a winding line of points and how wide the range is along it.
struct Limb {
    points: Vec<Vec2>,
    /// The width at the thickest point, as a share of the range's width.
    scale: f32,
    /// Whether the limb narrows toward both ends (the main ridge) or only toward its far end (a
    /// branch, which joins the ridge at its start).
    both_ends: bool,
}

impl Limb {
    /// How wide the limb is a share `t` (0 to 1) of the way along it, from 0.15 to 1.
    fn taper(&self, t: f32) -> f32 {
        let fade = if self.both_ends {
            smoothstep(0.0, 0.3, t).min(smoothstep(1.0, 0.7, t))
        } else {
            1.0 - smoothstep(0.0, 1.0, t)
        };
        0.15 + 0.85 * fade
    }
}

/// Raises a randomly placed range of mountains about `length` meters long and `width` meters
/// across: a winding main ridge that narrows toward its ends and swells and pinches along the
/// way, with a few shorter branches off to the sides, and foothills around it all. It goes on
/// the land in `heights` (`nx` by `nz` samples, centered on the origin by `half`). `None` if no
/// land has room for one.
pub fn mountain_range(heights: &[f32], nx: usize, nz: usize, half: Vec2, seed: u32, length: f32, width: f32) -> Option<Vec<f32>> {
    let mut stroke = HillStroke::begin(heights, nx, nz, seed, false, Peak::Mountains);
    let mut rng = Rng::new(seed ^ 0x2B7E_1516);

    let mut ridge: Vec<Vec2> = Vec::new();
    let (mut starts, mut draws) = (0, 0);
    while starts < RANGE_TRIES && draws < RANGE_TRIES * 40 {
        draws += 1;
        let i = rng.range(0.0, (nx - 1) as f32) as usize;
        let j = rng.range(0.0, (nz - 1) as f32) as usize;
        if stroke.inland(j * nx + i) < RANGE_COAST_MARGIN * 2.0 {
            continue;
        }
        starts += 1;
        let start = vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
        let candidate = stroke.trace_spine(start, rng.range(0.0, std::f32::consts::TAU), length, half);
        if candidate.len() > ridge.len() {
            ridge = candidate;
        }
    }
    // Settle for less than was asked for if the land is cramped, but not for a stub.
    if (ridge.len() as f32 - 1.0) * RANGE_STEP < (length * 0.4).min(300.0) {
        return None;
    }

    // Branches leave the ridge at an angle, alternating sides, and are shorter and slimmer.
    let mut limbs = Vec::new();
    let branches = ((ridge.len() as f32 * RANGE_STEP / 500.0) as usize).clamp(1, 6);
    let mut side = if rng.range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
    for _ in 0..branches {
        let k = rng.range(0.15, 0.85) * (ridge.len() - 1) as f32;
        let k = (k as usize).min(ridge.len() - 2);
        let along = ridge[k + 1] - ridge[k];
        let heading = along.y.atan2(along.x) + side * rng.range(0.6, 1.1);
        side = -side;
        let points = stroke.trace_spine(ridge[k], heading, length * rng.range(0.2, 0.4), half);
        if points.len() >= 5 {
            limbs.push(Limb { points, scale: rng.range(0.55, 0.8), both_ends: false });
        }
    }
    limbs.insert(0, Limb { points: ridge, scale: 1.0, both_ends: true });

    let mut out = heights.to_vec();
    for limb in &limbs {
        // Start fresh, so the brush doesn't drag from where the last limb ended.
        stroke.last = None;
        let last = (limb.points.len() - 1).max(1) as f32;
        for pass in 0..RANGE_PASSES {
            let walk: Vec<usize> = if pass % 2 == 0 {
                (0..limb.points.len()).collect()
            } else {
                (0..limb.points.len()).rev().collect()
            };
            for k in walk {
                let p = limb.points[k];
                // Broad massifs and slender saddles along the way, on top of the taper.
                let noise = stroke.noise.fbm(p.x / 130.0 + 5.0, p.y / 130.0 - 9.0, 2, 0.5).clamp(-1.0, 1.0);
                let swell = 0.9 + 0.55 * noise;
                let radius = width * 0.5 * limb.scale * limb.taper(k as f32 / last) * swell.clamp(0.4, 1.4);
                stroke.paint(&mut out, half, p, radius, RANGE_DT);
            }
        }
    }
    stroke.finish(&mut out);
    (out != heights).then_some(out)
}
