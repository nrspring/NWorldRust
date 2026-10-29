//! What another application does with a map: load one (or, with no file given, build a small one),
//! and ask it questions. Nothing here draws anything.
//!
//! ```text
//! cargo run -p nworld-map --example query_map -- path/to/map.nworld
//! ```

use nworld_map::continents::ContinentParams;
use nworld_map::storage;
use nworld_map::world::{Resource, World};

fn main() {
    let world = match std::env::args().nth(1) {
        Some(path) => {
            let saved = storage::load(std::path::Path::new(&path)).expect("couldn't load the map");
            World::from_saved(saved).expect("the map doesn't fit")
        }
        None => {
            let mut world = World::new(2048.0, 2048.0);
            world.generate_continents(&ContinentParams { coverage: 0.3, count: 2, roughness: 0.5, regularity: 0.5, edge_buffer: 0.1, seed: 7 });
            world.add_shallows(60.0);
            world.add_rivers(2, 20.0, 0.5, 3);
            world.divide_into_regions(400.0, 1);
            world
        }
    };

    let size = world.size();
    println!("A map {} m by {} m.", size.x, size.y);
    println!("Highest ground: {:.0} m.", world.max_surface_height());

    // The point at the middle of the map, and a few others along a line through it.
    for k in 0..5 {
        let x = (k as f32 - 2.0) * size.x * 0.2;
        let z = 0.0;
        println!(
            "({x:>7.0}, {z:>5.0}): {:>5.1} m high, {:?}, region {:?}, {:.0} m2 of forest within 200 m",
            world.surface_height(x, z),
            world.water_class(x, z),
            world.region_at(x, z).map(|region| (region.kind(), region.raw())),
            world.resource_near(Resource::Forest, x, z, 200.0),
        );
    }
}
