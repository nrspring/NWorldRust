//! Stone: the boulders and loose rock drawn where the stone resource is painted.
//!
//! A spot on a fixed grid across the world gets a rock with a chance equal to the stone density
//! there, thinned out and gathered into clumps by noise so the stone lies in patches, with many
//! small rocks and a few big ones, all rounded boulders. Rocks lie anywhere on dry ground, however
//! steep or high.

use macroquad::prelude::*;

use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use crate::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a rock can lie on.
const ROCK_SPACING: f32 = 4.0;
/// Rocks gather in clumps about this many meters across.
const CLUMP_SIZE: f32 = 26.0;

/// How rocks are drawn: boulders, piles and big boulders, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Stone,
        shapes: vec![boulder_mesh(), pile_mesh(), big_boulder_mesh()],
        generate: rocks_in_chunk,
        fade: (150.0, 420.0),
        draw_chunks: 8,
        wind: 0.0,
    }
}

/// The rocks of chunk (`cx`, `cz`).
fn rocks_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / ROCK_SPACING) as i32;
    let mut rocks = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * ROCK_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * ROCK_SPACING - half.y;
            let density = density_at(world, Resource::Stone, x, z);
            if density <= 0.0 {
                continue;
            }
            // Clumps: the chance swings between a third and all of the density by place.
            let clump = 0.3 + 0.7 * value_noise(x / CLUMP_SIZE, z / CLUMP_SIZE, 7);
            if accept >= density * clump {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            if heights[j * nx + i] < water[j * nx + i] {
                continue;
            }

            // Mostly small rocks, with a few big ones.
            let [big, squat, tint, rank] = randoms(gb ^ 0x1D, ga);
            let scale = 0.35 + 1.6 * big * big * big;
            let shape = match (scale > 1.0, variety < 0.5, variety < 0.7) {
                (true, true, _) => 2,
                (true, false, _) => 1,
                (false, _, true) => 0,
                (false, _, false) => 1,
            };
            rocks.push(Prop {
                pos: vec3(x, world.surface_height(x, z), z),
                scale,
                aspect: 0.65 + 0.5 * squat,
                yaw: variety * 97.0,
                tint,
                rank,
                shape,
            });
        }
    }
    rocks
}

const DARK: Vec3 = Vec3::new(0.27, 0.26, 0.25);
const LIGHT: Vec3 = Vec3::new(0.50, 0.48, 0.45);

/// A single rounded boulder.
fn boulder_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.8, 0.0), vec3(1.6, 1.0, 1.4), DARK, LIGHT, 0.3, 1);
    m.vertices
}

/// Three rocks heaped together.
fn pile_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.7, 0.0), vec3(1.3, 0.95, 1.2), DARK, LIGHT, 0.3, 2);
    m.blob(vec3(1.4, 0.45, 0.5), vec3(0.95, 0.7, 0.85), DARK, LIGHT, 0.3, 3);
    m.blob(vec3(-0.9, 0.4, -0.9), vec3(0.85, 0.65, 0.75), DARK, LIGHT, 0.3, 4);
    m.vertices
}

/// A big, broad boulder with a small rock at its foot.
fn big_boulder_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.95, 0.0), vec3(1.7, 1.15, 1.5), DARK, LIGHT, 0.25, 5);
    m.blob(vec3(1.5, 0.4, 0.6), vec3(0.8, 0.55, 0.7), DARK, LIGHT, 0.3, 6);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 10 m world with stone everywhere in its west half.
    fn quarry() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Stone)[j * nx + i] = 255;
            }
        }
        world
    }

    fn all_rocks(world: &World) -> Vec<Prop> {
        (0..16).flat_map(|k| rocks_in_chunk(world, k % 4, k / 4)).collect()
    }

    #[test]
    fn rocks_lie_where_the_stone_is_and_nowhere_else() {
        let world = quarry();
        let rocks = all_rocks(&world);
        assert!(rocks.len() > 300, "{} rocks", rocks.len());
        assert!(rocks.iter().all(|r| r.pos.x < CELL_SIZE), "a rock outside the stone");
        assert!(rocks.iter().all(|r| (r.pos.y - 10.0).abs() < 1e-3));
        assert_eq!(rocks.len(), all_rocks(&world).len());
    }

    #[test]
    fn there_are_many_small_rocks_and_a_few_big_ones_of_every_shape() {
        let rocks = all_rocks(&quarry());
        let small = rocks.iter().filter(|r| r.scale < 0.9).count() as f32 / rocks.len() as f32;
        let big = rocks.iter().filter(|r| r.scale > 1.6).count() as f32 / rocks.len() as f32;
        assert!(small > 0.5, "only {small} are small");
        assert!(big > 0.01 && big < 0.2, "{big} are big");
        for shape in 0..3 {
            assert!(rocks.iter().any(|r| r.shape == shape), "no rocks of shape {shape}");
        }
    }

    #[test]
    fn rocks_gather_in_clumps() {
        let world = quarry();
        let rocks = all_rocks(&world);
        // Count rocks in 16 m squares: clumping makes some empty and some crowded.
        let mut cells = std::collections::HashMap::new();
        for r in &rocks {
            *cells.entry(((r.pos.x / 16.0).floor() as i32, (r.pos.z / 16.0).floor() as i32)).or_insert(0usize) += 1;
        }
        let counts: Vec<usize> = (-8..0).flat_map(|i| (-8..8).map(move |j| (i, j))).map(|c| cells.get(&c).copied().unwrap_or(0)).collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(hi >= lo * 2 + 3, "squares hold {lo} to {hi} rocks");
    }

    #[test]
    fn rocks_stay_out_of_the_water() {
        let mut world = quarry();
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        for j in 0..nz {
            for i in 0..16 {
                heights[j * nx + i] = -20.0;
            }
        }
        world.set_terrain(heights, vec![0.0; nx * nz]);
        let half_x = world.half_extents().x;
        let rocks = all_rocks(&world);
        assert!(!rocks.is_empty());
        assert!(rocks.iter().all(|r| ((r.pos.x + half_x) / CELL_SIZE) as usize >= 15), "a rock in the sea");
    }

    #[test]
    fn rock_meshes_are_small_and_sit_on_the_ground() {
        for mesh in [boulder_mesh(), pile_mesh(), big_boulder_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest > -1.0 && lowest < 0.5, "the rock starts at {lowest}");
            assert!((1.0..4.0).contains(&highest), "a rock {highest} m tall");
        }
    }
}
