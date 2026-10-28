//! Continent generator: adds new continents to a map.
//!
//! Continent centers are spread over the map. Every heightmap sample gets a "landness" value
//! from its nearest continent, measured in a domain-warped space (simplex noise feeding simplex
//! noise) so outlines bend and curl, then shaped with large-scale and fractal simplex noise.
//! Channels along the borders between continents and a buffer around the map edge are kept as
//! open water. Sea level is set at the percentile that makes the new land cover the requested
//! share of the map.
//!
//! Generation is additive: land already on the map is kept as it is, new continents are placed
//! in open water, and a channel of water is kept between them and the existing land.

use std::f32::consts::PI;

use macroquad::prelude::{vec2, Vec2};

use crate::gen_common::{add_relief, ExistingLand, Relief, Rng, EXISTING_MIN_GAP, LAND_HEIGHT};
use crate::noise::{smoothstep, Simplex};
use crate::world::DEEP_OCEAN;

/// Meters of elevation per unit of landness right at the coast.
const COAST_SLOPE: f32 = 60.0;
/// How quickly the sea floor drops away from the coast (meters per squared unit of landness).
const SHELF_DROP: f32 = 3000.0;
/// The floor is only gentle for the first sample or so off the coast, which is all the renderer
/// needs to trace the coastline smoothly; past this much landness it plunges to the deep ocean,
/// so continents leave no shallow water around them.
const SHELF_KNEE: f32 = 0.015;
const SHELF_CLIFF: f32 = 800_000.0;
/// The fine wobble of every coast: its waves per map side, and how far it moves a coast, in
/// shares of the longest side, on the smoothest and (added) the roughest setting.
const FINE_FREQ: f32 = 40.0;
const FINE_WARP: f32 = 0.008;
const FINE_WARP_ROUGH: f32 = 0.025;
/// How far the keep-out line outside the edge buffer wanders inland, and the width of the
/// band beyond it over which land is eased back in, as shares of the longest side.
const EDGE_WOBBLE: f32 = 0.06;
const EDGE_EASE: f32 = 0.02;
/// Minimum width of the water channel between two continents, and the band beyond it over
/// which land eases away, in multiples of continent radius.
const CHANNEL: f32 = 0.12;
const CHANNEL_EASE: f32 = 0.15;
/// How much larger than the plain equal-area radius continents are drawn; see
/// `place_continents`.
const OUTLINE_SCALE: f32 = 1.25;
/// Random spots tried when placing each continent; more spreads them more evenly.
const PLACEMENT_CANDIDATES: usize = 30;
/// Continent-scale shape noise has about this many waves per (typical) continent radius.
const SHAPE_WAVES: f32 = 0.55;
/// Typical width of the water channel kept between new land and land already on the map, and
/// the band beyond it over which new land eases away, as shares of the longest side.
const EXISTING_GAP: f32 = 0.02;
const EXISTING_EASE: f32 = 0.05;
/// Beyond this distance from existing land, the channel rules don't apply.
const EXISTING_REACH: f32 = 0.14;
/// When adding to existing land, new land never takes more than this share of the room that is
/// actually free, so a crowded map keeps open water instead of having every gap packed with land.
const MAX_FREE_SHARE: f32 = 0.5;

#[derive(Clone, Copy)]
pub struct ContinentParams {
    /// Share of the whole map that these new continents add as land, 0..1; less if the room
    /// left over can't hold that much.
    pub coverage: f32,
    pub count: u32,
    /// Coastline roughness, 0 (smooth) ..1 (very ragged).
    pub roughness: f32,
    /// How regular continent shapes are, 0 (stretched out, lobed, full of bays and peninsulas)
    /// ..1 (compact and rounded).
    pub regularity: f32,
    /// Width of the open-water border around the map, as a share of its shorter side.
    pub edge_buffer: f32,
    pub seed: u32,
}

struct Continent {
    center: Vec2,
    radius: f32,
    /// Unit vector along the continent's long axis.
    axis: Vec2,
    /// How far the continent is stretched along its long axis (and squeezed across it by the
    /// same factor, so its area stays the same); 1 is round.
    stretch: f32,
}

impl Continent {
    /// Radius of the circle placement treats this continent as, big enough to cover its long
    /// arms when it is stretched.
    fn reach(&self) -> f32 {
        self.radius * (1.0 + (self.stretch - 1.0) * 0.6)
    }
}

/// Everything needed to evaluate landness at a point. Coordinates are normalized so the map's
/// longest side is 1.
struct Shape {
    noise: Simplex,
    continents: Vec<Continent>,
    extent: Vec2,
    roughness: f32,
    /// 0 (regular, compact shapes) ..1 (stretched, lobed, broken-up shapes).
    irregularity: f32,
    /// Spatial frequency of continent-scale shape noise, so its lobes scale with continent size.
    shape_freq: f32,
    /// Edge buffer width in normalized units.
    buffer: f32,
}

