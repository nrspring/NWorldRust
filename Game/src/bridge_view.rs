//! Drawing the bridges: each as a stone arch, a slab that rises from the ground at one end to a crown over the
//! water and comes down at the other, with a low wall along each side. One that has failed is darker, and has
//! fallen in at the middle. This is the game's own drawing, and not part of drawing the map.

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_map::world::World;

use crate::bridge::{Bridge, End};
use crate::day::DaySave;
use crate::failure::Thing;
use crate::greyscale;

/// Half the width of the roadway between the walls, plus the walls, in meters.
const HALF_WIDTH: f32 = 2.7;
/// How thick the walls along the sides are, and how high they stand above the roadway.
const WALL_THICKNESS: f32 = 0.55;
const WALL_HEIGHT: f32 = 0.9;
/// How thick the slab under the roadway is, in meters.
const SLAB: f32 = 1.4;
/// How far above the ground the roadway lies at each end.
const ABOVE_GROUND: f32 = 0.6;
/// How high above the water the underside of the slab is at the crown, and the least the arch rises.
const CLEARANCE: f32 = 1.8;
const LEAST_RISE: f32 = 1.2;
/// The stone, and how much each block of it differs from the next.
const STONE: [f32; 3] = [0.66, 0.64, 0.60];
const VARIATION: f32 = 0.08;
/// How far apart, along the bridge, the rows of the mesh are, in meters.
const STEP: f32 = 2.0;
/// How far apart the points are that the water is looked for along a bridge, in meters.
const LOOK_EVERY: f32 = 1.0;

/// The bridges as meshes, made again when they change.
#[derive(Default)]
pub struct BridgeRenderer {
    built: Vec<Bridge>,
    built_failed: Vec<Thing>,
    mesh: Option<Mesh>,
    /// The mesh with what is greyed made grey, and what that was (see [`greyscale::version`]).
    grey: Option<(u64, Mesh)>,
}

impl BridgeRenderer {
    /// Draws the bridges of `day`, as seen by the camera that is set, which must still be the one the map was drawn
    /// with.
    pub fn draw(&mut self, world: &World, day: &DaySave) {
        if self.built != day.bridges || self.built_failed != day.failed {
            let bridges: Vec<(Bridge, bool)> = day.bridges.iter().map(|b| (*b, day.is_failed(Thing::Bridge(b.id)))).collect();
            self.mesh = build(world, &bridges);
            self.grey = None;
            self.built = day.bridges.clone();
            self.built_failed = day.failed.clone();
        }
        let Some(mesh) = &self.mesh else { return };
        let Some(version) = greyscale::version() else {
            self.grey = None;
            draw_mesh(mesh);
            return;
        };
        if self.grey.as_ref().is_none_or(|(made_for, _)| *made_for != version) {
            self.grey = Some((version, greyscale::mesh(mesh, |x, z| greyscale::greyed(world, x, z))));
        }
        if let Some((_, grey)) = &self.grey {
            draw_mesh(grey);
        }
    }
}

/// One mesh of every bridge, each with whether it has failed, or none if there are none.
fn build(world: &World, bridges: &[(Bridge, bool)]) -> Option<Mesh> {
    let (mut vertices, mut indices): (Vec<Vertex>, Vec<u16>) = (Vec::new(), Vec::new());
    for (bridge, failed) in bridges {
        // A mesh's indices are 16 bits, so a bridge that would pass them is left out rather than broken.
        if vertices.len() + 8 * 4 * ((bridge.first.x - bridge.second.x).hypot(bridge.first.z - bridge.second.z) / STEP).ceil() as usize + 64 > u16::MAX as usize {
            break;
        }
        add_bridge(world, bridge.end(End::First), bridge.end(End::Second), *failed, &mut vertices, &mut indices);
    }
    (!vertices.is_empty()).then(|| Mesh { vertices, indices, texture: None })
}

/// The heights along a bridge from `first` to `second`, which are worked out once for the whole of it.
struct Arch {
    /// The ground at each end, plus a little.
    low: [f32; 2],
    /// How much higher than a straight line between the ends the roadway is at the middle.
    rise: f32,
}

impl Arch {
    fn new(world: &World, first: Vec2, second: Vec2) -> Self {
        let low = [world.surface_height(first.x, first.y) + ABOVE_GROUND, world.surface_height(second.x, second.y) + ABOVE_GROUND];
        // The highest the water stands under it, which the arch has to clear.
        let length = first.distance(second);
        let looks = (length / LOOK_EVERY).ceil().max(1.0) as usize;
        let water = (0..=looks).map(|k| first.lerp(second, k as f32 / looks as f32)).map(|p| world.surface_height(p.x, p.y)).fold(f32::NEG_INFINITY, f32::max);
        let middle = (low[0] + low[1]) * 0.5;
        Self { low, rise: (water + CLEARANCE + SLAB - middle).max(LEAST_RISE) }
    }

