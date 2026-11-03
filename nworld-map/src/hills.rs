//! The hills, mountains and lakes brushes: grow rolling 10 to 25 m hills, or jagged 26 to 50 m
//! mountains with foothills around them, on land, dig lakes into it, and slowly wear ground back
//! down. Also places whole mountain ranges and lakes at random.

use glam::*;

use crate::gen_common::{ExistingLand, LAND_HEIGHT};
use crate::noise::{smoothstep, Simplex};
use crate::world::{Region, CELL_SIZE, SEA_LEVEL};

mod lakes;
mod range;
#[cfg(test)]
mod tests;

use lakes::LAKE_COAST_MARGIN;
pub use lakes::scatter_lakes;
pub use range::mountain_range;

pub const MIN_HILL: f32 = 10.0;
pub const MAX_HILL: f32 = 25.0;
pub const MIN_MOUNTAIN: f32 = 26.0;
pub const MAX_MOUNTAIN: f32 = 50.0;
/// How far the hill noise is stretched toward the ends of the range.
const HILL_CONTRAST: f32 = 2.4;
/// How narrow mountain ridges are: higher drops the ground away from a ridge line faster.
const MOUNTAIN_RIDGE_SHARPNESS: f32 = 2.0;
/// How narrow the secondary ridges that branch off the main ones are.
const MOUNTAIN_SPUR_SHARPNESS: f32 = 2.2;
/// How far below the bottom of the mountain range, as a share of its height, the ground between
/// peaks can fall.
const MOUNTAIN_VALLEY: f32 = 0.45;
/// How hard mountain heights are stretched toward the top of their range.
const MOUNTAIN_STRETCH: f32 = 2.5;
/// How much the crags roughen mountain tops, as a share of their height range.
const MOUNTAIN_CRAGS: f32 = 0.12;
/// Wavelengths in meters of the mountains' features: the massifs that group tall peaks
/// together, the main ridges, the spurs off them and the crags.
const MASSIF_SIZE: f32 = 380.0;
const RIDGE_SIZE: f32 = 150.0;
const SPUR_SIZE: f32 = 62.0;
const CRAG_SIZE: f32 = 22.0;
/// A mountain stroke also grows hills this many times as far out as the brush reaches.
const FOOTHILL_REACH: f32 = 2.5;
/// How strongly each smoothing pass of a finished stroke's rim pulls toward the neighbors.
const SMOOTH_STRENGTH: f32 = 0.8;

/// What a raising brush grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Peak {
    Hills,
    Mountains,
    /// Digs lakes: ground sunk below the level of the water, which is that of the lake the brush
    /// touches, or else the lowest ground under it.
    Lakes,
}

impl Peak {
    /// The lowest and highest the tops reach, in meters.
    pub fn range(self) -> (f32, f32) {
        match self {
            Peak::Hills => (MIN_HILL, MAX_HILL),
            Peak::Mountains => (MIN_MOUNTAIN, MAX_MOUNTAIN),
            Peak::Lakes => (0.0, 0.0),
        }
    }

    /// How fast the brush raises ground at its center, in meters per second.
    fn rate(self) -> f32 {
        match self {
            Peak::Hills => 5.0,
            Peak::Mountains => 20.0,
            // Digging: how fast the ground sinks toward the lake.
            Peak::Lakes => 10.0,
        }
    }

    /// The brush is at full strength inside this share of its radius and eases off to nothing
    /// at the rim. Mountains have a short rim, so their sides rise steeply from the ground
    /// around them.
    fn core(self) -> f32 {
        match self {
            Peak::Hills => 0.3,
            Peak::Mountains => 0.85,
            Peak::Lakes => 1.0,
        }
    }

    /// How many times the rim of a finished stroke is smoothed. Hills melt into the lowlands;
    /// mountains keep their steep sides, and the foothills around them are already gentle.
    fn smooth_passes(self) -> usize {
        match self {
            Peak::Hills => 4,
            Peak::Mountains | Peak::Lakes => 0,
        }
    }

    /// How fast the erasing brush lowers ground at its center, in meters per second. It never
    /// takes land below [`LAND_HEIGHT`].
    fn erase_rate(self) -> f32 {
        match self {
            Peak::Hills => 8.0,
            Peak::Mountains => 16.0,
            Peak::Lakes => 0.0,
        }
    }

