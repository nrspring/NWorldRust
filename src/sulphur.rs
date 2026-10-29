//! Sulphur: the yellow crystal clusters drawn where the sulphur resource is painted.
//!
//! A spot on a fixed grid across the world gets a cluster with a chance equal to the sulphur
//! density there, thinned out to round vents by noise, so the crystals gather around a few spots
//! with bare ground between them. Most clusters are small with a few big ones, and they lie
//! anywhere on dry ground.

use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use crate::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a cluster can lie on.
const CLUSTER_SPACING: f32 = 4.0;
/// Vents of crystals are about this many meters across, and as far apart again.
const VENT_SIZE: f32 = 11.0;

/// How clusters are drawn: crystal spikes, crusts and big clusters, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Sulphur,
        shapes: vec![spikes_mesh(), crust_mesh(), cluster_mesh()],
        generate: clusters_in_chunk,
        fade: (130.0, 380.0),
        draw_chunks: 7,
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
            // Vents: the crystals are thick inside round patches, with a trace between them.
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
            let scale = 0.4 + 1.4 * big * big * big;
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

/// Deep yellow at the foot of a crystal, pale sulphur-yellow at its tip.
const DARK: Vec3 = Vec3::new(0.60, 0.48, 0.06);
const LIGHT: Vec3 = Vec3::new(0.98, 0.93, 0.32);

fn crystal(m: &mut MeshBuilder, base: Vec3, tip: Vec3, radius: f32) {
    m.spike(base, tip, radius, 4, DARK, LIGHT);
}

/// A few crystals leaning out of the ground.
fn spikes_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    crystal(&mut m, vec3(0.0, 0.0, 0.0), vec3(0.15, 1.5, 0.1), 0.34);
    crystal(&mut m, vec3(0.45, 0.0, 0.2), vec3(0.95, 1.0, 0.45), 0.26);
    crystal(&mut m, vec3(-0.4, 0.0, 0.25), vec3(-0.85, 0.9, 0.5), 0.24);
    m.vertices
}

/// A low yellow crust with short crystals standing on it.
fn crust_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.blob(vec3(0.0, 0.2, 0.0), vec3(1.7, 0.35, 1.4), DARK, LIGHT, 0.3, 41);
    crystal(&mut m, vec3(0.3, 0.2, 0.1), vec3(0.4, 1.15, 0.2), 0.28);
    crystal(&mut m, vec3(-0.6, 0.2, -0.3), vec3(-0.8, 0.8, -0.4), 0.22);
    crystal(&mut m, vec3(0.7, 0.15, -0.6), vec3(0.9, 0.7, -0.8), 0.2);
    m.vertices
}

/// A big cluster: one tall crystal among shorter ones.
fn cluster_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    crystal(&mut m, vec3(0.0, 0.0, 0.0), vec3(0.2, 2.1, 0.1), 0.42);
    crystal(&mut m, vec3(0.55, 0.0, 0.2), vec3(1.2, 1.4, 0.6), 0.3);
    crystal(&mut m, vec3(-0.5, 0.0, 0.3), vec3(-1.1, 1.5, 0.7), 0.3);
    crystal(&mut m, vec3(0.1, 0.0, -0.55), vec3(0.0, 1.2, -1.2), 0.28);
    crystal(&mut m, vec3(-0.3, 0.0, -0.3), vec3(-0.6, 0.8, -0.7), 0.2);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for mesh in [spikes_mesh(), crust_mesh(), cluster_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest > -0.5 && lowest < 0.3, "the cluster starts at {lowest}");
            assert!((0.9..4.0).contains(&highest), "a cluster {highest} m tall");
        }
    }
}
