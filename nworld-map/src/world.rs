//! The world: a finite heightmap with the water standing on it, the resources painted on it and the
//! regions it is divided into, and what can be asked of them. It knows nothing about how any of it
//! is drawn; a display builds its own meshes and textures from what is here, using the revision and
//! stamps to know what has changed.

use glam::*;

use crate::gen_common::ExistingLand;
use crate::regions::RegionId;
use crate::storage::SavedMap;

static NEXT_WORLD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub const SEA_LEVEL: f32 = 0.0;
/// Spacing between heightmap samples, in world units (meters).
pub const CELL_SIZE: f32 = 4.0;
/// Every point of a new world starts this far below sea level.
pub const DEEP_OCEAN: f32 = -200.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// A sea world with a 3 m shelf in its west quarter and a 6 m one beside it, an island in its
    /// east, and a 60 m deep lake inland of the island.
    fn world_for_ships() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![DEEP_OCEAN; nx * nz];
        let mut water = vec![SEA_LEVEL; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                heights[j * nx + i] = match i {
                    0..=15 => -3.0,
                    16..=31 => -6.0,
                    48.. => 10.0,
                    _ => DEEP_OCEAN,
                };
            }
        }
        for j in 30..=34 {
            for i in 52..=56 {
                heights[j * nx + i] = -50.0;
                water[j * nx + i] = 10.0;
            }
        }
        world.set_terrain(heights, water);
        world
    }

    #[test]
    fn ships_find_shallow_deep_and_dry_water() {
        let world = world_for_ships();
        // World x is -128 + 4 * sample column.
        let at = |column: f32, row: f32| world.water_class(-128.0 + 4.0 * column, -128.0 + 4.0 * row);
        assert_eq!(at(5.0, 20.0), WaterClass::Shallow, "3 m of sea");
        assert_eq!(at(20.0, 20.0), WaterClass::Deep, "6 m of sea");
        assert_eq!(at(40.0, 20.0), WaterClass::Deep, "the open sea");
        assert_eq!(at(60.0, 20.0), WaterClass::Dry, "the island");
        assert_eq!(at(54.0, 32.0), WaterClass::Inland, "a lake is inland however deep it is dug");
        // Between a shallow and a deep sample the answer follows the water's depth there.
        assert_eq!(at(15.5, 20.0), WaterClass::Shallow, "4.5 m of sea");
        assert_eq!(at(16.0, 20.0), WaterClass::Deep);
        // Off the map is the nearest edge.
        assert_eq!(world.water_class(-5000.0, 0.0), WaterClass::Shallow);
        assert_eq!(world.water_class(5000.0, 0.0), WaterClass::Dry);
    }

    #[test]
    fn a_river_mouth_is_inland_only_as_far_as_its_own_samples() {
        let mut world = world_for_ships();
        let nx = world.samples().0;
        let (mut heights, mut water) = (world.heights().to_vec(), world.water().to_vec());
        // A river sample at column 41 beside the open sea at column 40, on row 20.
        heights[20 * nx + 41] = -1.0;
        water[20 * nx + 41] = 0.5;
        world.set_terrain(heights, water);
        let at = |column: f32| world.water_class(-128.0 + 4.0 * column, -128.0 + 4.0 * 20.0);
        assert_eq!(at(40.0), WaterClass::Deep);
        assert_eq!(at(40.4), WaterClass::Deep, "nearer the sea's sample");
        assert_eq!(at(41.0), WaterClass::Inland);
    }

    #[test]
    fn resources_near_a_point_add_up_by_area() {
        let mut world = World::new(512.0, 512.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        // A solid 40 x 40 sample patch of forest at half density around the middle.
        for j in 44..84 {
            for i in 44..84 {
                world.resource_mut(Resource::Forest)[j * nx + i] = 128;
            }
        }
        // The patch covers samples 44 to 83, so its middle is at sample 63.5.
        let middle = 63.5 * CELL_SIZE - world.half_extents().x;
        let whole_patch = world.resource_near(Resource::Forest, middle, middle, 200.0);
        let expected = 40.0 * 40.0 * CELL_SIZE * CELL_SIZE * 128.0 / 255.0;
        assert!((whole_patch - expected).abs() < 1.0, "{whole_patch} vs {expected}");
        // A small circle inside the patch holds about its own area at half density.
        let inside = world.resource_near(Resource::Forest, middle, middle, 40.0);
        let circle = std::f32::consts::PI * 40.0 * 40.0 * 128.0 / 255.0;
        assert!((inside - circle).abs() < circle * 0.1, "{inside} vs {circle}");
        // Other resources, and places far away, have none.
        assert_eq!(world.resource_near(Resource::Stone, middle, middle, 200.0), 0.0);
        assert_eq!(world.resource_near(Resource::Forest, middle + 600.0, middle, 100.0), 0.0);
        // A circle hanging off the map edge only counts what is on it.
        assert_eq!(world.resource_near(Resource::Forest, -5000.0, 0.0, 100.0), 0.0);
    }

    #[test]
    fn a_point_is_in_the_region_of_its_nearest_sample() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        assert_eq!(world.region_at(0.0, 0.0), None, "no regions yet");
        let mut regions = vec![0u16; nx * nz];
        regions[32 * nx + 32] = 0x4005;
        world.set_regions(regions);
        // The middle of the map is sample (32, 32).
        let id = world.region_at(1.0, -1.0).expect("a region");
        assert_eq!(id.raw(), 0x4005);
        assert_eq!(world.region_at(10.0, 0.0), None);
        // Off the map is the nearest edge.
        assert_eq!(world.region_at(-9000.0, -9000.0), None);
    }

    #[test]
    fn dug_lakes_are_inland_water_everywhere_it_is_looked_up() {
        let mut world = World::new(1024.0, 1024.0);
        let (nx, nz) = world.samples();
        // An island of 10 m ground with 20 samples of open sea all round it.
        let mut heights = vec![DEEP_OCEAN; nx * nz];
        for j in 20..nz - 20 {
            for i in 20..nx - 20 {
                heights[j * nx + i] = 10.0;
            }
        }
        let water = vec![SEA_LEVEL; nx * nz];
        let (heights, water, added) =
            crate::hills::scatter_lakes(&heights, &water, nx, nz, world.half_extents(), 5, 4, 200.0).expect("room for lakes");
        assert!(added > 0);
        world.set_terrain(heights, water);
        let half = world.half_extents();
        let mut lake_samples = 0;
        for j in 28..nz - 28 {
            for i in 28..nx - 28 {
                let idx = j * nx + i;
                if world.heights()[idx] >= world.water()[idx] {
                    continue;
                }
                lake_samples += 1;
                let (x, z) = (i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                assert!(world.water()[idx] > SEA_LEVEL, "a lake whose water is at sea level");
                assert_eq!(world.water_class(x, z), WaterClass::Inland, "lake sample ({i}, {j})");
                assert_eq!(crate::regions::kind_of(world.heights()[idx], world.water()[idx]), crate::regions::RegionKind::Land);
            }
        }
        assert!(lake_samples > 50, "only {lake_samples} lake samples");
    }

    #[test]
    fn dug_rivers_are_inland_water_up_to_where_they_meet_the_sea() {
        let mut world = World::new(1024.0, 1024.0);
        let (nx, nz) = world.samples();
        // An island sloping from 40 m in the east down to the sea in the west, with 20 samples of
        // open sea all round it.
        let mut heights = vec![DEEP_OCEAN; nx * nz];
        for j in 20..nz - 20 {
            for i in 20..nx - 20 {
                heights[j * nx + i] = 1.0 + (i - 20) as f32 * 0.2;
            }
        }
        let water = vec![SEA_LEVEL; nx * nz];
        let half = world.half_extents();
        let params = crate::rivers::RiverParams { from: vec2(-300.0, 0.0), to: vec2(300.0, 0.0), width: 24.0, winding: 0.5, seed: 3 };
        let dug = crate::rivers::dig(&heights, &water, nx, nz, half, &params).expect("a river");
        world.set_terrain(dug.heights, dug.water);
        let (mut river, mut at_sea_level) = (0, 0);
        for j in 28..nz - 28 {
            for i in 28..nx - 28 {
                let idx = j * nx + i;
                if world.heights()[idx] >= world.water()[idx] {
                    continue;
                }
                river += 1;
                let (x, z) = (i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
                if world.water()[idx] > SEA_LEVEL {
                    assert_eq!(world.water_class(x, z), WaterClass::Inland, "river sample ({i}, {j})");
                } else {
                    at_sea_level += 1;
                }
            }
        }
        assert!(river > 50, "only {river} river samples");
        assert!(at_sea_level * 20 < river, "{at_sea_level} of {river} river samples are at sea level");
    }

    #[test]
    fn painting_a_lake_moves_the_beaches_with_it() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![20.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        assert!(!world.near_water[32 * nx + 34]);

        world.heights_mut()[32 * nx + 32] = 14.0;
        world.water_mut()[32 * nx + 32] = 20.0;
        let region = Region { i0: 32, j0: 32, i1: 32, j1: 32 };
        let texels = world.refresh_region(region);
        assert!(world.near_water[32 * nx + 34], "the new lake has a beach");
        assert!(texels.i0 <= 28 && texels.i1 >= 36 && texels.j0 <= 28 && texels.j1 >= 36, "{texels:?}");
    }
}

/// Heightmap cells along a chunk side. The map is a whole number of chunks across, which are the
/// units a display can track changes by.
pub const CHUNK_CELLS: usize = 16;
/// Map sides are whole numbers of chunks.
pub const CHUNK_SIZE: f32 = CELL_SIZE * CHUNK_CELLS as f32;
pub const MIN_MAP_SIZE: f32 = 256.0;
pub const MAX_MAP_SIZE: f32 = 12288.0;

/// Whether a map side of this many heightmap cells is a size the world can be built at.
pub fn cells_valid(cells: usize) -> bool {
    let meters = cells as f32 * CELL_SIZE;
    snap_map_size(meters) == meters
}

/// Rounds a requested map side (meters) to the nearest size the world can be built at.
pub fn snap_map_size(meters: f32) -> f32 {
    ((meters / CHUNK_SIZE).round() * CHUNK_SIZE).clamp(MIN_MAP_SIZE, MAX_MAP_SIZE)
}

/// Something painted onto the map as a layer of density, from 0 to 255 at each sample: forest,
/// stone, iron, copper, sulphur or oil. A display shows them as it likes; the editor draws trees,
/// rocks, ore, crystals and tar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Forest,
    Stone,
    Iron,
    Copper,
    Sulphur,
    Oil,
}

