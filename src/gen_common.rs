//! Pieces shared by the continent and island generators.

use macroquad::prelude::Vec2;
use crate::noise::{smoothstep, Simplex};
use crate::world::CELL_SIZE;

/// Baseline land height, in meters, before lowland relief is added.
pub const LAND_HEIGHT: f32 = 1.0;

/// How much new land rises above the shore.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Relief {
    /// Rolling lowlands up to 10 m.
    Lowlands,
    /// Ground that climbs from the coast toward the interior, between 1 and 25 m, with
    /// variation along the way.
    Continental,
}

/// How far inland a continent takes to reach its full height, at most, in meters.
const CONTINENT_REACH: f32 = 700.0;
/// The tallest continental ground, in meters.
const CONTINENT_TOP: f32 = 25.0;

/// Add rolling terrain to newly generated land, fading it in over 60 m from the shoreline. The
/// coast and existing land stay untouched, including the first land sample.
pub fn add_relief(heights: &mut [f32], existing: &[f32], nx: usize, nz: usize, seed: u32, relief: Relief) {
    let water: Vec<f32> = heights.iter().map(|&h| if h < 0.0 { 0.0 } else { -1.0 }).collect();
    let longest = (nx - 1).max(nz - 1) as f32;
    let Some(distance) = ExistingLand::new(&water, nx, nz, longest) else { return };
    let noise = Simplex::new(seed ^ 0xA4B1_C293);
    // Continental ground climbs over the distance to the deepest interior, up to a limit.
    let deepest = distance.dist.iter().copied().fold(0.0f32, |m, d| if d.is_finite() { m.max(d) } else { m });
    let reach = (deepest * longest * CELL_SIZE * 0.8).clamp(120.0, CONTINENT_REACH);
    for (idx, h) in heights.iter_mut().enumerate() {
        if *h < 0.0 || existing[idx] >= 0.0 {
            continue;
        }
        let x = (idx % nx) as f32 * CELL_SIZE;
        let z = (idx / nx) as f32 * CELL_SIZE;
        let inland = (distance.dist[idx] * longest - 1.0).max(0.0) * CELL_SIZE;
        let fade = smoothstep(0.0, 60.0, inland);
        *h += match relief {
            Relief::Lowlands => {
                let broad = noise.fbm(x / 180.0, z / 180.0, 3, 0.5).clamp(-1.0, 1.0);
                let detail = noise.fbm(x / 65.0 + 31.0, z / 65.0 - 47.0, 2, 0.45).clamp(-1.0, 1.0);
                fade * (4.0 + 3.3 * broad + 1.7 * detail).clamp(0.0, 9.0)
            }
            Relief::Continental => {
                // The coast stays low and the interior high; broad swells and finer bumps
                // make some stretches climb sooner and others later.
                let climb = smoothstep(0.0, reach, inland);
                let broad = noise.fbm(x / 320.0, z / 320.0, 3, 0.5).clamp(-1.0, 1.0);
                let detail = noise.fbm(x / 90.0 + 31.0, z / 90.0 - 47.0, 2, 0.45).clamp(-1.0, 1.0);
                let swing = (0.5 + 0.5 * climb) * (6.5 * broad + 3.0 * detail);
                fade * (2.0 + 15.0 * climb + swing).clamp(0.0, CONTINENT_TOP - LAND_HEIGHT)
            }
        };
    }
}

/// New land never gets closer than this to land already on the map, as a share of the longest
/// side, so it can't touch it.
pub const EXISTING_MIN_GAP: f32 = 0.006;

/// Small, fast, seedable random number generator (SplitMix64).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed as u64 ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * unit
    }
}

/// Land already on the map, as the distance from every heightmap sample to the nearest land
/// sample, in the generator's normalized units (the map's longest side is 1).
pub struct ExistingLand {
    pub dist: Vec<f32>,
    nx: usize,
    nz: usize,
    longest: f32,
}

impl ExistingLand {
    /// `None` if there is no land yet.
    pub fn new(heights: &[f32], nx: usize, nz: usize, longest: f32) -> Option<Self> {
        if !heights.iter().any(|&h| h >= 0.0) {
            return None;
        }
        // Two-pass chamfer distance transform: exact for straight and diagonal runs, within a
        // few percent elsewhere, which is plenty for keeping a channel of open water.
        let mut dist: Vec<f32> = heights
            .iter()
            .map(|&h| if h >= 0.0 { 0.0 } else { f32::INFINITY })
            .collect();
        let diag = std::f32::consts::SQRT_2;
        for j in 0..nz {
            for i in 0..nx {
                let k = j * nx + i;
                let mut d = dist[k];
                if i > 0 {
                    d = d.min(dist[k - 1] + 1.0);
                }
                if j > 0 {
                    d = d.min(dist[k - nx] + 1.0);
                    if i > 0 {
                        d = d.min(dist[k - nx - 1] + diag);
                    }
                    if i + 1 < nx {
                        d = d.min(dist[k - nx + 1] + diag);
                    }
                }
                dist[k] = d;
            }
        }
        for j in (0..nz).rev() {
            for i in (0..nx).rev() {
                let k = j * nx + i;
                let mut d = dist[k];
                if i + 1 < nx {
                    d = d.min(dist[k + 1] + 1.0);
                }
                if j + 1 < nz {
                    d = d.min(dist[k + nx] + 1.0);
                    if i + 1 < nx {
                        d = d.min(dist[k + nx + 1] + diag);
                    }
                    if i > 0 {
                        d = d.min(dist[k + nx - 1] + diag);
                    }
                }
                dist[k] = d;
            }
        }
        dist.iter_mut().for_each(|d| *d /= longest);
        Some(Self { dist, nx, nz, longest })
    }

