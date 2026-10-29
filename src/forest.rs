//! Forests: the trees drawn where the forest resource is painted.
//!
//! A spot on a fixed grid across the world gets a tree with a chance equal to the forest
//! density there, unless the ground is wrong for one: water, beaches, steep slopes, the high
//! rock above the treeline.

use macroquad::prelude::*;

use crate::noise::smoothstep;
use crate::props::{density_at, randoms, MeshBuilder, Prop, PropKind, PropVertex};
use crate::world::{Resource, World, CELL_SIZE, CHUNK_SIZE};

/// Meters between the spots a tree can stand on.
const TREE_SPACING: f32 = 4.0;
/// Trees only stand on ground up to about this high, and below this slope (rise over run).
const TREELINE: f32 = 36.0;
const MAX_SLOPE: f32 = 0.7;
/// Beaches, and the ground this close above water near it, stay clear of trees.
const SHORE_CLEARANCE: f32 = 3.0;

/// How trees are drawn: conifers and broadleaf trees, out to a few hundred meters.
pub fn kind() -> PropKind {
    PropKind {
        resource: Resource::Forest,
        shapes: vec![conifer_mesh(), broadleaf_mesh()],
        generate: trees_in_chunk,
        fade: (260.0, 620.0),
        far_shapes: vec![conifer_far_mesh(), broadleaf_far_mesh()],
        near_range: 220.0,
        wind: 1.0,
    }
}

/// The trees of chunk (`cx`, `cz`).
fn trees_in_chunk(world: &World, cx: usize, cz: usize) -> Vec<Prop> {
    let (nx, nz) = world.samples();
    let half = world.half_extents();
    let (heights, water) = (world.heights(), world.water());
    let per_side = (CHUNK_SIZE / TREE_SPACING) as i32;
    let mut trees = Vec::new();
    for gj in 0..per_side {
        for gi in 0..per_side {
            // Spots sit on a grid over the whole world, each jittered within its own cell.
            let (ga, gb) = (cx as i32 * per_side + gi, cz as i32 * per_side + gj);
            let [rx, rz, accept, variety] = randoms(ga, gb);
            let x = (ga as f32 + 0.15 + 0.7 * rx) * TREE_SPACING - half.x;
            let z = (gb as f32 + 0.15 + 0.7 * rz) * TREE_SPACING - half.y;
            if accept >= density_at(world, Resource::Forest, x, z) {
                continue;
            }

            let i = (((x + half.x) / CELL_SIZE).round() as usize).min(nx - 1);
            let j = (((z + half.y) / CELL_SIZE).round() as usize).min(nz - 1);
            let idx = j * nx + i;
            let (ground, level) = (heights[idx], water[idx]);
            // Not in water, and not on a beach or the bank of a river or lake.
            if ground < level || ground < 0.75 || (world.beach_nearby(idx) && ground - level < SHORE_CLEARANCE) {
                continue;
            }
            // Not up the high rock, and not up a cliff.
            let treeline = TREELINE + 4.0 * (variety - 0.5);
            let (i0, i1) = (i.saturating_sub(1), (i + 1).min(nx - 1));
            let (j0, j1) = (j.saturating_sub(1), (j + 1).min(nz - 1));
            let slope = vec2(
                (heights[j * nx + i1] - heights[j * nx + i0]) / ((i1 - i0) as f32 * CELL_SIZE),
                (heights[j1 * nx + i] - heights[j0 * nx + i]) / ((j1 - j0) as f32 * CELL_SIZE),
            )
            .length();
            if ground > treeline || slope > MAX_SLOPE {
                continue;
            }

            // Conifers take over higher up, and trees shrink toward the treeline.
            let conifer = variety < 0.15 + 0.85 * smoothstep(5.0, 26.0, ground);
            let size = (0.8 + 0.45 * randoms(gb, ga)[0]) * (1.0 - 0.4 * smoothstep(24.0, TREELINE, ground));
            trees.push(Prop {
                pos: vec3(x, world.surface_height(x, z), z),
                scale: size,
                aspect: 1.0,
                yaw: variety * 97.0,
                tint: randoms(ga ^ 0x55, gb)[1],
                rank: randoms(ga, gb ^ 0x2B)[2],
                shape: if conifer { 0 } else { 1 },
            });
        }
    }
    trees
}

fn conifer_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.trunk(0.0, 2.0, 0.35, 6);
    let (dark, light) = (vec3(0.05, 0.17, 0.09), vec3(0.14, 0.33, 0.15));
    // Three tiers of foliage, each narrower than the one below.
    m.cone(1.4, 5.0, 2.7, 7, dark, light);
    m.cone(3.2, 7.8, 2.1, 7, dark, light);
    m.cone(5.5, 10.5, 1.4, 7, dark, light);
    m.vertices
}