impl Resource {
    pub const ALL: [Resource; 6] =
        [Resource::Forest, Resource::Stone, Resource::Iron, Resource::Copper, Resource::Sulphur, Resource::Oil];
    /// How many resources there are.
    pub const COUNT: usize = Self::ALL.len();

    /// The resource's place in lists with one entry per resource.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// What a point of the map is, as far as ships are concerned.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WaterClass {
    /// Ground above the water.
    Dry,
    /// Rivers and lakes: water above sea level, however deep. Only small ships sail it.
    Inland,
    /// Sea no deeper than [`SHALLOW_LIMIT`]. Large and small ships sail it.
    Shallow,
    /// Sea deeper than [`SHALLOW_LIMIT`]. Only large ships sail it.
    Deep,
}

pub struct World {
    /// Heightmap cells along x (west to east) and z (north to south).
    cells_x: usize,
    cells_z: usize,
    /// (cells_x + 1) × (cells_z + 1) elevation samples, row-major from the north-west corner.
    heights: Vec<f32>,
    /// The level of the water at each sample, which holds wherever the ground is lower: sea level
    /// everywhere except around a lake, where it is the lake's own level.
    water: Vec<f32>,
    /// For each sample, whether water is close enough that there might be a beach there; see
    /// [`near_water`].
    near_water: Vec<bool>,
    /// How much of each [`Resource`] there is at each sample, from 0 (none) to 255 (the densest).
    resources: [Vec<u8>; Resource::COUNT],
    /// The region each sample is in, as a [`RegionId`] number, or 0 where there are none yet.
    regions: Vec<u16>,
    chunks_x: usize,
    /// For each chunk and resource, a number that changes whenever anything its props depend on
    /// does.
    stamps: Vec<[u32; Resource::COUNT]>,
    /// For each chunk, a number that changes whenever the ground or water in or around it does.
    terrain_stamps: Vec<u32>,
    /// How many building steps have been run on this world; see [`World::next_seed`].
    generations: u32,
    /// Which world this is, and how many times its terrain has changed; see [`World::revision`].
    id: u64,
    version: u64,
}

