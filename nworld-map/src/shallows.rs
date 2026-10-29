//! Shallow and deep sea water. How deep the sea is decides which it is: the floor of shallow
//! water lies a few meters down, and deep water goes down to the ocean floor. The brush paints
//! one onto the other, and the shelf makes shallows of the sea along every coast.

use glam::*;

use crate::noise::smoothstep;
use crate::world::{Region, CELL_SIZE, DEEP_OCEAN, SEA_LEVEL, SHALLOW_LIMIT};

/// How deep the shallow water the brush paints is, in meters.
pub const SHALLOW_DEPTH: f32 = 3.0;
// Painted shallows must count as shallow water for ships.
const _: () = assert!(SHALLOW_DEPTH < SHALLOW_LIMIT);
/// How quickly the brush works: a sample under it goes from deep to shallow (or back) in about
/// this long, in seconds, with the brush held still.
const BRUSH_SECONDS: f32 = 0.35;
/// The brush is at full strength inside this share of its radius and eases off to nothing at the
/// edge, so painted shallows have soft edges.
const BRUSH_CORE: f32 = 0.55;
/// On a shelf the sea floor deepens this much per meter from the coast, over the part of the
/// shelf that is shallow, to `SHALLOW_DEPTH` or a little more at its edge.
const SHELF_GRADIENT_END: f32 = 0.9;

/// Whether the sample is sea: under water that sits at sea level.
fn is_sea(heights: &[f32], water: &[f32], idx: usize) -> bool {
    heights[idx] < water[idx] && water[idx] <= SEA_LEVEL
}

/// One press-drag-release of the shallows brush: paints shallow water onto deep sea, or with
/// `deepen` deep water over shallow. It only works on sea, never on land, lakes or rivers, and
/// it never makes shallow water deeper or deep water shallower than it paints.
pub struct ShallowStroke {
    nx: usize,
    nz: usize,
    deepen: bool,
    /// The heights when the stroke began, which every change is worked out from.
    base: Vec<f32>,
    /// Which samples are sea.
    sea: Vec<bool>,
    /// How far each sample has come from its starting depth toward the painted one, from 0 to 1.
    progress: Vec<f32>,
    last: Option<Vec2>,
}

impl ShallowStroke {
    pub fn begin(heights: &[f32], water: &[f32], nx: usize, nz: usize, deepen: bool) -> Self {
        Self {
            nx,
            nz,
            deepen,
            base: heights.to_vec(),
            sea: (0..heights.len()).map(|k| is_sea(heights, water, k)).collect(),
            progress: vec![0.0; heights.len()],
            last: None,
        }
    }

    /// Whether the stroke paints deep water.
    pub fn deepens(&self) -> bool {
        self.deepen
    }

    /// The heights the stroke started from.
    pub fn base(&self) -> &[f32] {
        &self.base
    }

    /// Paints with a brush of `radius` meters at `center` (world x and z, with the map centered on
    /// the origin), along the line from wherever the last call painted, and returns the part of
    /// the map that changed, if any. `dt` is the seconds since the last call.
    pub fn paint(&mut self, heights: &mut [f32], half: Vec2, center: Vec2, radius: f32, dt: f32) -> Option<Region> {
        let from = self.last.unwrap_or(center);
        self.last = Some(center);

        let reach = |lo: f32, hi: f32, n: usize, offset: f32| {
            let first = ((lo - radius + offset) / CELL_SIZE).floor().max(0.0) as usize;
            let last = (((hi + radius + offset) / CELL_SIZE).ceil().max(0.0) as usize).min(n - 1);
            (first, last)
        };
        let (i0, i1) = reach(from.x.min(center.x), from.x.max(center.x), self.nx, half.x);
        let (j0, j1) = reach(from.y.min(center.y), from.y.max(center.y), self.nz, half.y);
        let step = dt / BRUSH_SECONDS;
        let target = if self.deepen { DEEP_OCEAN } else { -SHALLOW_DEPTH };

        let mut changed: Option<Region> = None;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let idx = j * self.nx + i;
                if !self.sea[idx] {
                    continue;
                }
                let at = vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                let d = distance_to_segment(at, from, center);
                if d > radius {
                    continue;
                }
                // The brush's strength here, which sets how far the sample may go.
                let weight = 1.0 - smoothstep(BRUSH_CORE, 1.0, d / radius);
                let now = self.progress[idx];
                let next = now.max((now + step).min(weight));
                if next == now {
                    continue;
                }
                self.progress[idx] = next;
                let base = self.base[idx];
                let moved = base + (target - base) * smoothstep(0.0, 1.0, next);
                // Painting never undoes itself: shallow water stays shallow where it already was
                // shallower than painted, and deep water stays deep where it was deeper.
                heights[idx] = if self.deepen { base.min(moved) } else { base.max(moved) };
                let sample = Region { i0: i, j0: j, i1: i, j1: j };
                changed = Some(changed.map_or(sample, |c| c.union(sample)));
            }
        }
        changed
    }
}

