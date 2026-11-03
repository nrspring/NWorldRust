//! The tint laid over the ground a unit can still get to, made to stand out from the map: a square for each cell, held just
//! over the ground, green where the unit would have most of its movement points left and amber where it would have the
//! least; a bright line all round the edge of where it can get to, which is also where water, mountains and the end of its
//! points stop it; and a fainter line at each 50 points of the way, so that how far it is can be read. Nothing here draws
//! it; the squares are made into meshes for [`crate::movement_view`] to draw.

use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;
use nworld_map::world::{World, CELL_SIZE};

use crate::movement::Reach;

/// How high over the ground the tint is held, and the lines over that, in meters.
const LIFT: f32 = 0.9;
const RING_LIFT: f32 = 1.15;
const EDGE_LIFT: f32 = 1.4;
/// Most squares in one mesh, so that its 16 bit indices don't run out.
const QUADS_PER_MESH: usize = 14_000;
/// The tint where the most movement points would be left, where half would, and where the fewest would, and how
/// see-through it is. It goes through yellow, so that the middle is as bright as the ends.
const MOST_LEFT: [f32; 3] = [0.10, 0.82, 0.42];
const HALF_LEFT: [f32; 3] = [0.98, 0.88, 0.12];
const LEAST_LEFT: [f32; 3] = [0.98, 0.50, 0.08];
const ALPHA: u8 = 150;
/// The line round the edge: how wide it is, in meters, and its color. The lines in the tint: how many points apart they
/// are, how wide, and their color.
const EDGE_WIDTH: f32 = 1.6;
const EDGE_COLOR: [u8; 4] = [255, 255, 255, 240];
const RING_EVERY: f32 = 50.0;
const RING_WIDTH: f32 = 0.8;
const RING_COLOR: [u8; 4] = [255, 255, 255, 130];

/// A flat square, or strip, over the ground: its corners in world x and z, how high it is held, and its color.
#[derive(Clone, Debug, PartialEq)]
struct Quad {
    corners: [Vec2; 4],
    lift: f32,
    color: [u8; 4],
}

/// The tint for a cell that would leave this share, from 0 to 1, of the movement points.
fn tint(left: f32) -> [u8; 4] {
    let left = left.clamp(0.0, 1.0);
    let (from, to, along) = if left < 0.5 { (LEAST_LEFT, HALF_LEFT, left * 2.0) } else { (HALF_LEFT, MOST_LEFT, (left - 0.5) * 2.0) };
    let c = |k: usize| ((from[k] + (to[k] - from[k]) * along) * 255.0) as u8;
    [c(0), c(1), c(2), ALPHA]
}

/// The four ways from a cell to the one beside it.
const SIDES: [Vec2; 4] = [Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0)];

/// A strip `width` wide along the side of the cell at `at` that faces `side`, on the inside of it.
fn strip(at: Vec2, side: Vec2, width: f32, lift: f32, color: [u8; 4]) -> Quad {
    let (half, along) = (CELL_SIZE * 0.5, vec2(-side.y, side.x));
    let outer = at + side * half;
    let inner = outer - side * width;
    Quad { corners: [outer + along * half, outer - along * half, inner - along * half, inner + along * half], lift, color }
}

/// Everything that is laid over the ground for `reach`, lowest first: the tint, then the lines in it, then the line round the
/// edge.
fn quads(reach: &Reach) -> Vec<Quad> {
    let cells: Vec<(Vec2, f32)> = reach.cells().collect();
    let mut tints = Vec::with_capacity(cells.len());
    let (mut rings, mut edges) = (Vec::new(), Vec::new());
    for &(at, cost) in &cells {
        let half = CELL_SIZE * 0.5;
        let corners = [vec2(-half, -half), vec2(half, -half), vec2(half, half), vec2(-half, half)].map(|c| at + c);
        tints.push(Quad { corners, lift: LIFT, color: tint(1.0 - cost / reach.points().max(1.0)) });
        for side in SIDES {
            match reach.cost_to(at + side * CELL_SIZE) {
                // Where it can't go on, the edge of where it can get to.
                None => edges.push(strip(at, side, EDGE_WIDTH, EDGE_LIFT, EDGE_COLOR)),
                // Between one 50 points of the way and the next: a line, drawn by the cell that is the farther.
                Some(next) if (cost / RING_EVERY).floor() > (next / RING_EVERY).floor() => rings.push(strip(at, side, RING_WIDTH, RING_LIFT, RING_COLOR)),
                Some(_) => {}
            }
        }
    }
    tints.extend(rings);
    tints.extend(edges);
    tints
}

/// The meshes that lay `reach` over the ground of `world`.
pub fn build(world: &World, reach: &Reach) -> Vec<Mesh> {
    build_in_chunks(world, reach, QUADS_PER_MESH)
}

