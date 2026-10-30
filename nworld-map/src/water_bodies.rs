//! Bodies of inland water: the rivers and the lakes of a map. The map doesn't remember which samples a river was dug
//! from, or which a lake, so both are worked out from the water itself.
//!
//! A **lake** is water that is wide in every direction: a sample of inland water that is at least [`LAKE_CORE`]
//! samples from the nearest shore is the heart of a lake, the water within [`LAKE_REACH`] samples of it belongs
//! to it, and hearts that touch are one lake. A **river** is the rest of the inland water, each patch of it whose
//! samples all touch (even across a corner) being one river, so a river with branches is one river. A river that runs
//! into a lake ends at the lake. A river that is wider than a lake's heart is counted as a lake where it is, which is
//! the price of working it out from the water.
//!
//! A body is told apart from the others by its **anchor**: the number of its first sample, counting along the rows
//! of the map from the north-west corner. That stays the same for as long as the map does.

use std::collections::HashMap;

use glam::{vec2, Vec2};

use crate::world::{World, CELL_SIZE, SEA_LEVEL};

/// Water this many samples or more from any shore is the heart of a lake.
const LAKE_CORE: u32 = 5;
/// How many samples out from its heart a lake reaches, along its water.
const LAKE_REACH: u32 = 8;

/// Which body of water: the number of its first sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u32);

/// What is known of one body of water.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterBody {
    /// How many samples it has.
    pub samples: u32,
    /// A point of the water near its middle, in world x and z, for a label.
    pub label: Vec2,
    /// Whether it is a lake, and not a river.
    pub lake: bool,
}

/// The bodies of inland water of a map, found once from [`World::heights`] and [`World::water`]. It doesn't change as
/// the world does: make it again if the water is changed.
#[derive(Debug, Default)]
pub struct WaterBodies {
    nx: usize,
    nz: usize,
    half: Vec2,
    /// For each sample, the number of its body plus one, or 0 for none.
    body_of: Vec<u32>,
    bodies: HashMap<BodyId, WaterBody>,
    /// The anchors, in the order of the samples they are.
    order: Vec<BodyId>,
}

/// The samples that touch `k`, even across a corner.
fn neighbours(k: usize, nx: usize, nz: usize) -> impl Iterator<Item = usize> {
    let (i, j) = ((k % nx) as i64, (k / nx) as i64);
    (-1i64..=1).flat_map(move |dj| (-1i64..=1).map(move |di| (di, dj))).filter_map(move |(di, dj)| {
        let (ni, nj) = (i + di, j + dj);
        ((di, dj) != (0, 0) && ni >= 0 && nj >= 0 && ni < nx as i64 && nj < nz as i64).then(|| nj as usize * nx + ni as usize)
    })
}

/// For each sample of water, how many samples away the nearest shore is (1 beside the shore, counting the edge of the
/// map as a shore), and 0 for a sample that is not water.
fn shore_distance(wet: &[bool], nx: usize, nz: usize) -> Vec<u32> {
    let far = u32::MAX / 2;
    let mut depth: Vec<u32> = wet.iter().map(|&w| if w { far } else { 0 }).collect();
    let at = |depth: &[u32], i: i64, j: i64| if i < 0 || j < 0 || i >= nx as i64 || j >= nz as i64 { 0 } else { depth[j as usize * nx + i as usize] };
    // Two sweeps are enough for distances that count a step across a corner as one.
    for j in 0..nz as i64 {
        for i in 0..nx as i64 {
            let k = j as usize * nx + i as usize;
            if depth[k] != 0 {
                let near = [at(&depth, i - 1, j), at(&depth, i - 1, j - 1), at(&depth, i, j - 1), at(&depth, i + 1, j - 1)];
                depth[k] = depth[k].min(near.into_iter().min().unwrap() + 1);
            }
        }
    }
    for j in (0..nz as i64).rev() {
        for i in (0..nx as i64).rev() {
            let k = j as usize * nx + i as usize;
            if depth[k] != 0 {
                let near = [at(&depth, i + 1, j), at(&depth, i + 1, j + 1), at(&depth, i, j + 1), at(&depth, i - 1, j + 1)];
                depth[k] = depth[k].min(near.into_iter().min().unwrap() + 1);
            }
        }
    }
    depth
}

/// Gives `number` to `start` and every sample that touches it, and those that touch them, and so on, that has no
/// number yet and that `ok` allows. Returns them all.
fn flood(start: usize, number: usize, nx: usize, nz: usize, owner: &mut [usize], ok: impl Fn(usize) -> bool) -> Vec<usize> {
    let mut members = Vec::new();
    owner[start] = number;
    let mut stack = vec![start];
    while let Some(k) = stack.pop() {
        members.push(k);
        for next in neighbours(k, nx, nz) {
            if owner[next] == usize::MAX && ok(next) {
                owner[next] = number;
                stack.push(next);
            }
        }
    }
    members
}

