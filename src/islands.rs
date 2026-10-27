//! Island generator: sprinkles islands into open water.
//!
//! Islands are small, noise-roughened, stretched blobs. They are placed in open water only:
//! outside the edge buffer, clear of land already on the map, and clear of each other. They can
//! be scattered evenly or grouped into archipelagos. Unlike continents, an island's coastline
//! is fixed where its own shape says, so the number and size of islands are what you ask for,
//! not a share of the map.

use std::f32::consts::PI;

use macroquad::prelude::{vec2, Vec2};

use crate::gen_common::{add_relief, ExistingLand, Relief, Rng, EXISTING_MIN_GAP, LAND_HEIGHT};
use crate::noise::Simplex;
use crate::world::{CELL_SIZE, DEEP_OCEAN};

/// Land falls off from an island's middle as `1 - s^FALLOFF` (s is 1 at the nominal radius).
const FALLOFF: f32 = 2.0;
/// Islands are stretched by up to this much extra along a random axis, with their area kept.
const MAX_EXTRA_STRETCH: f32 = 1.4;
/// How far island-scale lobes can push a coast, and how far the island's shape is warped, at the
/// most irregular, in landness units and in island radii.
const LOBE_AMPLITUDE: f32 = 0.6;
const WARP_RADII: f32 = 0.45;
/// How many islands an archipelago has, roughly.
const ISLANDS_PER_CLUSTER: f32 = 7.0;
/// Spacing of islands within an archipelago, from a base share of the longest side plus a
/// multiple of a typical island's radius.
const CLUSTER_SPREAD_BASE: f32 = 0.035;
const CLUSTER_SPREAD_PER_RADIUS: f32 = 2.5;
/// Tries per island before giving up on finding room for it.
const PLACEMENT_BATCHES: usize = 12;
const CANDIDATES_PER_BATCH: usize = 10;

#[derive(Clone, Copy)]
pub struct IslandParams {
    /// How many islands to try to place; fewer are added if there isn't room.
    pub count: u32,
    /// Typical island diameter, as a share of the map's shorter side.
    pub size: f32,
    /// 0 (all islands the same size) ..1 (a wide mix of tiny and large).
    pub variety: f32,
    /// 0 (grouped into archipelagos) ..1 (spread evenly over the open water).
    pub scatter: f32,
    /// Coastline roughness, 0 (smooth) ..1 (very ragged).
    pub roughness: f32,
    /// Width of the open-water border around the map, as a share of its shorter side.
    pub edge_buffer: f32,
    pub seed: u32,
}

/// Noise settings shared by every island in a run.
struct Shaping {
    noise: Simplex,
    /// How far fine coastline noise moves the outline, in landness units (1 = the radius).
    amplitude: f32,
    octaves: u32,
    persistence: f32,
}

struct Island {
    /// Where the island sits; its shape doesn't depend on this.
    center: Vec2,
    /// Nominal radius in normalized units (the map's longest side is 1).
    radius: f32,
    /// Unit vector along the island's long axis, and how much it is stretched along it.
    axis: Vec2,
    stretch: f32,
    /// How lumpy this island is, 0 (a smooth oval) ..1; every island gets its own, so a group
    /// mixes round and ragged ones.
    irregularity: f32,
    /// Where this island samples the shared noise, so islands don't share a shape.
    noise_offset: Vec2,
    /// Radius of a circle around the center that holds the whole island, measured from its
    /// actual shape.
    reach: f32,
}

