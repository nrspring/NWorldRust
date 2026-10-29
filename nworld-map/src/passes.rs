//! Mountain passes: a level road cut through the land between two points.
//!
//! The road bed runs along a smooth line through as many points as are picked, gently winding
//! between them, climbing or falling evenly between the heights of the ground at its two ends,
//! and is level across its whole width.
//! Where the land rises above the bed, as it does crossing a mountain, it is cut away, with walls
//! rising to the ground on both sides; where the land falls below it, as it does crossing a
//! valley, it is built up, with embankments falling away to the ground. Water is left alone:
//! lakes, rivers and the sea are not dug into or built over.

use std::f32::consts::TAU;

use glam::*;

use crate::gen_common::Rng;
use crate::noise::{smoothstep, Simplex};
use crate::world::{CELL_SIZE, SEA_LEVEL};

/// Meters between the points the road's line is worked out at.
const STATION_SPACING: f32 = CELL_SIZE * 0.5;
/// The shortest road, in meters.
const MIN_LENGTH: f32 = CELL_SIZE * 15.0;
/// The road keeps this far inside the edge of the map, in meters.
const EDGE_MARGIN: f32 = CELL_SIZE * 3.0;
/// Walls and embankments round off into the road bed over about this many meters.
const FOOT: f32 = 6.0;
/// Walls are shaped for land up to this many meters above the road bed, the highest a pass cuts;
/// beyond that, they stop short of the ground and leave a cliff.
const MAX_CUT: f32 = 150.0;
/// Walls reach out no further than this from the road bed, in meters.
const MAX_REACH: f32 = 600.0;
/// The ground the road cuts into stays at least this far above the water.
const DRY_MARGIN: f32 = 0.3;
/// Lines of winding tried, when it winds at all, and the one that needs the least cut is used.
const CANDIDATES: u32 = 8;

/// What the Passes tool needs to cut a pass.
pub struct PassParams {
    /// The points the road passes through, in order, as world x and z: at least two. The first and
    /// last, its ends, must be on dry land.
    pub points: Vec<Vec2>,
    /// How wide the level road bed is, in meters.
    pub width: f32,
    /// How steep the walls are, as rise over run: 1 is a 45 degree slope.
    pub steepness: f32,
    /// How much the line winds, from 0 (straight) to 1.
    pub winding: f32,
    pub seed: u32,
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
    fn nearest(&self, p: Vec2) -> usize {
        let i = (((p.x + self.half.x) / CELL_SIZE).round().max(0.0) as usize).min(self.nx - 1);
        let j = (((p.y + self.half.y) / CELL_SIZE).round().max(0.0) as usize).min(self.nz - 1);
        j * self.nx + i
    }

    /// Whether the sample holds water, or has some standing next to it, as the shores of lakes
    /// and rivers do: ground the road must not touch.
    fn wet(&self, idx: usize) -> bool {
        self.heights[idx] < self.water[idx] || self.water[idx] > SEA_LEVEL
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
}

/// One point along the road's line.
#[derive(Clone, Copy)]
struct Station {
    pos: Vec2,
    /// The height of the road bed here.
    bed: f32,
}

/// Cuts a pass between the two points of `params` into the map and returns the new heights, or
/// says why it can't be done.
pub fn cut(
    heights: &[f32],
    water: &[f32],
    nx: usize,
    nz: usize,
    half: Vec2,
    params: &PassParams,
) -> Result<Vec<f32>, &'static str> {
    let terrain = Terrain { heights, water, nx, nz, half };
    let points = distinct(&params.points);
    if points.len() < 2 || points.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>() < MIN_LENGTH {
        return Err("Pick points farther apart.");
    }
    let (a, b) = (terrain.nearest(points[0]), terrain.nearest(points[points.len() - 1]));
    if terrain.wet(a) || terrain.wet(b) || heights[a] <= SEA_LEVEL || heights[b] <= SEA_LEVEL {
        return Err("A pass needs dry land at both ends.");
    }

