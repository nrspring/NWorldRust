//! Rivers: a winding channel dug from the higher of two points down to the lower one.
//!
//! The river's course is a meandering line between the points. Water only runs downhill, so the
//! surface of the river is worked out along that line first: it starts at the lowest dry ground
//! beside the source and from there never rises, falling a little all the way, and it comes
//! down to sea level (or the level of the lake) at the mouth. Wherever the land climbs in the
//! river's way, the river cuts through it. The channel and its banks are then dug around the
//! line, and the water levels of its samples are set to the surface, which slopes with the river.

use std::f32::consts::TAU;

use glam::*;

use crate::gen_common::Rng;
use crate::noise::{smoothstep, Simplex};
use crate::world::{CELL_SIZE, SEA_LEVEL};

/// Meters between the points the river's course is worked out at.
const STATION_SPACING: f32 = CELL_SIZE * 0.5;
/// The narrowest a channel gets, as half its width, so every stretch holds some samples.
const MIN_HALF_WIDTH: f32 = CELL_SIZE * 0.8;
/// The two points must be at least this far apart, in meters.
const MIN_LENGTH: f32 = CELL_SIZE * 6.0;
/// A bank climbs `BANK_SLOPE` meters for every meter it goes out from the channel at first, and
/// steepens as it goes, by `BANK_STEEPENING` more per meter squared, so a river through a hill
/// runs in a gorge rather than a wide valley.
const BANK_SLOPE: f32 = 0.35;
const BANK_STEEPENING: f32 = 0.02;
/// Digging reaches this far beyond the channel, in meters, which is enough to cut a bank up the
/// highest ground there is.
const BANK_REACH: f32 = 60.0;
/// The river's surface falls at least this much for every meter it runs, so it always flows.
const MIN_FALL: f32 = 0.002;
/// Near its mouth the surface comes down to the level of the water it flows into at this
/// rate, which is how the river reaches the sea or the lake instead of ending in a step.
const MOUTH_FALL: f32 = 0.012;
/// The ground beside the river is looked at this far past the channel, in meters, to find how
/// low the water must sit to stay in its banks.
const BANK_LOOK: f32 = CELL_SIZE * 2.0;
/// Meters kept between the river and the map's edge.
const EDGE_MARGIN: f32 = CELL_SIZE * 3.0;
/// Courses tried for a winding river; the one that digs the least is kept.
const CANDIDATES: u32 = 12;
/// A random river starts at least this far from the sea, in meters.
const RANDOM_MIN_LENGTH: f32 = 150.0;
/// A random river's start counts as far enough from the sea, in meters, to be as likely as any
/// farther one; nearer ones are less likely, in proportion.
const RANDOM_LONG_ENOUGH: f32 = 500.0;
/// Starting points drawn for each random river, which are then tried in a random order.
const RANDOM_DRAWS: usize = 80;
/// How many of the drawn starting points are tried before giving up.
const RANDOM_TRIES: usize = 6;
/// A random river's mouth is any shore within this many times the distance to the nearest.
const RANDOM_MOUTH_SPREAD: f32 = 1.6;

/// What river to dig.
pub struct RiverParams {
    /// The two ends, as world x and z. Which one is the source is up to the land: the river
    /// flows from the higher one.
    pub from: Vec2,
    pub to: Vec2,
    /// How wide the channel is in the middle of the river, in meters.
    pub width: f32,
    /// How much the river winds, from 0 (straight) to 1.
    pub winding: f32,
    pub seed: u32,
}

/// The map with a river dug in.
pub struct Dug {
    pub heights: Vec<f32>,
    pub water: Vec<f32>,
}

/// The heightmap and the water on it, with the lay of the samples.
struct Terrain<'a> {
    heights: &'a [f32],
    water: &'a [f32],
    nx: usize,
    nz: usize,
    half: Vec2,
}

