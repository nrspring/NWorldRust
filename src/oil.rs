//! Oil: the black tar pools and seeps drawn where the oil resource is painted.
//!
//! A spot on a fixed grid across the world gets a pool with a chance equal to the oil density
//! there, thinned out to broad fields by noise, so the oil lies in wide patches with bare ground
//! between them. Pools only form on gentle ground. Most are small with a few big ones.

use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::props::{density_at, randoms, value_noise, MeshBuilder, Prop, PropKind, PropVertex};
use crate::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a pool can lie on.
const POOL_SPACING: f32 = 4.0;
/// Oil fields are about this many meters across.
const FIELD_SIZE: f32 = 34.0;
/// Pools form on ground no steeper than this (rise over run).
const MAX_SLOPE: f32 = 0.35;

/// How pools are drawn: puddles, groups of puddles and tar mounds, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Oil,
        shapes: vec![puddle_mesh(), pools_mesh(), seep_mesh()],
        generate: pools_in_chunk,
        fade: (110.0, 330.0),
        far_shapes: vec![puddle_far_mesh(), pools_far_mesh(), seep_far_mesh()],
        near_range: 90.0,
        wind: 0.0,
    }
}

/// The pools of chunk (`cx`, `cz`).
fn pools_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / POOL_SPACING) as i32;
    let mut pools = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * POOL_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * POOL_SPACING - half.y;
            let density = density_at(world, Resource::Oil, x, z);
            if density <= 0.0 {
                continue;
            }
            // Fields: pools are common inside broad patches, with a few between them.
            let field = smoothstep(0.42, 0.62, value_noise(x / FIELD_SIZE, z / FIELD_SIZE, 41));
            if accept >= density * (0.08 + 0.52 * field) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            if heights[j * nx + i] < water[j * nx + i] {
                continue;
            }
            // Oil only pools on gentle ground.
            let (i0, i1) = (i.saturating_sub(1), (i + 1).min(nx - 1));
            let (j0, j1) = (j.saturating_sub(1), (j + 1).min(nz - 1));
            let slope = vec2(
                (heights[j * nx + i1] - heights[j * nx + i0]) / ((i1 - i0) as f32 * CELL_SIZE),
                (heights[j1 * nx + i] - heights[j0 * nx + i]) / ((j1 - j0) as f32 * CELL_SIZE),
            )
            .length();
            if slope > MAX_SLOPE {
                continue;
            }

            // Mostly small pools, with a few big ones.
            let [big, squat, tint, rank] = randoms(gb ^ 0x5A, ga);
            let shape = if variety < 0.5 {
                0
            } else if variety < 0.8 {
                1
            } else {
                2
            };
            pools.push(Prop {
                pos: vec3(x, world.surface_height(x, z), z),
                scale: 0.5 + 1.3 * big * big * big,
                aspect: 0.8 + 0.4 * squat,
                yaw: variety * 97.0,
                tint,
                rank,
                shape,
            });
        }
    }
    pools
}

/// Black tar with a faint sheen on top.
const DARK: Vec3 = Vec3::new(0.02, 0.02, 0.03);
const LIGHT: Vec3 = Vec3::new(0.13, 0.12, 0.17);

fn pool(m: &mut MeshBuilder, center: Vec2, radii: Vec2, seed: i32) {
    m.blob(vec3(center.x, 0.1, center.y), vec3(radii.x, 0.16, radii.y), DARK, LIGHT, 0.25, seed);
}

/// A single flat puddle.
fn puddle_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    pool(&mut m, vec2(0.0, 0.0), vec2(1.8, 1.5), 51);
    m.vertices
}

/// A few puddles run together.
fn pools_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    pool(&mut m, vec2(0.0, 0.0), vec2(1.7, 1.4), 52);
    pool(&mut m, vec2(2.0, 0.9), vec2(1.1, 1.0), 53);
    pool(&mut m, vec2(-1.5, -1.3), vec2(1.0, 0.9), 54);
    m.vertices
}