    /// They fade out over this many meters inland from the shore, so they never end in a
    /// cliff at the water's edge.
    fn shore_fade(self) -> f32 {
        match self {
            Peak::Hills => 120.0,
            Peak::Mountains => 24.0,
            Peak::Lakes => LAKE_COAST_MARGIN,
        }
    }

}

/// One press-drag-release of the hills or mountains brush, painted onto the map's heights as it
/// goes.
///
/// Hills and mountains grow while the brush is held over them and stop at a height that depends on where the
/// sample sits under the brush and in the stroke's starting terrain, so holding on for longer
/// never turns the brush's edge into a cliff.
pub struct HillStroke {
    /// The heights when the stroke began.
    base: Vec<f32>,
    /// The strongest brush weight (0 to 1) that has reached each sample.
    coverage: Vec<f32>,
    /// The water levels when the stroke began.
    base_water: Vec<f32>,
    /// The water levels now: those of the lakes the stroke has made are added to the map's.
    water: Vec<f32>,
    /// Distance to the nearest water, if the map has any.
    shore: Option<ExistingLand>,
    /// Distance to the nearest sea, not counting lakes. Only the lakes brush needs it, and only
    /// on a map that has lakes already; otherwise `shore` is the same.
    sea: Option<ExistingLand>,
    longest: f32,
    noise: Simplex,
    nx: usize,
    nz: usize,
    last: Option<Vec2>,
    touched: Option<Region>,
    /// Whether the stroke lowers the ground instead of raising peaks.
    erase: bool,
    /// What a raising stroke grows; an erasing one wears ground down faster for mountains.
    peak: Peak,
}

impl HillStroke {
    /// Starts a stroke on `heights` (`nx` by `nz` samples) that raises `peak`s, or with `erase`
    /// wears the ground down. The same `seed` always grows the same ones in the same place.
    pub fn begin(heights: &[f32], nx: usize, nz: usize, seed: u32, erase: bool, peak: Peak) -> Self {
        Self::begin_on(heights, &vec![SEA_LEVEL; heights.len()], nx, nz, seed, erase, peak)
    }

    /// Like [`begin`](Self::begin), on a map whose water stands at the levels in `water`.
    pub fn begin_on(heights: &[f32], water: &[f32], nx: usize, nz: usize, seed: u32, erase: bool, peak: Peak) -> Self {
        let longest = (nx - 1).max(nz - 1) as f32;
        // Mark water as "land" so the distance transform measures how far inland each sample is.
        let mark = |wet: &dyn Fn(f32, f32) -> bool| -> Vec<f32> {
            heights.iter().zip(water).map(|(&h, &w)| if wet(h, w) { 0.0 } else { -1.0 }).collect()
        };
        let flooded = mark(&|h, w| h < w);
        let has_lakes = water.iter().any(|&w| w > SEA_LEVEL);
        let sea = (peak == Peak::Lakes && has_lakes)
            .then(|| ExistingLand::new(&mark(&|h, w| h < w && w <= SEA_LEVEL), nx, nz, longest))
            .flatten();
        Self {
            base: heights.to_vec(),
            coverage: vec![0.0; heights.len()],
            base_water: water.to_vec(),
            water: water.to_vec(),
            shore: ExistingLand::new(&flooded, nx, nz, longest),
            sea,
            longest,
            noise: Simplex::new(seed ^ 0x5A17_E3D9),
            nx,
            nz,
            last: None,
            touched: None,
            erase,
            peak,
        }
    }

    /// Whether the stroke lowers the ground instead of raising peaks.
    pub fn erases(&self) -> bool {
        self.erase
    }

    /// The heights the stroke began with.
    pub fn base(&self) -> &[f32] {
        &self.base
    }

    /// The water levels the stroke began with.
    pub fn base_water(&self) -> &[f32] {
        &self.base_water
    }

    /// The water levels as the stroke has left them.
    pub fn water(&self) -> &[f32] {
        &self.water
    }

    /// Meters from the sample to the nearest water.
    fn inland(&self, idx: usize) -> f32 {
        self.meters(&self.shore, idx)
    }

    /// Meters from the sample to the nearest sea, not counting lakes.
    fn sea_inland(&self, idx: usize) -> f32 {
        if self.sea.is_some() {
            self.meters(&self.sea, idx)
        } else {
            self.inland(idx)
        }
    }

