//! The hills and mountains brushes: grow rolling 10 to 25 m hills or 26 to 100 m mountains on
//! land, easing them into the lowlands, and slowly wear ground back down.

use macroquad::prelude::*;

use crate::gen_common::{ExistingLand, LAND_HEIGHT};
use crate::noise::{smoothstep, Simplex};
use crate::world::{Region, World, CELL_SIZE};

pub const MIN_HILL: f32 = 10.0;
pub const MAX_HILL: f32 = 25.0;
pub const MIN_MOUNTAIN: f32 = 26.0;
pub const MAX_MOUNTAIN: f32 = 100.0;
/// How far the height noise is stretched toward the ends of the range.
const HILL_CONTRAST: f32 = 2.4;
/// The brush is at full strength inside this share of its radius and eases off to nothing at
/// the rim.
const BRUSH_CORE: f32 = 0.3;
/// How many times the rim of a finished stroke is smoothed, and how strongly each time.
const SMOOTH_PASSES: usize = 4;
const SMOOTH_STRENGTH: f32 = 0.8;

/// What a raising brush grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Peak {
    Hills,
    Mountains,
}

impl Peak {
    /// The lowest and highest the tops reach, in meters.
    pub fn range(self) -> (f32, f32) {
        match self {
            Peak::Hills => (MIN_HILL, MAX_HILL),
            Peak::Mountains => (MIN_MOUNTAIN, MAX_MOUNTAIN),
        }
    }

    /// How fast the brush raises ground at its center, in meters per second.
    fn rate(self) -> f32 {
        match self {
            Peak::Hills => 5.0,
            Peak::Mountains => 12.0,
        }
    }

    /// How fast the erasing brush lowers ground at its center, in meters per second. It never
    /// takes land below [`LAND_HEIGHT`].
    fn erase_rate(self) -> f32 {
        match self {
            Peak::Hills => 8.0,
            Peak::Mountains => 16.0,
        }
    }

    /// They fade out over this many meters inland from the shore, so they never end in a
    /// cliff at the water's edge.
    fn shore_fade(self) -> f32 {
        match self {
            Peak::Hills => 120.0,
            Peak::Mountains => 240.0,
        }
    }