    let tries = if params.winding > 0.0 { CANDIDATES } else { 1 };
    let mut best: Option<(f32, Vec<Station>)> = None;
    for k in 0..tries {
        let seed = params.seed.wrapping_add(k.wrapping_mul(0x9E37_79B9));
        let stations = line(&terrain, &points, params.width, params.winding, seed);
        // How much ground the road has to take away, on average, along the way.
        let cut: f32 = stations.iter().map(|s| (heights[terrain.nearest(s.pos)] - s.bed).max(0.0)).sum::<f32>()
            / stations.len() as f32;
        if best.as_ref().is_none_or(|(c, _)| cut < *c) {
            best = Some((cut, stations));
        }
    }
    let (_, stations) = best.expect("at least one line");
    Ok(shape(&terrain, &stations, params))
}

/// The points with any that sit on top of the one before dropped.
fn distinct(points: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_none_or(|&q| q.distance(p) > 1.0) {
            out.push(p);
        }
    }
    out
}

/// A point of the smooth line through the picked points: where it is, which stretch between two
/// picked points it is on, how far along that stretch in meters and how long the stretch is.
struct Spot {
    pos: Vec2,
    stretch: usize,
    along: f32,
    length: f32,
}

/// The smooth line through `points` (at least two): a Catmull-Rom spline, which passes through
/// every one of them, sampled every [`STATION_SPACING`] meters or so.
fn spline(points: &[Vec2]) -> Vec<Spot> {
    let n = points.len();
    let at = |k: isize| {
        // Past either end, the line carries straight on.
        if k < 0 {
            points[0] * 2.0 - points[1]
        } else if k as usize >= n {
            points[n - 1] * 2.0 - points[n - 2]
        } else {
            points[k as usize]
        }
    };
    let mut spots = Vec::new();
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(i as isize - 1), points[i], points[i + 1], at(i as isize + 2));
        let length = p1.distance(p2);
        let steps = (length / STATION_SPACING).ceil().max(1.0) as usize;
        // The last stretch also takes its end point.
        let last = if i == n - 2 { steps } else { steps - 1 };
        for k in 0..=last {
            let t = k as f32 / steps as f32;
            let pos = 0.5
                * (p1 * 2.0
                    + (p2 - p0) * t
                    + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * (t * t)
                    + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * (t * t * t));
            spots.push(Spot { pos, stretch: i, along: t * length, length });
        }
    }
    spots
}

/// The smooth line through `points`, with no winding, as a list of positions: what the road
/// follows between them, for showing where it will go.
pub fn route(points: &[Vec2]) -> Vec<Vec2> {
    let points = distinct(points);
    if points.len() < 2 {
        return points;
    }
    spline(&points).into_iter().map(|s| s.pos).collect()
}

/// A line of stations through `points`, straight at each of them and between two swinging to
/// either side by an amount that grows with `winding`, with the road bed rising or falling
/// evenly from the height of the ground at the first to that at the last.
fn line(terrain: &Terrain, points: &[Vec2], width: f32, winding: f32, seed: u32) -> Vec<Station> {
    let noise = Simplex::new(seed);
    let spots = spline(points);
    let limit = terrain.half - Vec2::splat(EDGE_MARGIN);

    let mut positions: Vec<Vec2> = Vec::with_capacity(spots.len());
    for (k, spot) in spots.iter().enumerate() {
        // Each stretch between two picked points winds on its own, by gentle bends, and comes
        // back to the line at both its ends.
        let wavelength = (spot.length / (1.0 + 3.0 * winding)).max(8.0 * width);
        let rng = |salt: u32| Rng::new(seed ^ (spot.stretch as u32 + 1).wrapping_mul(salt));
        let amplitude = (winding * 0.3 * wavelength).min(0.25 * spot.length) * rng(0x5bd1_e995).range(0.6, 1.0);
        let phase = rng(0x9E37_79B1).range(0.0, TAU);
        let u = spot.along / wavelength + spot.stretch as f32 * 13.7;
        let t = spot.along / spot.length;
        let ends = smoothstep(0.0, 0.15, t) * smoothstep(0.0, 0.15, 1.0 - t);
        let wave = 0.75 * (TAU * u + phase + 1.2 * noise.noise(u * 0.6, 0.5)).sin() + 0.4 * noise.noise(u * 1.7 + 3.1, 7.7);

        let ahead = spots[(k + 1).min(spots.len() - 1)].pos;
        let behind = spots[k.saturating_sub(1)].pos;
        let dir = (ahead - behind).normalize_or_zero();
        let side = vec2(-dir.y, dir.x);
        positions.push((spot.pos + side * (amplitude * ends * wave)).clamp(-limit, limit));
    }

    // The bed follows the ground at the two ends, evenly along the length of the road.
    let mut along = vec![0.0f32; positions.len()];
    for k in 1..positions.len() {
        along[k] = along[k - 1] + positions[k].distance(positions[k - 1]);
    }
    let total = along[along.len() - 1].max(1.0);
    let start = terrain.heights[terrain.nearest(points[0])];
    let end = terrain.heights[terrain.nearest(points[points.len() - 1])];
    positions
        .into_iter()
        .zip(along)
        .map(|(pos, along)| Station { pos, bed: start + (end - start) * (along / total) })
        .collect()
}