impl Shape {
    fn edge_distance(&self, p: Vec2) -> f32 {
        p.x.min(self.extent.x - p.x).min(p.y).min(self.extent.y - p.y)
    }

    /// Two layers of low-frequency domain warping, the second fed by the first, so continent
    /// outlines bend and curl instead of being round. Kept smooth so it bends shapes without
    /// smearing them.
    fn warp(&self, u: Vec2) -> Vec2 {
        let n = &self.noise;
        let a = vec2(
            n.fbm(u.x * 1.3 + 1.3, u.y * 1.3 + 7.2, 3, 0.5),
            n.fbm(u.x * 1.3 + 8.3, u.y * 1.3 + 2.8, 3, 0.5),
        );
        let b = vec2(
            n.fbm(u.x * 2.0 + a.x * 1.5 + 5.2, u.y * 2.0 + a.y * 1.5 + 1.7, 3, 0.5),
            n.fbm(u.x * 2.0 + a.x * 1.5 + 9.4, u.y * 2.0 + a.y * 1.5 + 6.1, 3, 0.5),
        );
        // A fine wobble on top, which moves every coast sideways by about the same amount
        // wherever it runs. The coastline detail added to the landness can't do that: where the
        // landness slopes steeply, as along the channels between continents, it hardly moves the
        // coast, which would come out smooth next to ragged ones.
        let fine = vec2(
            n.fbm(u.x * FINE_FREQ + 3.7, u.y * FINE_FREQ - 8.1, 4, 0.55),
            n.fbm(u.x * FINE_FREQ - 12.9, u.y * FINE_FREQ + 4.4, 4, 0.55),
        );
        u + b * (0.07 + 0.05 * self.roughness + 0.04 * self.irregularity)
            + fine * (FINE_WARP + FINE_WARP_ROUGH * self.roughness)
    }

    /// Landness at `u`, or `None` where only water is allowed: the edge buffer, the channels
    /// between continents, and the neighbourhood of existing land. `from_existing` is the
    /// distance from `u` to the nearest existing land, or infinity if there is none.
    fn landness(&self, u: Vec2, from_existing: f32) -> Option<f32> {
        // Distance beyond the inner edge of the buffer; inside the buffer only water is allowed.
        let past_buffer = self.edge_distance(u) - self.buffer;
        if past_buffer < 0.0 {
            return None;
        }
        let r = self.roughness;
        let q = self.warp(u);

        // Nearest and second-nearest continent, measured in multiples of each one's radius so
        // bigger continents claim more of the map.
        let (mut s1, mut s2) = (f32::MAX, f32::MAX);
        for c in &self.continents {
            // Distance in the continent's own frame: stretched along its long axis and
            // squeezed across it.
            let d = q - c.center;
            let along = d.dot(c.axis) / c.stretch;
            let across = d.perp_dot(c.axis) * c.stretch;
            let s = (along * along + across * across).sqrt() / c.radius;
            if s < s1 {
                (s2, s1) = (s1, s);
            } else if s < s2 {
                s2 = s;
            }
        }
        // Fractal coastline detail: barely any on smooth coasts, bays, capes and islets on
        // rough ones. Sampled unwarped so the warp doesn't stretch it into streaks.
        let octaves = 4 + (r * 4.0).round() as u32;
        let detail = self.noise.fbm(u.x * 9.0 - 7.7, u.y * 9.0 + 2.9, octaves, 0.4 + 0.2 * r)
            * (0.03 + 0.30 * r);

        // Large-scale noise gives every continent its own lobes, peninsulas and inland seas.
        let lobes = 0.7 * self.noise.fbm(u.x * 1.8 + 3.1, u.y * 1.8 - 5.3, 3, 0.5);
        // Continent-scale noise, in the warped space so it swirls: big bays, peninsulas and
        // inland seas, in proportion to how irregular the shapes should be.
        let t = self.irregularity;
        let bends = 0.5 * t
            * self.noise.fbm(q.x * self.shape_freq + 11.9, q.y * self.shape_freq - 6.1, 4, 0.55);
        // Land falls off from a continent's middle. Regular shapes use a steep quadratic fall,
        // which resists swelling and keeps outlines round; irregular ones a gentler, nearly
        // linear fall, which lets the noise push the coast far out or in.
        let falloff = s1.powf(2.0 - 0.6 * t);
        let mut v = 1.0 - falloff + lobes + bends + detail;

        if s2 < f32::MAX {
            // A channel of open water runs along the (warped) border between two continents,
            // so they can never join. Its edge gets the same coastline detail, and land eases
            // away from it so coasts don't follow its line.
            let gap = s2 - s1 + detail * 0.5;
            if gap < CHANNEL {
                return None;
            }
            let ease = 1.0 - smoothstep(CHANNEL, CHANNEL + CHANNEL_EASE, gap);
            v -= ease * ease * 0.8;
        }

        if from_existing < EXISTING_REACH {
            // A channel of open water around land that is already on the map, so new continents
            // never join onto it. Its width wanders with the same kinds of noise as every other
            // coast, from narrow straits to wide sounds, but never below a minimum.
            let required = (EXISTING_GAP + detail * 0.1 + lobes * 0.03).max(EXISTING_MIN_GAP);
            if from_existing < required {
                return None;
            }
            let ease = 1.0 - smoothstep(required, required + EXISTING_EASE, from_existing);
            v -= ease * ease * 0.8;
        }

        // As a backstop, a keep-out band just outside the buffer, along a wobbly line, pushes
        // land far under water. Coasts that still reach it follow its wobble and never reach
        // the buffer's straight edge.
        // Broad meanders, plus the same fine roughness as every other coast.
        // Exponential mapping: swings widely but never reaches zero, so the line never runs
        // flat along the buffer.
        let meander = (0.5 * (2.5 * self.noise.fbm(u.x * 1.6 + 21.7, u.y * 1.6 - 13.3, 4, 0.5)).exp()).min(1.5);
        let rough = self.noise.fbm(u.x * 9.0 + 5.5, u.y * 9.0 - 3.3, octaves, 0.4 + 0.2 * r);
        let wobble = (meander * EDGE_WOBBLE + rough * (0.004 + 0.035 * r)).max(0.0);
        // Strong enough that land stays out even when a lot of land pulls sea level far down.
        let keep_out = 1.0 - smoothstep(0.0, EDGE_EASE, past_buffer - wobble);
        Some(v - keep_out * 50.0)
    }
}