impl Terrain<'_> {
    /// The index of the sample nearest a world position.
    fn nearest(&self, p: Vec2) -> usize {
        let i = (((p.x + self.half.x) / CELL_SIZE).round().max(0.0) as usize).min(self.nx - 1);
        let j = (((p.y + self.half.y) / CELL_SIZE).round().max(0.0) as usize).min(self.nz - 1);
        j * self.nx + i
    }

    /// Whether the sample is under water: the sea or a lake.
    fn flooded(&self, idx: usize) -> bool {
        self.heights[idx] < self.water[idx]
    }

    /// Height of what shows at the sample nearest `p`: the ground or the water over it.
    fn surface(&self, p: Vec2) -> f32 {
        let idx = self.nearest(p);
        self.heights[idx].max(self.water[idx])
    }

    /// Calls `f` with the index of every sample within `radius` meters of `p`, and how far it is.
    fn for_samples_within(&self, p: Vec2, radius: f32, mut f: impl FnMut(usize, f32)) {
        let span = |c: f32, n: usize| {
            let lo = ((c - radius) / CELL_SIZE).floor().max(0.0) as usize;
            let hi = (((c + radius) / CELL_SIZE).ceil().max(0.0) as usize).min(n - 1);
            lo..=hi
        };
        for j in span(p.y + self.half.y, self.nz) {
            for i in span(p.x + self.half.x, self.nx) {
                let at = vec2(i as f32 * CELL_SIZE - self.half.x, j as f32 * CELL_SIZE - self.half.y);
                let d = at.distance(p);
                if d <= radius {
                    f(j * self.nx + i, d);
                }
            }
        }
    }

    /// The lowest dry ground within `radius` meters of `p`.
    fn lowest_dry(&self, p: Vec2, radius: f32) -> f32 {
        let mut lowest = f32::INFINITY;
        self.for_samples_within(p, radius, |idx, _| {
            if !self.flooded(idx) {
                lowest = lowest.min(self.heights[idx]);
            }
        });
        lowest
    }
}

/// One point along the river's course.
#[derive(Clone)]
struct Station {
    pos: Vec2,
    /// Meters along the course from its start.
    along: f32,
    half_width: f32,
    /// How deep the water is in the middle of the channel.
    depth: f32,
}

/// A river's course, with the level of the water at each of its stations.
struct Course {
    stations: Vec<Station>,
    levels: Vec<f32>,
    /// How far below the ground beside it the water sits, on average: how much has to be dug.
    cut: f32,
}

/// Digs a river between the two points of `params` into the map and returns the result, or
/// says why it can't be done.
pub fn dig(
    heights: &[f32],
    water: &[f32],
    nx: usize,
    nz: usize,
    half: Vec2,
    params: &RiverParams,
) -> Result<Dug, &'static str> {
    let terrain = Terrain { heights, water, nx, nz, half };
    let (mut from, mut to) = (params.from, params.to);
    if terrain.surface(from) < terrain.surface(to) {
        std::mem::swap(&mut from, &mut to);
    }
    if from.distance(to) < MIN_LENGTH {
        return Err("Pick two points farther apart.");
    }

    let tries = if params.winding > 0.0 { CANDIDATES } else { 1 };
    let mut best: Option<Course> = None;
    for k in 0..tries {
        let seed = params.seed.wrapping_add(k.wrapping_mul(0x9E37_79B9));
        let stations = stations(&terrain, from, to, params.width, params.winding, seed);
        if let Some(course) = plan(&terrain, stations) {
            if best.as_ref().is_none_or(|b| course.cut < b.cut) {
                best = Some(course);
            }
        }
    }
    let course = best.ok_or("A river needs dry land between its two points.")?;
    Ok(carve(&terrain, &course))
}

/// Digs up to `count` rivers `width` meters wide, winding by `winding` (0 to 1), each from a
/// random point inland on a landmass down to the sea, and returns the map with them and how
/// many fit, or `None` if none did.
pub fn scatter(
    heights: &[f32],
    water: &[f32],
    nx: usize,
    nz: usize,
    half: Vec2,
    seed: u32,
    count: u32,
    width: f32,
    winding: f32,
) -> Option<(Dug, usize)> {
    let mut rng = Rng::new(seed);
    let mut current = Dug { heights: heights.to_vec(), water: water.to_vec() };
    let mut added = 0;
    for k in 0..count {
        let terrain = Terrain { heights: &current.heights, water: &current.water, nx, nz, half };
        let river_seed = seed.wrapping_add(k.wrapping_mul(0x85EB_CA6B));
        if let Some(next) = random_river(&terrain, &mut rng, width, winding, river_seed) {
            current = next;
            added += 1;
        }
    }
    (added > 0).then_some((current, added))
}

