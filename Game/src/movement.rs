//! How far a unit can go in a day. A unit has movement points, and the ground it crosses costs them by the meter: open
//! land the most, a path less, and a road and a bridge, which are the same, the least. Water can't be crossed, except on
//! a bridge, and neither can a mountain, except by a path or a road.
//!
//! The ground is worked on in the cells the map itself is made of (see [`CELL_SIZE`]). A unit moves between a cell and
//! the eight cells round it, and what it costs is how far that is, times what the cell it goes into costs a meter. A path,
//! a road or a bridge is the cells along the lines it is drawn on, so a unit can step on to one anywhere along it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use macroquad::math::{vec2, Vec2};
use nworld_map::regions::MOUNTAIN_HEIGHT;
use nworld_map::world::{WaterClass, World, CELL_SIZE};

/// The movement points a unit has each day.
pub const DAILY_POINTS: u32 = 200;
/// What a meter costs in movement points, on open land, on a path, and on a road or a bridge.
pub const LAND_COST: f32 = 1.0;
pub const PATH_COST: f32 = 0.6;
pub const ROAD_COST: f32 = 0.3;
/// How far apart the points are that a line is laid down in cells with, in meters: closer than a cell, so none is skipped.
const LINE_STEP: f32 = CELL_SIZE * 0.5;

/// What a cell of ground is, for moving over it. Each is faster than the one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ground {
    Land,
    Path,
    Road,
    Bridge,
}

impl Ground {
    /// What a meter of it costs, in movement points.
    pub const fn cost(self) -> f32 {
        match self {
            Self::Land => LAND_COST,
            Self::Path => PATH_COST,
            Self::Road | Self::Bridge => ROAD_COST,
        }
    }
}

/// Where the paths, the roads and the bridges are, as the lines they go along. The ones that have failed are not there.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    pub paths: Vec<Vec<Vec2>>,
    pub roads: Vec<Vec<Vec2>>,
    /// The two ends of each bridge, which the bridge goes straight between.
    pub bridges: Vec<(Vec2, Vec2)>,
}

/// Everywhere a unit can get to with the movement points it has, and what it costs to get there.
#[derive(Clone, Debug)]
pub struct Reach {
    /// The cell numbers of the corner of the window of cells that was searched, and how many cells wide it is.
    low: (i32, i32),
    size: usize,
    /// What it costs to get to each cell of the window: infinite for the cells that can't be got to.
    cost: Vec<f32>,
    /// The movement points the search was for.
    points: f32,
    half: Vec2,
    /// Where it started.
    from: Vec2,
    /// The cell each cell was got to from, on the cheapest way: `usize::MAX` for the one it started in and those that
    /// weren't got to.
    came_from: Vec<usize>,
}

impl Reach {
    fn index(&self, cell: (i32, i32)) -> Option<usize> {
        let (i, j) = (cell.0 - self.low.0, cell.1 - self.low.1);
        (i >= 0 && j >= 0 && (i as usize) < self.size && (j as usize) < self.size).then(|| j as usize * self.size + i as usize)
    }

    fn cell_of(&self, at: Vec2) -> (i32, i32) {
        cell_of(self.half, at)
    }

    /// What it costs to get to the cell `at` is in, if it can be got to.
    pub fn cost_to(&self, at: Vec2) -> Option<f32> {
        let cost = self.cost[self.index(self.cell_of(at))?];
        (cost <= self.points).then_some(cost)
    }

    /// The middle of the cell `at` is in.
    pub fn snap(&self, at: Vec2) -> Vec2 {
        centre_of(self.half, self.cell_of(at))
    }

    /// Every cell that can be got to, as its middle and what it costs to get there.
    pub fn cells(&self) -> impl Iterator<Item = (Vec2, f32)> + '_ {
        self.cost.iter().enumerate().filter(|(_, cost)| **cost <= self.points).map(|(n, cost)| {
            let cell = (self.low.0 + (n % self.size) as i32, self.low.1 + (n / self.size) as i32);
            (centre_of(self.half, cell), *cost)
        })
    }

    /// The way to the cell `at` is in, if it can be got to: where it started, and then the middle of each cell along the
    /// cheapest way, to the middle of that one.
    pub fn route_to(&self, at: Vec2) -> Option<Vec<Vec2>> {
        let mut n = self.index(self.cell_of(at))?;
        if self.cost[n] > self.points {
            return None;
        }
        let mut route = Vec::new();
        while n != usize::MAX {
            let cell = (self.low.0 + (n % self.size) as i32, self.low.1 + (n / self.size) as i32);
            route.push(centre_of(self.half, cell));
            n = self.came_from[n];
        }
        route.reverse();
        route[0] = self.from;
        Some(route)
    }

    /// The movement points the search was for.
    pub fn points(&self) -> f32 {
        self.points
    }
}