/// Picks continent sizes and spreads their centers across the area inside the edge buffer.
fn place_continents(
    rng: &mut Rng,
    shapes: &mut Rng,
    count: u32,
    base: f32,
    irregularity: f32,
    buffer: f32,
    extent: Vec2,
    existing: Option<&ExistingLand>,
) -> Vec<Continent> {
    let inner = buffer + EDGE_WOBBLE * 0.5;

    // Biggest first, so the large continents claim space before the small ones fill in.
    let mut radii: Vec<f32> = (0..count).map(|_| base * rng.range(0.35, 1.65).sqrt()).collect();
    radii.sort_by(|a, b| b.total_cmp(a));

    let mut continents: Vec<Continent> = Vec::new();
    for radius in radii {
        // Irregular continents are stretched out along a random direction. Their shapes come
        // from their own random stream, so changing the regularity doesn't move them around.
        let angle = shapes.range(0.0, PI);
        let stretch = (1.0 + irregularity * shapes.range(0.4, 2.0)).sqrt();
        // Placement treats a stretched continent as a bigger circle, so its long arms keep
        // clear of its neighbours.
        let reach = radius * (1.0 + (stretch - 1.0) * 0.6);
        // Clearance from a spot to everything already there: the gap to each placed
        // continent's outline, to land already on the map, and to the buffer, measured edge to
        // edge so bigger continents get proportionally more room.
        let clearance = |p: Vec2| {
            let to_edge = p.x.min(extent.x - p.x).min(p.y).min(extent.y - p.y) - inner - reach;
            let to_existing = existing.map_or(f32::INFINITY, |e| e.at(p) - EXISTING_GAP - reach);
            continents
                .iter()
                .map(|c| c.center.distance(p) - c.reach() - reach)
                .fold(to_edge.min(to_existing), f32::min)
        };
        // Best of many random candidates: the one with the most clearance, so continents
        // spread over the whole map. Random sampling keeps the layout irregular.
        let center = (0..PLACEMENT_CANDIDATES)
            .map(|_| vec2(rng.range(inner, extent.x - inner), rng.range(inner, extent.y - inner)))
            .max_by(|a, b| clearance(*a).total_cmp(&clearance(*b)))
            .unwrap();
        continents.push(Continent {
            center,
            radius,
            axis: vec2(angle.cos(), angle.sin()),
            stretch,
        });
    }
    continents
}

