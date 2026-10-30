//! Helpers for the tests.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// A data folder of its own, removed again when dropped.
pub struct TempRoot(pub PathBuf);

impl TempRoot {
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("nworld-test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A map 4 km across with one continent on it, for tests that need real ground.
pub fn small_continent() -> nworld_map::world::World {
    use nworld_map::continents::ContinentParams;
    let mut world = nworld_map::world::World::new(4096.0, 4096.0);
    world.generate_continents(&ContinentParams { coverage: 0.35, count: 1, roughness: 0.5, regularity: 0.5, edge_buffer: 0.1, seed: 3 });
    world
}

/// A point on dry land and a point in the sea, found on a grid over the map.
pub fn land_and_sea(world: &nworld_map::world::World) -> (macroquad::math::Vec2, macroquad::math::Vec2) {
    use nworld_map::world::WaterClass;
    let half = world.half_extents();
    let (mut land, mut sea) = (None, None);
    for j in 0..40 {
        for i in 0..40 {
            let p = macroquad::math::vec2(-half.x + (i as f32 + 0.5) * half.x * 2.0 / 40.0, -half.y + (j as f32 + 0.5) * half.y * 2.0 / 40.0);
            match world.water_class(p.x, p.y) {
                WaterClass::Dry => land = land.or(Some(p)),
                _ => sea = sea.or(Some(p)),
            }
        }
    }
    (land.expect("no land"), sea.expect("no sea"))
}

/// A point with dry land all around it, out to `radius` meters, found on a grid over the map.
pub fn inland_spot(world: &nworld_map::world::World, radius: f32) -> macroquad::math::Vec2 {
    use nworld_map::world::WaterClass;
    let half = world.half_extents();
    for j in 0..80 {
        for i in 0..80 {
            let p = macroquad::math::vec2(-half.x + (i as f32 + 0.5) * half.x * 2.0 / 80.0, -half.y + (j as f32 + 0.5) * half.y * 2.0 / 80.0);
            let all_dry = (0..16).all(|k| {
                let a = std::f32::consts::TAU * k as f32 / 16.0;
                let q = p + macroquad::math::vec2(a.cos(), a.sin()) * radius;
                world.water_class(q.x, q.y) == WaterClass::Dry
            });
            if all_dry && world.water_class(p.x, p.y) == WaterClass::Dry {
                return p;
            }
        }
    }
    panic!("no land with {radius} m of dry ground around it");
}

/// [`small_continent`] divided into regions about 120 m across.
pub fn regioned_continent() -> nworld_map::world::World {
    let mut world = small_continent();
    world.divide_into_regions(120.0, 1);
    world
}

/// `count` points on dry land below the mountains, spread over the map.
pub fn land_spots(world: &nworld_map::world::World, count: usize) -> Vec<macroquad::math::Vec2> {
    use nworld_map::regions::MOUNTAIN_HEIGHT;
    use nworld_map::world::WaterClass;
    let half = world.half_extents();
    let mut land = Vec::new();
    for j in 0..60 {
        for i in 0..60 {
            let p = macroquad::math::vec2(-half.x + (i as f32 + 0.5) * half.x * 2.0 / 60.0, -half.y + (j as f32 + 0.5) * half.y * 2.0 / 60.0);
            if world.water_class(p.x, p.y) == WaterClass::Dry && world.surface_height(p.x, p.y) < MOUNTAIN_HEIGHT {
                land.push(p);
            }
        }
    }
    assert!(land.len() >= count, "only {} land points", land.len());
    let step = land.len() / count;
    (0..count).map(|k| land[k * step]).collect()
}
