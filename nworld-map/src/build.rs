//! Building a map: the steps that lay out land and sea, raise mountains, dig lakes and rivers,
//! cut passes and divide the map into regions. Each is a method of [`World`] that changes it in
//! place and says what it did; none of them draws anything, or needs anything drawn.
//!
//! Every step is seeded: the seed asked for is mixed with how many steps the world has had, so
//! the same steps in the same order on the same world always give the same map.

use glam::Vec2;

use crate::continents::{self, ContinentParams};
use crate::hills;
use crate::islands::{self, IslandParams};
use crate::passes::{self, PassParams};
use crate::regions;
use crate::rivers::{self, RiverParams};
use crate::shallows;
use crate::world::World;

/// Stretches of one kind of ground smaller than this many samples are absorbed by their
/// surroundings before the map is divided into regions.
const SPECK_AREA: u32 = 30;

impl World {
    /// Adds continents to the land already on the map.
    pub fn generate_continents(&mut self, params: &ContinentParams) {
        let (cells_x, cells_z) = self.cells();
        let params = ContinentParams { seed: self.next_seed(params.seed), ..*params };
        let heights = continents::generate(&params, cells_x, cells_z, self.heights());
        self.set_heights(heights);
    }

    /// Sprinkles islands into the open water, and returns how many were added, which can be fewer
    /// than asked for if there is no room.
    pub fn add_islands(&mut self, params: &IslandParams) -> usize {
        let (cells_x, cells_z) = self.cells();
        let params = IslandParams { seed: self.next_seed(params.seed), ..*params };
        let (heights, added) = islands::generate(&params, cells_x, cells_z, self.heights());
        if heights != self.heights() {
            self.set_heights(heights);
        }
        added
    }

    /// Raises a randomly placed mountain range `length` meters long and `width` across. Returns
    /// false if there is no land with room for one.
    pub fn add_mountain_range(&mut self, length: f32, width: f32, seed: u32) -> bool {
        let (nx, nz) = self.samples();
        let seed = self.next_seed(seed);
        let Some(heights) = hills::mountain_range(self.heights(), nx, nz, self.half_extents(), seed, length, width) else {
            return false;
        };
        self.set_heights(heights);
        true
    }

    /// Digs randomly placed lakes, about `size` meters across, and returns how many fit, which can
    /// be fewer than asked for or none.
    pub fn add_lakes(&mut self, count: u32, size: f32, seed: u32) -> usize {
        let (nx, nz) = self.samples();
        let seed = self.next_seed(seed);
        let Some((heights, water, added)) = hills::scatter_lakes(self.heights(), self.water(), nx, nz, self.half_extents(), seed, count, size)
        else {
            return 0;
        };
        self.set_terrain(heights, water);
        added
    }