/// Adds continents to a heightmap of `cells_x` × `cells_z` cells, given as
/// (cells_x + 1) × (cells_z + 1) samples, row-major from the north-west corner, and returns
/// the new heights in the same layout. Existing land is kept exactly as it is.
pub fn generate(params: &ContinentParams, cells_x: usize, cells_z: usize, existing: &[f32]) -> Vec<f32> {
    let (nx, nz) = (cells_x + 1, cells_z + 1);
    assert_eq!(existing.len(), nx * nz, "heightmap size mismatch");
    let longest = cells_x.max(cells_z) as f32;
    let extent = vec2(cells_x as f32, cells_z as f32) / longest;
    let buffer = params.edge_buffer * extent.min_element();
    let existing_land = ExistingLand::new(existing, nx, nz, longest);

    // Plan around the room that is really free, so continents are sized to fit the gaps left
    // by existing land instead of swelling into them.
    let free = (0..nx * nz)
        .filter(|&k| {
            let (i, j) = (k % nx, k / nx);
            let edge = i.min(nx - 1 - i).min(j).min(nz - 1 - j) as f32 / longest;
            edge >= buffer && existing_land.as_ref().is_none_or(|e| e.dist[k] >= EXISTING_GAP)
        })
        .count() as f32
        / (nx * nz) as f32;
    // On an empty map the slider's full range is available; once there is land to work
    // around, new land is held to a share of the free room.
    let coverage = if existing_land.is_some() {
        params.coverage.min(free * MAX_FREE_SHARE)
    } else {
        params.coverage
    };

    let mut rng = Rng::new(params.seed);
    let mut shape_rng = Rng::new(params.seed ^ 0x5DEE_CE66);
    let count = params.count.max(1);
    let irregularity = (1.0 - params.regularity).clamp(0.0, 1.0);
    // Radius at which `count` equal circles would cover the requested share of the map,
    // enlarged to make up for land lost to channels and coastline noise, so sea level ends up
    // near each continent's natural outline instead of swelling continents to fill the gaps.
    let base = (coverage * extent.x * extent.y / (PI * count as f32)).sqrt() * OUTLINE_SCALE;
    let continents = place_continents(
        &mut rng,
        &mut shape_rng,
        count,
        base,
        irregularity,
        buffer,
        extent,
        existing_land.as_ref(),
    );
    let shape = Shape {
        noise: Simplex::new(params.seed),
        continents,
        extent,
        roughness: params.roughness,
        irregularity,
        shape_freq: SHAPE_WAVES / base,
        buffer,
    };

    // Evaluate landness for every sample, splitting the rows across all CPU cores. Samples in
    // the edge buffer get -inf so they can never become land.
    let mut values = vec![0.0f32; nx * nz];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows_per_band = nz.div_ceil(threads);
    std::thread::scope(|scope| {
        for (band, chunk) in values.chunks_mut(rows_per_band * nx).enumerate() {
            let (shape, existing_land) = (&shape, &existing_land);
            scope.spawn(move || {
                let first = band * rows_per_band * nx;
                for (k, v) in chunk.iter_mut().enumerate() {
                    let idx = first + k;
                    let (i, j) = (idx % nx, idx / nx);
                    let u = vec2(i as f32, j as f32) / longest;
                    let from_existing = existing_land.as_ref().map_or(f32::INFINITY, |e| e.dist[idx]);
                    *v = shape.landness(u, from_existing).unwrap_or(f32::NEG_INFINITY);
                }
            });
        }
    });

    // Sea level sits so the requested share of all samples becomes new land; if too little
    // room is left, every eligible sample becomes land.
    let eligible = values.iter().filter(|v| v.is_finite()).count();
    let land = ((coverage * values.len() as f32).round() as usize).min(eligible);
    let sea_level = if land == 0 {
        f32::INFINITY
    } else {
        let mut sorted = values.clone();
        *sorted.select_nth_unstable_by(values.len() - land, f32::total_cmp).1
    };

    let mut heights: Vec<f32> = values
        .into_iter()
        .zip(existing)
        .map(|(v, &old)| {
            if !v.is_finite() {
                return old.max(DEEP_OCEAN);
            }
            // Elevation follows landness smoothly through the coast, changing gently over
            // several samples there, so the renderer can trace the coastline between samples
            // as a smooth curve. Land rises to its baseline height near the coast; the sea
            // deepens slowly for a sample or so and then plunges to deep ocean.
            let x = v - sea_level;
            let new = if x >= 0.0 {
                (0.25 + x * COAST_SLOPE).min(LAND_HEIGHT)
            } else {
                -(0.25 - x * COAST_SLOPE + x * x * SHELF_DROP + SHELF_CLIFF * (-x - SHELF_KNEE).max(0.0).powi(2))
                    .min(-DEEP_OCEAN)
            };
            // Keep whichever is higher, so existing land and its shallows survive untouched.
            new.max(old)
        })
        .collect();
    add_relief(&mut heights, existing, nx, nz, params.seed, Relief::Continental);
    heights
}
