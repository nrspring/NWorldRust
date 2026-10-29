//! Resources painted onto the map as layers of density, and the numbers that place things from them.
//!
//! The map only stores how dense each resource is at each sample, from 0 to 255. This is the brush
//! that paints a layer, and the deterministic noise that anything built on the layer (props to draw,
//! deposits to mine) can use to pick spots on a fixed grid across the world.

use glam::*;

use crate::noise::smoothstep;
use crate::world::{Region, Resource, World, CELL_SIZE};

/// How fast the brush works: a sample under it goes from its starting density to the painted
/// one in about this long, in seconds, with the brush held still.
const BRUSH_SECONDS: f32 = 0.35;
/// The brush is at full strength inside this share of its radius, then eases off to nothing.
const BRUSH_CORE: f32 = 0.55;

/// One press-drag-release of a resource brush: paints a resource of a chosen density on dry
/// land, or with `erase` clears it.
pub struct DensityStroke {
    nx: usize,
    nz: usize,
    erase: bool,
    /// The density painted, from 0 to 255.
    target: f32,
    /// The densities when the stroke began.
    base: Vec<u8>,
    /// Which samples are dry land.
    land: Vec<bool>,
    /// How far each sample has come from its starting density toward the painted one, 0 to 1.
    progress: Vec<f32>,
    last: Option<Vec2>,
}

impl DensityStroke {
    /// Starts a stroke on `layer` (`nx` by `nz` samples) that paints `density` (0 to 1), or
    /// with `erase` clears it.
    pub fn begin(layer: &[u8], heights: &[f32], water: &[f32], nx: usize, nz: usize, erase: bool, density: f32) -> Self {
        Self {
            nx,
            nz,
            erase,
            target: (density.clamp(0.0, 1.0) * 255.0).round(),
            base: layer.to_vec(),
            land: heights.iter().zip(water).map(|(&h, &w)| h >= w).collect(),
            progress: vec![0.0; layer.len()],
            last: None,
        }
    }

    /// Whether the stroke clears the resource.
    pub fn erases(&self) -> bool {
        self.erase
    }

    /// The densities the stroke started from.
    pub fn base(&self) -> &[u8] {
        &self.base
    }

    /// Paints with a brush of `radius` meters at `center` (world x and z, with the map centered on
    /// the origin), along the line from wherever the last call painted, and returns the part of
    /// the map that changed, if any. `dt` is the seconds since the last call.
    pub fn paint(&mut self, layer: &mut [u8], half: Vec2, center: Vec2, radius: f32, dt: f32) -> Option<Region> {
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
        let target = if self.erase { 0.0 } else { self.target };

        let mut changed: Option<Region> = None;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let idx = j * self.nx + i;
                if !self.land[idx] {
                    continue;
                }
                let at = vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                let d = distance_to_segment(at, from, center);
                if d > radius {
                    continue;
                }
                let weight = 1.0 - smoothstep(BRUSH_CORE, 1.0, d / radius);
                let now = self.progress[idx];
                let next = now.max((now + step).min(weight));
                if next == now {
                    continue;
                }
                self.progress[idx] = next;
                let base = self.base[idx] as f32;
                let moved = base + (target - base) * smoothstep(0.0, 1.0, next);
                // Painting never undoes itself: denser ground stays as dense as it was where it
                // was denser than painted, and clearing never adds.
                let value = if self.erase { base.min(moved) } else { base.max(moved) };
                let value = value.round() as u8;
                if value != layer[idx] {
                    layer[idx] = value;
                    let sample = Region { i0: i, j0: j, i1: i, j1: j };
                    changed = Some(changed.map_or(sample, |c| c.union(sample)));
                }
            }
        }
        changed
    }
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// Four independent random numbers in 0..1 for the spot at grid position (`a`, `b`).
pub fn randoms(a: i32, b: i32) -> [f32; 4] {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA6B) ^ 0x2C1B_3C6D;
    let mut next = || {
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        h = h.wrapping_mul(0x297A_2D39);
        h ^= h >> 15;
        (h >> 8) as f32 / (1u32 << 24) as f32
    };
    [next(), next(), next(), next()]
}