fn cell_of(half: Vec2, at: Vec2) -> (i32, i32) {
    (((at.x + half.x) / CELL_SIZE).round() as i32, ((at.y + half.y) / CELL_SIZE).round() as i32)
}

fn centre_of(half: Vec2, cell: (i32, i32)) -> Vec2 {
    vec2(cell.0 as f32 * CELL_SIZE - half.x, cell.1 as f32 * CELL_SIZE - half.y)
}

/// Whether the ground at `at` can be walked on, if it is `ground`: not off the map, not water unless it is a bridge, and
/// not a mountain unless it is a path or a road as well.
fn passable(world: &World, at: Vec2, cell: (i32, i32), ground: Ground) -> bool {
    let (nx, nz) = world.samples();
    if cell.0 < 0 || cell.1 < 0 || cell.0 as usize >= nx || cell.1 as usize >= nz {
        return false;
    }
    if ground == Ground::Bridge {
        return true;
    }
    world.water_class(at.x, at.y) == WaterClass::Dry && (ground != Ground::Land || world.surface_height(at.x, at.y) < MOUNTAIN_HEIGHT)
}

/// Where a unit at `from` with `points` movement points can get to, over the ground of `world`, with the paths, roads and
/// bridges of `network`.
pub fn reach(world: &World, network: &Network, from: Vec2, points: f32) -> Reach {
    let half = world.half_extents();
    let start = cell_of(half, from);
    // The farthest it can possibly get is along a road, so that is how far to look.
    let radius = (points / ROAD_COST / CELL_SIZE).ceil() as i32 + 1;
    let size = (2 * radius + 1) as usize;
    let low = (start.0 - radius, start.1 - radius);
    let mut result = Reach { low, size, cost: vec![f32::INFINITY; size * size], points, half, from, came_from: vec![usize::MAX; size * size] };

    // What each cell of the window is: land, unless a line goes through it.
    let mut ground = vec![Ground::Land; size * size];
    let mut lay = |line: &[Vec2], kind: Ground| {
        for pair in line.windows(2) {
            let steps = (pair[0].distance(pair[1]) / LINE_STEP).ceil().max(1.0) as usize;
            for k in 0..=steps {
                let cell = cell_of(half, pair[0].lerp(pair[1], k as f32 / steps as f32));
                if let Some(n) = result.index(cell) {
                    ground[n] = ground[n].max(kind);
                }
            }
        }
    };
    network.paths.iter().for_each(|line| lay(line, Ground::Path));
    network.roads.iter().for_each(|line| lay(line, Ground::Road));
    network.bridges.iter().for_each(|(a, b)| lay(&[*a, *b], Ground::Bridge));

    // Whether each cell can be walked on, found when first asked.
    let mut open: Vec<Option<bool>> = vec![None; size * size];
    let mut is_open = |result: &Reach, cell: (i32, i32)| -> bool {
        let Some(n) = result.index(cell) else { return false };
        *open[n].get_or_insert_with(|| passable(world, centre_of(half, cell), cell, ground[n]))
    };

    // Dijkstra's way: always on from the cell that costs least to have got to. The costs are not negative, so their bits
    // sort in the same order as they do.
    let mut queue = BinaryHeap::new();
    let first = result.index(start).expect("the start is in the middle of the window");
    result.cost[first] = 0.0;
    queue.push(Reverse((0.0f32.to_bits(), first)));
    while let Some(Reverse((bits, n))) = queue.pop() {
        let here = f32::from_bits(bits);
        if here > result.cost[n] {
            continue;
        }
        let cell = (low.0 + (n % size) as i32, low.1 + (n / size) as i32);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let next = (cell.0 + di, cell.1 + dj);
            let Some(m) = result.index(next) else { continue };
            if !is_open(&result, next) {
                continue;
            }
            // A diagonal step may not cut the corner of what can't be walked on, unless it is along a bridge.
            if di != 0 && dj != 0 && !(ground[n] == Ground::Bridge && ground[m] == Ground::Bridge) && !(is_open(&result, (cell.0 + di, cell.1)) && is_open(&result, (cell.0, cell.1 + dj))) {
                continue;
            }
            let step = if di != 0 && dj != 0 { CELL_SIZE * std::f32::consts::SQRT_2 } else { CELL_SIZE };
            let through = here + step * ground[m].cost();
            if through < result.cost[m] && through <= points {
                result.cost[m] = through;
                result.came_from[m] = n;
                queue.push(Reverse((through.to_bits(), m)));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    /// A spot with a lot of dry ground round it.
    fn spot(world: &World) -> Vec2 {
        test_support::inland_spot(world, 150.0)
    }

    #[test]
    fn each_kind_of_ground_costs_less_a_meter_than_the_last() {
        assert!(Ground::Land.cost() > Ground::Path.cost() && Ground::Path.cost() > Ground::Road.cost());
        assert_eq!(Ground::Bridge.cost(), Ground::Road.cost(), "a bridge acts the same as a road");
        assert_eq!((LAND_COST, PATH_COST, ROAD_COST), (1.0, 0.6, 0.3));
        assert_eq!(DAILY_POINTS, 200);
    }

    #[test]
    fn over_open_land_a_unit_gets_about_as_many_meters_as_it_has_points_and_never_over_water_or_mountains() {
        let world = test_support::small_continent();
        let from = spot(&world);
        let reach = reach(&world, &Network::default(), from, 100.0);
        let mut farthest: f32 = 0.0;
        for (at, cost) in reach.cells() {
            // Going by cells is never shorter than going straight, nor more than a cell or so longer than the longest way
            // round, which is 8% over.
            assert!(cost >= at.distance(reach.snap(from)) - 0.01, "{at}: {cost}");
            assert!(cost <= 100.0);
            assert_eq!(world.water_class(at.x, at.y), WaterClass::Dry, "{at} is water");
            assert!(world.surface_height(at.x, at.y) < MOUNTAIN_HEIGHT, "{at} is a mountain");
            farthest = farthest.max(at.distance(from));
        }
        assert!(farthest > 20.0 && farthest <= 104.0, "no more than 100 m, and not stuck: {farthest}");
        assert_eq!(reach.cost_to(from), Some(0.0), "it is where it is for nothing");
    }

    #[test]
    fn a_road_is_over_three_times_as_far_as_land_and_a_path_not_quite_twice() {
        let world = test_support::small_continent();
        let from = spot(&world);
        // A line through where it stands, 150 m each way over dry ground, and 40 points to spend on it.
        let line = vec![from - vec2(150.0, 0.0), from + vec2(150.0, 0.0)];
        let along = |network: Network| {
            let reach = reach(&world, &network, from, 40.0);
            reach.cells().map(|(at, _)| at.x - from.x).fold(0.0f32, f32::max)
        };
        let road = along(Network { roads: vec![line.clone()], ..Network::default() });
        let path = along(Network { paths: vec![line.clone()], ..Network::default() });
        let land = along(Network::default());
        assert!((road - 133.0).abs() < 6.0, "40 points on a road is about 133 m: {road}");
        assert!((path - 66.0).abs() < 6.0, "about 67 m on a path: {path}");
        assert!(land <= 42.0, "no more than 40 m on land: {land}");
        assert!(road > path && path > land, "{road} {path} {land}");
    }

    #[test]
    fn it_can_step_on_to_a_road_anywhere_along_it_and_off_again_at_land_cost() {
        let world = test_support::small_continent();
        let from = spot(&world);
        // A road 40 m to the east, going north and south.
        let x = from.x + 40.0;
        let network = Network { roads: vec![vec![vec2(x, from.y - 120.0), vec2(x, from.y + 120.0)]], ..Network::default() };
        let reach = reach(&world, &network, from, 100.0);
        // 40 m of land to the road, then along it: 100 m up the road costs 40 + 30.
        let up = reach.cost_to(vec2(x, from.y + 100.0)).expect("up the road");
        assert!((60.0..=80.0).contains(&up), "{up}");
        // Without the road, that is not in reach.
        let without = super::reach(&world, &Network::default(), from, 100.0);
        assert_eq!(without.cost_to(vec2(x, from.y + 100.0)), None);
        // And 30 m off the road, at the far end, is land cost on top.
        let off = reach.cost_to(vec2(x + 20.0, from.y + 60.0));
        assert!(off.is_some_and(|c| c > reach.cost_to(vec2(x, from.y + 60.0)).unwrap()));
    }

    #[test]
    fn a_unit_cannot_cross_water_but_can_on_a_bridge() {
        let world = test_support::small_continent();
        let half = world.half_extents();
        // A dry cell on a shore with at least 4 cells of water to the east of it.
        let (nx, nz) = world.samples();
        let at = |i: usize, j: usize| vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y);
        let shore = (2..nz - 2)
            .flat_map(|j| (8..nx - 8).map(move |i| (i, j)))
            .find(|&(i, j)| {
                world.water_class(at(i, j).x, at(i, j).y) == WaterClass::Dry
                    && (1..=4).all(|k| world.water_class(at(i + k, j).x, at(i + k, j).y) != WaterClass::Dry)
                    && (1..=6).all(|k| world.water_class(at(i - k, j).x, at(i - k, j).y) == WaterClass::Dry && world.surface_height(at(i - k, j).x, at(i - k, j).y) < MOUNTAIN_HEIGHT)
            })
            .expect("a shore");
        let (from, over) = (at(shore.0 - 3, shore.1), at(shore.0 + 3, shore.1));
        let without = reach(&world, &Network::default(), from, 100.0);
        assert_eq!(without.cost_to(over), None, "no crossing the water");
        let bridge = Network { bridges: vec![(at(shore.0, shore.1), at(shore.0 + 4, shore.1))], ..Network::default() };
        let with = reach(&world, &bridge, from, 100.0);
        let cost = with.cost_to(over).expect("on the bridge");
        // 12 m of land, then the bridge at a road's price.
        assert!(cost < 12.0 + 0.3 * 16.0 + 1.0, "{cost}");
        assert!(with.cost_to(at(shore.0 + 2, shore.1)).is_some(), "the middle of the bridge");
        assert_eq!(with.cost_to(at(shore.0 + 6, shore.1)), None, "and the water beyond it is no more crossed");
    }

    #[test]
    fn a_spot_is_snapped_to_its_cell_and_one_too_far_is_not_reached() {
        let world = test_support::small_continent();
        let from = spot(&world);
        let reach = reach(&world, &Network::default(), from, 100.0);
        let there = from + vec2(40.0, 0.0);
        assert!(reach.cost_to(there).is_some_and(|cost| (39.0..=45.0).contains(&cost)));
        assert_eq!(reach.cost_to(from + vec2(900.0, 0.0)), None, "too far");
        assert_eq!(reach.snap(there), reach.snap(there + vec2(0.4, -0.4)), "the same cell is the same place");
        assert_eq!(reach.snap(there).distance(there) <= CELL_SIZE, true);
    }

    #[test]
    fn the_way_there_goes_cell_by_cell_from_where_it_started_to_the_spot() {
        let world = test_support::small_continent();
        let from = spot(&world);
        let reach = reach(&world, &Network::default(), from, 100.0);
        let there = from + vec2(30.0, 40.0);
        let route = reach.route_to(there).expect("in reach");
        assert_eq!(route[0], from);
        assert_eq!(*route.last().unwrap(), reach.snap(there));
        for pair in route.windows(2) {
            assert!(pair[0].distance(pair[1]) <= CELL_SIZE * 1.5, "no jumps: {} to {}", pair[0], pair[1]);
        }
        // It can't be shorter than straight, and costs what the cell says on open land.
        let length: f32 = route.windows(2).map(|p| p[0].distance(p[1])).sum();
        assert!(length >= from.distance(reach.snap(there)) - 0.01);
        assert!((length - reach.cost_to(there).unwrap()).abs() < CELL_SIZE, "on open land the way costs its length: {length}");
        assert_eq!(reach.route_to(from + vec2(900.0, 0.0)), None, "none to somewhere it can't get to");
        assert_eq!(reach.route_to(from).unwrap(), vec![from], "none to go to where it is");
    }

    #[test]
    fn fewer_points_reach_less() {
        let world = test_support::small_continent();
        let from = spot(&world);
        let count = |points: f32| reach(&world, &Network::default(), from, points).cells().count();
        assert!(count(100.0) > count(50.0) && count(50.0) > count(10.0) && count(10.0) > 1);
        assert_eq!(count(0.0), 1, "with none left it is only where it is");
    }
}