/// The heights with the road bed, its walls and its embankments shaped into them.
fn shape(terrain: &Terrain, stations: &[Station], params: &PassParams) -> Vec<f32> {
    let hw = 0.5 * params.width.max(CELL_SIZE);
    let steepness = params.steepness.max(0.1);
    let reach = hw + (FOOT + MAX_CUT / steepness).min(MAX_REACH);

    // For every sample: how far it is from the nearest station, and that station's bed.
    let n = terrain.heights.len();
    let mut nearest = vec![(f32::INFINITY, 0.0f32); n];
    for station in stations {
        terrain.for_samples_within(station.pos, reach, |idx, d| {
            if d < nearest[idx].0 {
                nearest[idx] = (d, station.bed);
            }
        });
    }

    let mut heights = terrain.heights.to_vec();
    for idx in 0..n {
        let (d, bed) = nearest[idx];
        if !d.is_finite() || terrain.wet(idx) {
            continue;
        }
        let ground = terrain.heights[idx];
        let out = (d - hw).max(0.0);
        // The wall rises (or the embankment falls) at the asked slope, rounded off at its foot.
        let slope = steepness * ((out * out + FOOT * FOOT).sqrt() - FOOT);
        let target = if d <= hw {
            bed
        } else if ground >= bed {
            ground.min(bed + slope)
        } else {
            ground.max(bed - slope)
        };
        // Cutting never takes the ground down to the water's edge, or the road would flood.
        heights[idx] = if target < ground { target.max(SEA_LEVEL + DRY_MARGIN).min(ground) } else { target };
    }
    heights
}

#[cfg(test)]
mod tests {
    use super::*;

    const NX: usize = 201;
    const NZ: usize = 101;

    fn half() -> Vec2 {
        vec2((NX - 1) as f32 * CELL_SIZE * 0.5, (NZ - 1) as f32 * CELL_SIZE * 0.5)
    }

    fn map(ground: impl Fn(f32, f32) -> f32) -> (Vec<f32>, Vec<f32>) {
        let half = half();
        let heights = (0..NZ)
            .flat_map(|j| (0..NX).map(move |i| (i, j)))
            .map(|(i, j)| ground(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y))
            .collect();
        (heights, vec![SEA_LEVEL; NX * NZ])
    }

    fn at(x: f32, z: f32) -> usize {
        let half = half();
        let i = ((x + half.x) / CELL_SIZE).round() as usize;
        let j = ((z + half.y) / CELL_SIZE).round() as usize;
        j * NX + i
    }