impl Island {
    /// How far inside the coast `local` (an offset from the island's center) is, as a landness
    /// value: above zero is land. Noise is sampled relative to the center, so the shape stays
    /// the same wherever the island is put.
    fn landness(&self, shaping: &Shaping, local: Vec2) -> f32 {
        let n = &shaping.noise;
        let off = self.noise_offset;
        let sample = |freq: f32, shift: Vec2, octaves: u32, persistence: f32| {
            let p = local * freq + off + shift;
            n.fbm(p.x, p.y, octaves, persistence)
        };

        // Bend the island's space with slow noise, so its outline curls and bulges instead of
        // being an oval, then measure distance in its stretched frame.
        let warp_freq = 0.6 / self.radius;
        let warp = vec2(
            sample(warp_freq, vec2(17.0, 0.0), 2, 0.5),
            sample(warp_freq, vec2(0.0, -31.0), 2, 0.5),
        );
        let bent = local + warp * (WARP_RADII * self.radius * self.irregularity);
        let along = bent.dot(self.axis) / self.stretch;
        let across = bent.perp_dot(self.axis) * self.stretch;
        let s = (along * along + across * across).sqrt() / self.radius;

        let detail = sample(0.8 / self.radius, Vec2::ZERO, shaping.octaves, shaping.persistence);
        let lobes = sample(0.5 / self.radius, vec2(-9.0, 5.0), 3, 0.5);
        1.0 - s.powf(FALLOFF) + detail * shaping.amplitude + lobes * LOBE_AMPLITUDE * self.irregularity
    }

    /// Finds the island's real extent by sampling its shape around its center.
    fn measure_reach(&mut self, shaping: &Shaping) {
        // The farthest the shape could possibly reach, to know how far to look.
        let swing = 0.8 * (shaping.amplitude + LOBE_AMPLITUDE * self.irregularity);
        let bound = (self.radius * self.stretch * (1.0 + swing).powf(1.0 / FALLOFF)
            + WARP_RADII * self.radius * self.irregularity)
            * 1.05;
        let step = self.radius / 10.0;
        let n = (bound / step).ceil() as i32;
        let mut farthest = self.radius * 0.5;
        for j in -n..=n {
            for i in -n..=n {
                let local = vec2(i as f32, j as f32) * step;
                if self.landness(shaping, local) > 0.0 {
                    farthest = farthest.max(local.length());
                }
            }
        }
        self.reach = (farthest + step * 1.5) * 1.03;
    }
}

/// Height in meters at `d` meters inside (positive) or outside (negative) the coast. Land
/// reaches its baseline height within a few meters, and the sea deepens away from the coast, so
/// the renderer can trace the coastline smoothly between heightmap samples.
fn coast_profile(d: f32) -> f32 {
    if d >= 0.0 {
        (0.25 + 0.25 * d).min(LAND_HEIGHT)
    } else {
        let out = -d;
        -(0.25 + 0.25 * out + 0.05 * out * out).min(-DEEP_OCEAN)
    }
}

/// A standard normal random number, from the sum of a few uniform ones.
fn gaussian(rng: &mut Rng) -> f32 {
    let sum: f32 = (0..4).map(|_| rng.range(0.0, 1.0)).sum();
    (sum - 2.0) * 3.0f32.sqrt()
}

struct Planner<'a> {
    extent: Vec2,
    /// Edge buffer width in normalized units.
    buffer: f32,
    existing: Option<&'a ExistingLand>,
    placed: Vec<Island>,
}

impl Planner<'_> {
    fn edge_distance(&self, p: Vec2) -> f32 {
        p.x.min(self.extent.x - p.x).min(p.y).min(self.extent.y - p.y)
    }

    /// Spare room around `island` if it were put at `p`: how far it could grow before running
    /// into the buffer, existing land or another island. Negative if it doesn't fit.
    fn clearance(&self, p: Vec2, island: &Island) -> f32 {
        let from_buffer = self.edge_distance(p) - self.buffer - island.reach;
        let from_land = self
            .existing
            .map_or(f32::INFINITY, |e| e.at(p) - island.reach - land_gap(island.radius));
        self.placed
            .iter()
            .map(|q| {
                p.distance(q.center) - island.reach - q.reach - island_gap(island.radius.min(q.radius))
            })
            .fold(from_buffer.min(from_land), f32::min)
    }

    /// How much open water surrounds `p`, ignoring the size of whatever goes there; used to
    /// pick where archipelagos go.
    fn openness(&self, p: Vec2, others: &[Vec2]) -> f32 {
        let from_land = self.existing.map_or(f32::INFINITY, |e| e.at(p));
        others
            .iter()
            .map(|o| p.distance(*o) * 0.5)
            .fold((self.edge_distance(p) - self.buffer).min(from_land), f32::min)
    }
}

