//! Where the regions of a map are, and how they meet: a center for each region and, for each pair of regions
//! that touch, the middle point of the border between them. A game that lays paths, roads or borders over
//! the map works from these.
//!
//! The center of a region is the sample of it nearest the average of all its samples, so it always lies inside
//! the region even when the region is curved. The middle point of an edge is the point of the border between
//! two regions that is halfway along it.

use std::collections::HashMap;

use glam::{vec2, Vec2};

use crate::regions::RegionId;
use crate::world::{World, CELL_SIZE};

/// A region that touches another, and the middle of the border between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub region: RegionId,
    /// The middle point of the border, in world x and z.
    pub midpoint: Vec2,
}

/// The centers and the borders of the regions of a map, found once from [`World::regions`]. It doesn't change
/// as the world does: make it again if the regions are made again.
#[derive(Debug, Default)]
pub struct RegionMap {
    centers: HashMap<u16, Vec2>,
    neighbors: HashMap<u16, Vec<Neighbor>>,
}

impl RegionMap {
    /// Finds the centers and borders of the regions of `world`. A map with no regions gives an empty one.
    pub fn build(world: &World) -> Self {
        let (nx, nz) = world.samples();
        let half = world.half_extents();
        let regions = world.regions();
        let position = |i: usize, j: usize| vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);

        // The average of each region's samples.
        let mut sums: HashMap<u16, (Vec2, u32)> = HashMap::new();
        for j in 0..nz {
            for i in 0..nx {
                let region = regions[j * nx + i];
                if RegionId::from_raw(region).is_some() {
                    let entry = sums.entry(region).or_insert((Vec2::ZERO, 0));
                    entry.0 += position(i, j);
                    entry.1 += 1;
                }
            }
        }
        let averages: HashMap<u16, Vec2> = sums.into_iter().map(|(region, (sum, count))| (region, sum / count as f32)).collect();

        // Each region's center is its sample nearest that, and the borders are where a sample's region
        // differs from the one beside it or below it.
        let mut nearest: HashMap<u16, (f32, Vec2)> = HashMap::new();
        let mut borders: HashMap<(u16, u16), Vec<Vec2>> = HashMap::new();
        for j in 0..nz {
            for i in 0..nx {
                let region = regions[j * nx + i];
                if RegionId::from_raw(region).is_none() {
                    continue;
                }
                let here = position(i, j);
                let away = here.distance_squared(averages[&region]);
                let best = nearest.entry(region).or_insert((f32::INFINITY, here));
                if away < best.0 {
                    *best = (away, here);
                }
                for (other_i, other_j) in [(i + 1, j), (i, j + 1)] {
                    if other_i >= nx || other_j >= nz {
                        continue;
                    }
                    let other = regions[other_j * nx + other_i];
                    if other != region && RegionId::from_raw(other).is_some() {
                        let key = (region.min(other), region.max(other));
                        borders.entry(key).or_default().push((here + position(other_i, other_j)) * 0.5);
                    }
                }
            }
        }

        let centers: HashMap<u16, Vec2> = nearest.into_iter().map(|(region, (_, at))| (region, at)).collect();
        let mut neighbors: HashMap<u16, Vec<Neighbor>> = HashMap::new();
        for ((a, b), points) in borders {
            let midpoint = middle_of(&points);
            if let (Some(ra), Some(rb)) = (RegionId::from_raw(a), RegionId::from_raw(b)) {
                neighbors.entry(a).or_default().push(Neighbor { region: rb, midpoint });
                neighbors.entry(b).or_default().push(Neighbor { region: ra, midpoint });
            }
        }
        // The same order every time, whatever order the borders were found in.
        for list in neighbors.values_mut() {
            list.sort_by_key(|n| n.region.raw());
        }
        Self { centers, neighbors }
    }

    /// Whether there are no regions.
    pub fn is_empty(&self) -> bool {
        self.centers.is_empty()
    }

    /// The center of `region`, in world x and z.
    pub fn center(&self, region: RegionId) -> Option<Vec2> {
        self.centers.get(&region.raw()).copied()
    }

    /// The regions that touch `region`, with the middle of each border, in order of their numbers.
    pub fn neighbors(&self, region: RegionId) -> &[Neighbor] {
        self.neighbors.get(&region.raw()).map_or(&[], Vec::as_slice)
    }

    /// The middle point of the border between `a` and `b`, if they touch.
    pub fn edge_midpoint(&self, a: RegionId, b: RegionId) -> Option<Vec2> {
        self.neighbors(a).iter().find(|n| n.region == b).map(|n| n.midpoint)
    }
}

