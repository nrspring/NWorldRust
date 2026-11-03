//! Lakes: digging them with the brush, and scattering them at random.

use glam::*;

use super::{HillStroke, Peak};
use crate::gen_common::{Rng, LAND_HEIGHT};
use crate::noise::smoothstep;
use crate::world::{Region, CELL_SIZE, SEA_LEVEL};

/// A lake's shoreline wanders by up to this share of its radius.
const LAKE_WANDER: f32 = 0.35;
/// Lakes stay this far from the sea, in meters, easing in over as far again, so they never
/// break through to it.
pub(super) const LAKE_COAST_MARGIN: f32 = 30.0;
/// The longest a lake's banks run before meeting the ground around it, in meters.
const LAKE_MAX_BANK: f32 = 150.0;
/// Spots tried for each randomly placed lake.
const LAKE_TRIES: usize = 80;
/// Of the spots that fit, this many are compared and the flattest used, so a lake's surface
/// doesn't end up standing above lower ground beside it.
const LAKE_CANDIDATES: usize = 6;
/// A lake won't spread onto ground this many meters lower than its water.
const LAKE_SPILL: f32 = 1.0;
impl HillStroke {
    /// How high the ground ends up at sample `idx` (at world `x`, `z`, `distance` from the center
    /// of a lake of `radius`) when the lake's water stands at `level`: below the water inside the
    /// shoreline, which wanders as the noise pushes it in and out, and rising along banks to the
    /// ground the stroke began with, over a distance that grows with how high that ground stands
    /// above the water. It never goes up, and fades out toward the coast so a lake can't break
    /// through to the sea.
    fn lake_height(&self, idx: usize, (x, z): (f32, f32), distance: f32, radius: f32, level: f32) -> f32 {
        let base = self.base[idx];
        let wander = 1.0 + LAKE_WANDER * self.noise.fbm(x / 110.0 + 13.0, z / 110.0 - 29.0, 2, 0.5).clamp(-1.0, 1.0);
        let e = distance / wander;
        let shaped = if e < radius {
            let depth = (radius * 0.12).clamp(2.0, 10.0);
            level - depth * (1.0 - (e / radius) * (e / radius))
        } else {
            let above = (base - level).max(0.0);
            let bank = (above * 6.0).clamp(8.0, LAKE_MAX_BANK);
            let f = ((e - radius) / bank).min(1.0);
            level + above * (1.0 - (1.0 - f) * (1.0 - f))
        };
        let allowed = smoothstep(LAKE_COAST_MARGIN, 2.0 * LAKE_COAST_MARGIN, self.sea_inland(idx));
        base + (shaped.min(base) - base) * allowed
    }