    fn meters(&self, field: &Option<ExistingLand>, idx: usize) -> f32 {
        match field {
            Some(field) => (field.dist[idx] * self.longest - 1.0).max(0.0) * CELL_SIZE,
            None => f32::INFINITY,
        }
    }

    /// The height `peak`s reach at a spot where the brush is at full strength.
    ///
    /// Hills roll like the lowlands do but over longer distances. Mountains run along sharp
    /// ridge lines and change height every few dozen meters, so they come out jagged.
    fn target(&self, peak: Peak, x: f32, z: f32) -> f32 {
        let (low, high) = peak.range();
        let unit = match peak {
            Peak::Hills => {
                let broad = self.noise.fbm(x / 260.0, z / 260.0, 3, 0.5);
                let detail = self.noise.fbm(x / 140.0 + 31.0, z / 140.0 - 47.0, 2, 0.45);
                // Noise rarely strays far from its middle, so stretch it until the tallest hills
                // reach the top of the range and the lowest reach the bottom, flattening there.
                let mix = (HILL_CONTRAST * (0.6 * broad + 0.4 * detail)).clamp(-1.0, 1.0);
                0.5 + 0.5 * mix
            }
            Peak::Mountains => {
                // Ridge lines run where the noise crosses zero, with the ground falling away
                // steeply to either side. Spurs branch off the main ridges, and the highest
                // summits are where the two meet, within a massif that is tall; elsewhere the
                // peaks are lower and the ground between them drops to saddles and valleys.
                // Rough noise on top breaks everything into crags.
                let massif = self.noise.fbm(x / MASSIF_SIZE - 17.0, z / MASSIF_SIZE + 9.0, 3, 0.5);
                let height = smoothstep(-0.30, 0.45, massif);
                let ridge = (1.0 - self.noise.fbm(x / RIDGE_SIZE + 7.0, z / RIDGE_SIZE - 3.0, 4, 0.55).abs() * MOUNTAIN_RIDGE_SHARPNESS)
                    .clamp(0.0, 1.0);
                let spur = (1.0 - self.noise.fbm(x / SPUR_SIZE - 5.0, z / SPUR_SIZE + 13.0, 3, 0.5).abs() * MOUNTAIN_SPUR_SHARPNESS)
                    .clamp(0.0, 1.0);
                let summit = ridge * ridge * (0.55 + 0.45 * spur * spur);
                let crags = self.noise.fbm(x / CRAG_SIZE + 11.0, z / CRAG_SIZE - 23.0, 2, 0.5);
                let raw = 0.08 + 0.92 * summit * (0.3 + 0.7 * height) + MOUNTAIN_CRAGS * crags;
                // Noise rarely strays far from its middle, so stretch it, easing off toward the
                // top so the tallest summits come to a rounded point instead of a flat cap.
                let stretched = (MOUNTAIN_STRETCH * raw.max(0.0)).tanh() / MOUNTAIN_STRETCH.tanh();
                // The lowest ground between peaks is below the foot of the mountains, down to
                // the height of hills, so valleys show green between the rock.
                stretched * (1.0 + MOUNTAIN_VALLEY) - MOUNTAIN_VALLEY
            }
            Peak::Lakes => 0.0,
        };
        match peak {
            Peak::Mountains => (low + (high - low) * unit.clamp(-MOUNTAIN_VALLEY, 1.0)).clamp(MIN_HILL, high),
            _ => (low + (high - low) * unit.clamp(0.0, 1.0)).clamp(low, high),
        }
    }

    /// Raises the sample at `idx` (at world `x`, `z`) by up to `amount` meters toward the
    /// ceiling a `peak` brush of this `weight` allows there, and returns its new height. The
    /// ceiling eases from the stroke's starting ground up to the peak's height as the brush
    /// and the distance from the shore allow, and never goes past the top of the range.
    fn grow(&self, peak: Peak, idx: usize, (x, z): (f32, f32), weight: f32, amount: f32, current: f32) -> f32 {
        let base = self.base[idx];
        let grown = weight * smoothstep(0.0, peak.shore_fade(), self.inland(idx));
        let top = peak.range().1;
        let ceiling = base.max(base + (self.target(peak, x, z) - base) * grown).min(base.max(top));
        if current >= ceiling {
            current
        } else {
            (current + amount * weight).min(ceiling)
        }
    }