/// One river from a random inland point to the sea, or `None` if no point works.
fn random_river(terrain: &Terrain, rng: &mut Rng, width: f32, winding: f32, seed: u32) -> Option<Dug> {
    let (nx, n) = (terrain.nx, terrain.heights.len());
    let ocean = |idx: usize| terrain.flooded(idx) && terrain.water[idx] <= SEA_LEVEL;
    let pos = |idx: usize| vec2((idx % nx) as f32 * CELL_SIZE - terrain.half.x, (idx / nx) as f32 * CELL_SIZE - terrain.half.y);
    // The sea's shore: ocean samples with dry ground beside them.
    let shore: Vec<usize> = (0..n)
        .filter(|&idx| {
            let (i, j) = (idx % nx, idx / nx);
            ocean(idx)
                && [(i > 0).then(|| idx - 1), (i + 1 < nx).then(|| idx + 1), (j > 0).then(|| idx - nx), (idx + nx < n).then(|| idx + nx)]
                    .into_iter()
                    .flatten()
                    .any(|k| !terrain.flooded(k))
        })
        .collect();
    if shore.is_empty() {
        return None;
    }

    // Draw a number of starting points over all the land and put them in a random order that
    // leans toward the ones far from the sea, so rivers tend to be long, but only up to
    // `RANDOM_LONG_ENOUGH`: past that, every spot is as likely as any other.
    let mut starts: Vec<(f32, Vec2, f32)> = Vec::new();
    for _ in 0..RANDOM_DRAWS {
        let source = (rng.range(0.0, 1.0) * n as f32) as usize % n;
        // Dry, inland ground that isn't a lake's bank.
        if terrain.flooded(source) || terrain.heights[source] < 2.0 || terrain.water[source] > SEA_LEVEL {
            continue;
        }
        let from = pos(source);
        let nearest = shore.iter().map(|&k| pos(k).distance(from)).fold(f32::INFINITY, f32::min);
        if nearest >= RANDOM_MIN_LENGTH {
            let weight = nearest.min(RANDOM_LONG_ENOUGH) / RANDOM_LONG_ENOUGH;
            starts.push((rng.range(0.0001, 1.0).powf(1.0 / weight), from, nearest));
        }
    }
    starts.sort_by(|a, b| b.0.total_cmp(&a.0));

    for (_, from, nearest) in starts.into_iter().take(RANDOM_TRIES) {
        // The mouth is somewhere along the shore not much farther than the nearest part of it.
        let (mut mouth, mut seen) = (shore[0], 0.0);
        for &k in &shore {
            if pos(k).distance(from) <= nearest * RANDOM_MOUTH_SPREAD {
                seen += 1.0;
                if rng.range(0.0, 1.0) * seen < 1.0 {
                    mouth = k;
                }
            }
        }
        let params = RiverParams { from, to: pos(mouth), width, winding, seed };
        if let Ok(dug) = dig(terrain.heights, terrain.water, nx, terrain.nz, terrain.half, &params) {
            return Some(dug);
        }
    }
    None
}

/// A meandering line of stations from `from` to `to`: straight at both ends, and in between
/// swinging to either side by an amount that grows with `winding`.
fn stations(terrain: &Terrain, from: Vec2, to: Vec2, width: f32, winding: f32, seed: u32) -> Vec<Station> {
    let chord = to - from;
    let length = chord.length();
    let dir = chord / length;
    let side = vec2(-dir.y, dir.x);
    let noise = Simplex::new(seed);
    let phase = Rng::new(seed).range(0.0, TAU);

    // A wide river swings in wide bends; a winding one in more of them and further out.
    let wavelength = (length / (1.5 + 5.5 * winding)).max(10.0 * width);
    let amplitude = (winding * 0.45 * wavelength).min(0.4 * length) * Rng::new(seed ^ 0x5bd1_e995).range(0.6, 1.0);
    let limit = terrain.half - Vec2::splat(EDGE_MARGIN);

    let steps = (length / STATION_SPACING).ceil().max(1.0) as usize;
    let mut stations: Vec<Station> = Vec::with_capacity(steps + 1);
    let mut along = 0.0;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let s = t * length;
        let u = s / wavelength;
        let ends = smoothstep(0.0, 0.15, t) * smoothstep(0.0, 0.15, 1.0 - t);
        let wave = 0.75 * (TAU * u + phase + 1.2 * noise.noise(u * 0.6, 0.5)).sin() + 0.4 * noise.noise(u * 1.7 + 3.1, 7.7);
        let pos = (from + dir * s + side * (amplitude * ends * wave)).clamp(-limit, limit);
        if let Some(last) = stations.last() {
            along += pos.distance(last.pos);
        }

        // The river is narrow at its source and wobbles in width after that.
        let taper = 0.55 + 0.45 * smoothstep(0.0, 0.25, t);
        let wobble = 1.0 + 0.18 * noise.noise(u * 0.8 + 11.0, 3.3);
        let half_width = (0.5 * width * taper * wobble).max(MIN_HALF_WIDTH);
        stations.push(Station {
            pos,
            along,
            half_width,
            depth: (half_width * 0.12).clamp(0.5, 4.0),
        });
    }
    stations
}

