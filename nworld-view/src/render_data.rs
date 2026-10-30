//! What the editor feeds its shaders: the map's data encoded as textures, one texel per heightmap
//! sample. Which numbers go in which channel is a matter between these functions and the shaders in
//! `render.rs`; the map itself has no part in it.

use std::collections::HashMap;

use nworld_map::regions::RegionId;
use nworld_map::world::{Region, Resource, World, SEA_LEVEL};

/// How many textures hold the resources for the terrain shader, four to a texture.
pub const RESOURCE_TEXTURES: usize = Resource::COUNT.div_ceil(4);

/// Elevations up to this many meters are told apart in the surface texture; higher ground
/// reads as this.
pub const MAX_SURFACE_ELEVATION: f32 = 50.0;

/// How far above the water the shader tells ground apart, in the alpha channel.
const MAX_ABOVE_WATER: f32 = 8.0;

/// How deep the water is told apart, in the alpha channel; deeper water reads as this. Must
/// match `WATER_DEPTH_RANGE` in the terrain shader.
const MAX_WATER_DEPTH: f32 = 64.0;

/// Lakes and rivers read as water no deeper than this, in meters, so they stay shallow water.
const MAX_INLAND_DEPTH: f32 = 4.0;

/// A texel for the terrain shader: the coast in red, elevation in green, whether water is near
/// (on dry ground) or the water is inland (under water) in blue, and the height above the water,
/// or the depth of it, in alpha, as described on [`surface_texels`].
/// `w` is the water level at the sample.
fn surface_texel(h: f32, w: f32, near_water: bool) -> [u8; 4] {
    let coast = ((0.5 + (h - w) * 0.5).clamp(0.0, 1.0) * 255.0).round();
    let elevation = (((h - SEA_LEVEL).max(0.0) / MAX_SURFACE_ELEVATION).min(1.0) * 255.0).round();
    // Depth is stored as a square root, which tells shallow depths apart finely and deep ones
    // coarsely.
    let above = if h >= w {
        ((h - w) / MAX_ABOVE_WATER).clamp(0.0, 1.0)
    } else if w <= SEA_LEVEL {
        ((w - h) / MAX_WATER_DEPTH).clamp(0.0, 1.0).sqrt()
    } else {
        ((w - h).min(MAX_INLAND_DEPTH) / MAX_WATER_DEPTH).clamp(0.0, 1.0).sqrt()
    };
    // Blue is 255 on inland water (the shader tells it from sea by this) and 0 on sea; on dry
    // ground it marks water nearby.
    let blue = if h < w { w > SEA_LEVEL } else { near_water };
    [coast as u8, elevation as u8, if blue { 255 } else { 0 }, (above * 255.0).round() as u8]
}

/// The heightmap as RGBA texels for the terrain shader, one per sample in the same order.
/// Red is height around the water level, from 0 (1 m or more under water) through 128 (at the
/// water) to 255 (1 m or more above); the shader traces the coast where it crosses the
/// middle, which follows a smooth curve between samples. Green encodes
/// elevation from 0 to 50 m, which the shader colors: 25 shades of green up to 25 m, grey
/// rock from 26 m and snow from 44 m. Alpha is how far the ground stands above the water, up
/// to 8 m, which is as far up as a beach goes, and where the ground is under water, how deep
/// the water is, up to 64 m (stored as a square root), which tells shallow water from deep.
/// Lakes and rivers, whose water is above sea level, read as shallow however deep they are dug,
/// and blue marks them as inland.
pub fn surface_texels(world: &World) -> Vec<u8> {
    let (nx, nz) = world.samples();
    surface_texels_in(world, Region { i0: 0, j0: 0, i1: nx - 1, j1: nz - 1 })
}

/// The part of [`surface_texels`] inside `region`, row by row.
pub fn surface_texels_in(world: &World, region: Region) -> Vec<u8> {
    let (nx, _) = world.samples();
    let mut texels = Vec::with_capacity(region.width() * region.height() * 4);
    for j in region.j0..=region.j1 {
        let row = j * nx + region.i0..=j * nx + region.i1;
        let near = &world.near_water()[row.clone()];
        let water = &world.water()[row.clone()];
        texels.extend(
            world.heights()[row]
                .iter()
                .zip(water)
                .zip(near)
                .flat_map(|((&h, &w), &near)| surface_texel(h, w, near)),
        );
    }
    texels
}

/// The resource densities of the samples in `region` as RGBA texels for texture `texture` of
/// the terrain shader, row by row. Texture 0 holds forest in red, stone in green, iron in blue
/// and copper in alpha; texture 1 sulphur in red and oil in green; see [`RESOURCE_TEXTURES`].
pub fn resource_texels_in(world: &World, region: Region, texture: usize) -> Vec<u8> {
    let (nx, _) = world.samples();
    let channels: [Option<&[u8]>; 4] = std::array::from_fn(|c| Resource::ALL.get(texture * 4 + c).map(|&kind| world.resource(kind)));
    let mut texels = Vec::with_capacity(region.width() * region.height() * 4);
    for j in region.j0..=region.j1 {
        for k in j * nx + region.i0..=j * nx + region.i1 {
            texels.extend(channels.map(|layer| layer.map_or(0, |layer| layer[k])));
        }
    }
    texels
}

/// The regions as RGBA texels for the terrain shader, one per sample: the low byte of the
/// region's number in red and its high byte (which also holds its kind) in green.
pub fn region_texels(world: &World) -> Vec<u8> {
    world.regions().iter().flat_map(|&r| [r as u8, (r >> 8) as u8, 0, 255]).collect()
}