impl World {
    /// A new world of the given size in meters (snapped with [`snap_map_size`]): deep ocean
    /// everywhere, centered on the origin.
    pub fn new(width: f32, height: f32) -> Self {
        let cells = |meters: f32| (snap_map_size(meters) / CELL_SIZE) as usize;
        let (cells_x, cells_z) = (cells(width), cells(height));
        let (chunks_x, chunks_z) = (cells_x / CHUNK_CELLS, cells_z / CHUNK_CELLS);
        let samples = (cells_x + 1) * (cells_z + 1);
        Self {
            cells_x,
            cells_z,
            heights: vec![DEEP_OCEAN; samples],
            water: vec![SEA_LEVEL; samples],
            near_water: vec![true; samples],
            resources: std::array::from_fn(|_| vec![0; samples]),
            regions: vec![0; samples],
            chunks_x,
            stamps: vec![[0; Resource::COUNT]; chunks_x * chunks_z],
            terrain_stamps: vec![0; chunks_x * chunks_z],
            generations: 0,
            id: NEXT_WORLD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            version: 0,
        }
    }

    /// A world from a saved map, or why the map doesn't fit.
    pub fn from_saved(saved: SavedMap) -> Result<Self, String> {
        let mut world = Self::new(saved.cells_x as f32 * CELL_SIZE, saved.cells_z as f32 * CELL_SIZE);
        if world.cells() != (saved.cells_x, saved.cells_z) {
            return Err("The map size in this file isn't supported.".to_owned());
        }
        world.set_terrain(saved.heights, saved.water);
        for (kind, layer) in Resource::ALL.into_iter().zip(saved.resources) {
            world.set_resource(kind, layer);
        }
        world.set_regions(saved.regions);
        world.generations = saved.generations;
        Ok(world)
    }

