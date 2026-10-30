//! Iron: the rust-stained ore outcrops drawn where the iron resource is painted.
//!
//! A spot on a fixed grid across the world gets an outcrop with a chance equal to the iron
//! density there, thinned out to thin winding veins by noise, so the ore runs in seams through the
//! painted ground. Most outcrops are small rounded lumps with a few big ones, and they lie anywhere
//! on dry ground.

use macroquad::prelude::*;

use nworld_map::noise::smoothstep;
use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use nworld_map::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots an outcrop can lie on.
const OUTCROP_SPACING: f32 = 4.0;
/// Veins wind across the ground in bands about this many meters apart.
const VEIN_SPACING: f32 = 22.0;
/// How far the vein noise can be from its middle value and still be in a vein, as a share of
/// its range; bigger means fatter veins.
const VEIN_WIDTH: f32 = 0.2;

/// How outcrops are drawn: lumps, clusters and big boulders of ore, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Iron,
        shapes: vec![lump_mesh(), cluster_mesh(), big_lump_mesh()],
        generate: outcrops_in_chunk,
        fade: (130.0, 380.0),
        far_shapes: vec![lump_far_mesh(), cluster_far_mesh(), big_lump_far_mesh()],
        near_range: 110.0,
        wind: 0.0,
    }
}

/// The outcrops of chunk (`cx`, `cz`).
fn outcrops_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / OUTCROP_SPACING) as i32;
    let mut outcrops = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * OUTCROP_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * OUTCROP_SPACING - half.y;
            let density = density_at(world, Resource::Iron, x, z);
            if density <= 0.0 {
                continue;
            }
            // Veins: the ore is thickest where the noise crosses its middle value, with a trace
            // of it between.
            let off_vein = (value_noise(x / VEIN_SPACING, z / VEIN_SPACING, 13) - 0.5).abs() * 2.0;
            let vein = 1.0 - smoothstep(0.0, VEIN_WIDTH, off_vein);
            if accept >= density * (0.12 + 0.88 * vein) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            if heights[j * nx + i] < water[j * nx + i] {
                continue;
            }

            // Mostly small outcrops, with a few big ones.
            let [big, squat, tint, rank] = randoms(gb ^ 0x2F, ga);
            let scale = 0.35 + 1.5 * big * big * big;
            let shape = match (scale > 1.1, variety < 0.5, variety < 0.75) {
                (true, true, _) => 2,
                (true, false, _) => 1,
                (false, _, true) => 0,
                (false, _, false) => 1,
            };
            outcrops.push(Prop {
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
    outcrops
}

/// Dark iron-grey with rust on its lit faces.
const DARK: Vec3 = Vec3::new(0.24, 0.13, 0.10);
const LIGHT: Vec3 = Vec3::new(0.58, 0.29, 0.14);

/// A single rounded lump of ore.
fn lump_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.75, 0.0), vec3(1.4, 0.95, 1.2), DARK, LIGHT, 0.3, 11);
    m.vertices
}

/// Three lumps heaped together.
fn cluster_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.65, 0.0), vec3(1.15, 0.85, 1.05), DARK, LIGHT, 0.3, 12);
    m.blob(vec3(1.2, 0.4, 0.45), vec3(0.8, 0.6, 0.7), DARK, LIGHT, 0.3, 13);
    m.blob(vec3(-0.8, 0.35, -0.8), vec3(0.7, 0.55, 0.65), DARK, LIGHT, 0.3, 14);
    m.vertices
}

/// A big, broad boulder of ore with a lump at its foot.
fn big_lump_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.95, 0.0), vec3(1.6, 1.15, 1.4), DARK, LIGHT, 0.25, 15);
    m.blob(vec3(1.4, 0.4, 0.5), vec3(0.75, 0.55, 0.65), DARK, LIGHT, 0.3, 16);
    m.vertices
}

/// Each outcrop shape from afar: one coarse lump.
fn lump_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.0, 0.75, 0.0), vec3(1.4, 0.95, 1.2), DARK, LIGHT);
    m.vertices
}

fn cluster_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.2, 0.55, 0.0), vec3(1.9, 0.85, 1.5), DARK, LIGHT);
    m.vertices
}

fn big_lump_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.3, 0.9, 0.1), vec3(1.9, 1.15, 1.5), DARK, LIGHT);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_shapes_are_simple_versions_of_the_near_ones() {
        crate::props::check_far_shapes(&kind());
    }

    /// A flat 10 m world with iron everywhere in its west half.
    fn mine() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Iron)[j * nx + i] = 255;
            }
        }
        world
    }

    fn all_outcrops(world: &World) -> Vec<Prop> {
        (0..16).flat_map(|k| outcrops_in_chunk(world, k % 4, k / 4)).collect()
    }

    #[test]
    fn outcrops_lie_where_the_iron_is_and_nowhere_else() {
        let world = mine();
        let outcrops = all_outcrops(&world);
        assert!(outcrops.len() > 150, "{} outcrops", outcrops.len());
        assert!(outcrops.iter().all(|o| o.pos.x < CELL_SIZE), "an outcrop outside the iron");
        assert!(outcrops.iter().all(|o| (o.pos.y - 10.0).abs() < 1e-3));
        assert_eq!(outcrops.len(), all_outcrops(&world).len());
    }

    #[test]
    fn there_are_many_small_outcrops_and_a_few_big_ones_of_every_shape() {
        let outcrops = all_outcrops(&mine());
        let small = outcrops.iter().filter(|o| o.scale < 0.8).count() as f32 / outcrops.len() as f32;
        let big = outcrops.iter().filter(|o| o.scale > 1.5).count() as f32 / outcrops.len() as f32;
        assert!(small > 0.5, "only {small} are small");
        assert!(big > 0.01 && big < 0.2, "{big} are big");
        for shape in 0..3 {
            assert!(outcrops.iter().any(|o| o.shape == shape), "no outcrops of shape {shape}");
        }
    }

    #[test]
    fn outcrops_run_in_veins() {
        let outcrops = all_outcrops(&mine());
        // Count outcrops in 16 m squares: veins leave some squares empty and crowd others.
        let mut cells = std::collections::HashMap::new();
        for o in &outcrops {
            *cells.entry(((o.pos.x / 16.0).floor() as i32, (o.pos.z / 16.0).floor() as i32)).or_insert(0usize) += 1;
        }
        let counts: Vec<usize> = (-8..0).flat_map(|i| (-8..8).map(move |j| (i, j))).map(|c| cells.get(&c).copied().unwrap_or(0)).collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(hi >= lo * 2 + 3, "squares hold {lo} to {hi} outcrops");
    }

    #[test]
    fn outcrops_stay_out_of_the_water() {
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
        let outcrops = all_outcrops(&world);
        assert!(!outcrops.is_empty());
        assert!(outcrops.iter().all(|o| ((o.pos.x + half_x) / CELL_SIZE) as usize >= 15), "an outcrop in the sea");
    }

    #[test]
    fn outcrop_meshes_are_small_and_sit_on_the_ground() {
        for mesh in [lump_mesh(), cluster_mesh(), big_lump_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest > -1.0 && lowest < 0.5, "the outcrop starts at {lowest}");
            assert!((1.0..4.0).contains(&highest), "an outcrop {highest} m tall");
        }
    }
}