    fn pass(heights: &[f32], water: &[f32], from: Vec2, to: Vec2, width: f32, winding: f32) -> Result<Vec<f32>, &'static str> {
        through(heights, water, &[from, to], width, winding)
    }

    fn through(heights: &[f32], water: &[f32], points: &[Vec2], width: f32, winding: f32) -> Result<Vec<f32>, &'static str> {
        cut(heights, water, NX, NZ, half(), &PassParams { points: points.to_vec(), width, steepness: 1.0, winding, seed: 3 })
    }

    /// Flat 10 m ground with a 100 m ridge running north to south through the middle.
    fn ridge() -> (Vec<f32>, Vec<f32>) {
        map(|x, _| 10.0 + 90.0 * (-(x * x) / (2.0 * 60.0 * 60.0)).exp())
    }

    #[test]
    fn a_straight_pass_cuts_a_level_road_through_a_ridge() {
        let (heights, water) = ridge();
        let cut = pass(&heights, &water, vec2(-300.0, 0.0), vec2(300.0, 0.0), 40.0, 0.0).unwrap();
        // The ridge is 100 m high where the road crosses it; the road bed is the 10 m of the
        // ground at both ends, along the whole road and across its width.
        for x in (-280..=280).step_by(20) {
            for z in [-15.0, 0.0, 15.0] {
                let h = cut[at(x as f32, z)];
                assert!((h - 10.0).abs() < 0.6, "the road is {h} m high at ({x}, {z})");
            }
        }
        // The ridge is still there away from the road, and the walls rise to it.
        assert!(cut[at(0.0, 150.0)] > 90.0);
        let wall: Vec<f32> = (0..=8).map(|k| cut[at(0.0, 20.0 + k as f32 * 8.0)]).collect();
        assert!(wall.windows(2).all(|w| w[1] >= w[0] - 1e-3), "walls only rise: {wall:?}");
        assert!(wall[8] > wall[0] + 30.0, "the wall climbs to {}", wall[8]);
    }

    #[test]
    fn the_road_climbs_evenly_between_its_ends() {
        // Ground rising from 10 m at the west to 50 m at the east, with a ridge across it.
        let (heights, water) = map(|x, _| 30.0 + x * (40.0 / 800.0) + 60.0 * (-(x * x) / (2.0 * 50.0 * 50.0)).exp());
        let from = vec2(-300.0, 0.0);
        let to = vec2(300.0, 0.0);
        let (a, b) = (heights[at(-300.0, 0.0)], heights[at(300.0, 0.0)]);
        let cut = pass(&heights, &water, from, to, 40.0, 0.0).unwrap();
        let grade = (b - a) / 600.0;
        let road: Vec<f32> = (-290..=290).step_by(10).map(|x| cut[at(x as f32, 0.0)]).collect();
        for (k, h) in road.iter().enumerate() {
            let expected = a + grade * (10.0 + k as f32 * 10.0);
            assert!((h - expected).abs() < 1.0, "{h} m at step {k}, expected {expected}");
        }
        assert!(road.windows(2).all(|w| w[1] > w[0] - 1e-3), "the road never falls on the way up");
    }

    #[test]
    fn valleys_are_built_up_to_the_road_bed() {
        // 20 m ground with a 12 m deep gully across the road.
        let (heights, water) = map(|x, _| 20.0 - 12.0 * (-(x * x) / (2.0 * 25.0 * 25.0)).exp());
        let cut = pass(&heights, &water, vec2(-300.0, 0.0), vec2(300.0, 0.0), 30.0, 0.0).unwrap();
        for z in [-10.0, 0.0, 10.0] {
            assert!((cut[at(0.0, z)] - 20.0).abs() < 0.8, "the gully is {} m deep at z = {z}", cut[at(0.0, z)]);
        }
        // Away from the road the gully is as it was.
        assert!((cut[at(0.0, 150.0)] - heights[at(0.0, 150.0)]).abs() < 1e-3);
    }

    #[test]
    fn only_the_ground_near_the_road_changes() {
        let (heights, water) = ridge();
        let cut = pass(&heights, &water, vec2(-300.0, 0.0), vec2(300.0, 0.0), 40.0, 0.0).unwrap();
        let changed: Vec<usize> = (0..heights.len()).filter(|&k| (cut[k] - heights[k]).abs() > 1e-4).collect();
        assert!(!changed.is_empty());
        // Nothing north or south of the walls, which reach out as far as the ridge is high.
        let z_of = |idx: usize| ((idx / NX) as f32 * CELL_SIZE - half().y).abs();
        assert!(changed.iter().all(|&k| z_of(k) < 200.0), "ground changed far from the road");
    }

    #[test]
    fn water_is_left_alone() {
        let (mut heights, mut water) = ridge();
        // A lake beside the road.
        for j in 0..NZ {
            for i in 0..NX {
                let (x, z) = (i as f32 * CELL_SIZE - half().x, j as f32 * CELL_SIZE - half().y);
                if (x - 0.0).abs() < 30.0 && (z - 40.0).abs() < 12.0 {
                    heights[j * NX + i] = 70.0;
                    water[j * NX + i] = 85.0;
                }
            }
        }
        let cut = pass(&heights, &water, vec2(-300.0, 0.0), vec2(300.0, 0.0), 40.0, 0.0).unwrap();
        for k in 0..heights.len() {
            if heights[k] < water[k] || water[k] > SEA_LEVEL {
                assert_eq!(cut[k], heights[k], "water ground changed at {k}");
            }
        }
    }

    #[test]
    fn a_pass_needs_dry_land_at_both_ends_and_some_length() {
        let (mut heights, water) = ridge();
        assert!(pass(&heights, &water, vec2(0.0, 0.0), vec2(20.0, 0.0), 40.0, 0.0).is_err());
        // Sea at the east end.
        for j in 0..NZ {
            for i in (NX - 20)..NX {
                heights[j * NX + i] = -50.0;
            }
        }
        assert!(pass(&heights, &water, vec2(-300.0, 0.0), vec2(380.0, 0.0), 40.0, 0.0).is_err());
    }

    #[test]
    fn a_pass_through_several_points_bends_to_follow_them_and_stays_level() {
        let (heights, water) = ridge();
        // Over the north end of the ridge in a bend, not straight across the middle of it.
        let points = [vec2(-300.0, 0.0), vec2(0.0, 120.0), vec2(300.0, 0.0)];
        let cut = through(&heights, &water, &points, 40.0, 0.0).unwrap();
        // Level at the picked point in the middle, across the road, though the ridge is 90 m
        // high there; and the line down the middle of the map, which a straight pass would
        // have cut, is left standing high.
        for z in [105.0, 120.0, 135.0] {
            assert!((cut[at(0.0, z)] - 10.0).abs() < 0.8, "{} m high at z = {z}", cut[at(0.0, z)]);
        }
        assert!(cut[at(0.0, 0.0)] > 40.0, "the middle of the ridge is {} m high", cut[at(0.0, 0.0)]);
        // The route runs through every picked point.
        let route = route(&points);
        for p in points {
            assert!(route.iter().any(|r| r.distance(p) < 3.0), "the route misses {p}");
        }
    }

    #[test]
    fn a_pass_through_a_bend_climbs_evenly_along_its_whole_length() {
        let (heights, water) = map(|x, _| 20.0 + x * 0.05 + 70.0 * (-(x * x) / (2.0 * 60.0 * 60.0)).exp());
        let points = [vec2(-300.0, -60.0), vec2(-100.0, 60.0), vec2(100.0, -60.0), vec2(300.0, 60.0)];
        let cut = through(&heights, &water, &points, 40.0, 0.0).unwrap();
        // The road never falls or leaves the heights of its two ends on the way.
        let route = route(&points);
        let bed = |r: &Vec2| cut[at(r.x, r.y)];
        let (low, high) = (bed(&route[0]), bed(&route[route.len() - 1]));
        assert!(high > low + 10.0, "the ends are only {low} and {high} m high");
        let mut last = low;
        for r in &route {
            let h = bed(r);
            assert!(h >= low - 0.6 && h <= high + 0.6, "bed {h} m at {r}");
            assert!(h >= last - 0.8, "the bed falls from {last} to {h} m at {r}");
            last = last.max(h);
        }
    }

    #[test]
    fn a_winding_pass_is_still_level_and_goes_where_asked() {
        let (heights, water) = ridge();
        let cut = pass(&heights, &water, vec2(-300.0, 0.0), vec2(300.0, 0.0), 40.0, 0.8).unwrap();
        // It still joins the two ends, and wherever its bed runs it is the ends' level.
        assert!((cut[at(-300.0, 0.0)] - heights[at(-300.0, 0.0)]).abs() < 0.6);
        assert!((cut[at(300.0, 0.0)] - heights[at(300.0, 0.0)]).abs() < 0.6);
        let lowered = (0..heights.len()).filter(|&k| cut[k] < heights[k] - 1.0).count();
        assert!(lowered > 100, "only {lowered} samples cut");
    }
}