    /// The world as a saved map.
    pub fn to_saved(&self) -> SavedMap {
        SavedMap {
            cells_x: self.cells_x,
            cells_z: self.cells_z,
            generations: self.generations,
            heights: self.heights.clone(),
            water: self.water.clone(),
            resources: Resource::ALL.map(|kind| self.resource(kind).to_vec()),
            regions: self.regions.clone(),
        }
    }

    /// The seed for the next building step: the one asked for, mixed with how many steps there
    /// have been on this world, so running the same step again adds something different, even after
    /// an undo. The same world, step, seed and number of earlier steps give the same result.
    pub(crate) fn next_seed(&mut self, seed: u32) -> u32 {
        let salted = seed.wrapping_add(self.generations.wrapping_mul(0x9E37_79B9));
        self.generations += 1;
        salted
    }

    /// How many building steps have been run on this world.
    pub fn generations(&self) -> u32 {
        self.generations
    }

    /// Identifies this world and the state of its terrain: it differs whenever the heights
    /// have changed since it was last read, or for a different world.
    pub fn revision(&self) -> (u64, u64) {
        (self.id, self.version)
    }

    /// Width (x) and height (z) of the world in meters.
    pub fn size(&self) -> Vec2 {
        vec2(self.cells_x as f32, self.cells_z as f32) * CELL_SIZE
    }