/// A tar mound seeping into a pool around it.
fn seep_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    pool(&mut m, vec2(0.0, 0.0), vec2(2.1, 1.9), 55);
    m.blob(vec3(0.0, 0.3, 0.0), vec3(0.8, 0.55, 0.8), DARK, LIGHT, 0.2, 56);
    m.vertices
}

/// Each pool shape from afar: one coarse flat lump.
fn puddle_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.0, 0.1, 0.0), vec3(1.8, 0.16, 1.5), DARK, LIGHT);
    m.vertices
}

fn pools_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.2, 0.1, -0.1), vec3(3.0, 0.16, 2.6), DARK, LIGHT);
    m.vertices
}

fn seep_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.octa(vec3(0.0, 0.3, 0.0), vec3(2.1, 0.5, 1.9), DARK, LIGHT);
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_shapes_are_simple_versions_of_the_near_ones() {
        crate::props::check_far_shapes(&kind());
    }

    /// A flat 10 m world with oil everywhere in its west half.
    fn field() -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Oil)[j * nx + i] = 255;
            }
        }
        world
    }

    fn all_pools(world: &World) -> Vec<Prop> {
        (0..16).flat_map(|k| pools_in_chunk(world, k % 4, k / 4)).collect()
    }

    #[test]
    fn pools_lie_where_the_oil_is_and_nowhere_else() {
        let world = field();
        let pools = all_pools(&world);
        assert!(pools.len() > 100, "{} pools", pools.len());
        assert!(pools.iter().all(|p| p.pos.x < CELL_SIZE), "a pool outside the oil");
        assert!(pools.iter().all(|p| (p.pos.y - 10.0).abs() < 1e-3));
        assert_eq!(pools.len(), all_pools(&world).len());
    }

    #[test]
    fn there_are_many_small_pools_and_a_few_big_ones_of_every_shape() {
        let pools = all_pools(&field());
        let small = pools.iter().filter(|p| p.scale < 0.8).count() as f32 / pools.len() as f32;
        let big = pools.iter().filter(|p| p.scale > 1.5).count() as f32 / pools.len() as f32;
        assert!(small > 0.5, "only {small} are small");
        assert!(big > 0.01 && big < 0.2, "{big} are big");
        for shape in 0..3 {
            assert!(pools.iter().any(|p| p.shape == shape), "no pools of shape {shape}");
        }
    }

    #[test]
    fn pools_gather_in_fields() {
        let pools = all_pools(&field());
        // Count pools in 16 m squares: fields leave some squares nearly empty and crowd others.
        let mut cells = std::collections::HashMap::new();
        for p in &pools {
            *cells.entry(((p.pos.x / 16.0).floor() as i32, (p.pos.z / 16.0).floor() as i32)).or_insert(0usize) += 1;
        }
        let counts: Vec<usize> = (-8..0).flat_map(|i| (-8..8).map(move |j| (i, j))).map(|c| cells.get(&c).copied().unwrap_or(0)).collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(hi >= lo * 2 + 3, "squares hold {lo} to {hi} pools");
    }

    #[test]
    fn pools_stay_out_of_the_water_and_off_steep_ground() {
        let mut world = field();
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                match i {
                    0..=15 => heights[j * nx + i] = -20.0,
                    // A steep slope.
                    16..=25 => heights[j * nx + i] = 10.0 + (i - 16) as f32 * 4.0,
                    _ => heights[j * nx + i] = 46.0,
                }
            }
        }
        world.set_terrain(heights, vec![0.0; nx * nz]);
        let half_x = world.half_extents().x;
        let pools = all_pools(&world);
        assert!(!pools.is_empty());
        let column = |p: &Prop| ((p.pos.x + half_x) / CELL_SIZE) as usize;
        // A pool between columns 25 and 26 belongs to column 26, where the ground is level again.
        assert!(pools.iter().all(|p| column(p) >= 25), "a pool in the sea or on the slope");
    }

    #[test]
    fn pool_meshes_are_flat_and_sit_on_the_ground() {
        for mesh in [puddle_mesh(), pools_mesh(), seep_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!((-0.3..0.2).contains(&lowest), "the pool starts at {lowest}");
            assert!((0.1..1.2).contains(&highest), "a pool {highest} m tall");
        }
    }
}