    /// Paints with a brush of `radius` meters centered on `center` (world x and z, with the map
    /// centered on the origin), along the line from wherever the last call painted. Changes the
    /// heights in place and returns the part of the map that changed, if any.
    ///
    /// `dt` is the time in seconds since the last call. Each call raises hills or lowers the
    /// ground a little more, even with the brush held still.
    pub fn paint(&mut self, heights: &mut [f32], half: Vec2, center: Vec2, radius: f32, dt: f32) -> Option<Region> {
        let from = self.last.unwrap_or(center);
        self.last = Some(center);

        let spacing = (radius * 0.25).max(CELL_SIZE);
        let dabs = ((center - from).length() / spacing).ceil().max(1.0) as usize;
        // Spread a frame's change over the dabs along the path. Foothills grow more slowly
        // than the mountains they surround.
        let rate = if self.erase { self.peak.erase_rate() } else { self.peak.rate() };
        let foothill_rate = Peak::Mountains.rate() * 0.5;
        let amounts = (rate * dt / dabs as f32, foothill_rate * dt / dabs as f32);
        let mut changed: Option<Region> = None;
        for k in 1..=dabs {
            let at = from.lerp(center, k as f32 / dabs as f32);
            if let Some(region) = self.dab(heights, half, at, radius, amounts) {
                changed = Some(changed.map_or(region, |c| c.union(region)));
            }
        }
        if let Some(region) = changed {
            self.touched = Some(self.touched.map_or(region, |t| t.union(region)));
        }
        changed
    }

    /// One dab of the brush at `at`. `amounts` is how many meters to raise (or, erasing, lower)
    /// the ground at the brush's center, and how many to raise foothills around mountains.
    fn dab(&mut self, heights: &mut [f32], half: Vec2, at: Vec2, radius: f32, amounts: (f32, f32)) -> Option<Region> {
        if self.peak == Peak::Lakes {
            // Lakes only dig; there is nothing for the lowering brush to do.
            return if self.erase { None } else { self.dig(heights, half, at, radius, amounts.0) };
        }
        // Mountains also grow foothills out beyond the brush.
        let foothills = !self.erase && self.peak == Peak::Mountains;
        let reach = if foothills { radius * FOOTHILL_REACH } else { radius };

        // Samples sit at (index * CELL_SIZE - half); find the ones the brush can reach.
        let to_index = |v: f32, h: f32, n: usize| ((v + h) / CELL_SIZE).clamp(0.0, (n - 1) as f32);
        let i0 = to_index(at.x - reach, half.x, self.nx).ceil() as usize;
        let i1 = to_index(at.x + reach, half.x, self.nx).floor() as usize;
        let j0 = to_index(at.y - reach, half.y, self.nz).ceil() as usize;
        let j1 = to_index(at.y + reach, half.y, self.nz).floor() as usize;
        if i0 > i1 || j0 > j1 {
            return None;
        }

        let mut changed: Option<Region> = None;
        for j in j0..=j1 {
            let z = j as f32 * CELL_SIZE - half.y;
            for i in i0..=i1 {
                let x = i as f32 * CELL_SIZE - half.x;
                let distance = (vec2(x, z) - at).length();
                if distance >= reach {
                    continue;
                }
                let idx = j * self.nx + i;
                let current = heights[idx];
                let t = distance / radius;
                let height = if self.erase {
                    let weight = 1.0 - smoothstep(self.peak.core(), 1.0, t);
                    self.coverage[idx] = self.coverage[idx].max(weight);
                    // Wear land down toward the lowest ground; water and ground already at that
                    // level stay as they are.
                    if current <= LAND_HEIGHT {
                        continue;
                    }
                    (current - amounts.0 * weight).max(LAND_HEIGHT)
                } else if foothills {
                    let hill_weight = 1.0 - smoothstep(Peak::Hills.core(), 1.0, distance / reach);
                    self.coverage[idx] = self.coverage[idx].max(hill_weight);
                    let mut height = self.grow(Peak::Hills, idx, (x, z), hill_weight, amounts.1, current);
                    if t < 1.0 {
                        let weight = 1.0 - smoothstep(Peak::Mountains.core(), 1.0, t);
                        height = height.max(self.grow(Peak::Mountains, idx, (x, z), weight, amounts.0, current));
                    }
                    height
                } else {
                    let weight = 1.0 - smoothstep(self.peak.core(), 1.0, t);
                    self.coverage[idx] = self.coverage[idx].max(weight);
                    self.grow(self.peak, idx, (x, z), weight, amounts.0, current)
                };
                if height != current {
                    heights[idx] = height;
                    let sample = Region { i0: i, j0: j, i1: i, j1: j };
                    changed = Some(changed.map_or(sample, |c| c.union(sample)));
                }
            }
        }
        changed
    }