    pub fn half_extents(&self) -> Vec2 {
        self.size() * 0.5
    }

    /// Heightmap cells along x and z.
    pub fn cells(&self) -> (usize, usize) {
        (self.cells_x, self.cells_z)
    }

    /// Every elevation sample, row-major from the north-west corner, (cells_x + 1) per row.
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// The water level at every sample, laid out as in `heights`: wherever the ground is lower
    /// than this it is under water. It is sea level except around lakes.
    pub fn water(&self) -> &[f32] {
        &self.water
    }

    /// How much of `kind` there is at every sample, laid out as in `heights`: 0 for none up to 255
    /// for the densest. Where its props actually stand is worked out from this and the terrain.
    pub fn resource(&self, kind: Resource) -> &[u8] {
        &self.resources[kind.index()]
    }

    /// Mutable densities of `kind`. After changing some, call
    /// [`resource_changed`](Self::resource_changed) for the part that changed.
    pub fn resource_mut(&mut self, kind: Resource) -> &mut [u8] {
        &mut self.resources[kind.index()]
    }

    /// Replaces every density of `kind` (laid out as in `heights`).
    pub fn set_resource(&mut self, kind: Resource, layer: Vec<u8>) {
        assert_eq!(layer.len(), self.heights.len(), "resource map size mismatch");
        self.resources[kind.index()] = layer;
        self.stamps.iter_mut().for_each(|s| s[kind.index()] = s[kind.index()].wrapping_add(1));
    }