    /// Distance to the nearest existing land from a point in normalized units.
    pub fn at(&self, p: Vec2) -> f32 {
        let i = ((p.x * self.longest).round().max(0.0) as usize).min(self.nx - 1);
        let j = ((p.y * self.longest).round().max(0.0) as usize).min(self.nz - 1);
        self.dist[j * self.nx + i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{continents::{self, ContinentParams}, islands::{self, IslandParams}, world::DEEP_OCEAN};

    fn range(heights: &[f32]) -> (f32, f32) {
        let inland: Vec<f32> = heights.iter().copied().filter(|&h| h >= LAND_HEIGHT).collect();
        assert!(!inland.is_empty());
        (
            inland.iter().copied().fold(f32::INFINITY, f32::min),
            inland.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    }

    #[test]
    fn continents_rise_from_low_coasts_to_higher_interiors() {
        let cells = 512;
        let n = cells + 1;
        let ocean = vec![DEEP_OCEAN; n * n];
        let heights = continents::generate(&ContinentParams {
            coverage: 0.3, count: 2, roughness: 0.4, regularity: 0.4,
            edge_buffer: 0.05, seed: 42,
        }, cells, cells, &ocean);
        let (min, max) = range(&heights);
        assert!(min < 2.0, "coasts should be low, got {min}");
        assert!(max <= 25.0, "continents top out at 25 m, got {max}");
        assert!(max > 15.0, "interiors should climb well above the coast, got {max}");

        // Compare the average height of land by how far it is from the water.
        let water: Vec<f32> = heights.iter().map(|&h| if h < 0.0 { 0.0 } else { -1.0 }).collect();
        let distance = ExistingLand::new(&water, n, n, (n - 1) as f32).unwrap();
        let (mut near, mut far) = ((0.0, 0), (0.0, 0));
        for (idx, &h) in heights.iter().enumerate() {
            let meters = distance.dist[idx] * (n - 1) as f32 * CELL_SIZE;
            if h < 0.0 {
                continue;
            }
            if meters < 100.0 {
                near = (near.0 + h, near.1 + 1);
            } else if meters > 200.0 {
                far = (far.0 + h, far.1 + 1);
            }
        }
        assert!(near.1 > 0 && far.1 > 0, "expected both coastal and deep inland land");
        let (near, far) = (near.0 / near.1 as f32, far.0 / far.1 as f32);
        assert!(far > near + 5.0, "inland {far} m should stand well above the coast {near} m");
    }

    #[test]
    fn both_generators_make_varied_lowlands() {
        let cells = 256;
        let ocean = vec![DEEP_OCEAN; (cells + 1) * (cells + 1)];
        let continents = continents::generate(&ContinentParams {
            coverage: 0.3, count: 2, roughness: 0.4, regularity: 0.4,
            edge_buffer: 0.05, seed: 42,
        }, cells, cells, &ocean);
        let (islands, _) = islands::generate(&IslandParams {
            count: 10, size: 0.08, variety: 0.4, scatter: 0.5,
            roughness: 0.4, edge_buffer: 0.05, seed: 42,
        }, cells, cells, &ocean);
        for (heights, top) in [(&continents, 25.0), (&islands, 10.0)] {
            let (min, max) = range(heights);
            assert!(max - min > 2.0, "lowlands should vary in elevation");
            assert!(max <= top, "land should stay at or below {top} m");
        }
    }

    #[test]
    fn relief_preserves_water_shore_and_existing_land() {
        let nx = 40;
        let nz = 40;
        let mut heights = vec![LAND_HEIGHT; nx * nz];
        for j in 0..nz { heights[j * nx] = -1.0; }
        let mut existing = vec![DEEP_OCEAN; nx * nz];
        existing[20 * nx + 20] = 3.0;
        heights[20 * nx + 20] = 3.0;
        add_relief(&mut heights, &existing, nx, nz, 42, Relief::Lowlands);
        assert_eq!(heights[20 * nx], -1.0);
        assert_eq!(heights[20 * nx + 1], LAND_HEIGHT);
        assert_eq!(heights[20 * nx + 20], 3.0);
        assert!(heights[20 * nx + 15] > LAND_HEIGHT);
    }
}