/// Works out the level of the water along the course, or `None` if it has no stretch of dry
/// land to run over. The course is cut short where it first runs into water.
fn plan(terrain: &Terrain, stations: Vec<Station>) -> Option<Course> {
    let start = stations.iter().position(|s| !terrain.flooded(terrain.nearest(s.pos)))?;
    let end = stations[start..]
        .iter()
        .position(|s| terrain.flooded(terrain.nearest(s.pos)))
        .map_or(stations.len(), |k| start + k);
    if end - start < 2 {
        return None;
    }
    // Where the river flows into water, that water's level is as low as it can fall.
    let mouth = stations.get(end).map(|s| (s.along, terrain.water[terrain.nearest(s.pos)]));
    let floor = mouth.map_or(SEA_LEVEL, |(_, level)| level.max(SEA_LEVEL));

    let used = &stations[start..end];
    let mut levels: Vec<f32> = Vec::with_capacity(used.len());
    let mut cut = 0.0;
    for (k, station) in used.iter().enumerate() {
        // Never above the lowest ground beside the river, or it would spill out of its banks.
        let lowest = terrain.lowest_dry(station.pos, station.half_width + BANK_LOOK);
        let fall = if k == 0 { 0.0 } else { (station.along - used[k - 1].along) * MIN_FALL };
        let mut level = (levels.last().copied().unwrap_or(f32::INFINITY) - fall).min(lowest);
        if let Some((mouth_along, _)) = mouth {
            level = level.min(floor + (mouth_along - station.along) * MOUTH_FALL);
        }
        let level = level.max(floor);
        cut += (lowest - level).max(0.0);
        levels.push(level);
    }
    Some(Course {
        cut: cut / used.len() as f32,
        stations: used.to_vec(),
        levels,
    })
}