    /// Notes that the densities of `kind` at the samples in `region` changed.
    pub fn resource_changed(&mut self, kind: Resource, region: Region) {
        let (cx0, cx1) = (region.i0.saturating_sub(1) / CHUNK_CELLS, ((region.i1 + 1) / CHUNK_CELLS).min(self.chunks_x - 1));
        let chunks_z = self.stamps.len() / self.chunks_x;
        let (cz0, cz1) = (region.j0.saturating_sub(1) / CHUNK_CELLS, ((region.j1 + 1) / CHUNK_CELLS).min(chunks_z - 1));
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let stamp = &mut self.stamps[cz * self.chunks_x + cx][kind.index()];
                *stamp = stamp.wrapping_add(1);
            }
        }
    }

    /// The region every sample is in, laid out as in `heights`, as the numbers of
    /// [`RegionId`]s (0 for none). Regions stay as they were made however the terrain changes
    /// after, until they are made again.
    pub fn regions(&self) -> &[u16] {
        &self.regions
    }

    /// Replaces every sample's region (laid out as in `heights`).
    pub fn set_regions(&mut self, regions: Vec<u16>) {
        assert_eq!(regions.len(), self.heights.len(), "region map size mismatch");
        self.regions = regions;
    }

    /// The region the point (`x`, `z`) is in, or `None` before the map is divided into regions.
    /// Points off the map count as the nearest edge.
    pub fn region_at(&self, x: f32, z: f32) -> Option<RegionId> {
        let half = self.half_extents();
        let i = (((x + half.x) / CELL_SIZE).round().max(0.0) as usize).min(self.cells_x);
        let j = (((z + half.y) / CELL_SIZE).round().max(0.0) as usize).min(self.cells_z);
        RegionId::from_raw(self.regions[j * (self.cells_x + 1) + i])
    }

    /// Chunks along x and along z.
    pub fn chunk_counts(&self) -> (usize, usize) {
        (self.chunks_x, self.stamps.len() / self.chunks_x)
    }

    /// A number that is different whenever the terrain, the water or the `kind` resource in or
    /// around chunk (`cx`, `cz`) has changed since it was last read.
    pub fn chunk_stamp(&self, kind: Resource, cx: usize, cz: usize) -> u32 {
        self.stamps[cz * self.chunks_x + cx][kind.index()]
    }

    /// A number that is different whenever the ground or the water in or around chunk (`cx`, `cz`)
    /// has changed since it was last read, for a display to know which of its meshes to rebuild.
    pub fn terrain_stamp(&self, cx: usize, cz: usize) -> u32 {
        self.terrain_stamps[cz * self.chunks_x + cx]
    }

    /// How much of `kind` lies within `radius` meters of (`x`, `z`), as the area in square meters
    /// that fully dense cover would take up: painted at half density, a patch counts for half its
    /// area. Divide by the circle's area, `PI * radius * radius`, for how much of it is covered.
    /// Parts of the circle off the map count for nothing.
    pub fn resource_near(&self, kind: Resource, x: f32, z: f32, radius: f32) -> f32 {
        let half = self.half_extents();
        let (nx, nz) = self.samples();
        // Samples sit at whole cells from the north-west corner.
        let (cx, cz) = ((x + half.x) / CELL_SIZE, (z + half.y) / CELL_SIZE);
        let reach = radius / CELL_SIZE;
        let (i0, i1) = ((cx - reach).ceil().max(0.0) as usize, (cx + reach).floor().min((nx - 1) as f32) as usize);
        let (j0, j1) = ((cz - reach).ceil().max(0.0) as usize, (cz + reach).floor().min((nz - 1) as f32) as usize);
        let layer = &self.resources[kind.index()];
        let mut total = 0.0;
        for j in j0..=j1 {
            let dz = j as f32 - cz;
            for i in i0..=i1 {
                let dx = i as f32 - cx;
                if dx * dx + dz * dz <= reach * reach {
                    total += layer[j * nx + i] as f32;
                }
            }
        }
        total / 255.0 * CELL_SIZE * CELL_SIZE
    }

    /// What kind of water, if any, covers the point (`x`, `z`). Water whose level is above sea
    /// level is inland (a river or lake, however deep it is dug), found from the nearest sample;
    /// sea is shallow down to [`SHALLOW_LIMIT`] and deep below. Points off the map count as the
    /// nearest edge.
    pub fn water_class(&self, x: f32, z: f32) -> WaterClass {
        let half = self.half_extents();
        let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, self.cells_x as f32);
        let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, self.cells_z as f32);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let (i1, j1) = ((i + 1).min(self.cells_x), (j + 1).min(self.cells_z));
        let nx = self.cells_x + 1;
        let lerp = |layer: &[f32]| {
            let top = layer[j * nx + i] + (layer[j * nx + i1] - layer[j * nx + i]) * tx;
            let bottom = layer[j1 * nx + i] + (layer[j1 * nx + i1] - layer[j1 * nx + i]) * tx;
            top + (bottom - top) * tz
        };
        let (ground, level) = (lerp(&self.heights), lerp(&self.water));
        if ground >= level {
            WaterClass::Dry
        } else if self.water[(fz.round() as usize) * nx + fx.round() as usize] > SEA_LEVEL {
            WaterClass::Inland
        } else if level - ground <= SHALLOW_LIMIT {
            WaterClass::Shallow
        } else {
            WaterClass::Deep
        }
    }

    /// Whether the sample is close to water, which is where beaches are; see [`near_water`].
    pub fn beach_nearby(&self, idx: usize) -> bool {
        self.near_water[idx]
    }

    /// For each sample, whether water is close enough that there might be a beach there, laid out
    /// as in `heights`.
    pub fn near_water(&self) -> &[bool] {
        &self.near_water
    }

    /// Replaces every elevation sample (laid out as in `heights`), keeping the water levels.
    pub fn set_heights(&mut self, heights: Vec<f32>) {
        let water = std::mem::take(&mut self.water);
        self.set_terrain(heights, water);
    }

    /// Replaces every elevation sample and water level.
    pub fn set_terrain(&mut self, heights: Vec<f32>, water: Vec<f32>) {
        assert_eq!(heights.len(), self.heights.len(), "heightmap size mismatch");
        assert_eq!(water.len(), self.heights.len(), "water map size mismatch");
        self.near_water = near_water(&heights, &water, self.cells_x + 1, self.cells_z + 1);
        self.heights = heights;
        self.water = water;
        self.version += 1;
        self.stamps.iter_mut().flatten().for_each(|s| *s = s.wrapping_add(1));
        self.terrain_stamps.iter_mut().for_each(|s| *s = s.wrapping_add(1));
    }

    /// Height of the visible surface (land or water) at a world position.
    pub fn surface_height(&self, x: f32, z: f32) -> f32 {
        let half = self.half_extents();
        let fx = ((x + half.x) / CELL_SIZE).clamp(0.0, self.cells_x as f32);
        let fz = ((z + half.y) / CELL_SIZE).clamp(0.0, self.cells_z as f32);
        let (i, j) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let top = self.surface_sample(i, j) + (self.surface_sample(i + 1, j) - self.surface_sample(i, j)) * tx;
        let bottom = self.surface_sample(i, j + 1) + (self.surface_sample(i + 1, j + 1) - self.surface_sample(i, j + 1)) * tx;
        top + (bottom - top) * tz
    }

    /// Height of the visible surface, land or water, at a heightmap sample; a sample off the map
    /// counts as the nearest one on it.
    pub fn surface_sample(&self, gi: i32, gj: i32) -> f32 {
        let i = gi.clamp(0, self.cells_x as i32) as usize;
        let j = gj.clamp(0, self.cells_z as i32) as usize;
        let idx = j * (self.cells_x + 1) + i;
        self.heights[idx].max(self.water[idx])
    }

    /// Heightmap samples along x and z: one more than the cells.
    pub fn samples(&self) -> (usize, usize) {
        (self.cells_x + 1, self.cells_z + 1)
    }

    /// Mutable elevation samples, laid out as in [`heights`](Self::heights). After changing
    /// some, call [`refresh_region`](Self::refresh_region) for the part that changed. Changes
    /// made this way must not turn land into water or water into land; use
    /// [`set_heights`](Self::set_heights) for that.
    pub fn heights_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    /// Mutable water levels, laid out as in [`water`](Self::water); see
    /// [`heights_mut`](Self::heights_mut).
    pub fn water_mut(&mut self) -> &mut [f32] {
        &mut self.water
    }

    /// Notes that the samples in `region` changed after [`heights_mut`](Self::heights_mut) or
    /// [`water_mut`](Self::water_mut): works out again where the beaches are and bumps the stamps
    /// of the chunks affected. Returns the samples whose surroundings may have changed, which
    /// reach a little beyond `region` where the beaches move.
    pub fn refresh_region(&mut self, region: Region) -> Region {
        self.version += 1;
        let touched = self.refresh_near_water(region);
        // A chunk's normals look as far as one stride (up to 8 cells) past its own samples.
        const REACH: usize = 8;
        let cx0 = region.i0.saturating_sub(REACH) / CHUNK_CELLS;
        let cz0 = region.j0.saturating_sub(REACH) / CHUNK_CELLS;
        let cx1 = ((region.i1 + REACH) / CHUNK_CELLS).min(self.chunks_x - 1);
        let cz1 = ((region.j1 + REACH) / CHUNK_CELLS).min(self.cells_z / CHUNK_CELLS - 1);
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                let idx = cz * self.chunks_x + cx;
                self.stamps[idx].iter_mut().for_each(|s| *s = s.wrapping_add(1));
                self.terrain_stamps[idx] = self.terrain_stamps[idx].wrapping_add(1);
            }
        }
        touched
    }

    /// Works out again which samples near `region` have water close by, after its heights or
    /// water levels changed. Returns the samples that were looked at.
    fn refresh_near_water(&mut self, region: Region) -> Region {
        let (nx, nz) = self.samples();
        let reach = BEACH_REACH as usize;
        let span = Region {
            i0: region.i0.saturating_sub(reach),
            j0: region.j0.saturating_sub(reach),
            i1: (region.i1 + reach).min(nx - 1),
            j1: (region.j1 + reach).min(nz - 1),
        };
        let flooded = |i: usize, j: usize| self.heights[j * nx + i] < self.water[j * nx + i];
        let mut near = Vec::with_capacity(span.width() * span.height());
        for j in span.j0..=span.j1 {
            for i in span.i0..=span.i1 {
                let found = (j.saturating_sub(reach)..=(j + reach).min(nz - 1)).any(|nj| {
                    (i.saturating_sub(reach)..=(i + reach).min(nx - 1)).any(|ni| {
                        let (di, dj) = (ni.abs_diff(i) as f32, nj.abs_diff(j) as f32);
                        di * di + dj * dj <= BEACH_REACH * BEACH_REACH && flooded(ni, nj)
                    })
                });
                near.push(found);
            }
        }
        let mut k = 0;
        for j in span.j0..=span.j1 {
            for i in span.i0..=span.i1 {
                self.near_water[j * nx + i] = near[k];
                k += 1;
            }
        }
        span
    }

    /// Highest point of the visible surface anywhere in the world.
    pub fn max_surface_height(&self) -> f32 {
        self.heights.iter().copied().fold(SEA_LEVEL, f32::max)
    }
}