    /// The height of the roadway a fraction `t` of the way along.
    fn top(&self, t: f32) -> f32 {
        self.low[0] + (self.low[1] - self.low[0]) * t + self.rise * 4.0 * t * (1.0 - t)
    }
}

/// How much darker the stone of a bridge that has failed is, and the middle part of it, as the fractions of its
/// length between which it has fallen in.
const RUIN_SHADE: f32 = 0.6;
const FALLEN_IN: (f32, f32) = (0.34, 0.66);

/// Adds the bridge from `first` to `second` to the mesh being made, whole or, if it has `failed`, ruined.
fn add_bridge(world: &World, first: Vec2, second: Vec2, failed: bool, vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>) {
    let length = first.distance(second);
    if length < 1.0 {
        return;
    }
    let along = (second - first) / length;
    let side = vec2(-along.y, along.x);
    let arch = Arch::new(world, first, second);
    let steps = (length / STEP).ceil().max(1.0) as usize;

    // A point at lateral offset `lateral` from the line of the bridge, `t` of the way along it, at `height`.
    let point = |t: f32, lateral: f32, height: f32| {
        let at = first.lerp(second, t) + side * lateral;
        vec3(at.x, height, at.y)
    };
    let inner = HALF_WIDTH - WALL_THICKNESS;
    for step in 0..steps {
        let (t0, t1) = (step as f32 / steps as f32, (step + 1) as f32 / steps as f32);
        // A bridge that has failed has no middle: those blocks have fallen away.
        if failed && (t0 + t1) * 0.5 > FALLEN_IN.0 && (t0 + t1) * 0.5 < FALLEN_IN.1 {
            continue;
        }
        let (top0, top1) = (arch.top(t0), arch.top(t1));
        let (under0, under1) = (top0 - SLAB, top1 - SLAB);
        // Each block of stone is a little different from the next.
        let block = (1.0 + VARIATION * ((step as f32 * 12.9898).sin() * 43758.547).fract().mul_add(2.0, -1.0)) * if failed { RUIN_SHADE } else { 1.0 };
        let shade = |light: f32| {
            let c = |k: usize| (STONE[k] * light * block * 255.0).clamp(0.0, 255.0) as u8;
            [c(0), c(1), c(2), 255]
        };
        // The roadway between the walls, then for each side the wall's top, its face towards the roadway and its
        // face towards the water, which runs down to the underside of the slab; and the underside.
        quad(vertices, indices, [point(t0, -inner, top0), point(t0, inner, top0), point(t1, inner, top1), point(t1, -inner, top1)], shade(1.05));
        for sign in [-1.0f32, 1.0] {
            let (a, b) = (inner * sign, HALF_WIDTH * sign);
            quad(vertices, indices, [point(t0, a, top0 + WALL_HEIGHT), point(t0, b, top0 + WALL_HEIGHT), point(t1, b, top1 + WALL_HEIGHT), point(t1, a, top1 + WALL_HEIGHT)], shade(1.14));
            quad(vertices, indices, [point(t0, a, top0), point(t0, a, top0 + WALL_HEIGHT), point(t1, a, top1 + WALL_HEIGHT), point(t1, a, top1)], shade(0.86));
            quad(vertices, indices, [point(t0, b, under0), point(t0, b, top0 + WALL_HEIGHT), point(t1, b, top1 + WALL_HEIGHT), point(t1, b, under1)], shade(0.74));
        }
        quad(vertices, indices, [point(t0, -HALF_WIDTH, under0), point(t0, HALF_WIDTH, under0), point(t1, HALF_WIDTH, under1), point(t1, -HALF_WIDTH, under1)], shade(0.52));
    }

    // The two ends are closed off with a face each, so that the slab doesn't look hollow from along the river.
    for (t, flip) in [(0.0f32, 1.0f32), (1.0, -1.0)] {
        let (top, under) = (arch.top(t), arch.top(t) - SLAB);
        quad(vertices, indices, [point(t, -HALF_WIDTH * flip, under), point(t, HALF_WIDTH * flip, under), point(t, HALF_WIDTH * flip, top + WALL_HEIGHT), point(t, -HALF_WIDTH * flip, top + WALL_HEIGHT)], [150, 146, 138, 255]);
    }
}