impl WaterBodies {
    /// Finds the bodies of inland water of `world`: samples below the level of water that stands above the sea.
    pub fn build(world: &World) -> Self {
        let (nx, nz) = world.samples();
        let half = world.half_extents();
        let (heights, water) = (world.heights(), world.water());
        let wet: Vec<bool> = (0..nx * nz).map(|k| water[k] > SEA_LEVEL && heights[k] < water[k]).collect();
        let depth = shore_distance(&wet, nx, nz);
        let position = |k: usize| vec2((k % nx) as f32 * CELL_SIZE - half.x, (k / nx) as f32 * CELL_SIZE - half.y);

        // The heart of each lake, then the rest of its water out to its shore, then the rivers.
        let none = usize::MAX;
        let mut owner = vec![none; nx * nz];
        let mut groups: Vec<(bool, Vec<usize>)> = Vec::new();
        for start in 0..nx * nz {
            if owner[start] == none && depth[start] >= LAKE_CORE {
                let members = flood(start, groups.len(), nx, nz, &mut owner, |k| depth[k] >= LAKE_CORE);
                groups.push((true, members));
            }
        }
        // Every lake grows together, a step at a time, out to its shore: the shore of a round lake is up to a few more
        // steps from its heart than the heart is wide, and water between two hearts goes to the nearer.
        let mut frontier: Vec<usize> = (0..nx * nz).filter(|&k| owner[k] != none).collect();
        for _ in 0..LAKE_REACH {
            let mut next_frontier = Vec::new();
            for &k in &frontier {
                for next in neighbours(k, nx, nz) {
                    if owner[next] == none && wet[next] {
                        owner[next] = owner[k];
                        groups[owner[k]].1.push(next);
                        next_frontier.push(next);
                    }
                }
            }
            frontier = next_frontier;
        }
        for start in 0..nx * nz {
            if owner[start] == none && wet[start] {
                let members = flood(start, groups.len(), nx, nz, &mut owner, |k| wet[k]);
                groups.push((false, members));
            }
        }

        // Each is known by the first of its samples, in the order of the map.
        let mut found: Vec<(usize, bool, Vec<usize>)> = groups.into_iter().map(|(lake, members)| (members.iter().copied().min().unwrap(), lake, members)).collect();
        found.sort_by_key(|(anchor, _, _)| *anchor);
        let mut body_of = vec![0u32; nx * nz];
        let mut bodies = HashMap::new();
        let mut order = Vec::new();
        for (number, (anchor, lake, members)) in found.into_iter().enumerate() {
            for &k in &members {
                body_of[k] = number as u32 + 1;
            }
            let positions: Vec<Vec2> = members.iter().map(|&k| position(k)).collect();
            let mean = positions.iter().copied().sum::<Vec2>() / positions.len() as f32;
            let label = positions.iter().copied().min_by(|a, b| a.distance_squared(mean).total_cmp(&b.distance_squared(mean))).unwrap_or(mean);
            let id = BodyId(anchor as u32);
            bodies.insert(id, WaterBody { samples: members.len() as u32, label, lake });
            order.push(id);
        }
        Self { nx, nz, half, body_of, bodies, order }
    }

    /// Whether there is no inland water.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Every body, in the order they were found (by their anchors).
    pub fn ids(&self) -> &[BodyId] {
        &self.order
    }

    /// What is known of the body `id`.
    pub fn body(&self, id: BodyId) -> Option<&WaterBody> {
        self.bodies.get(&id)
    }

    /// The body of water that the sample nearest the point (`x`, `z`) is in, if it is in one.
    pub fn at(&self, x: f32, z: f32) -> Option<BodyId> {
        if self.body_of.is_empty() {
            return None;
        }
        let i = (((x + self.half.x) / CELL_SIZE).round().max(0.0) as usize).min(self.nx - 1);
        let j = (((z + self.half.y) / CELL_SIZE).round().max(0.0) as usize).min(self.nz - 1);
        self.id_of_sample(j * self.nx + i)
    }

    /// The body of water nearest the point (`x`, `z`), looking out as far as `radius` meters, if there is one: the
    /// body at the point if it is in water, and otherwise the one whose nearest sample is the closest.
    pub fn near(&self, x: f32, z: f32, radius: f32) -> Option<BodyId> {
        self.near_matching(x, z, radius, |_| true)
    }