/// Sea this many meters deep or less is shallow water, which every ship can cross; deeper sea is
/// deep water. A display should start showing the sea as deep at this depth.
pub const SHALLOW_LIMIT: f32 = 5.0;

/// Samples this close to water (in samples) can have beach; beaches are at most about 12 m
/// wide, three samples, so this leaves a sample to spare.
const BEACH_REACH: f32 = 4.0;

/// Whether each sample of an `nx` by `nz` heightmap lies within [`BEACH_REACH`] of water, which
/// fills the ground lower than its `level`.
fn near_water(heights: &[f32], level: &[f32], nx: usize, nz: usize) -> Vec<bool> {
    // The distance transform measures distance to "land", so mark the water as land.
    let water: Vec<f32> = heights.iter().zip(level).map(|(&h, &w)| if h < w { 0.0 } else { -1.0 }).collect();
    match ExistingLand::new(&water, nx, nz, 1.0) {
        Some(distance) => distance.dist.iter().map(|&d| d <= BEACH_REACH).collect(),
        None => vec![false; heights.len()],
    }
}

/// A rectangle of heightmap samples, both corners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub i0: usize,
    pub j0: usize,
    pub i1: usize,
    pub j1: usize,
}

impl Region {
    pub fn width(&self) -> usize {
        self.i1 - self.i0 + 1
    }

