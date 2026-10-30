//! Sulphur: the yellow lumps and crusts drawn where the sulphur resource is painted.
//!
//! A spot on a fixed grid across the world gets a cluster with a chance equal to the sulphur
//! density there, thinned out to round vents by noise, so the sulphur gathers around a few spots
//! with bare ground between them. Most clusters are small with a few big ones, and they lie
//! anywhere on dry ground.

use macroquad::prelude::*;

use nworld_map::noise::smoothstep;
use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use nworld_map::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a cluster can lie on.
const CLUSTER_SPACING: f32 = 4.0;
/// Vents of sulphur are about this many meters across, and as far apart again.
const VENT_SIZE: f32 = 11.0;

/// How clusters are drawn: lumps, crusts and big heaps, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Sulphur,
        shapes: vec![lump_mesh(), crust_mesh(), cluster_mesh()],
        generate: clusters_in_chunk,
        fade: (130.0, 380.0),
        far_shapes: vec![lump_far_mesh(), crust_far_mesh(), cluster_far_mesh()],
        near_range: 110.0,
        wind: 0.0,
    }
}

/// The clusters of chunk (`cx`, `cz`).
fn clusters_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / CLUSTER_SPACING) as i32;
    let mut clusters = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * CLUSTER_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * CLUSTER_SPACING - half.y;
            let density = density_at(world, Resource::Sulphur, x, z);
            if density <= 0.0 {
                continue;
            }
            // Vents: the sulphur is thick inside round patches, with a trace between them.
            let vent = smoothstep(0.55, 0.75, value_noise(x / VENT_SIZE, z / VENT_SIZE, 31));
            if accept >= density * (0.08 + 0.92 * vent) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            if heights[j * nx + i] < water[j * nx + i] {
                continue;
            }

            // Mostly small clusters, with a few big ones.
            let [big, squat, tint, rank] = randoms(gb ^ 0x4D, ga);
            let scale = 0.35 + 1.2 * big * big * big;
            let shape = match (scale > 0.9, variety < 0.5, variety < 0.7) {
                (true, true, _) => 2,
                (true, false, _) => 1,
                (false, _, true) => 0,
                (false, _, false) => 1,
            };
            clusters.push(Prop {
                pos: vec3(x, world.surface_height(x, z), z),
                scale,
                aspect: 0.8 + 0.4 * squat,
                yaw: variety * 97.0,
                tint,
                rank,
                shape,
            });
        }
    }
    clusters
}

/// Deep yellow in the hollows, pale sulphur-yellow on top.
const DARK: Vec3 = Vec3::new(0.62, 0.50, 0.07);
const LIGHT: Vec3 = Vec3::new(0.98, 0.92, 0.30);

fn lump(m: &mut MeshBuilder, center: Vec3, radii: Vec3, seed: i32) {
    m.blob(center, radii, DARK, LIGHT, 0.3, seed);
}

/// A single rounded lump of sulphur with a small one beside it.
fn lump_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    lump(&mut m, vec3(0.0, 0.5, 0.0), vec3(1.0, 0.75, 0.9), 41);
    lump(&mut m, vec3(0.9, 0.3, 0.4), vec3(0.5, 0.35, 0.45), 42);
    m.vertices
}

/// A low crust with a couple of lumps swelling out of it.
fn crust_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    lump(&mut m, vec3(0.0, 0.25, 0.0), vec3(1.8, 0.4, 1.5), 43);
    lump(&mut m, vec3(0.4, 0.6, 0.1), vec3(0.75, 0.55, 0.65), 44);
    lump(&mut m, vec3(-0.7, 0.4, -0.4), vec3(0.5, 0.4, 0.45), 45);
    m.vertices
}

/// A big heap of lumps.
fn cluster_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    lump(&mut m, vec3(0.0, 0.65, 0.0), vec3(1.3, 1.0, 1.2), 46);
    lump(&mut m, vec3(1.2, 0.45, 0.5), vec3(0.75, 0.55, 0.7), 47);
    lump(&mut m, vec3(-0.9, 0.4, -0.8), vec3(0.7, 0.5, 0.65), 48);
    lump(&mut m, vec3(0.3, 0.35, -1.1), vec3(0.5, 0.4, 0.5), 49);
    m.vertices
}

