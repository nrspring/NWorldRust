# nworld-map

The NWorld map as a library: the terrain, the water standing on it, the resources and regions laid
over it, the steps that build them, saving and loading, and the questions a game can ask of it.

It draws nothing. It has two dependencies, `glam` for vectors and `miniz_oxide` for compressing
map files, and no graphics library, window or GPU. The NWorld editor (the parent crate) draws
maps; any other application can use them without it. A test (`tests/no_display_code.rs`) fails if
a graphics dependency or any drawing code finds its way in.

What an application lays over a map (roads, cities, armies) is its own business and has no place
here.

## The map

A [`World`](src/world.rs) is a square or rectangular grid of samples 4 m apart, from 256 m up to
12,288 m a side. Each sample has:

- a **height** (the ground) and a **water level** (the water standing over ground lower than it:
  sea level for the sea, higher for a lake or river);
- a **density** of each resource, 0 to 255: forest, stone, iron, copper, sulphur and oil;
- the **region** it belongs to, once the map has been divided into regions.

## Asking questions

```rust
use nworld_map::{storage, world::{World, Resource, WaterClass}};

let saved = storage::load(std::path::Path::new("map.nworld"))?;
let world = World::from_saved(saved)?;

world.surface_height(x, z);                      // metres, ground or water surface
world.water_class(x, z);                         // Dry, Inland, Shallow or Deep
world.resource_near(Resource::Iron, x, z, 300.0); // iron within 300 m, as m2 of full cover
world.region_at(x, z);                           // which region, and its kind of ground
```

`water_class` is what decides which ships can go where: large ships sail deep and shallow water,
small ones shallow and inland. Regions each hold one kind of ground (deep water, shallow water,
mountain or land), and land regions include lakes and rivers. Run `cargo run -p nworld-map
--example query_map` to see it, with or without a map file.

## Building maps

Every step of making a map is a method of `World` (see [`build`](src/build.rs)): `generate_continents`,
`add_islands`, `add_mountain_range`, `add_lakes`, `add_rivers`, `add_river`, `add_shallows`,
`add_pass` and `divide_into_regions`. They are seeded, and the same steps in the same order on the
same world always give the same map. The brushes the editor paints with (`hills::HillStroke`,
`shallows::ShallowStroke`, `density::DensityStroke`) are here too, working on a world's arrays.

## Drawing maps

A display builds its own meshes and textures from the arrays a world exposes (`heights`, `water`,
`resource`, `regions`). To know what to redraw, it reads the world's change counters instead of
being told:

- `revision()` changes whenever the terrain does, and differs for a different world;
- `terrain_stamp(cx, cz)` changes whenever the ground or water in or around a 64 m chunk does;
- `chunk_stamp(resource, cx, cz)` changes whenever anything a resource's display depends on does.

An editing tool that changes arrays directly (`heights_mut`, `water_mut`, `resource_mut`) tells the
world which part changed (`refresh_region`, `resource_changed`), which updates the counters.

## Saving

`storage` reads and writes `.nworld` files: compressed, validated on load, and written through a
temporary file so a failed save can't damage an existing map. Files from older versions of the
format still load.