    /// How much the ground varies under a disc of `reach` meters around `at`: the height of the
    /// highest sample above the lowest.
    fn relief(&self, heights: &[f32], half: Vec2, at: Vec2, reach: f32) -> f32 {
        let to_index = |v: f32, h: f32, n: usize| ((v + h) / CELL_SIZE).clamp(0.0, (n - 1) as f32);
        let i0 = to_index(at.x - reach, half.x, self.nx).ceil() as usize;
        let i1 = to_index(at.x + reach, half.x, self.nx).floor() as usize;
        let j0 = to_index(at.y - reach, half.y, self.nz).ceil() as usize;
        let j1 = to_index(at.y + reach, half.y, self.nz).floor() as usize;
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for j in j0..=j1 {
            for i in i0..=i1 {
                if (vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y) - at).length() < reach {
                    let h = heights[j * self.nx + i];
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
        }
        (hi - lo).max(0.0)
    }

    /// Ends the stroke by smoothing the rim of what was painted, where the hills ease down into
    /// the lowlands, so no step is left behind. Returns the part of the map that changed.
    pub fn finish(&mut self, heights: &mut [f32]) -> Option<Region> {
        let touched = self.touched?;
        let (i0, j0) = (touched.i0.saturating_sub(1), touched.j0.saturating_sub(1));
        let (i1, j1) = ((touched.i1 + 1).min(self.nx - 1), (touched.j1 + 1).min(self.nz - 1));
        let region = Region { i0, j0, i1, j1 };
        if self.peak.smooth_passes() == 0 {
            return Some(region);
        }

        // Only the rim moves: samples the brush touched but didn't reach at full strength, and
        // far enough inland that smoothing can't disturb the coast.
        let rim: Vec<usize> = (j0..=j1)
            .flat_map(|j| (i0..=i1).map(move |i| (i, j)))
            .map(|(i, j)| j * self.nx + i)
            .filter(|&idx| {
                self.coverage[idx] < 1.0
                    && self.base[idx] >= 0.0
                    && self.inland(idx) > 3.0 * CELL_SIZE
                    && (self.coverage[idx] > 0.0 || self.near_painted(idx))
            })
            .collect();

        for _ in 0..self.peak.smooth_passes() {
            for &idx in &rim {
                let old = heights[idx];
                let (i, j) = (idx % self.nx, idx / self.nx);
                let mut sum = 0.0;
                let mut count = 0.0;
                for dj in -1i32..=1 {
                    for di in -1i32..=1 {
                        let (ni, nj) = (i as i32 + di, j as i32 + dj);
                        if (di, dj) == (0, 0) || ni < 0 || nj < 0 || ni >= self.nx as i32 || nj >= self.nz as i32 {
                            continue;
                        }
                        sum += heights[nj as usize * self.nx + ni as usize];
                        count += 1.0;
                    }
                }
                let smoothed = old + (sum / count - old) * SMOOTH_STRENGTH;
                heights[idx] = if self.erase {
                    smoothed.min(self.base[idx]).max(LAND_HEIGHT.min(self.base[idx]))
                } else {
                    smoothed.max(self.base[idx])
                };
            }
        }
        Some(region)
    }

    /// Whether a neighbor of the sample was reached by the brush.
    fn near_painted(&self, idx: usize) -> bool {
        let (i, j) = (idx % self.nx, idx / self.nx);
        (j.saturating_sub(1)..=(j + 1).min(self.nz - 1))
            .any(|nj| (i.saturating_sub(1)..=(i + 1).min(self.nx - 1)).any(|ni| self.coverage[nj * self.nx + ni] > 0.0))
    }
}