/// The straight-line distance from every sample to the nearest land (height 0 or more), in
/// samples, or `None` if there is no land. Exact, so that the shelf's rings come out round: two
/// passes of the one-dimensional lower-envelope transform, along the rows and then the columns.
fn distance_to_land(heights: &[f32], nx: usize, nz: usize) -> Option<Vec<f32>> {
    const FAR: f32 = 1e12;
    if !heights.iter().any(|&h| h >= 0.0) {
        return None;
    }
    let mut squared: Vec<f32> = heights.iter().map(|&h| if h >= 0.0 { 0.0 } else { FAR }).collect();
    let mut line = Vec::new();
    for (count, stride, lines, line_stride) in [(nx, 1, nz, nx), (nz, nx, nx, 1)] {
        for l in 0..lines {
            line.clear();
            line.extend((0..count).map(|k| squared[l * line_stride + k * stride]));
            let done = lower_envelope(&line);
            for (k, d) in done.into_iter().enumerate() {
                squared[l * line_stride + k * stride] = d;
            }
        }
    }
    Some(squared.into_iter().map(f32::sqrt).collect())
}

/// For each position along a line: the least over all positions `q` of `f[q]` plus the squared
/// distance to `q` (Felzenszwalb and Huttenlocher's distance transform).
fn lower_envelope(f: &[f32]) -> Vec<f32> {
    let n = f.len();
    let mut out = vec![0.0; n];
    // Positions of the parabolas that make up the envelope, and where each takes over.
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    let mut k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let d = q as f32 - v[k] as f32;
        *o = d * d + f[v[k]];
    }
    out
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// How deep the sea floor is, in meters, at `meters` from the coast on a shelf `width` meters
/// wide: a gentle slope down through the shallows, then a drop to the ocean floor around the
/// shelf's edge.
pub fn shelf_depth(meters: f32, width: f32) -> f32 {
    let t = meters / width;
    let shallows = 0.5 + (SHALLOW_DEPTH - 0.5) * (t / SHELF_GRADIENT_END).min(1.0);
    let drop = -DEEP_OCEAN * smoothstep(0.8, 1.2, t);
    (shallows + drop).min(-DEEP_OCEAN)
}