/// Smooth random value noise in 0..1 that varies over about one unit of `x` and `z`; `seed`
/// picks a different pattern.
pub fn value_noise(x: f32, z: f32, seed: i32) -> f32 {
    let (i, j) = (x.floor(), z.floor());
    let (tx, tz) = (x - i, z - j);
    let (tx, tz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let at = |di: i32, dj: i32| randoms(i as i32 + di + seed, j as i32 + dj - seed)[0];
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * tx;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * tx;
    top + (bottom - top) * tz
}

/// The density of `kind` at a world position, 0 to 1, between samples.
pub fn density_at(world: &World, kind: Resource, x: f32, z: f32) -> f32 {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, (nx - 1) as f32);
    let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, (nz - 1) as f32);
    let (i, j) = (fx.floor() as usize, fz.floor() as usize);
    let (i1, j1) = ((i + 1).min(nx - 1), (j + 1).min(nz - 1));
    let (tx, tz) = (fx - i as f32, fz - j as f32);
    let layer = world.resource(kind);
    let at = |i: usize, j: usize| layer[j * nx + i] as f32;
    let top = at(i, j) + (at(i1, j) - at(i, j)) * tx;
    let bottom = at(i, j1) + (at(i1, j1) - at(i, j1)) * tx;
    (top + (bottom - top) * tz) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 81;

    fn half() -> Vec2 {
        Vec2::splat((N - 1) as f32 * CELL_SIZE * 0.5)
    }

    fn paint(stroke: &mut DensityStroke, layer: &mut [u8], at: Vec2, radius: f32, seconds: f32) {
        for _ in 0..20 {
            stroke.paint(layer, half(), at, radius, seconds / 20.0);
        }
    }

    /// Flat land at 10 m everywhere.
    fn land() -> (Vec<f32>, Vec<f32>) {
        (vec![10.0; N * N], vec![0.0; N * N])
    }

    #[test]
    fn the_brush_paints_with_soft_edges_and_clears() {
        let (h, w) = land();
        let mut layer = vec![0u8; N * N];
        let mut stroke = DensityStroke::begin(&layer, &h, &w, N, N, false, 0.8);
        paint(&mut stroke, &mut layer, Vec2::ZERO, 80.0, 2.0);
        let mid = N / 2;
        assert_eq!(layer[mid * N + mid], 204, "80% density");
        let rim = layer[mid * N + mid + 18];
        assert!(rim > 0 && rim < 100, "rim is {rim}");
        assert_eq!(layer[mid * N + mid + 30], 0);

        let mut clear = DensityStroke::begin(&layer, &h, &w, N, N, true, 0.8);
        assert!(clear.erases());
        paint(&mut clear, &mut layer, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(layer[mid * N + mid], 0);
    }

    #[test]
    fn the_brush_only_works_on_dry_land_and_never_thins_denser_ground() {
        let (mut h, mut w) = land();
        let mid = N / 2;
        // A pond.
        h[mid * N + mid] = 2.0;
        w[mid * N + mid] = 5.0;
        let mut layer = vec![0u8; N * N];
        layer[mid * N + mid + 1] = 255;
        let mut stroke = DensityStroke::begin(&layer, &h, &w, N, N, false, 0.5);
        paint(&mut stroke, &mut layer, Vec2::ZERO, 80.0, 2.0);
        assert_eq!(layer[mid * N + mid], 0, "nothing under water");
        assert_eq!(layer[mid * N + mid + 1], 255, "denser ground stays");
        assert_eq!(layer[(mid + 1) * N + mid], 128);
    }

    #[test]
    fn value_noise_is_smooth_and_varied() {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        let mut last = value_noise(0.0, 0.0, 3);
        for k in 1..400 {
            let v = value_noise(k as f32 * 0.05, 1.3, 3);
            assert!((v - last).abs() < 0.2, "a jump from {last} to {v}");
            last = v;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo < 0.35 && hi > 0.65, "ranges only {lo} to {hi}");
    }

}