/// The point of `points`, which lie along a border, that is halfway along it: the one in the middle when they
/// are put in order along the line between the two that are farthest apart.
fn middle_of(points: &[Vec2]) -> Vec2 {
    let mean = points.iter().copied().sum::<Vec2>() / points.len() as f32;
    let farthest_from = |from: Vec2| points.iter().copied().max_by(|a, b| a.distance_squared(from).total_cmp(&b.distance_squared(from))).unwrap_or(from);
    let start = farthest_from(mean);
    let end = farthest_from(start);
    let along = (end - start).normalize_or_zero();
    let mut ordered: Vec<Vec2> = points.to_vec();
    // Ties are broken by the point itself, so that the answer doesn't depend on the order they came in.
    ordered.sort_by(|a, b| (*a - start).dot(along).total_cmp(&(*b - start).dot(along)).then(a.x.total_cmp(&b.x)).then(a.y.total_cmp(&b.y)));
    ordered[ordered.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u16 = 0xC001;
    const B: u16 = 0xC002;
    const C: u16 = 0xC003;

    fn id(raw: u16) -> RegionId {
        RegionId::from_raw(raw).unwrap()
    }

    /// A world 256 m across (65 samples each way) with regions laid out by `region(i, j)`.
    fn world_with(region: impl Fn(usize, usize) -> u16) -> World {
        let mut world = World::new(256.0, 256.0);
        let (nx, nz) = world.samples();
        let mut regions = vec![0u16; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                regions[j * nx + i] = region(i, j);
            }
        }
        world.set_regions(regions);
        world
    }

    #[test]
    fn a_map_with_no_regions_has_none() {
        let map = RegionMap::build(&World::new(256.0, 256.0));
        assert!(map.is_empty());
        assert!(map.center(id(A)).is_none());
        assert!(map.neighbors(id(A)).is_empty());
    }

    #[test]
    fn two_regions_side_by_side_meet_at_the_middle_of_their_border() {
        // A is the left half (samples 0..32), B the right half.
        let world = world_with(|i, _| if i < 32 { A } else { B });
        let map = RegionMap::build(&world);
        let half = world.half_extents();

        let (a, b) = (map.center(id(A)).unwrap(), map.center(id(B)).unwrap());
        assert_eq!(world.region_at(a.x, a.y), Some(id(A)), "a center is inside its region");
        assert_eq!(world.region_at(b.x, b.y), Some(id(B)));
        assert!(a.x < 0.0 && b.x > 0.0);
        // Their centers are about halfway down the map, as the regions run the whole way.
        assert!(a.y.abs() <= CELL_SIZE && b.y.abs() <= CELL_SIZE, "{a} {b}");

        // The border is the column between sample 31 and 32, which is at x = 31.5 * 4 - 128 = -2, and the
        // middle of it is halfway down the map.
        let edge = map.edge_midpoint(id(A), id(B)).unwrap();
        assert!((edge.x - (31.5 * CELL_SIZE - half.x)).abs() < 0.01, "{edge}");
        assert!(edge.y.abs() <= CELL_SIZE, "{edge}");
        assert_eq!(map.edge_midpoint(id(B), id(A)), Some(edge), "the same from either side");
    }

    #[test]
    fn neighbors_are_listed_for_both_regions_in_order() {
        // A | B on top, C along the whole bottom.
        let world = world_with(|i, j| if j >= 40 { C } else if i < 32 { A } else { B });
        let map = RegionMap::build(&world);
        let touching = |r: u16| map.neighbors(id(r)).iter().map(|n| n.region.raw()).collect::<Vec<_>>();
        assert_eq!(touching(A), [B, C]);
        assert_eq!(touching(B), [A, C]);
        assert_eq!(touching(C), [A, B]);
        // A and C meet along the bottom of A, and the middle of that is under the middle of A.
        let ac = map.edge_midpoint(id(A), id(C)).unwrap();
        let a = map.center(id(A)).unwrap();
        assert!((ac.x - a.x).abs() <= 2.0 * CELL_SIZE, "{ac} {a}");
    }

    #[test]
    fn regions_that_do_not_touch_are_not_neighbors() {
        // A, a gap of no region, then B.
        let world = world_with(|i, _| if i < 20 { A } else if i > 40 { B } else { 0 });
        let map = RegionMap::build(&world);
        assert!(map.edge_midpoint(id(A), id(B)).is_none());
        assert!(map.neighbors(id(A)).is_empty(), "no region is not a neighbor");
    }

    #[test]
    fn the_center_of_a_curved_region_is_still_inside_it() {
        // An L: the left column and the bottom row of the map, whose average is outside it.
        let world = world_with(|i, j| if i < 10 || j >= 55 { A } else { B });
        let map = RegionMap::build(&world);
        let a = map.center(id(A)).unwrap();
        assert_eq!(world.region_at(a.x, a.y), Some(id(A)), "{a}");
    }

    #[test]
    fn the_middle_of_a_curved_border_is_on_the_border() {
        // B is a block in the middle of A, so their border is a ring.
        let world = world_with(|i, j| if (20..44).contains(&i) && (20..44).contains(&j) { B } else { A });
        let map = RegionMap::build(&world);
        let edge = map.edge_midpoint(id(A), id(B)).unwrap();
        // It is one of the border points, which are 48 m from the middle of the block along its sides and
        // about 68 m at its corners.
        let b = map.center(id(B)).unwrap();
        let from_center = edge.distance(b);
        assert!((44.0..72.0).contains(&from_center), "{from_center}");
    }

    #[test]
    fn it_is_the_same_every_time() {
        let world = world_with(|i, j| if i < 32 { if j < 30 { A } else { C } } else { B });
        let (first, second) = (RegionMap::build(&world), RegionMap::build(&world));
        for raw in [A, B, C] {
            assert_eq!(first.center(id(raw)), second.center(id(raw)));
            assert_eq!(first.neighbors(id(raw)), second.neighbors(id(raw)));
        }
    }
}