/// [`build`], with at most `per_mesh` squares to a mesh.
fn build_in_chunks(world: &World, reach: &Reach, per_mesh: usize) -> Vec<Mesh> {
    quads(reach)
        .chunks(per_mesh)
        .map(|chunk| {
            let (mut vertices, mut indices): (Vec<Vertex>, Vec<u16>) = (Vec::new(), Vec::new());
            for quad in chunk {
                let first = vertices.len() as u16;
                for corner in quad.corners {
                    vertices.push(Vertex { position: vec3(corner.x, world.surface_height(corner.x, corner.y) + quad.lift, corner.y), uv: Vec2::ZERO, color: quad.color, normal: Vec4::ZERO });
                }
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
            }
            Mesh { vertices, indices, texture: None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::{self, Network};
    use crate::test_support;

    fn some_reach(points: f32) -> (World, Reach) {
        let world = test_support::small_continent();
        let from = test_support::inland_spot(&world, 150.0);
        let reach = movement::reach(&world, &Network::default(), from, points);
        (world, reach)
    }

    #[test]
    fn the_tint_goes_from_amber_to_green_as_more_points_are_left() {
        assert_eq!(tint(0.0)[..3], [249, 127, 20]);
        assert_eq!(tint(0.5)[..3], [249, 224, 30], "yellow in the middle");
        assert_eq!(tint(1.0)[..3], [25, 209, 107]);
        assert!(tint(0.25)[1] > tint(0.0)[1] && tint(0.25)[1] < tint(0.5)[1] && tint(0.75)[0] < tint(0.5)[0]);
        assert!(tint(0.3)[3] < 255 && tint(0.3)[3] > 100, "it is see-through, but not faint");
        assert_eq!(tint(7.0), tint(1.0), "out of range is the nearest end");
        assert_eq!(tint(-3.0), tint(0.0));
    }

    #[test]
    fn every_cell_that_can_be_got_to_is_a_square_of_tint() {
        let (_, reach) = some_reach(200.0);
        let tints = quads(&reach).into_iter().filter(|q| q.lift == LIFT).count();
        assert_eq!(tints, reach.cells().count());
    }

    #[test]
    fn there_is_a_line_on_every_side_of_the_edge_of_where_it_can_get_to_and_none_inside_it() {
        let (_, reach) = some_reach(200.0);
        // Counted another way: the sides of the cells that can be got to whose other side can't.
        let exposed: usize = reach.cells().map(|(at, _)| SIDES.iter().filter(|side| reach.cost_to(at + **side * CELL_SIZE).is_none()).count()).sum();
        let edges = quads(&reach).into_iter().filter(|q| q.lift == EDGE_LIFT).count();
        assert!(exposed > 40, "there is an edge: {exposed}");
        assert_eq!(edges, exposed);
    }

    #[test]
    fn there_is_a_fainter_line_at_each_50_points_of_the_way() {
        let (_, reach) = some_reach(200.0);
        let rings = quads(&reach).into_iter().filter(|q| q.lift == RING_LIFT).count();
        assert!(rings > 30, "rings at 50, 100 and 150 points: {rings}");
        let (_, short) = some_reach(40.0);
        assert_eq!(quads(&short).into_iter().filter(|q| q.lift == RING_LIFT).count(), 0, "40 points never get to 50");
    }

    #[test]
    fn the_lines_are_over_the_tint_and_the_edge_is_the_brightest_and_widest() {
        assert!(LIFT < RING_LIFT && RING_LIFT < EDGE_LIFT);
        assert!(EDGE_WIDTH > RING_WIDTH && EDGE_COLOR[3] > RING_COLOR[3]);
        let (_, reach) = some_reach(200.0);
        let all = quads(&reach);
        let order: Vec<f32> = all.iter().map(|q| q.lift).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "lowest first, for them to be drawn over each other");
    }

    #[test]
    fn everything_is_held_over_the_ground_at_its_own_height() {
        let (world, reach) = some_reach(100.0);
        let meshes = build(&world, &reach);
        assert!(meshes.iter().all(|m| m.vertices.len() <= u16::MAX as usize), "the indices have to reach them");
        for vertex in meshes.iter().flat_map(|m| &m.vertices) {
            let held = vertex.position.y - world.surface_height(vertex.position.x, vertex.position.z);
            assert!([LIFT, RING_LIFT, EDGE_LIFT].iter().any(|lift| (held - lift).abs() < 1e-3), "{held}");
        }
    }

    #[test]
    fn a_big_reach_is_split_into_meshes_that_fit() {
        let (world, reach) = some_reach(200.0);
        let count = quads(&reach).len();
        let meshes = build_in_chunks(&world, &reach, 50);
        assert_eq!(meshes.len(), count.div_ceil(50));
        assert!(meshes.iter().all(|m| m.vertices.len() <= 200));
        assert_eq!(meshes.iter().map(|m| m.vertices.len()).sum::<usize>(), count * 4, "none are lost");
        assert!(QUADS_PER_MESH * 4 <= u16::MAX as usize);
    }
}
