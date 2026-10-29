//! Copper: the green-streaked ore nuggets drawn where the copper resource is painted.
//!
//! A spot on a fixed grid across the world gets a nugget with a chance equal to the copper
//! density there, thinned out to separate pockets by noise, so the ore lies in round patches with
//! bare ground between them. Most nuggets are small with a few big ones, and they lie anywhere on
//! dry ground.

use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use crate::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a nugget can lie on.
const NUGGET_SPACING: f32 = 4.0;
/// Pockets of ore are about this many meters across, and as far apart again.
const POCKET_SIZE: f32 = 13.0;

/// How nuggets are drawn: rough lumps, heaps and spires of ore, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Copper,
        shapes: vec![nugget_mesh(), heap_mesh(), spire_mesh()],
        generate: nuggets_in_chunk,
        fade: (130.0, 380.0),
        draw_chunks: 7,
        wind: 0.0,
    }
}

/// The nuggets of chunk (`cx`, `cz`).
fn nuggets_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / NUGGET_SPACING) as i32;
    let mut nuggets = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * NUGGET_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * NUGGET_SPACING - half.y;
            let density = density_at(world, Resource::Copper, x, z);
            if density <= 0.0 {
                continue;
            }
            // Pockets: the ore is thick inside round patches, with a trace between them.
            let pocket = smoothstep(0.52, 0.72, value_noise(x / POCKET_SIZE, z / POCKET_SIZE, 21));
            if accept >= density * (0.1 + 0.9 * pocket) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            if heights[j * nx + i] < water[j * nx + i] {
                continue;
            }

            // Mostly small nuggets, with a few big ones.
            let [big, squat, tint, rank] = randoms(gb ^ 0x3B, ga);
            let scale = 0.35 + 1.7 * big * big * big;
            let shape = match (scale > 1.0, variety < 0.5, variety < 0.7) {
                (true, true, _) => 2,
                (true, false, _) => 1,
                (false, _, true) => 0,
                (false, _, false) => 1,
            };
            nuggets.push(Prop {
                pos: vec3(x, world.surface_height(x, z), z),
                scale,
                aspect: 0.7 + 0.5 * squat,
                yaw: variety * 97.0,
                tint,
                rank,
                shape,
            });
        }
    }
    nuggets
}

/// Copper-brown with blue-green verdigris on its lit faces.
const DARK: Vec3 = Vec3::new(0.30, 0.17, 0.10);
const LIGHT: Vec3 = Vec3::new(0.26, 0.64, 0.50);

/// A single rough nugget.
fn nugget_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.6, 0.0), vec3(1.1, 0.8, 1.0), DARK, LIGHT, 0.5, 31);
    m.vertices
}

/// Four nuggets heaped together.
fn heap_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.55, 0.0), vec3(0.9, 0.75, 0.85), DARK, LIGHT, 0.5, 32);
    m.blob(vec3(1.0, 0.35, 0.4), vec3(0.65, 0.5, 0.6), DARK, LIGHT, 0.5, 33);
    m.blob(vec3(-0.7, 0.3, -0.7), vec3(0.6, 0.45, 0.55), DARK, LIGHT, 0.5, 34);
    m.blob(vec3(0.2, 0.3, -1.0), vec3(0.5, 0.4, 0.5), DARK, LIGHT, 0.5, 35);
    m.vertices
}

/// A spire of ore with a nugget at its foot.
fn spire_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 1.1, 0.0), vec3(0.5, 1.3, 0.45), DARK, LIGHT, 0.45, 36);
    m.blob(vec3(0.7, 0.35, 0.25), vec3(0.55, 0.45, 0.5), DARK, LIGHT, 0.5, 37);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 10 m world with copper everywhere in its west half.
    fn mine() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Copper)[j * nx + i] = 255;
            }
        }
        world
    }

    fn all_nuggets(world: &World) -> Vec<Prop> {
        (0..16).flat_map(|k| nuggets_in_chunk(world, k % 4, k / 4)).collect()
    }

    #[test]
    fn nuggets_lie_where_the_copper_is_and_nowhere_else() {
        let world = mine();
        let nuggets = all_nuggets(&world);
        assert!(nuggets.len() > 150, "{} nuggets", nuggets.len());
        assert!(nuggets.iter().all(|n| n.pos.x < CELL_SIZE), "a nugget outside the copper");
        assert!(nuggets.iter().all(|n| (n.pos.y - 10.0).abs() < 1e-3));
        assert_eq!(nuggets.len(), all_nuggets(&world).len());
    }

    #[test]
    fn there_are_many_small_nuggets_and_a_few_big_ones_of_every_shape() {
        let nuggets = all_nuggets(&mine());
        let small = nuggets.iter().filter(|n| n.scale < 0.7).count() as f32 / nuggets.len() as f32;
        let big = nuggets.iter().filter(|n| n.scale > 1.3).count() as f32 / nuggets.len() as f32;
        assert!(small > 0.5, "only {small} are small");
        assert!(big > 0.01 && big < 0.2, "{big} are big");
        for shape in 0..3 {
            assert!(nuggets.iter().any(|n| n.shape == shape), "no nuggets of shape {shape}");
        }
    }

    #[test]
    fn nuggets_lie_in_pockets() {
        let nuggets = all_nuggets(&mine());
        // Count nuggets in 16 m squares: pockets leave some squares nearly empty and crowd others.
        let mut cells = std::collections::HashMap::new();
        for n in &nuggets {
            *cells.entry(((n.pos.x / 16.0).floor() as i32, (n.pos.z / 16.0).floor() as i32)).or_insert(0usize) += 1;
        }
        let counts: Vec<usize> = (-8..0).flat_map(|i| (-8..8).map(move |j| (i, j))).map(|c| cells.get(&c).copied().unwrap_or(0)).collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(hi >= lo * 2 + 3, "squares hold {lo} to {hi} nuggets");
    }

    #[test]
    fn nuggets_stay_out_of_the_water() {
        let mut world = mine();
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        for j in 0..nz {
            for i in 0..16 {
                heights[j * nx + i] = -20.0;
            }
        }
        world.set_terrain(heights, vec![0.0; nx * nz]);
        let half_x = world.half_extents().x;
        let nuggets = all_nuggets(&world);
        assert!(!nuggets.is_empty());
        assert!(nuggets.iter().all(|n| ((n.pos.x + half_x) / CELL_SIZE) as usize >= 15), "a nugget in the sea");
    }

    #[test]
    fn nugget_meshes_are_small_and_sit_on_the_ground() {
        for mesh in [nugget_mesh(), heap_mesh(), spire_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 500);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest > -1.0 && lowest < 0.5, "the nugget starts at {lowest}");
            assert!((1.0..4.0).contains(&highest), "a nugget {highest} m tall");
        }
    }
}