/// Makes the sea within `width` meters of land shallow, deepening gently out from the coast.
/// Water already shallower stays as it is. Returns the new heights, or `None` if nothing changed
/// (no land, or no deep water near it).
pub fn shelf(heights: &[f32], water: &[f32], nx: usize, nz: usize, width: f32) -> Option<Vec<f32>> {
    // Land is anything above sea level, which counts a lake's bed too, but lakes aren't beside
    // the sea.
    let land = distance_to_land(heights, nx, nz)?;
    let mut shelved = heights.to_vec();
    let mut changed = false;
    for k in 0..heights.len() {
        if !is_sea(heights, water, k) {
            continue;
        }
        let meters = land[k] * CELL_SIZE;
        if meters >= width * 1.2 {
            continue;
        }
        let wanted = -shelf_depth(meters, width);
        if wanted > shelved[k] {
            shelved[k] = wanted;
            changed = true;
        }
    }
    changed.then_some(shelved)
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 81;

    fn half() -> Vec2 {
        Vec2::splat((N - 1) as f32 * CELL_SIZE * 0.5)
    }

    /// Open deep sea.
    fn deep_sea() -> (Vec<f32>, Vec<f32>) {
        (vec![DEEP_OCEAN; N * N], vec![SEA_LEVEL; N * N])
    }

    fn paint(stroke: &mut ShallowStroke, heights: &mut [f32], at: Vec2, radius: f32, seconds: f32) {
        // Several frames, as held with the mouse.
        for _ in 0..20 {
            stroke.paint(heights, half(), at, radius, seconds / 20.0);
        }
    }

    #[test]
    fn the_brush_paints_shallow_water_onto_deep_with_soft_edges() {
        let (mut h, w) = deep_sea();
        let mut stroke = ShallowStroke::begin(&h, &w, N, N, false);
        paint(&mut stroke, &mut h, Vec2::ZERO, 80.0, 2.0);
        let mid = N / 2;
        assert!((h[mid * N + mid] + SHALLOW_DEPTH).abs() < 0.01, "center is {}", h[mid * N + mid]);
        // The rim is part way between, and well outside the brush nothing changed.
        let rim = h[mid * N + mid + (75.0 / CELL_SIZE) as usize];
        assert!(rim < -SHALLOW_DEPTH - 1.0 && rim > DEEP_OCEAN + 1.0, "rim is {rim}");
        assert_eq!(h[mid * N + mid + 30], DEEP_OCEAN);
    }

    #[test]
    fn the_other_button_paints_deep_water_back() {
        let (mut h, w) = deep_sea();
        let mut stroke = ShallowStroke::begin(&h, &w, N, N, false);
        paint(&mut stroke, &mut h, Vec2::ZERO, 80.0, 2.0);
        let mut back = ShallowStroke::begin(&h, &w, N, N, true);
        assert!(back.deepens());
        paint(&mut back, &mut h, Vec2::ZERO, 80.0, 2.0);
        let mid = N / 2;
        assert!((h[mid * N + mid] - DEEP_OCEAN).abs() < 0.01, "center is {}", h[mid * N + mid]);
    }

    #[test]
    fn it_works_on_sea_only() {
        let (mut h, mut w) = deep_sea();
        let mid = N / 2;
        // Land, and a lake whose bed is well below the sea.
        h[mid * N + mid] = 5.0;
        h[mid * N + mid + 2] = -3.0;
        w[mid * N + mid + 2] = 12.0;
        let mut stroke = ShallowStroke::begin(&h, &w, N, N, false);
        paint(&mut stroke, &mut h, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(h[mid * N + mid], 5.0);
        assert_eq!(h[mid * N + mid + 2], -3.0);
        assert!((h[mid * N + mid + 1] + SHALLOW_DEPTH).abs() < 0.01);
    }

    #[test]
    fn it_never_deepens_shallower_water_or_raises_deeper_water_when_painting_shallow() {
        let (mut h, w) = deep_sea();
        let mid = N / 2;
        h[mid * N + mid] = -1.0;
        let mut stroke = ShallowStroke::begin(&h, &w, N, N, false);
        paint(&mut stroke, &mut h, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(h[mid * N + mid], -1.0);
    }

    #[test]
    fn a_fast_drag_leaves_no_gaps() {
        let (mut h, w) = deep_sea();
        let mut stroke = ShallowStroke::begin(&h, &w, N, N, false);
        stroke.paint(&mut h, half(), vec2(-100.0, 0.0), 40.0, 0.5);
        stroke.paint(&mut h, half(), vec2(100.0, 0.0), 40.0, 0.5);
        stroke.paint(&mut h, half(), vec2(100.0, 0.0), 40.0, 0.5);
        let mid = N / 2;
        assert!(h[mid * N + mid] > -SHALLOW_DEPTH - 0.5, "the middle of the path is {}", h[mid * N + mid]);
    }

    #[test]
    fn shelves_deepen_with_distance_from_the_coast() {
        let width = 400.0;
        assert!(shelf_depth(0.0, width) < 1.0);
        assert!(shelf_depth(width * 0.5, width) < SHALLOW_DEPTH);
        assert!(shelf_depth(width * 0.5, width) > shelf_depth(0.0, width));
        assert!(shelf_depth(width * 1.2, width) >= -DEEP_OCEAN * 0.99);
        let mut last = 0.0;
        for k in 0..=24 {
            let depth = shelf_depth(k as f32 * 20.0, width);
            assert!(depth >= last, "the floor rises at {} m", k * 20);
            last = depth;
        }
    }

    #[test]
    fn a_shelf_makes_shallows_around_land_and_leaves_the_rest() {
        let (mut h, w) = deep_sea();
        let mid = N / 2;
        for j in mid - 4..=mid + 4 {
            for i in mid - 4..=mid + 4 {
                h[j * N + i] = 3.0;
            }
        }
        let shelved = shelf(&h, &w, N, N, 100.0).unwrap();
        // Land is untouched; next to it the sea is shallow; far out it's still deep.
        assert_eq!(shelved[mid * N + mid], 3.0);
        let beside = shelved[mid * N + mid + 5];
        assert!(beside > -3.0 && beside < 0.0, "beside the coast: {beside}");
        assert_eq!(shelved[mid * N + 1], DEEP_OCEAN);
        // Rising to the shelf's edge.
        assert!(shelved[mid * N + mid + 5] > shelved[mid * N + mid + 12]);
        // No land, no shelf.
        assert!(shelf(&vec![DEEP_OCEAN; N * N], &w, N, N, 100.0).is_none());
    }

    #[test]
    fn distances_to_land_are_exact() {
        let mut h = vec![DEEP_OCEAN; N * N];
        h[10 * N + 20] = 1.0;
        h[60 * N + 70] = 1.0;
        let d = distance_to_land(&h, N, N).unwrap();
        assert_eq!(d[10 * N + 20], 0.0);
        assert!((d[10 * N + 23] - 3.0).abs() < 1e-4);
        assert!((d[14 * N + 23] - 5.0).abs() < 1e-4, "a 3-4-5 triangle is {}", d[14 * N + 23]);
        // Nearer to the second.
        assert!((d[60 * N + 66] - 4.0).abs() < 1e-4);
        assert!(distance_to_land(&vec![DEEP_OCEAN; N * N], N, N).is_none());
    }
}