/// The color each sample's region is tinted with: red, green and blue, and alpha 255 where it is in one of
/// `colors`' regions, and all zero where it isn't. Laid out like [`region_texels`].
pub fn region_tint_texels(world: &World, colors: &[(RegionId, [u8; 3])]) -> Vec<u8> {
    let tint_of: HashMap<u16, [u8; 3]> = colors.iter().map(|(id, color)| (id.raw(), *color)).collect();
    world
        .regions()
        .iter()
        .flat_map(|r| match tint_of.get(r) {
            Some([red, green, blue]) => [*red, *green, *blue, 255],
            None => [0, 0, 0, 0],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nworld_map::world::{DEEP_OCEAN, SHALLOW_LIMIT};

    #[test]
    fn region_tints_cover_only_the_regions_given() {
        let mut world = World::new(256.0, 256.0);
        let mut regions = vec![0u16; world.heights().len()];
        regions[0] = 0x4005;
        regions[1] = 0x4006;
        regions[2] = 0x4005;
        world.set_regions(regions);
        let texels = region_tint_texels(&world, &[(RegionId::from_raw(0x4005).unwrap(), [10, 20, 30])]);
        assert_eq!(texels.len(), world.heights().len() * 4);
        assert_eq!(&texels[0..4], [10, 20, 30, 255]);
        assert_eq!(&texels[4..8], [0, 0, 0, 0], "another region");
        assert_eq!(&texels[8..12], [10, 20, 30, 255]);
        assert_eq!(&texels[12..16], [0, 0, 0, 0], "no region");
    }


    #[test]
    fn surface_encodes_lowland_and_hill_elevation_without_changing_coast() {
        let mut world = World::new(256.0, 256.0);
        let mut heights = world.heights().to_vec();
        heights[..6].copy_from_slice(&[-1.0, 0.0, 1.0, 7.0, 50.0, 200.0]);
        world.set_heights(heights);
        let texels = surface_texels(&world);
        let pixels: Vec<_> = texels.chunks_exact(4).take(6).collect();
        assert_eq!(pixels.iter().map(|p| p[0]).collect::<Vec<_>>(), [0, 128, 255, 255, 255, 255]);
        assert_eq!(pixels[0][1], 0);
        assert_eq!(pixels[1][1], 0);
        assert!(pixels[2][1] < pixels[3][1]);
        assert!(pixels[3][1] < pixels[4][1]);
        assert_eq!(pixels[4][1], 255);
        assert_eq!(pixels[5][1], 255);
    }

    /// A 256 m world of flat 20 m ground with a pit of 14 m at its middle, filled by a lake whose
    /// surface stands at 20 m.
    fn world_with_lake() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![20.0; nx * nz];
        let mut water = vec![SEA_LEVEL; nx * nz];
        for j in 29..=35 {
            for i in 29..=35 {
                water[j * nx + i] = 20.0;
            }
        }
        heights[32 * nx + 32] = 14.0;
        world.set_terrain(heights, water);
        world
    }

    #[test]
    fn a_lake_is_drawn_at_its_own_level() {
        let world = world_with_lake();
        let nx = world.samples().0;
        assert_eq!(world.surface_height(0.0, 0.0), 20.0, "the surface is the lake's, not the pit's");

        let texels = surface_texels(&world);
        let texel = |i: usize, j: usize| &texels[(j * nx + i) * 4..(j * nx + i) * 4 + 4];
        assert_eq!(texel(32, 32)[0], 0, "the pit is water");
        assert!(texel(32, 32)[3] > 0, "the pit's water has some depth");
        // Ground at the lake's level is its shore, and ground beyond its water is dry.
        assert_eq!(texel(31, 32)[0], 128);
        assert_eq!(texel(10, 10)[0], 255);
        assert_eq!(texel(10, 10)[3], 255, "20 m above the sea is far above the beach");
        assert_eq!(texel(31, 32)[3], 0, "the lake's shore is level with its water");
        // Beaches are found beside the lake, not far from it.
        assert!(world.near_water()[32 * nx + 36]);
        assert!(!world.near_water()[10 * nx + 10]);
    }

    #[test]
    fn the_surface_tells_shallow_water_from_deep() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![DEEP_OCEAN; nx * nz];
        let mut water = vec![SEA_LEVEL; nx * nz];
        heights[0] = -3.0;
        // A lake dug 60 m below its surface.
        heights[1] = -40.0;
        water[1] = 20.0;
        world.set_terrain(heights, water);
        let texels = surface_texels(&world);
        let depth = |k: usize| texels[k * 4 + 3];
        assert_eq!(depth(2), 255, "the open sea is as deep as it gets");
        assert!(depth(0) > 0 && depth(0) < 80, "shallow sea reads as {}", depth(0));
        assert!(depth(1) <= 70, "a lake is shallow however deep it is dug, not {}", depth(1));
        assert!(depth(0) < depth(1), "3 m is shallower than 4 m");
    }

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
    fn the_shader_starts_deep_water_where_ships_do() {
        let from_the_limit = format!("smoothstep({:.1}, 30.0, depth)", SHALLOW_LIMIT);
        assert!(crate::render::TERRAIN_FRAGMENT.contains(&from_the_limit), "update the shader's water colours");
    }

    #[test]
    fn the_surface_marks_inland_water_in_blue() {
        let world = world_for_ships();
        let nx = world.samples().0;
        let texels = surface_texels(&world);
        let blue = |i: usize, j: usize| texels[(j * nx + i) * 4 + 2];
        assert_eq!(blue(54, 32), 255, "a lake");
        assert_eq!(blue(5, 20), 0, "shallow sea");
        assert_eq!(blue(40, 20), 0, "deep sea");
        assert_eq!(blue(60, 20), 0, "dry ground far from the water");
        assert_eq!(blue(50, 32), 255, "the lake's shore");
    }

}