fn broadleaf_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.trunk(0.0, 3.4, 0.42, 6);
    let (dark, light) = (vec3(0.07, 0.22, 0.08), vec3(0.22, 0.42, 0.14));
    m.blob(vec3(0.0, 5.4, 0.0), vec3(3.4, 2.8, 3.4), dark, light, 0.0, 0);
    m.blob(vec3(1.4, 4.3, 0.9), vec3(2.2, 1.9, 2.2), dark, light, 0.0, 0);
    m.blob(vec3(-1.3, 4.6, -0.9), vec3(2.0, 1.8, 2.0), dark, light, 0.0, 0);
    m.vertices
}

/// A conifer from afar: one cone, with no trunk.
fn conifer_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.cone(0.5, 10.5, 2.6, 6, vec3(0.05, 0.17, 0.09), vec3(0.14, 0.33, 0.15));
    m.vertices
}

/// A broadleaf tree from afar: one coarse ball of leaves on a thin spike of a trunk.
fn broadleaf_far_mesh() -> Vec<PropVertex> {
    let mut m = MeshBuilder::default();
    m.cone(0.0, 4.6, 0.5, 3, vec3(0.20, 0.13, 0.08), vec3(0.30, 0.20, 0.12));
    m.octa(vec3(0.0, 4.2, 0.0), vec3(3.6, 4.0, 3.6), vec3(0.07, 0.22, 0.08), vec3(0.22, 0.42, 0.14));
    m.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_shapes_are_simple_versions_of_the_near_ones() {
        crate::props::check_far_shapes(&kind());
    }

    #[test]
    fn trees_stand_where_the_forest_is_and_nowhere_else() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![0.0; nx * nz]);
        // Forest in the west half.
        for j in 0..nz {
            for i in 0..nx / 2 {
                world.resource_mut(Resource::Forest)[j * nx + i] = 255;
            }
        }
        let mut all = Vec::new();
        for cz in 0..4 {
            for cx in 0..4 {
                all.extend(trees_in_chunk(&world, cx, cz));
            }
        }
        // A full forest has a tree for most spots, about one per 16 square meters.
        assert!(all.len() > 800, "{} trees", all.len());
        assert!(all.iter().all(|t| t.pos.x < 0.0 + CELL_SIZE), "a tree outside the forest");
        assert!(all.iter().all(|t| (t.pos.y - 10.0).abs() < 1e-3));
        // Both kinds of tree grow, and the same world gives the same trees.
        assert!(all.iter().any(|t| t.shape == 0) && all.iter().any(|t| t.shape == 1));
        assert_eq!(all.len(), (0..16).map(|k| trees_in_chunk(&world, k % 4, k / 4).len()).sum::<usize>());
    }

    #[test]
    fn trees_keep_off_water_beaches_cliffs_and_the_high_rock() {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut heights = vec![10.0; nx * nz];
        let water = vec![0.0; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                match i {
                    0..=15 => heights[j * nx + i] = -20.0,
                    // A cliff.
                    16..=19 => heights[j * nx + i] = 10.0 + (i - 16) as f32 * 8.0,
                    // High ground well above the treeline.
                    40..=64 => heights[j * nx + i] = 45.0,
                    _ => {}
                }
            }
        }
        world.set_terrain(heights, water);
        world.resource_mut(Resource::Forest).iter_mut().for_each(|d| *d = 255);
        let mut all = Vec::new();
        for cz in 0..4 {
            for cx in 0..4 {
                all.extend(trees_in_chunk(&world, cx, cz));
            }
        }
        let half_x = world.half_extents().x;
        let column = |t: &Prop| ((t.pos.x + half_x) / CELL_SIZE) as usize;
        assert!(!all.is_empty());
        assert!(all.iter().all(|t| column(t) > 19), "a tree in the sea or on the cliff");
        assert!(all.iter().all(|t| !(40..=64).contains(&column(t))), "a tree on the high rock");
    }

    #[test]
    fn tree_meshes_are_small_and_stand_on_the_ground() {
        for mesh in [conifer_mesh(), broadleaf_mesh()] {
            assert!(mesh.len() % 3 == 0 && mesh.len() < 400);
            let lowest = mesh.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
            let highest = mesh.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
            assert!(lowest.abs() < 0.01, "the trunk starts at {lowest}");
            assert!((7.0..14.0).contains(&highest), "a tree {highest} m tall");
        }
    }
}