    /// The level of the water in a lake dug by a brush of `radius` at `at`: that of a lake the
    /// brush touches, so the two join up, or else that of the lowest ground it covers, where the
    /// lake would fill first. `None` if the brush can't dig here: no ground far enough from the sea
    /// is under it, or it would spill a lake it touches onto lower ground.
    fn lake_level(&self, heights: &[f32], half: Vec2, at: Vec2, radius: f32) -> Option<f32> {
        // The shoreline wanders out as far as this.
        let disc = radius * (1.0 + LAKE_WANDER) + 2.0 * CELL_SIZE;
        let touch = radius + 2.0 * CELL_SIZE;
        let to_index = |v: f32, h: f32, n: usize| ((v + h) / CELL_SIZE).clamp(0.0, (n - 1) as f32);
        let i0 = to_index(at.x - disc, half.x, self.nx).ceil() as usize;
        let i1 = to_index(at.x + disc, half.x, self.nx).floor() as usize;
        let j0 = to_index(at.y - disc, half.y, self.nz).ceil() as usize;
        let j1 = to_index(at.y + disc, half.y, self.nz).floor() as usize;

        let mut touching: Option<(f32, f32)> = None;
        let mut lowest = f32::MAX;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let distance = (vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y) - at).length();
                if distance >= disc {
                    continue;
                }
                let idx = j * self.nx + i;
                let w = self.water[idx];
                if w > SEA_LEVEL && heights[idx] < w {
                    if distance < touch && touching.is_none_or(|(d, _)| distance < d) {
                        touching = Some((distance, w));
                    }
                } else if self.sea_inland(idx) > LAKE_COAST_MARGIN {
                    lowest = lowest.min(self.base[idx]);
                }
            }
        }
        match touching {
            Some((_, level)) => (lowest >= level - LAKE_SPILL).then_some(level),
            None => (lowest < f32::MAX).then(|| lowest.max(LAND_HEIGHT)),
        }
    }

    /// One dab of the lakes brush: sinks the ground toward the lake by up to `amount` meters, and
    /// sets the water level where that leaves it under water.
    pub(super) fn dig(&mut self, heights: &mut [f32], half: Vec2, at: Vec2, radius: f32, amount: f32) -> Option<Region> {
        let level = self.lake_level(heights, half, at, radius)?;
        let reach = (radius + LAKE_MAX_BANK) * (1.0 + LAKE_WANDER);
        let to_index = |v: f32, h: f32, n: usize| ((v + h) / CELL_SIZE).clamp(0.0, (n - 1) as f32);
        let i0 = to_index(at.x - reach, half.x, self.nx).ceil() as usize;
        let i1 = to_index(at.x + reach, half.x, self.nx).floor() as usize;
        let j0 = to_index(at.y - reach, half.y, self.nz).ceil() as usize;
        let j1 = to_index(at.y + reach, half.y, self.nz).floor() as usize;
        if i0 > i1 || j0 > j1 {
            return None;
        }

        let mut changed: Option<Region> = None;
        let mark = |changed: &mut Option<Region>, i: usize, j: usize| {
            let sample = Region { i0: i, j0: j, i1: i, j1: j };
            *changed = Some(changed.map_or(sample, |c| c.union(sample)));
        };
        for j in j0..=j1 {
            let z = j as f32 * CELL_SIZE - half.y;
            for i in i0..=i1 {
                let x = i as f32 * CELL_SIZE - half.x;
                let distance = (vec2(x, z) - at).length();
                if distance >= reach {
                    continue;
                }
                let idx = j * self.nx + i;
                // Ground that holds another lake's water is left to that lake.
                if self.water[idx] > SEA_LEVEL && self.water[idx] != level {
                    continue;
                }
                let current = heights[idx];
                let target = self.lake_height(idx, (x, z), distance, radius, level);
                if target >= current {
                    continue;
                }
                let dug = (current - amount).max(target);
                heights[idx] = dug;
                mark(&mut changed, i, j);
                if dug < level {
                    self.water[idx] = level;
                    // The ground around the water shares its level, so the shore is drawn where
                    // the ground meets the water and not where it meets the sea's.
                    for nj in j.saturating_sub(1)..=(j + 1).min(self.nz - 1) {
                        for ni in i.saturating_sub(1)..=(i + 1).min(self.nx - 1) {
                            let n = nj * self.nx + ni;
                            if self.water[n] == SEA_LEVEL && heights[n] >= level {
                                self.water[n] = level;
                                mark(&mut changed, ni, nj);
                            }
                        }
                    }
                }
            }
        }
        if let Some(region) = changed {
            self.touched = Some(self.touched.map_or(region, |t| t.union(region)));
        }
        changed
    }
}

/// Digs `count` lakes about `size` meters across (each between half and all of that) into
/// randomly chosen land far enough from the coast, of the `nx` by `nz` samples in `heights`
/// (centered on the origin by `half`), whose water stands at the levels in `water`. Each lake
/// takes the level of any lake it touches, or else of the lowest ground it covers, and the
/// flattest of a few spots is used. Returns the new heights and water levels and how many lakes
/// fit, or `None` if none did.
#[allow(clippy::too_many_arguments)]
pub fn scatter_lakes(
    heights: &[f32],
    water: &[f32],
    nx: usize,
    nz: usize,
    half: Vec2,
    seed: u32,
    count: u32,
    size: f32,
) -> Option<(Vec<f32>, Vec<f32>, usize)> {
    let mut stroke = HillStroke::begin_on(heights, water, nx, nz, seed, false, Peak::Lakes);
    let mut rng = Rng::new(seed ^ 0x51ED_270B);
    let mut out = heights.to_vec();
    let mut added = 0;
    for _ in 0..count {
        let radius = size * 0.5 * rng.range(0.5, 1.0);
        // Dry land, with room for the whole lake and its banks before the sea.
        let room = radius * (1.0 + LAKE_WANDER) + 2.0 * LAKE_COAST_MARGIN;
        let mut spots: Vec<(f32, Vec2)> = Vec::new();
        for _ in 0..LAKE_TRIES {
            if spots.len() >= LAKE_CANDIDATES {
                break;
            }
            let i = rng.range(0.0, (nx - 1) as f32) as usize;
            let j = rng.range(0.0, (nz - 1) as f32) as usize;
            let idx = j * nx + i;
            if out[idx] >= stroke.water[idx].max(LAND_HEIGHT) && stroke.sea_inland(idx) >= room {
                let at = vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                spots.push((stroke.relief(&out, half, at, radius * 2.0), at));
            }
        }
        let Some(&(_, at)) = spots.iter().min_by(|a, b| a.0.total_cmp(&b.0)) else { continue };
        // Start fresh, and give the dab all the time it needs to reach full depth.
        stroke.last = None;
        for _ in 0..3 {
            stroke.paint(&mut out, half, at, radius, 2.0);
        }
        added += 1;
    }
    (added > 0 && (out != heights || stroke.water != water)).then_some((out, stroke.water, added))
}