    /// Cuts a pass through the points (world x and z, at least two): a level road bed `width`
    /// meters wide, with walls of the given `steepness` (rise over run) where it cuts through high
    /// ground and embankments where it crosses low, along a smooth line winding by `winding` from
    /// 0 to 1 between the points. The error is a message fit to show a user.
    pub fn add_pass(&mut self, points: &[Vec2], width: f32, steepness: f32, winding: f32, seed: u32) -> Result<(), &'static str> {
        let (nx, nz) = self.samples();
        let seed = self.next_seed(seed);
        let params = PassParams { points: points.to_vec(), width, steepness, winding, seed };
        let heights = passes::cut(self.heights(), self.water(), nx, nz, self.half_extents(), &params)?;
        self.set_heights(heights);
        Ok(())
    }

    /// Digs a river between two points (world x and z), `width` meters wide in the middle,
    /// winding by `winding` from 0 to 1. It flows from the higher point to the lower. The error is
    /// a message fit to show a user.
    pub fn add_river(&mut self, from: Vec2, to: Vec2, width: f32, winding: f32, seed: u32) -> Result<(), &'static str> {
        let (nx, nz) = self.samples();
        let seed = self.next_seed(seed);
        let params = RiverParams { from, to, width, winding, seed };
        let dug = rivers::dig(self.heights(), self.water(), nx, nz, self.half_extents(), &params)?;
        self.set_terrain(dug.heights, dug.water);
        Ok(())
    }

    /// Digs up to `count` rivers from random inland points down to the sea, and returns how many
    /// fit.
    pub fn add_rivers(&mut self, count: u32, width: f32, winding: f32, seed: u32) -> usize {
        let (nx, nz) = self.samples();
        let seed = self.next_seed(seed);
        let Some((dug, added)) = rivers::scatter(self.heights(), self.water(), nx, nz, self.half_extents(), seed, count, width, winding)
        else {
            return 0;
        };
        self.set_terrain(dug.heights, dug.water);
        added
    }

    /// Makes the sea within `width` meters of land shallow. Returns false if there was nowhere
    /// to: no land, or the sea there is already shallow.
    pub fn add_shallows(&mut self, width: f32) -> bool {
        let (nx, nz) = self.samples();
        let Some(heights) = shallows::shelf(self.heights(), self.water(), nx, nz, width) else {
            return false;
        };
        self.set_heights(heights);
        true
    }

    /// Divides the map into regions about `size` meters across on average (see [`regions`]),
    /// replacing any made before. If that would be more regions than a map can hold, they come
    /// out bigger. Returns how many regions there are.
    pub fn divide_into_regions(&mut self, size: f32, seed: u32) -> usize {
        let (nx, nz) = self.samples();
        let mut kinds = regions::classify(self.heights(), self.water());
        regions::despeckle(&mut kinds, nx, nz, SPECK_AREA);
        let seed = self.next_seed(seed);
        let mut size = size;
        let ids = loop {
            if let Some(ids) = regions::divide(&kinds, nx, nz, size, seed) {
                break ids;
            }
            size *= 1.4;
        };
        let count = ids.iter().map(|&id| id & 0x3FFF).max().unwrap_or(0) as usize;
        self.set_regions(ids);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Resource, WaterClass, DEEP_OCEAN};

    fn island_world() -> World {
        let mut world = World::new(2048.0, 2048.0);
        world.generate_continents(&ContinentParams { coverage: 0.3, count: 2, roughness: 0.5, regularity: 0.5, edge_buffer: 0.1, seed: 3 });
        world
    }

    #[test]
    fn the_same_steps_in_the_same_order_make_the_same_map() {
        let (a, b) = (island_world(), island_world());
        assert_eq!(a.heights(), b.heights());
        assert!(a.heights().iter().any(|&h| h > 0.0), "no land");
        assert!(a.heights().iter().any(|&h| h == DEEP_OCEAN), "no open sea");
        // Running the step again adds something different.
        let mut c = island_world();
        c.generate_continents(&ContinentParams { coverage: 0.3, count: 2, roughness: 0.5, regularity: 0.5, edge_buffer: 0.1, seed: 3 });
        assert_ne!(a.heights(), c.heights());
        assert_eq!(c.generations(), 2);
    }

    #[test]
    fn a_whole_map_can_be_built_and_asked_about_without_any_display() {
        let mut world = island_world();
        assert!(world.add_mountain_range(800.0, 120.0, 4));
        assert!(world.add_shallows(60.0));
        assert!(world.add_lakes(3, 120.0, 5) > 0);
        assert!(world.add_rivers(2, 20.0, 0.5, 7) > 0);
        let regions = world.divide_into_regions(400.0, 1);
        assert!(regions > 5, "{regions} regions");

        // Sea, shore and land can all be told apart by asking the map.
        let half = world.half_extents();
        let classes: std::collections::HashSet<_> = (0..world.heights().len())
            .step_by(37)
            .map(|k| {
                let (nx, _) = world.samples();
                world.water_class((k % nx) as f32 * 4.0 - half.x, (k / nx) as f32 * 4.0 - half.y)
            })
            .collect();
        for class in [WaterClass::Dry, WaterClass::Shallow, WaterClass::Deep] {
            assert!(classes.contains(&class), "no {class:?} on the map");
        }
        assert!(world.region_at(0.0, 0.0).is_some());
        assert_eq!(world.resource_near(Resource::Forest, 0.0, 0.0, 200.0), 0.0);
    }

    #[test]
    fn a_built_map_survives_saving_and_loading() {
        let mut world = island_world();
        world.add_lakes(2, 100.0, 5);
        world.divide_into_regions(500.0, 1);
        let saved = world.to_saved();
        let back = World::from_saved(crate::storage::decode(&crate::storage::encode(&saved)).unwrap()).unwrap();
        assert_eq!(back.heights(), world.heights());
        assert_eq!(back.water(), world.water());
        assert_eq!(back.regions(), world.regions());
        assert_eq!(back.generations(), world.generations());
    }
}