/// Digs the channel and banks of `course` and sets the water levels of the channel.
fn carve(terrain: &Terrain, course: &Course) -> Dug {
    let n = terrain.heights.len();
    // For every sample: the lowest any station's profile takes it, and the lowest surface of
    // the stations whose channel it lies in.
    let mut lowest = vec![f32::INFINITY; n];
    let mut channel = vec![f32::INFINITY; n];
    for (station, &level) in course.stations.iter().zip(&course.levels) {
        let hw = station.half_width;
        terrain.for_samples_within(station.pos, hw + BANK_REACH, |idx, d| {
            // Water that is already there is left as it is.
            if terrain.flooded(idx) {
                return;
            }
            let u = d / hw;
            let height = if u < 1.0 {
                level - station.depth * (1.0 - u * u)
            } else {
                let out = d - hw;
                level + BANK_SLOPE * out + BANK_STEEPENING * out * out
            };
            lowest[idx] = lowest[idx].min(height);
            if u < 1.0 {
                channel[idx] = channel[idx].min(level);
            }
        });
    }

    let mut heights = terrain.heights.to_vec();
    let mut water = terrain.water.to_vec();
    for idx in 0..n {
        heights[idx] = heights[idx].min(lowest[idx]);
        if channel[idx].is_finite() {
            water[idx] = channel[idx];
        }
    }

    // Each sample just outside the channel carries the water level of the channel beside it,
    // so the shore is drawn where the ground meets the water.
    let (nx, nz) = (terrain.nx, terrain.nz);
    let mut ring = vec![f32::INFINITY; n];
    for idx in (0..n).filter(|&idx| channel[idx].is_finite()) {
        let (i, j) = (idx % nx, idx / nx);
        for nj in j.saturating_sub(1)..=(j + 1).min(nz - 1) {
            for ni in i.saturating_sub(1)..=(i + 1).min(nx - 1) {
                let near = nj * nx + ni;
                if !channel[near].is_finite() {
                    ring[near] = ring[near].min(channel[idx]);
                }
            }
        }
    }
    for idx in (0..n).filter(|&idx| ring[idx].is_finite()) {
        if !terrain.flooded(idx) && heights[idx] >= ring[idx] && water[idx] < ring[idx] {
            water[idx] = ring[idx];
        }
    }
    Dug { heights, water }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NX: usize = 151;
    const NZ: usize = 101;

    fn half() -> Vec2 {
        vec2((NX - 1) as f32 * CELL_SIZE * 0.5, (NZ - 1) as f32 * CELL_SIZE * 0.5)
    }

    /// A map of `NX` by `NZ` samples whose ground at world (x, z) is `ground(x, z)`, with no
    /// water but the sea.
    fn map(ground: impl Fn(f32, f32) -> f32) -> (Vec<f32>, Vec<f32>) {
        let half = half();
        let heights = (0..NZ)
            .flat_map(|j| (0..NX).map(move |i| (i, j)))
            .map(|(i, j)| ground(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y))
            .collect();
        (heights, vec![SEA_LEVEL; NX * NZ])
    }

    fn river(heights: &[f32], water: &[f32], from: Vec2, to: Vec2, width: f32, winding: f32) -> Result<Dug, &'static str> {
        dig(heights, water, NX, NZ, half(), &RiverParams { from, to, width, winding, seed: 5 })
    }

    /// The samples the river put water on.
    fn channel(before: (&[f32], &[f32]), dug: &Dug) -> Vec<usize> {
        (0..dug.heights.len())
            .filter(|&k| dug.heights[k] < dug.water[k] && before.0[k] >= before.1[k])
            .collect()
    }

    fn x_of(idx: usize) -> f32 {
        (idx % NX) as f32 * CELL_SIZE - half().x
    }

    fn z_of(idx: usize) -> f32 {
        (idx / NX) as f32 * CELL_SIZE - half().y
    }

    /// A slope falling from west to east with a hill across the way in the middle.
    fn ridge(x: f32, _z: f32) -> f32 {
        20.0 - 0.03 * x + 12.0 * (-(x / 60.0).powi(2)).exp()
    }

    #[test]
    fn a_river_never_runs_uphill_even_over_a_hill() {
        let (h, w) = map(ridge);
        let dug = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 24.0, 0.0).unwrap();
        let wet = channel((&h, &w), &dug);
        assert!(wet.len() > 100, "{} samples of river", wet.len());

        // The lowest surface of each column of samples never rises toward the mouth.
        let mut columns = vec![f32::INFINITY; NX];
        for &k in &wet {
            columns[k % NX] = columns[k % NX].min(dug.water[k]);
        }
        let levels: Vec<f32> = columns.into_iter().filter(|l| l.is_finite()).collect();
        for pair in levels.windows(2) {
            assert!(pair[1] <= pair[0] + 0.02, "river climbs from {} to {}", pair[0], pair[1]);
        }
        assert!(levels[0] > levels[levels.len() - 1] + 3.0);

        // It cut through the hill: the water is below the hilltop it passes.
        let top = (0..NX).map(|i| h[50 * NX + i]).fold(f32::MIN, f32::max);
        assert!(levels.iter().any(|&l| l < top - 8.0));
    }

    #[test]
    fn the_water_stays_inside_its_banks() {
        let (h, w) = map(ridge);
        let dug = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 24.0, 0.0).unwrap();
        for &k in &channel((&h, &w), &dug) {
            let (i, j) = (k % NX, k / NX);
            for nk in [k - 1, k + 1, k - NX, k + NX] {
                let dry = dug.heights[nk] >= dug.water[nk];
                assert!(!dry || dug.heights[nk] >= dug.water[k] - 0.3, "water at ({i}, {j}) spills over ground at {}", dug.heights[nk]);
            }
        }
    }

    #[test]
    fn a_river_comes_down_to_the_sea() {
        // Land falling gently east to a coast, and open sea beyond.
        let (h, w) = map(|x, _| if x < 100.0 { 8.0 - 0.01 * (x + 300.0) } else { -20.0 });
        let dug = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 24.0, 0.0).unwrap();
        let wet = channel((&h, &w), &dug);
        let mouth = wet.iter().filter(|&&k| x_of(k) > 60.0).map(|&k| dug.water[k]).fold(f32::INFINITY, f32::min);
        assert!(mouth < 0.3, "the river reaches the sea at {mouth} m");
        // The sea itself is left alone.
        for k in (0..h.len()).filter(|&k| h[k] < w[k]) {
            assert_eq!((dug.heights[k], dug.water[k]), (h[k], w[k]));
        }
    }

    #[test]
    fn it_does_not_matter_which_end_is_clicked_first() {
        let (h, w) = map(ridge);
        let a = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 24.0, 0.5).unwrap();
        let b = river(&h, &w, vec2(250.0, 0.0), vec2(-250.0, 0.0), 24.0, 0.5).unwrap();
        assert!(a.heights == b.heights && a.water == b.water);
    }

    #[test]
    fn winding_swings_the_river_to_the_sides() {
        let (h, w) = map(|x, _| 20.0 - 0.01 * x);
        let reach = |winding: f32| {
            let dug = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 12.0, winding).unwrap();
            let wet = channel((&h, &w), &dug);
            (wet.iter().map(|&k| z_of(k).abs()).fold(0.0, f32::max), wet.len())
        };
        let (straight, straight_len) = reach(0.0);
        let (winding, winding_len) = reach(1.0);
        assert!(straight < 12.0, "a straight river strayed {straight} m");
        assert!(winding > 25.0, "a winding river strayed only {winding} m");
        assert!(winding_len > straight_len);
    }

    #[test]
    fn a_wider_river_has_a_wider_channel() {
        let (h, w) = map(|x, _| 20.0 - 0.01 * x);
        let area = |width: f32| channel((&h, &w), &river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), width, 0.0).unwrap()).len();
        assert!(area(60.0) > area(20.0) * 2);
    }

    #[test]
    fn random_rivers_run_from_inland_to_the_sea() {
        // An island: land falling from a high middle to the shore, in open sea.
        let (h, w) = map(|x, z| {
            let d = (x * x / 4.0 + z * z).sqrt();
            if d < 190.0 { 2.0 + 0.04 * (190.0 - d) } else { -30.0 }
        });
        let (dug, added) = scatter(&h, &w, NX, NZ, half(), 3, 2, 16.0, 0.5).unwrap();
        assert!(added >= 1);
        let wet = channel((&h, &w), &dug);
        assert!(wet.len() > 50);
        // Every river reaches the sea: wet samples lie beside the ocean.
        let beside_sea = wet.iter().any(|&k| {
            [k - 1, k + 1, k - NX, k + NX, k - NX - 1, k - NX + 1, k + NX - 1, k + NX + 1].into_iter().any(|n| h[n] < w[n])
        });
        assert!(beside_sea);
        // Same seed, same rivers.
        let (again, _) = scatter(&h, &w, NX, NZ, half(), 3, 2, 16.0, 0.5).unwrap();
        assert!(again.heights == dug.heights);
        // No land, no rivers.
        let (sea, _) = map(|_, _| -50.0);
        assert!(scatter(&sea, &w, NX, NZ, half(), 3, 2, 16.0, 0.5).is_none());
    }

    #[test]
    fn rivers_need_land() {
        let (h, w) = map(|_, _| -50.0);
        assert!(river(&h, &w, vec2(-200.0, 0.0), vec2(200.0, 0.0), 24.0, 0.5).is_err());
        let (h, w) = map(|x, _| 10.0 - 0.01 * x);
        assert!(river(&h, &w, vec2(0.0, 0.0), vec2(4.0, 0.0), 24.0, 0.5).is_err());
    }

    #[test]
    fn a_river_ends_where_it_runs_into_a_lake_and_leaves_the_lake_alone() {
        let (h, mut w) = map(|x, _| 30.0 - 0.02 * x);
        let mut h = h;
        // A lake at 24 m across the river's way.
        for k in 0..h.len() {
            if (x_of(k) - 20.0).abs() < 40.0 && z_of(k).abs() < 40.0 {
                h[k] = 20.0;
                w[k] = 24.0;
            }
        }
        let dug = river(&h, &w, vec2(-250.0, 0.0), vec2(250.0, 0.0), 24.0, 0.0).unwrap();
        for k in (0..h.len()).filter(|&k| h[k] < w[k]) {
            assert_eq!((dug.heights[k], dug.water[k]), (h[k], w[k]));
        }
        let wet = channel((&h, &w), &dug);
        assert!(!wet.is_empty());
        assert!(wet.iter().all(|&k| x_of(k) < 20.0));
        // It arrives at the lake's level.
        let arrival = wet.iter().filter(|&&k| x_of(k) > -40.0).map(|&k| dug.water[k]).fold(f32::INFINITY, f32::min);
        assert!((24.0..24.5).contains(&arrival), "arrives at {arrival} m");
    }
}