    /// Size in meters of the broad and the finer swells in their heights.
    fn swells(self) -> (f32, f32) {
        match self {
            Peak::Hills => (260.0, 140.0),
            Peak::Mountains => (420.0, 220.0),
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
    /// Distance to the nearest water, if the map has any.
    shore: Option<ExistingLand>,
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
        let longest = (nx - 1).max(nz - 1) as f32;
        // Mark water as "land" so the distance transform measures how far inland each sample is.
        let water: Vec<f32> = heights.iter().map(|&h| if h < 0.0 { 0.0 } else { -1.0 }).collect();
        Self {
            base: heights.to_vec(),
            coverage: vec![0.0; heights.len()],
            shore: ExistingLand::new(&water, nx, nz, longest),
            longest,
            noise: Simplex::new(seed ^ 0x5A17_E3D9 ^ (peak as u32).wrapping_mul(0x9E37_79B9)),
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

    /// Meters from the sample to the nearest water.
    fn inland(&self, idx: usize) -> f32 {
        match &self.shore {
            Some(shore) => (shore.dist[idx] * self.longest - 1.0).max(0.0) * CELL_SIZE,
            None => f32::INFINITY,
        }
    }

    /// The height the peaks reach at a spot where the brush is at full strength, rolling like
    /// the lowlands do but over longer distances.
    fn target(&self, x: f32, z: f32) -> f32 {
        let (broad_size, detail_size) = self.peak.swells();
        let broad = self.noise.fbm(x / broad_size, z / broad_size, 3, 0.5);
        let detail = self.noise.fbm(x / detail_size + 31.0, z / detail_size - 47.0, 2, 0.45);
        // Noise rarely strays far from its middle, so stretch it until the tallest peaks reach
        // the top of the range and the lowest reach the bottom, flattening there.
        let mix = (HILL_CONTRAST * (0.6 * broad + 0.4 * detail)).clamp(-1.0, 1.0);
        let (low, high) = self.peak.range();
        (low + (high - low) * (0.5 + 0.5 * mix)).clamp(low, high)
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
        // Spread a frame's change over the dabs along the path.
        let rate = if self.erase { self.peak.erase_rate() } else { self.peak.rate() };
        let amount = rate * dt / dabs as f32;
        let mut changed: Option<Region> = None;
        for k in 1..=dabs {
            let at = from.lerp(center, k as f32 / dabs as f32);
            if let Some(region) = self.dab(heights, half, at, radius, amount) {
                changed = Some(changed.map_or(region, |c| c.union(region)));
            }
        }
        if let Some(region) = changed {
            self.touched = Some(self.touched.map_or(region, |t| t.union(region)));
        }
        changed
    }

    fn dab(&mut self, heights: &mut [f32], half: Vec2, at: Vec2, radius: f32, amount: f32) -> Option<Region> {
        // Samples sit at (index * CELL_SIZE - half); find the ones the brush can reach.
        let to_index = |v: f32, h: f32, n: usize| ((v + h) / CELL_SIZE).clamp(0.0, (n - 1) as f32);
        let i0 = to_index(at.x - radius, half.x, self.nx).ceil() as usize;
        let i1 = to_index(at.x + radius, half.x, self.nx).floor() as usize;
        let j0 = to_index(at.y - radius, half.y, self.nz).ceil() as usize;
        let j1 = to_index(at.y + radius, half.y, self.nz).floor() as usize;
        if i0 > i1 || j0 > j1 {
            return None;
        }

        let mut changed: Option<Region> = None;
        for j in j0..=j1 {
            let z = j as f32 * CELL_SIZE - half.y;
            for i in i0..=i1 {
                let x = i as f32 * CELL_SIZE - half.x;
                let t = (vec2(x, z) - at).length() / radius;
                if t >= 1.0 {
                    continue;
                }
                let idx = j * self.nx + i;
                let weight = 1.0 - smoothstep(BRUSH_CORE, 1.0, t);
                let height = if self.erase {
                    self.coverage[idx] = self.coverage[idx].max(weight);
                    // Wear land down toward the lowest ground; water and ground already at that
                    // level stay as they are.
                    let current = heights[idx];
                    if current <= LAND_HEIGHT {
                        continue;
                    }
                    (current - amount * weight).max(LAND_HEIGHT)
                } else {
                    self.coverage[idx] = self.coverage[idx].max(weight);
                    // Grow toward a ceiling that eases from the old ground up to the peak's
                    // height as the brush and the distance from the shore allow.
                    let base = self.base[idx];
                    let grown = weight * smoothstep(0.0, self.peak.shore_fade(), self.inland(idx));
                    // Nothing grows past the top of its range, whatever the ground started at.
                    let top = self.peak.range().1;
                    let ceiling = base.max(base + (self.target(x, z) - base) * grown).min(base.max(top));
                    let current = heights[idx];
                    if current >= ceiling {
                        continue;
                    }
                    (current + amount * weight).min(ceiling)
                };
                if height != heights[idx] {
                    heights[idx] = height;
                    let sample = Region { i0: i, j0: j, i1: i, j1: j };
                    changed = Some(changed.map_or(sample, |c| c.union(sample)));
                }
            }
        }
        changed
    }

    /// Ends the stroke by smoothing the rim of what was painted, where the hills ease down into
    /// the lowlands, so no step is left behind. Returns the part of the map that changed.
    pub fn finish(&mut self, heights: &mut [f32]) -> Option<Region> {
        let touched = self.touched?;
        let (i0, j0) = (touched.i0.saturating_sub(1), touched.j0.saturating_sub(1));
        let (i1, j1) = ((touched.i1 + 1).min(self.nx - 1), (touched.j1 + 1).min(self.nz - 1));
        let region = Region { i0, j0, i1, j1 };

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

        for _ in 0..SMOOTH_PASSES {
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

/// The world x and z under the mouse, found by following the view ray down to the surface
/// (the sea counts as surface). `None` if the ray never reaches it.
pub fn pick_terrain(camera: &Camera3D, world: &World, mouse: Vec2) -> Option<Vec2> {
    const MAX_DISTANCE: f32 = 40_000.0;
    let ndc = vec2(2.0 * mouse.x / screen_width() - 1.0, 1.0 - 2.0 * mouse.y / screen_height());
    let inverse = camera.matrix().inverse();
    let unproject = |z: f32| {
        let p = inverse * vec4(ndc.x, ndc.y, z, 1.0);
        p.truncate() / p.w
    };
    let origin = camera.position;
    let dir = (unproject(1.0) - unproject(-1.0)).normalize();

    let above = |t: f32| {
        let p = origin + dir * t;
        p.y - world.surface_height(p.x, p.z)
    };
    // Take big steps while high above the ground and small ones close to it.
    let (mut t, mut prev) = (0.0f32, 0.0f32);
    while t < MAX_DISTANCE {
        if above(t) <= 0.0 {
            // Crossed the surface between `prev` and `t`; narrow it down.
            let (mut lo, mut hi) = (prev, t);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                if above(mid) > 0.0 { lo = mid } else { hi = mid }
            }
            let p = origin + dir * hi;
            return Some(vec2(p.x, p.z));
        }
        prev = t;
        t += (above(t) * 0.4).max(CELL_SIZE * 0.5);
    }
    None
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
                let t = stroke.target(i as f32 * 10.0, j as f32 * 10.0);
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
    fn mountains_climb_to_100_m_and_no_further() {
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
        assert!(top <= MAX_MOUNTAIN, "mountains stop at 100 m, got {top}");
        assert!(heights[centre] >= MIN_MOUNTAIN, "the middle is a mountain, got {}", heights[centre]);
        for (h, s) in heights.iter().zip(island(cells, 10)) {
            if s < 0.0 {
                assert_eq!(*h, s, "water is untouched");
            }
        }
    }

    #[test]
    fn mountain_heights_span_26_to_100() {
        let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Mountains);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for j in 0..300 {
            for i in 0..300 {
                let t = stroke.target(i as f32 * 20.0, j as f32 * 20.0);
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        assert!(lo >= MIN_MOUNTAIN && lo < MIN_MOUNTAIN + 2.0, "lowest was {lo}");
        assert!(hi <= MAX_MOUNTAIN && hi > MAX_MOUNTAIN - 1.0, "tallest was {hi}");
    }
}