    /// The same, but only among the bodies that `keep` is true of.
    pub fn near_matching(&self, x: f32, z: f32, radius: f32, keep: impl Fn(&WaterBody) -> bool) -> Option<BodyId> {
        let keeps = |id: BodyId| self.bodies.get(&id).is_some_and(&keep);
        if let Some(id) = self.at(x, z).filter(|&id| keeps(id)) {
            return Some(id);
        }
        let reach = (radius / CELL_SIZE).ceil().max(1.0) as i64;
        let (ci, cj) = (((x + self.half.x) / CELL_SIZE).round() as i64, ((z + self.half.y) / CELL_SIZE).round() as i64);
        let mut best: Option<(f32, BodyId)> = None;
        for dj in -reach..=reach {
            for di in -reach..=reach {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= self.nx as i64 || j >= self.nz as i64 {
                    continue;
                }
                let Some(id) = self.id_of_sample(j as usize * self.nx + i as usize).filter(|&id| keeps(id)) else { continue };
                let there = vec2(i as f32 * CELL_SIZE - self.half.x, j as f32 * CELL_SIZE - self.half.y);
                let away = there.distance(vec2(x, z));
                if away <= radius && best.is_none_or(|(d, _)| away < d) {
                    best = Some((away, id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    fn id_of_sample(&self, k: usize) -> Option<BodyId> {
        let number = *self.body_of.get(k)?;
        (number != 0).then(|| self.order[number as usize - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WaterClass;

    /// A map 512 m across of dry ground, 5 m above the sea.
    fn dry_world() -> World {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        world
    }

    /// Floods every sample for which `inside` is true, at the world position it gives, with water standing 5 m above the
    /// ground.
    fn flood(world: &mut World, inside: impl Fn(Vec2) -> bool) {
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        for j in 0..nz {
            for i in 0..nx {
                if inside(vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y)) {
                    world.water_mut()[j * nx + i] = 10.0;
                }
            }
        }
    }

    #[test]
    fn a_map_with_no_inland_water_has_no_bodies() {
        let bodies = WaterBodies::build(&dry_world());
        assert!(bodies.is_empty() && bodies.ids().is_empty());
        assert_eq!(bodies.at(0.0, 0.0), None);
        assert_eq!(bodies.near(0.0, 0.0, 100.0), None);
    }

    #[test]
    fn the_sea_is_not_a_body_of_inland_water() {
        // Ground below sea level, with the sea standing over it.
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(-20.0);
        let bodies = WaterBodies::build(&world);
        assert!(bodies.is_empty());
    }

    #[test]
    fn a_river_and_a_lake_are_two_bodies_and_a_river_is_not_a_lake() {
        let mut world = dry_world();
        // A river 12 m wide and 400 m long, and a lake 120 m across a good way from it.
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x.abs() < 200.0 && p.y > -6.0);
        flood(&mut world, |p| p.distance(vec2(0.0, 150.0)) < 60.0);
        let bodies = WaterBodies::build(&world);
        assert_eq!(bodies.ids().len(), 2);
        let river = bodies.at(0.0, 0.0).expect("the river");
        let lake = bodies.at(0.0, 150.0).expect("the lake");
        assert_ne!(river, lake);
        assert!(!bodies.body(river).unwrap().lake);
        assert!(bodies.body(lake).unwrap().lake);
        assert_eq!(bodies.at(0.0, -100.0), None, "dry ground is in no body");
    }

    #[test]
    fn a_river_that_runs_into_a_lake_is_a_river_and_a_lake_and_two_rivers_on_the_two_sides_are_two() {
        let mut world = dry_world();
        // A lake 120 m across at the middle, with a river 12 m wide running in from each side.
        flood(&mut world, |p| p.distance(Vec2::ZERO) < 60.0);
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x < -50.0 && p.x > -240.0);
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x > 50.0 && p.x < 240.0);
        let bodies = WaterBodies::build(&world);
        let (west, lake, east) = (bodies.at(-200.0, 0.0).unwrap(), bodies.at(0.0, 0.0).unwrap(), bodies.at(200.0, 0.0).unwrap());
        assert_eq!(bodies.ids().len(), 3);
        assert!(bodies.body(lake).unwrap().lake && !bodies.body(west).unwrap().lake && !bodies.body(east).unwrap().lake);
        assert_ne!(west, east, "the lake parts the river");
        assert_eq!(bodies.at(-30.0, 0.0), Some(lake), "the lake goes right out to its shore");
    }

    #[test]
    fn a_small_pond_is_not_a_lake_but_a_lake_in_the_corner_of_the_map_still_is() {
        let mut world = dry_world();
        flood(&mut world, |p| p.distance(vec2(-100.0, 0.0)) < 16.0);
        let bodies = WaterBodies::build(&world);
        assert!(!bodies.body(bodies.at(-100.0, 0.0).unwrap()).unwrap().lake, "a pond 32 m across");
        let mut world = dry_world();
        let half = world.half_extents();
        flood(&mut world, |p| p.distance(vec2(-half.x, -half.y)) < 60.0);
        let bodies = WaterBodies::build(&world);
        assert!(bodies.body(bodies.ids()[0]).unwrap().lake);
    }

    #[test]
    fn a_river_with_a_branch_is_one_body() {
        let mut world = dry_world();
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x.abs() < 200.0);
        flood(&mut world, |p| p.x.abs() < 6.0 && p.y > 0.0 && p.y < 150.0);
        let bodies = WaterBodies::build(&world);
        assert_eq!(bodies.ids().len(), 1);
        assert_eq!(bodies.at(-150.0, 0.0), bodies.at(0.0, 100.0), "the branch is part of it");
    }

    #[test]
    fn water_that_only_touches_at_a_corner_is_one_body() {
        let mut world = dry_world();
        let (nx, _) = world.samples();
        let sample = |i: usize, j: usize| j * nx + i;
        // Two samples, one up and one across from the other, touching only at a corner.
        world.water_mut()[sample(40, 40)] = 10.0;
        world.water_mut()[sample(41, 41)] = 10.0;
        let bodies = WaterBodies::build(&world);
        assert_eq!(bodies.ids().len(), 1);
        assert_eq!(bodies.body(bodies.ids()[0]).unwrap().samples, 2);
    }

    #[test]
    fn a_body_is_known_by_its_first_sample_and_the_same_every_time() {
        let mut world = dry_world();
        flood(&mut world, |p| p.distance(vec2(-100.0, -100.0)) < 30.0);
        flood(&mut world, |p| p.distance(vec2(100.0, 100.0)) < 30.0);
        let (first, second) = (WaterBodies::build(&world), WaterBodies::build(&world));
        assert_eq!(first.ids(), second.ids());
        assert!(first.ids().windows(2).all(|p| p[0] < p[1]), "in the order of their first samples");
        // The anchor is the number of the body's first sample, counting along the rows: the lake near the north-west
        // comes before the one near the south-east.
        assert_eq!(first.at(-100.0, -100.0), Some(first.ids()[0]));
        assert_eq!(first.at(100.0, 100.0), Some(first.ids()[1]));
    }

    #[test]
    fn near_can_be_told_to_look_only_at_lakes_or_only_at_rivers() {
        let mut world = dry_world();
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x.abs() < 200.0 && p.y > -6.0);
        flood(&mut world, |p| p.distance(vec2(0.0, 100.0)) < 40.0);
        let bodies = WaterBodies::build(&world);
        let (river, lake) = (bodies.at(0.0, 0.0).unwrap(), bodies.at(0.0, 100.0).unwrap());
        // Between them, but nearer the river.
        assert_eq!(bodies.near(0.0, 20.0, 80.0), Some(river));
        assert_eq!(bodies.near_matching(0.0, 20.0, 80.0, |b| b.lake), Some(lake));
        assert_eq!(bodies.near_matching(0.0, 100.0, 100.0, |b| !b.lake), Some(river), "the lake is not there to be found");
        assert_eq!(bodies.near_matching(0.0, 100.0, 5.0, |b| !b.lake), None);
    }

    #[test]
    fn the_label_of_a_body_is_in_the_water() {
        let mut world = dry_world();
        flood(&mut world, |p| p.distance(vec2(30.0, -40.0)) < 50.0);
        let bodies = WaterBodies::build(&world);
        let body = bodies.body(bodies.ids()[0]).unwrap();
        assert_eq!(world.water_class(body.label.x, body.label.y), WaterClass::Inland);
        assert!(body.label.distance(vec2(30.0, -40.0)) < 10.0, "near the middle: {}", body.label);
    }

    #[test]
    fn near_finds_water_a_little_way_off_but_not_far() {
        let mut world = dry_world();
        flood(&mut world, |p| p.y.abs() < 6.0 && p.x.abs() < 200.0);
        let bodies = WaterBodies::build(&world);
        let river = bodies.at(0.0, 0.0).unwrap();
        assert_eq!(bodies.near(0.0, 0.0, 1.0), Some(river), "in the water");
        assert_eq!(bodies.near(0.0, 20.0, 30.0), Some(river), "a few meters off");
        assert_eq!(bodies.near(0.0, 80.0, 30.0), None, "too far");
        assert_eq!(bodies.near(0.0, 80.0, 100.0), Some(river), "within a wider reach");
    }
}