/// Water kept between an island and existing land.
fn land_gap(radius: f32) -> f32 {
    0.01 + 0.3 * radius
}

/// Water kept between two islands.
fn island_gap(smaller_radius: f32) -> f32 {
    0.004 + 0.15 * smaller_radius
}

/// Adds up to `params.count` islands to a heightmap of `cells_x` × `cells_z` cells, given as
/// (cells_x + 1) × (cells_z + 1) samples, row-major from the north-west corner. Returns the new
/// heights in the same layout and how many islands were actually added. Existing land is kept
/// exactly as it is.
pub fn generate(params: &IslandParams, cells_x: usize, cells_z: usize, existing: &[f32]) -> (Vec<f32>, usize) {
    let (nx, nz) = (cells_x + 1, cells_z + 1);
    assert_eq!(existing.len(), nx * nz, "heightmap size mismatch");
    let longest = cells_x.max(cells_z) as f32;
    let extent = vec2(cells_x as f32, cells_z as f32) / longest;
    let buffer = params.edge_buffer * extent.min_element();
    let existing_land = ExistingLand::new(existing, nx, nz, longest);
    let rough = params.roughness;

    let mut rng = Rng::new(params.seed);
    let shaping = Shaping {
        noise: Simplex::new(params.seed),
        amplitude: 0.10 + 0.40 * rough,
        octaves: 3 + (rough * 4.0).round() as u32,
        persistence: 0.45 + 0.15 * rough,
    };

    // Make every island's shape first, biggest first, so the big ones get room before the
    // small ones, and measure how much room each really needs.
    let typical_radius = params.size * extent.min_element() * 0.5;
    let mut islands: Vec<Island> = (0..params.count)
        .map(|_| {
            let factor = (params.variety * 0.55 * gaussian(&mut rng)).exp().clamp(0.3, 3.0);
            let angle = rng.range(0.0, PI);
            let mut island = Island {
                center: Vec2::ZERO,
                radius: typical_radius * factor,
                axis: vec2(angle.cos(), angle.sin()),
                stretch: (1.0 + MAX_EXTRA_STRETCH * rng.range(0.0, 1.0)).sqrt(),
                irregularity: rng.range(0.25, 1.0),
                noise_offset: vec2(rng.range(0.0, 500.0), rng.range(0.0, 500.0)),
                reach: 0.0,
            };
            island.measure_reach(&shaping);
            island
        })
        .collect();
    islands.sort_by(|a, b| b.radius.total_cmp(&a.radius));

    let mut planner = Planner {
        extent,
        buffer,
        existing: existing_land.as_ref(),
        placed: Vec::new(),
    };

    // Archipelago centers, spread over the open water.
    let clusters = ((params.count as f32 / ISLANDS_PER_CLUSTER).round() as usize).max(1);
    let spread = CLUSTER_SPREAD_BASE + CLUSTER_SPREAD_PER_RADIUS * typical_radius;
    let inner = buffer + typical_radius;
    let random_point = |rng: &mut Rng| {
        vec2(
            rng.range(inner.min(extent.x * 0.5), (extent.x - inner).max(extent.x * 0.5)),
            rng.range(inner.min(extent.y * 0.5), (extent.y - inner).max(extent.y * 0.5)),
        )
    };
    let mut centers: Vec<Vec2> = Vec::new();
    for _ in 0..clusters {
        let best = (0..12)
            .map(|_| random_point(&mut rng))
            .max_by(|a, b| planner.openness(*a, &centers).total_cmp(&planner.openness(*b, &centers)))
            .unwrap();
        centers.push(best);
    }

    for mut island in islands {
        let clustered = rng.range(0.0, 1.0) > params.scatter;
        let mut spot = None;
        if clustered {
            // Near the middle of a random archipelago, in the first spot that fits.
            let cluster = centers[(rng.range(0.0, clusters as f32) as usize).min(clusters - 1)];
            spot = (0..PLACEMENT_BATCHES * CANDIDATES_PER_BATCH)
                .map(|_| cluster + vec2(gaussian(&mut rng), gaussian(&mut rng)) * spread)
                .find(|&p| planner.clearance(p, &island) >= 0.0);
        }
        if spot.is_none() {
            // Anywhere, preferring the spot with the most room, so islands spread out.
            for _ in 0..PLACEMENT_BATCHES {
                let best = (0..CANDIDATES_PER_BATCH)
                    .map(|_| random_point(&mut rng))
                    .max_by(|a, b| planner.clearance(*a, &island).total_cmp(&planner.clearance(*b, &island)))
                    .unwrap();
                if planner.clearance(best, &island) >= 0.0 {
                    spot = Some(best);
                    break;
                }
            }
        }
        if let Some(center) = spot {
            island.center = center;
            planner.placed.push(island);
        }
    }

    // Shape every placed island into a copy of the heightmap.
    let meters_per_unit = longest * CELL_SIZE;
    let mut heights = existing.to_vec();
    let mut added = 0;
    for island in &planner.placed {
        let reach_cells = island.reach * longest + 2.0;
        let (cx, cz) = (island.center.x * longest, island.center.y * longest);
        let (i0, i1) = (((cx - reach_cells).floor().max(0.0)) as usize, ((cx + reach_cells).ceil() as usize).min(nx - 1));
        let (j0, j1) = (((cz - reach_cells).floor().max(0.0)) as usize, ((cz + reach_cells).ceil() as usize).min(nz - 1));
        let mut land_samples = 0;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let idx = j * nx + i;
                let u = vec2(i as f32, j as f32) / longest;
                // Never in the buffer, and never touching land that is already there.
                if planner.edge_distance(u) < buffer
                    || existing_land.as_ref().is_some_and(|e| e.dist[idx] < EXISTING_MIN_GAP)
                {
                    continue;
                }
                let landness = island.landness(&shaping, u - island.center);
                // Approximate distance from the coast, in meters.
                let meters_in = landness * island.radius * meters_per_unit / FALLOFF;
                let h = coast_profile(meters_in);
                if h >= 0.0 {
                    land_samples += 1;
                }
                if h > heights[idx] {
                    heights[idx] = h;
                }
            }
        }
        if land_samples >= 2 {
            added += 1;
        }
    }
    add_relief(&mut heights, existing, nx, nz, params.seed, Relief::Lowlands);
    (heights, added)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELLS: usize = 512;

    fn ocean() -> Vec<f32> {
        vec![DEEP_OCEAN; (CELLS + 1) * (CELLS + 1)]
    }

    fn params(count: u32) -> IslandParams {
        IslandParams {
            count,
            size: 0.04,
            variety: 0.5,
            scatter: 0.5,
            roughness: 0.6,
            edge_buffer: 0.06,
            seed: 5,
        }
    }

    fn land_count(h: &[f32]) -> usize {
        h.iter().filter(|&&v| v >= 0.0).count()
    }

    #[test]
    fn adds_the_requested_islands_to_open_ocean() {
        let (heights, added) = generate(&params(20), CELLS, CELLS, &ocean());
        assert_eq!(added, 20);
        assert!(land_count(&heights) > 0);
    }

    #[test]
    fn never_enters_the_edge_buffer() {
        let p = IslandParams { count: 150, scatter: 0.0, ..params(150) };
        let (heights, _) = generate(&p, CELLS, CELLS, &ocean());
        let buffer = (p.edge_buffer * CELLS as f32).floor() as usize;
        let nx = CELLS + 1;
        for (k, &h) in heights.iter().enumerate() {
            let (i, j) = (k % nx, k / nx);
            let edge = i.min(CELLS - i).min(j).min(CELLS - j);
            assert!(edge >= buffer || h < 0.0, "land at ({i}, {j}) inside the {buffer}-cell buffer");
        }
    }

    #[test]
    fn leaves_existing_land_alone_and_keeps_clear_of_it() {
        // A square continent in the middle.
        let nx = CELLS + 1;
        let mut existing = ocean();
        for j in 180..330 {
            for i in 180..330 {
                existing[j * nx + i] = LAND_HEIGHT;
            }
        }
        let (heights, added) = generate(&params(40), CELLS, CELLS, &existing);
        assert!(added > 0);
        for (k, (&old, &new)) in existing.iter().zip(&heights).enumerate() {
            if old >= 0.0 {
                assert_eq!(old, new, "existing land changed at sample {k}");
            }
        }
        // No new land within a few cells of the old coast.
        for j in 170..340usize {
            for i in 170..340usize {
                let inside = (180..330).contains(&i) && (180..330).contains(&j);
                let near = (176..334).contains(&i) && (176..334).contains(&j);
                if near && !inside {
                    assert!(heights[j * nx + i] < 0.0, "new land touches the old coast at ({i}, {j})");
                }
            }
        }
    }

    #[test]
    fn places_fewer_when_there_is_no_room() {
        let p = IslandParams { size: 0.3, ..params(60) };
        let (_, added) = generate(&p, CELLS, CELLS, &ocean());
        assert!(added > 0 && added < 60, "added {added}");
    }

    /// Centers of the separate pieces of land in a heightmap.
    fn island_centers(heights: &[f32]) -> Vec<Vec2> {
        let nx = CELLS + 1;
        let mut seen = vec![false; heights.len()];
        let mut centers = Vec::new();
        for start in 0..heights.len() {
            if heights[start] < 0.0 || seen[start] {
                continue;
            }
            let (mut sum, mut count) = (Vec2::ZERO, 0.0);
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(k) = stack.pop() {
                sum += vec2((k % nx) as f32, (k / nx) as f32);
                count += 1.0;
                let (i, j) = (k % nx, k / nx);
                for (di, dj) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (ni, nj) = (i as i32 + di, j as i32 + dj);
                    if ni < 0 || nj < 0 || ni >= nx as i32 || nj >= nx as i32 {
                        continue;
                    }
                    let n = nj as usize * nx + ni as usize;
                    if heights[n] >= 0.0 && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            centers.push(sum / count);
        }
        centers
    }

    #[test]
    fn clustering_groups_islands_together() {
        // Clustered islands have closer neighbours than scattered ones.
        let nearest_neighbour = |scatter: f32, seed: u32| {
            let p = IslandParams { scatter, seed, ..params(30) };
            let (heights, _) = generate(&p, CELLS, CELLS, &ocean());
            let centers = island_centers(&heights);
            assert!(centers.len() >= 20, "only {} islands", centers.len());
            centers
                .iter()
                .map(|c| centers.iter().filter(|o| *o != c).map(|o| o.distance(*c)).fold(f32::MAX, f32::min))
                .sum::<f32>()
                / centers.len() as f32
        };
        let average = |scatter: f32| (1..=5).map(|seed| nearest_neighbour(scatter, seed)).sum::<f32>() / 5.0;
        let (clustered, scattered) = (average(0.0), average(1.0));
        assert!(clustered < scattered * 0.8, "{clustered} vs {scattered}");
    }

    #[test]
    fn same_seed_gives_the_same_islands() {
        let a = generate(&params(15), CELLS, CELLS, &ocean());
        let b = generate(&params(15), CELLS, CELLS, &ocean());
        assert_eq!(a.0, b.0);
        let c = generate(&IslandParams { seed: 6, ..params(15) }, CELLS, CELLS, &ocean());
        assert_ne!(a.0, c.0);
    }
}
