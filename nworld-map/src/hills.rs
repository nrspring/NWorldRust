//! The hills, mountains and lakes brushes: grow rolling 10 to 25 m hills, or jagged 26 to 50 m
//! mountains with foothills around them, on land, dig lakes into it, and slowly wear ground back
//! down. Also places whole mountain ranges and lakes at random.

use glam::*;

use crate::gen_common::{ExistingLand, Rng, LAND_HEIGHT};
use crate::noise::{smoothstep, Simplex};
use crate::world::{Region, CELL_SIZE, SEA_LEVEL};

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
/// Meters between the points a mountain range's spine is traced through.
const RANGE_STEP: f32 = 20.0;
/// A range stops this far from the water, so it never runs into the coast.
const RANGE_COAST_MARGIN: f32 = 60.0;
/// A lake's shoreline wanders by up to this share of its radius.
const LAKE_WANDER: f32 = 0.35;
/// Lakes stay this far from the sea, in meters, easing in over as far again, so they never
/// break through to it.
const LAKE_COAST_MARGIN: f32 = 30.0;
/// The longest a lake's banks run before meeting the ground around it, in meters.
const LAKE_MAX_BANK: f32 = 150.0;
/// Spots tried for each randomly placed lake.
const LAKE_TRIES: usize = 80;
/// Of the spots that fit, this many are compared and the flattest used, so a lake's surface
/// doesn't end up standing above lower ground beside it.
const LAKE_CANDIDATES: usize = 6;
/// A lake won't spread onto ground this many meters lower than its water.
const LAKE_SPILL: f32 = 1.0;
/// Spots tried as the start of a range; the one with the most room to run is used.
const RANGE_TRIES: usize = 24;
/// Times the brush walks along a range's spine, back and forth, to grow it fully.
const RANGE_PASSES: usize = 3;
/// Seconds of brush time at each point of the spine on each pass.
const RANGE_DT: f32 = 0.5;

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
    fn dig(&mut self, heights: &mut [f32], half: Vec2, at: Vec2, radius: f32, amount: f32) -> Option<Region> {
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

    /// Whether a neighbor of the sample was reached by the brush.
    fn near_painted(&self, idx: usize) -> bool {
        let (i, j) = (idx % self.nx, idx / self.nx);
        (j.saturating_sub(1)..=(j + 1).min(self.nz - 1))
            .any(|nj| (i.saturating_sub(1)..=(i + 1).min(self.nx - 1)).any(|ni| self.coverage[nj * self.nx + ni] > 0.0))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_common::LAND_HEIGHT;
    use crate::world::DEEP_OCEAN;

    /// A square island of lowland in the middle of open water.
    fn island(cells: usize, margin: usize) -> Vec<f32> {
        let n = cells + 1;
        let mut heights = vec![DEEP_OCEAN; n * n];
        for j in margin..=cells - margin {
            for i in margin..=cells - margin {
                heights[j * n + i] = LAND_HEIGHT;
            }
        }
        heights
    }

    fn painted(radius: f32) -> (Vec<f32>, Vec<f32>, usize) {
        let cells = 160;
        let n = cells + 1;
        let start = island(cells, 10);
        let mut heights = start.clone();
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let mut stroke = HillStroke::begin(&heights, n, n, 7, false, Peak::Hills);
        // Sweep back and forth long enough for the hills to finish growing.
        for k in 0..300 {
            let x = if k % 2 == 0 { -40.0 } else { 40.0 };
            stroke.paint(&mut heights, half, vec2(x, 0.0), radius, 0.1);
        }
        stroke.finish(&mut heights);
        (start, heights, n)
    }

    #[test]
    fn hills_reach_10_to_25_m_inland() {
        let (start, heights, n) = painted(200.0);
        let top = heights.iter().copied().fold(f32::MIN, f32::max);
        assert!(top <= MAX_HILL, "hills stay at or below 25 m, got {top}");
        let centre = (n / 2) * n + n / 2;
        assert!(heights[centre] >= MIN_HILL, "the middle of the brush is a hill");
        assert!(heights.iter().zip(&start).all(|(h, s)| h >= s), "painting never digs");
    }

    #[test]
    fn hill_heights_span_the_whole_range_and_stop_at_50() {
        let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Hills);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for j in 0..300 {
            for i in 0..300 {
                let t = stroke.target(Peak::Hills, i as f32 * 10.0, j as f32 * 10.0);
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        assert!(lo >= MIN_HILL && lo < MIN_HILL + 1.0, "lowest hill target was {lo}");
        assert!(hi <= MAX_HILL && hi > MAX_HILL - 0.5, "tallest hill target was {hi}");
    }

    #[test]
    fn hills_grow_slowly_while_the_brush_is_held() {
        let cells = 160;
        let n = cells + 1;
        let mut heights = island(cells, 10);
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let centre = (n / 2) * n + n / 2;
        let mut stroke = HillStroke::begin(&heights, n, n, 5, false, Peak::Hills);
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 100.0, 0.1);
        let first = heights[centre] - LAND_HEIGHT;
        assert!(first > 0.0 && first <= Peak::Hills.rate() * 0.1 + 1e-4, "one frame raised it by {first}");
        for _ in 0..100 {
            stroke.paint(&mut heights, half, vec2(0.0, 0.0), 100.0, 0.1);
        }
        assert!(heights[centre] >= MIN_HILL && heights[centre] <= MAX_HILL);
    }

    #[test]
    fn hills_vary_and_leave_water_and_coast_alone() {
        let (start, heights, _) = painted(200.0);
        let hills: Vec<f32> = heights.iter().copied().filter(|&h| h >= MIN_HILL).collect();
        let lo = hills.iter().copied().fold(f32::MAX, f32::min);
        let hi = hills.iter().copied().fold(f32::MIN, f32::max);
        assert!(hi - lo > 2.0, "hill tops should differ in height, got {lo} to {hi}");
        for (h, s) in heights.iter().zip(&start) {
            if *s < 0.0 {
                assert_eq!(h, s, "water is untouched");
            }
        }
        // The first land samples next to the water keep their lowland height.
        let n = 161;
        assert_eq!(heights[80 * n + 10], LAND_HEIGHT);
    }

    #[test]
    fn hill_edges_slope_instead_of_dropping_in_cliffs() {
        let (_, heights, n) = painted(200.0);
        let mut steepest: f32 = 0.0;
        for j in 0..n {
            for i in 0..n {
                let h = heights[j * n + i];
                if h < 0.0 {
                    continue;
                }
                for (di, dj) in [(1, 0), (0, 1)] {
                    if i + di < n && j + dj < n {
                        let other = heights[(j + dj) * n + i + di];
                        if other >= 0.0 {
                            steepest = steepest.max((h - other).abs() / CELL_SIZE);
                        }
                    }
                }
            }
        }
        assert!(steepest < 0.7, "steepest slope was {steepest}");
    }

    #[test]
    fn erasing_lowers_ground_slowly_down_to_the_minimum() {
        let (_, hilly, n) = painted(200.0);
        let half = Vec2::splat((n - 1) as f32 * CELL_SIZE * 0.5);
        let centre = (n / 2) * n + n / 2;
        let mut heights = hilly.clone();
        let mut stroke = HillStroke::begin(&heights, n, n, 7, true, Peak::Hills);
        assert!(stroke.erases());

        // One frame takes only a little off, even with the brush held still.
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 400.0, 0.1);
        let lowered = hilly[centre] - heights[centre];
        assert!(lowered > 0.0 && lowered <= Peak::Hills.erase_rate() * 0.1 + 1e-4, "lowered {lowered}");

        // Holding on keeps lowering it, but never below the minimum.
        for _ in 0..400 {
            stroke.paint(&mut heights, half, vec2(0.0, 0.0), 400.0, 0.1);
        }
        stroke.finish(&mut heights);
        let top = heights.iter().copied().fold(f32::MIN, f32::max);
        assert!(top <= LAND_HEIGHT + 0.01, "everything under the brush is down to {top}");
        for (idx, (h, before)) in heights.iter().zip(&hilly).enumerate() {
            assert!(h <= before, "erasing never raises ground");
            assert!(*h >= LAND_HEIGHT.min(*before), "erasing stops at the minimum, got {h} at {idx}");
            if *before < 0.0 {
                assert_eq!(h, before, "water is untouched at {idx}");
            }
        }
    }

    #[test]
    fn repainting_never_lowers_hills() {
        let cells = 160;
        let n = cells + 1;
        let mut heights = island(cells, 10);
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let mut stroke = HillStroke::begin(&heights, n, n, 3, false, Peak::Hills);
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
        let once = heights.clone();
        stroke.paint(&mut heights, half, vec2(10.0, 0.0), 150.0, 0.1);
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
        let twice = heights.clone();
        assert!(once.iter().zip(&twice).all(|(a, b)| b >= a));
    }

    #[test]
    fn mountains_climb_to_50_m_and_no_further() {
        let cells = 160;
        let n = cells + 1;
        let mut heights = island(cells, 10);
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let centre = (n / 2) * n + n / 2;
        let mut stroke = HillStroke::begin(&heights, n, n, 9, false, Peak::Mountains);
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
        let first = heights[centre] - LAND_HEIGHT;
        assert!(first > Peak::Hills.rate() * 0.1, "mountains grow faster than hills, got {first}");
        assert!(first <= Peak::Mountains.rate() * 0.1 + 1e-4);
        for _ in 0..600 {
            stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
        }
        stroke.finish(&mut heights);
        let top = heights.iter().copied().fold(f32::MIN, f32::max);
        assert!(top <= MAX_MOUNTAIN, "mountains stop at 50 m, got {top}");
        assert!(heights[centre] >= MIN_MOUNTAIN, "the middle is a mountain, got {}", heights[centre]);
        for (h, s) in heights.iter().zip(island(cells, 10)) {
            if s < 0.0 {
                assert_eq!(*h, s, "water is untouched");
            }
        }
    }

    #[test]
    fn a_mountain_range_is_a_long_chain_of_mountains_on_land() {
        let cells = 320;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let start = island(cells, 30);
        let range = mountain_range(&start, n, n, half, 5, 600.0, 100.0).expect("there is room for a range");
        assert!(range.iter().all(|&h| h <= MAX_MOUNTAIN + 0.01), "ranges stop at the top of the range");
        assert!(range.iter().zip(&start).all(|(&r, &s)| r >= s), "a range only raises ground");
        for (idx, &h) in start.iter().enumerate() {
            if h < 0.0 {
                assert_eq!(range[idx], h, "water is left alone");
            }
        }
        let peaks: Vec<Vec2> = (0..n * n)
            .filter(|&idx| range[idx] >= MIN_MOUNTAIN)
            .map(|idx| vec2((idx % n) as f32 * CELL_SIZE, (idx / n) as f32 * CELL_SIZE))
            .collect();
        assert!(peaks.len() > 100, "there should be mountains, got {} samples", peaks.len());
        let reach = peaks.iter().flat_map(|a| peaks.iter().map(move |b| (*a - *b).length())).fold(0.0, f32::max);
        assert!(reach > 300.0, "the mountains should run along a line, spanning {reach} m");
    }

    #[test]
    fn mountain_ranges_vary_with_the_seed_and_repeat_with_the_same_one() {
        let cells = 320;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let start = island(cells, 30);
        let a = mountain_range(&start, n, n, half, 5, 600.0, 100.0);
        assert_eq!(a, mountain_range(&start, n, n, half, 5, 600.0, 100.0));
        assert_ne!(a, mountain_range(&start, n, n, half, 6, 600.0, 100.0));
    }

    #[test]
    fn no_mountain_range_fits_in_open_water_or_on_a_sliver_of_land() {
        let cells = 160;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        assert!(mountain_range(&vec![DEEP_OCEAN; n * n], n, n, half, 1, 600.0, 100.0).is_none());
        assert!(mountain_range(&island(cells, 70), n, n, half, 1, 600.0, 100.0).is_none());
    }

    /// Digs a lake of `radius` at the middle of a 160-cell island of `ground` meters and returns
    /// the map as it began, as it ended, its water levels and its width.
    fn lake_on(ground: f32, radius: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>, usize) {
        let cells = 160;
        let n = cells + 1;
        let mut start = island(cells, 10);
        start.iter_mut().filter(|h| **h > 0.0).for_each(|h| *h = ground);
        let (heights, water) = dig_lake(&start, &vec![0.0; n * n], vec2(0.0, 0.0), radius);
        (start, heights, water, n)
    }

    /// Holds the lakes brush of `radius` meters at `at` on a 160-cell map, long enough to dig a
    /// lake, and returns the new heights and water levels.
    fn dig_lake(heights: &[f32], water: &[f32], at: Vec2, radius: f32) -> (Vec<f32>, Vec<f32>) {
        let n = 161;
        let half = Vec2::splat(160.0 * CELL_SIZE * 0.5);
        let mut heights = heights.to_vec();
        let mut stroke = HillStroke::begin_on(&heights, water, n, n, 3, false, Peak::Lakes);
        for _ in 0..200 {
            stroke.paint(&mut heights, half, at, radius, 0.1);
        }
        stroke.finish(&mut heights);
        (heights, stroke.water().to_vec())
    }

    /// An island whose ground climbs `slope` meters per meter eastward from `ground` at the middle.
    fn tilted_island(ground: f32, slope: f32) -> Vec<f32> {
        let mut heights = island(160, 10);
        for (idx, h) in heights.iter_mut().enumerate() {
            if *h > 0.0 {
                *h = ground + slope * ((idx % 161) as f32 * CELL_SIZE - 320.0);
            }
        }
        heights
    }

    /// The x of the centers of the samples under water in a map, in meters.
    fn flooded_xs(heights: &[f32], water: &[f32]) -> Vec<f32> {
        (0..heights.len())
            .filter(|&idx| heights[idx] < water[idx] && water[idx] > 0.0)
            .map(|idx| (idx % 161) as f32 * CELL_SIZE - 320.0)
            .collect()
    }

    #[test]
    fn a_lake_is_a_pool_at_the_level_of_the_ground_with_gentle_banks() {
        for ground in [3.0, 12.0, 30.0] {
            let (start, heights, water, n) = lake_on(ground, 40.0);
            let centre = (n / 2) * n + n / 2;
            assert_eq!(water[centre], ground, "the lake lies at the level of the flat ground around it");
            assert!(heights[centre] < ground - 1.0, "the middle is under water, got {}", heights[centre]);
            let pool = (0..heights.len()).filter(|&i| heights[i] < water[i] && water[i] > 0.0).count();
            assert!(pool > 20, "a lake of {pool} samples is too small");
            assert!(heights.iter().zip(&start).all(|(h, s)| h <= s), "lakes only lower ground");
            assert!(
                (0..heights.len()).all(|i| water[i] == 0.0 || water[i] == ground),
                "all of the water stands at the one level"
            );
            // Banks ease down: nothing near a cliff between neighbors.
            let mut steepest: f32 = 0.0;
            for j in 0..n - 1 {
                for i in 0..n - 1 {
                    // The island's own coast, where it drops into the deep sea, isn't a bank.
                    let h = heights[j * n + i];
                    for other in [heights[j * n + i + 1], heights[(j + 1) * n + i]] {
                        if h >= -30.0 && other >= -30.0 {
                            steepest = steepest.max((h - other).abs() / CELL_SIZE);
                        }
                    }
                }
            }
            assert!(steepest < 1.0, "banks over {ground} m ground are too steep: {steepest}");
        }
    }

    #[test]
    fn lakes_stay_clear_of_the_sea_and_only_the_lakes_brush_digs_them() {
        let cells = 160;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let start = island(cells, 10);
        // Right at the shore.
        let mut heights = start.clone();
        let mut stroke = HillStroke::begin(&heights, n, n, 3, false, Peak::Lakes);
        for _ in 0..100 {
            stroke.paint(&mut heights, half, vec2(-half.x + 10.0 * CELL_SIZE, 0.0), 40.0, 0.1);
        }
        for (idx, &h) in heights.iter().enumerate() {
            let (i, j) = (idx % n, idx / n);
            let near_coast = i < 10 + 6 || j < 10 + 6 || i > cells - 16 || j > cells - 16;
            if near_coast && start[idx] >= 0.0 {
                assert!(h >= 0.0, "the sea shouldn't flood in at sample {i},{j}: {h}");
            }
        }
        // Erasing with the lakes brush changes nothing.
        let mut erased = start.clone();
        let mut stroke = HillStroke::begin(&erased, n, n, 3, true, Peak::Lakes);
        stroke.paint(&mut erased, half, vec2(0.0, 0.0), 40.0, 1.0);
        assert_eq!(erased, start);
    }

    #[test]
    fn random_lakes_land_on_dry_ground_away_from_the_sea() {
        let cells = 320;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let start = island(cells, 30);
        let dry = vec![0.0; n * n];
        let (heights, water, added) = scatter_lakes(&start, &dry, n, n, half, 5, 4, 120.0).expect("there is room for lakes");
        assert!(added >= 1 && added <= 4);
        assert!(
            (0..n * n).any(|i| heights[i] < water[i] && water[i] > 0.0 && start[i] >= 0.0),
            "lakes were dug"
        );
        assert!(heights.iter().zip(&start).all(|(h, s)| h <= s));
        for (idx, &h) in start.iter().enumerate() {
            if h < 0.0 {
                assert_eq!(heights[idx], h, "the sea is left alone");
                assert_eq!(water[idx], 0.0, "and so is its water");
            }
        }
        // The same seed repeats, another differs, and open water has no room.
        assert_eq!(Some(&heights), scatter_lakes(&start, &dry, n, n, half, 5, 4, 120.0).as_ref().map(|r| &r.0));
        assert_ne!(Some(&heights), scatter_lakes(&start, &dry, n, n, half, 6, 4, 120.0).as_ref().map(|r| &r.0));
        assert!(scatter_lakes(&vec![DEEP_OCEAN; n * n], &dry, n, n, half, 5, 4, 120.0).is_none());
    }

    #[test]
    fn a_lake_on_a_slope_sits_at_the_lowest_ground_under_the_brush() {
        let start = tilted_island(12.0, 0.03);
        let (heights, water) = dig_lake(&start, &vec![0.0; 161 * 161], vec2(0.0, 0.0), 30.0);
        let centre = 80 * 161 + 80;
        // The shoreline can wander as far as 1.35 radii, plus a little, to the west: where the
        // ground is 12 - 0.03 * 48.5 = 10.5 m.
        let level = water[centre];
        assert!((10.2..10.9).contains(&level), "the lake's level is {level}");
        let xs = flooded_xs(&heights, &water);
        assert!(!xs.is_empty());
        for idx in 0..heights.len() {
            if water[idx] > 0.0 {
                assert_eq!(water[idx], level, "one lake, one level");
            }
        }
        // Far from the lake the water is still the sea's.
        assert_eq!(water[10 * 161 + 20], 0.0);
        assert_eq!(heights.iter().filter(|&&h| h < 0.0).count(), start.iter().filter(|&&h| h < 0.0).count());
    }

    #[test]
    fn a_stroke_that_touches_a_lake_matches_its_level() {
        let start = tilted_island(12.0, 0.03);
        let dry = vec![0.0; 161 * 161];
        let (once, water) = dig_lake(&start, &dry, vec2(-100.0, 0.0), 40.0);
        let level = water[80 * 161 + 80 - 25];
        assert!((6.5..8.0).contains(&level), "the first lake's level is {level}");
        let east_edge = flooded_xs(&once, &water).into_iter().fold(f32::MIN, f32::max);

        // A second stroke overlapping the first would, on its own, find higher ground to sit on.
        let (twice, water) = dig_lake(&once, &water, vec2(-40.0, 0.0), 40.0);
        let levels: Vec<f32> = (0..twice.len()).filter(|&i| water[i] > 0.0).map(|i| water[i]).collect();
        assert!(levels.iter().all(|&l| l == level), "the lakes joined up at one level");
        let new_edge = flooded_xs(&twice, &water).into_iter().fold(f32::MIN, f32::max);
        assert!(new_edge > east_edge + 20.0, "the lake grew from {east_edge} to {new_edge}");
    }

    #[test]
    fn a_lake_does_not_spread_onto_ground_lower_than_its_water() {
        let start = tilted_island(12.0, 0.03);
        let dry = vec![0.0; 161 * 161];
        let (once, water) = dig_lake(&start, &dry, vec2(100.0, 0.0), 40.0);
        let level = water[80 * 161 + 80 + 25];
        assert!(level > 12.5, "the first lake's level is {level}");
        // Downhill from the lake the ground lies lower than its water, so the brush does nothing.
        let (twice, water_after) = dig_lake(&once, &water, vec2(20.0, 0.0), 40.0);
        assert_eq!(twice, once);
        assert_eq!(water_after, water);
    }


    #[test]
    fn mountains_rise_steeply_where_hills_do_not() {
        let cells = 160;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let steepest = |peak: Peak| {
            let mut heights = island(cells, 10);
            let mut stroke = HillStroke::begin(&heights, n, n, 9, false, peak);
            for _ in 0..600 {
                stroke.paint(&mut heights, half, vec2(0.0, 0.0), 60.0, 0.1);
            }
            stroke.finish(&mut heights);
            let mut steepest: f32 = 0.0;
            for j in 0..n - 1 {
                for i in 0..n - 1 {
                    let h = heights[j * n + i];
                    if h < 0.0 {
                        continue;
                    }
                    for other in [heights[j * n + i + 1], heights[(j + 1) * n + i]] {
                        if other >= 0.0 {
                            steepest = steepest.max((h - other).abs() / CELL_SIZE);
                        }
                    }
                }
            }
            steepest
        };
        let (hills, mountains) = (steepest(Peak::Hills), steepest(Peak::Mountains));
        assert!(mountains > 2.0, "mountain sides should be steep, got {mountains}");
        assert!(mountains > hills * 2.0, "mountains {mountains} vs hills {hills}");
    }

    #[test]
    fn mountains_are_ringed_by_foothills_that_ease_into_the_lowlands() {
        let cells = 200;
        let n = cells + 1;
        let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
        let mut heights = island(cells, 10);
        let mut stroke = HillStroke::begin(&heights, n, n, 9, false, Peak::Mountains);
        for _ in 0..600 {
            stroke.paint(&mut heights, half, vec2(0.0, 0.0), 60.0, 0.1);
        }
        stroke.finish(&mut heights);
        let (mut ring, mut far) = (Vec::new(), Vec::new());
        for j in 0..n {
            for i in 0..n {
                let d = (vec2(i as f32 * CELL_SIZE, j as f32 * CELL_SIZE) - half).length();
                if d > 66.0 && d < 100.0 {
                    ring.push(heights[j * n + i]);
                } else if d > 190.0 && heights[j * n + i] >= 0.0 {
                    far.push(heights[j * n + i]);
                }
            }
        }
        let mean = ring.iter().sum::<f32>() / ring.len() as f32;
        assert!(ring.iter().all(|&h| h <= MAX_HILL + 0.01), "foothills are hills, not mountains");
        assert!(mean >= MIN_HILL, "the ring around the mountain stands at hill height, got {mean}");
        assert!(far.iter().all(|&h| h <= 10.0), "beyond the foothills the ground is untouched");
    }

    #[test]
    fn mountain_tops_vary_from_low_peaks_to_tall_ones() {
        let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Mountains);
        let mut tops = Vec::new();
        for j in 0..200 {
            for i in 0..200 {
                tops.push(stroke.target(Peak::Mountains, i as f32 * 15.0, j as f32 * 15.0));
            }
        }
        let share = |test: &dyn Fn(f32) -> bool| tops.iter().filter(|&&t| test(t)).count() as f32 / tops.len() as f32;
        let mean = tops.iter().sum::<f32>() / tops.len() as f32;
        let (low, mid, tall) = (share(&|t| t < 26.0), share(&|t| (26.0..42.0).contains(&t)), share(&|t| t >= 42.0));
        eprintln!("mean {mean:.1} m; below 26 m {low:.2}, 26 to 42 m {mid:.2}, 42 m and up {tall:.2}");
        // Some of everything: lots of low ground and saddles, a good share of middling peaks,
        // and a few tall ones standing above the rest.
        assert!(low > 0.12, "only {low} of the ground is below 26 m");
        assert!(mid > 0.3, "only {mid} is 26 to 42 m");
        assert!(tall > 0.03 && tall < 0.35, "{tall} reaches 42 m");
    }

    #[test]
    fn mountain_heights_run_from_valleys_to_the_top_of_the_range() {
        let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Mountains);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for j in 0..300 {
            for i in 0..300 {
                let t = stroke.target(Peak::Mountains, i as f32 * 20.0, j as f32 * 20.0);
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        assert!(lo >= MIN_HILL && lo < MIN_MOUNTAIN, "lowest was {lo}");
        assert!(hi <= MAX_MOUNTAIN && hi > MAX_MOUNTAIN - 1.0, "tallest was {hi}");
    }
}