    pub fn height(&self) -> usize {
        self.j1 - self.j0 + 1
    }

    /// The smallest region holding both.
    pub fn union(self, other: Region) -> Region {
        Region {
            i0: self.i0.min(other.i0),
            j0: self.j0.min(other.j0),
            i1: self.i1.max(other.i1),
            j1: self.j1.max(other.j1),
        }
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[test]
    fn large_maps_snap_to_whole_chunks_instead_of_being_cut_to_a_smaller_cap() {
        assert_eq!(snap_map_size(10000.0), 9984.0);
        assert_eq!(snap_map_size(6000.0), 6016.0);
        assert_eq!(snap_map_size(20000.0), MAX_MAP_SIZE);
        assert!(cells_valid((9984.0 / CELL_SIZE) as usize));
    }

    #[test]
    fn a_10000_by_6000_world_builds_and_generates() {
        use crate::continents::{self, ContinentParams};
        let t = std::time::Instant::now();
        let world = World::new(snap_map_size(10000.0), snap_map_size(6000.0));
        let (nx, nz) = ((9984.0 / CELL_SIZE) as usize, (6016.0 / CELL_SIZE) as usize);
        let heights = continents::generate(&ContinentParams {
            coverage: 0.3, count: 4, roughness: 0.4, regularity: 0.4,
            edge_buffer: 0.05, seed: 7,
        }, nx, nz, &vec![DEEP_OCEAN; (nx + 1) * (nz + 1)]);
        assert_eq!(heights.len(), (nx + 1) * (nz + 1));
        assert!(world.size().x > 9900.0);
        eprintln!("10000x6000 built and generated in {:?}", t.elapsed());
    }
}
