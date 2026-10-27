//! Pieces shared by the continent and island generators.

use macroquad::prelude::Vec2;
use crate::noise::{smoothstep, Simplex};
use crate::world::CELL_SIZE;

/// Baseline land height, in meters, before lowland relief is added.
pub const LAND_HEIGHT: f32 = 1.0;

/// Add rolling lowland terrain to newly generated land, fading it in over 60 m from the
/// shoreline. The coast and existing land stay untouched, including the first land sample.
pub fn add_lowland_relief(heights: &mut [f32], existing: &[f32], nx: usize, nz: usize, seed: u32) {
    let water: Vec<f32> = heights.iter().map(|&h| if h < 0.0 { 0.0 } else { -1.0 }).collect();
    let longest = (nx - 1).max(nz - 1) as f32;
    let Some(distance) = ExistingLand::new(&water, nx, nz, longest) else { return };
    let noise = Simplex::new(seed ^ 0xA4B1_C293);
    for (idx, h) in heights.iter_mut().enumerate() {
        if *h < 0.0 || existing[idx] >= 0.0 {
            continue;
        }
        let x = (idx % nx) as f32 * CELL_SIZE;
        let z = (idx / nx) as f32 * CELL_SIZE;
        let inland = (distance.dist[idx] * longest - 1.0).max(0.0) * CELL_SIZE;
        let broad = noise.fbm(x / 180.0, z / 180.0, 3, 0.5).clamp(-1.0, 1.0);
        let detail = noise.fbm(x / 65.0 + 31.0, z / 65.0 - 47.0, 2, 0.45).clamp(-1.0, 1.0);
        let hills = (4.0 + 3.3 * broad + 1.7 * detail).clamp(0.0, 9.0);
        *h += smoothstep(0.0, 60.0, inland) * hills;
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
        for heights in [&continents, &islands] {
            let inland: Vec<f32> = heights.iter().copied().filter(|&h| h >= LAND_HEIGHT).collect();
            assert!(!inland.is_empty());
            let min = inland.iter().copied().fold(f32::INFINITY, f32::min);
            let max = inland.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(max - min > 2.0, "lowlands should vary in elevation");
            assert!(max <= 10.0, "lowlands should stay at or below 10 m");
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
        add_lowland_relief(&mut heights, &existing, nx, nz, 42);
        assert_eq!(heights[20 * nx], -1.0);
        assert_eq!(heights[20 * nx + 1], LAND_HEIGHT);
        assert_eq!(heights[20 * nx + 20], 3.0);
        assert!(heights[20 * nx + 15] > LAND_HEIGHT);
    }
}