/// Adds a four sided face, seen from both sides, to the mesh being made.
fn quad(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, corners: [Vec3; 4], color: [u8; 4]) {
    let start = vertices.len() as u16;
    for corner in corners {
        vertices.push(Vertex { position: corner, uv: Vec2::ZERO, color, normal: Vec4::ZERO });
    }
    indices.extend([start, start + 1, start + 2, start, start + 2, start + 3, start, start + 2, start + 1, start, start + 3, start + 2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::BridgeId;
    use nworld_map::world::CELL_SIZE;

    /// A map of ground at 5 m with a lake whose water stands at `level`, over `x` from -10 to 10.
    fn river_world(level: f32) -> World {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        for j in 0..nz {
            for i in 0..nx {
                if (i as f32 * CELL_SIZE - half.x).abs() < 10.0 {
                    world.water_mut()[j * nx + i] = level;
                }
            }
        }
        world
    }

    fn mesh_of(world: &World, first: Vec2, second: Vec2) -> (Vec<Vertex>, Vec<u16>) {
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        add_bridge(world, first, second, false, &mut vertices, &mut indices);
        (vertices, indices)
    }

    #[test]
    fn a_bridge_is_a_sound_mesh() {
        let world = river_world(10.0);
        let (vertices, indices) = mesh_of(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0));
        assert!(!vertices.is_empty() && !indices.is_empty());
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        assert_eq!(indices.len() % 3, 0);
        assert!(vertices.iter().all(|v| v.position.is_finite()));
        // Everything lies between the two ends, and within the width of the bridge.
        assert!(vertices.iter().all(|v| v.position.x.abs() <= 30.01 && v.position.z.abs() <= HALF_WIDTH + 0.01));
    }

    #[test]
    fn the_roadway_comes_up_from_the_ground_and_clears_the_water_at_the_crown() {
        let world = river_world(10.0);
        let arch = Arch::new(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0));
        // At the ends it is just above the ground (5 m), and at the middle the underside of the slab is above the water.
        assert!((arch.top(0.0) - (5.0 + ABOVE_GROUND)).abs() < 0.01 && (arch.top(1.0) - (5.0 + ABOVE_GROUND)).abs() < 0.01);
        let crown = arch.top(0.5);
        assert!(crown - SLAB >= 10.0 + CLEARANCE - 0.01, "{crown}");
        assert!(crown > arch.top(0.25) && arch.top(0.25) > arch.top(0.0), "it rises to the crown");
        assert!((arch.top(0.3) - arch.top(0.7)).abs() < 0.01, "the same each side");
    }

    #[test]
    fn a_bridge_over_low_water_still_rises_a_little() {
        let world = river_world(5.0);
        let arch = Arch::new(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0));
        assert!(arch.rise >= LEAST_RISE);
    }

    #[test]
    fn the_stone_is_gray_and_the_faces_are_lit_differently() {
        let world = river_world(10.0);
        let (vertices, _) = mesh_of(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0));
        assert!(vertices.iter().all(|v| {
            let [r, g, b, a] = v.color;
            a == 255 && r.abs_diff(g) < 30 && g.abs_diff(b) < 30 && r > 60
        }));
        let shades: std::collections::HashSet<u8> = vertices.iter().map(|v| v.color[0]).collect();
        assert!(shades.len() > 4, "the roadway, the walls and the underside are not all one shade");
    }

    #[test]
    fn nothing_is_made_of_a_bridge_with_no_length_and_none_for_no_bridges() {
        let world = river_world(10.0);
        assert!(mesh_of(&world, vec2(5.0, 5.0), vec2(5.0, 5.0)).0.is_empty());
        assert!(build(&world, &[]).is_none());
        let bridge = Bridge { id: BridgeId::new(), first: vec2(-30.0, 0.0).into(), second: vec2(30.0, 0.0).into(), regions: [None; 2] };
        let mesh = build(&world, &[(bridge, false), (bridge, false)]).unwrap();
        assert_eq!(mesh.vertices.len(), 2 * mesh_of(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0)).0.len(), "each bridge adds its own");
    }

    #[test]
    fn a_bridge_that_has_failed_is_darker_and_has_fallen_in_at_the_middle() {
        let world = river_world(10.0);
        let whole = mesh_of(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0)).0;
        let (mut ruined, mut indices) = (Vec::new(), Vec::new());
        add_bridge(&world, vec2(-30.0, 0.0), vec2(30.0, 0.0), true, &mut ruined, &mut indices);
        assert!(indices.iter().all(|&i| (i as usize) < ruined.len()));
        assert!(ruined.len() < whole.len(), "blocks from the middle are gone: {} of {}", ruined.len(), whole.len());
        // None of what is left is in the middle third, over the water.
        assert!(ruined.iter().all(|v| v.position.x.abs() > 30.0 * 0.3 || v.position.x.abs() < 0.0), "nothing near the middle");
        let brightest = |vs: &[Vertex]| vs.iter().map(|v| v.color[0]).max().unwrap();
        assert!(brightest(&ruined) < brightest(&whole), "darker");
    }
}