/// Each sulphur shape from afar: one coarse lump.
fn lump_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.3, 0.5, 0.1), vec3(1.4, 0.75, 1.0), DARK, LIGHT);
    m.vertices
}

fn crust_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.0, 0.5, 0.0), vec3(1.8, 0.65, 1.5), DARK, LIGHT);
    m.vertices
}

fn cluster_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.2, 0.7, -0.1), vec3(1.9, 1.0, 1.7), DARK, LIGHT);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_shapes_are_simple_versions_of_the_near_ones() {
        crate::props::check_far_shapes(&kind());
    }

    /// A flat 10 m world with sulphur everywhere in its west half.
    fn vents() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Sulphur)[j * nx + i] = 255;
            }
        }
        world
    }

    fn all_clusters(world: &World) -> Vec<Prop> {
        (0..16).flat_map(|k| clusters_in_chunk(world, k % 4, k / 4)).collect()
    }

    #[test]
    fn clusters_lie_where_the_sulphur_is_and_nowhere_else() {
        let world = vents();
        let clusters = all_clusters(&world);
        assert!(clusters.len() > 100, "{} clusters", clusters.len());
        assert!(clusters.iter().all(|c| c.pos.x < CELL_SIZE), "a cluster outside the sulphur");
        assert!(clusters.iter().all(|c| (c.pos.y - 10.0).abs() < 1e-3));
        assert_eq!(clusters.len(), all_clusters(&world).len());
    }

    #[test]
    fn there_are_many_small_clusters_and_a_few_big_ones_of_every_shape() {
        let clusters = all_clusters(&vents());
        let small = clusters.iter().filter(|c| c.scale < 0.7).count() as f32 / clusters.len() as f32;
        let big = clusters.iter().filter(|c| c.scale > 1.2).count() as f32 / clusters.len() as f32;
        assert!(small > 0.5, "only {small} are small");
        assert!(big > 0.01 && big < 0.25, "{big} are big");
        for shape in 0..3 {
            assert!(clusters.iter().any(|c| c.shape == shape), "no clusters of shape {shape}");
        }
    }

    #[test]
    fn clusters_gather_around_vents() {
        let clusters = all_clusters(&vents());
        // Count clusters in 16 m squares: vents leave some squares nearly empty and crowd others.
        let mut cells = std::collections::HashMap::new();
        for c in &clusters {
            *cells.entry(((c.pos.x / 16.0).floor() as i32, (c.pos.z / 16.0).floor() as i32)).or_insert(0usize) += 1;
        }
        let counts: Vec<usize> = (-8..0).flat_map(|i| (-8..8).map(move |j| (i, j))).map(|c| cells.get(&c).copied().unwrap_or(0)).collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(hi >= lo * 2 + 3, "squares hold {lo} to {hi} clusters");
    }

    #[test]
    fn clusters_stay_out_of_the_water() {
        let mut world = vents();
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        for j in 0..nz {
            for i in 0..16 {
                heights[j * nx + i] = -20.0;
            }
        }
        world.set_terrain(heights, vec![0.0; nx * nz]);
        let half_x = world.half_extents().x;
        let clusters = all_clusters(&world);
        assert!(!clusters.is_empty());
        assert!(clusters.iter().all(|c| ((c.pos.x + half_x) / CELL_SIZE) as usize >= 15), "a cluster in the sea");
    }

    #[test]
    fn cluster_meshes_are_small_and_sit_on_the_ground() {
        for mesh in [lump_mesh(), crust_mesh(), cluster_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest > -0.5 && lowest < 0.3, "the cluster starts at {lowest}");
            assert!((0.8..4.0).contains(&highest), "a cluster {highest} m tall");
        }
    }
}
