//! Drawing the paths: each segment as a ribbon of packed dirt laid on the ground, soft at its edges. This is
//! the game's own, and not part of drawing the map.
//!
//! A path never crosses water. A segment is a straight line from the center of a region to the middle of one of
//! its edges, and where that line would cross a lake or a river, the path goes round instead, by the shortest
//! way over dry ground. If there is no way round, it is drawn as far as the water on each side and not over it.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
use nworld_map::world::{WaterClass, World, CELL_SIZE};

use crate::path::PathSegment;

/// How wide a path is, in meters, counting its soft edges.
const WIDTH: f32 = 7.0;
/// How far above the ground it lies, so that it isn't lost in it.
const LIFT: f32 = 0.35;
/// The dirt, and how much its brightness varies along a path.
const DIRT: [f32; 3] = [0.56, 0.42, 0.27];
const VARIATION: f32 = 0.10;
/// Most vertices in one mesh, which a mesh's 16 bit indices can't go past.
const MAX_VERTICES: usize = 60_000;
/// How far from water, in meters, the middle of a path keeps, so that its width stays off the water.
const SHORE_CLEARANCE: f32 = 3.0;
/// How far apart the points are that the way round is looked for among, and how far apart the points are
/// checked along a line for water, in meters.
const GRID: f32 = 6.0;
const CHECK_EVERY: f32 = 2.0;

/// The paths as meshes, made again when the paths change.
#[derive(Default)]
pub struct PathRenderer {
    built: Vec<PathSegment>,
    /// Where each segment goes, as lines of points over dry ground, so that the way round a lake is only
    /// worked out once.
    routes: HashMap<PathSegment, Vec<Vec<Vec2>>>,
    meshes: Vec<Mesh>,
}

impl PathRenderer {
    /// Draws `paths`, as seen by the camera that is set, which must still be the one the map was drawn with.
    pub fn draw(&mut self, world: &World, map: &RegionMap, paths: &[PathSegment]) {
        if self.built != paths {
            for segment in paths {
                if !self.routes.contains_key(segment) {
                    let route = ends(map, *segment).map(|(from, to)| route(world, from, to)).unwrap_or_default();
                    self.routes.insert(*segment, route);
                }
            }
            self.routes.retain(|segment, _| paths.contains(segment));
            let lines: Vec<&Vec<Vec2>> = paths.iter().filter_map(|segment| self.routes.get(segment)).flatten().collect();
            self.meshes = build(world, &lines);
            self.built = paths.to_vec();
        }
        for mesh in &self.meshes {
            draw_mesh(mesh);
        }
    }
}

/// The ends of `segment` on the map, in world x and z: the center of its region, and the middle of the edge it
/// shares with its neighbor.
pub fn ends(map: &RegionMap, segment: PathSegment) -> Option<(Vec2, Vec2)> {
    let (region, neighbor) = (RegionId::from_raw(segment.region)?, RegionId::from_raw(segment.neighbor)?);
    Some((map.center(region)?, map.edge_midpoint(region, neighbor)?))
}

fn dry(world: &World, at: Vec2) -> bool {
    world.water_class(at.x, at.y) == WaterClass::Dry
}

/// Whether the ground at `at` is dry, and so is some way round it.
fn clear(world: &World, at: Vec2) -> bool {
    dry(world, at) && [vec2(SHORE_CLEARANCE, 0.0), vec2(-SHORE_CLEARANCE, 0.0), vec2(0.0, SHORE_CLEARANCE), vec2(0.0, -SHORE_CLEARANCE)].iter().all(|o| dry(world, at + *o))
}

/// Whether the straight line from `a` to `b` is clear all along.
fn line_clear(world: &World, a: Vec2, b: Vec2) -> bool {
    let steps = (a.distance(b) / CHECK_EVERY).ceil().max(1.0) as usize;
    (0..=steps).all(|k| clear(world, a.lerp(b, k as f32 / steps as f32)))
}

/// Where a path from `from` to `to` goes: lines of points that are all over dry ground. Usually that is the straight
/// line. If water is in the way it goes round it, and if it can't, it is the straight line cut short where the
/// water is, so that more than one line comes back.
pub fn route(world: &World, from: Vec2, to: Vec2) -> Vec<Vec<Vec2>> {
    if line_clear(world, from, to) {
        return vec![vec![from, to]];
    }
    if let Some(way) = way_round(world, from, to) {
        return vec![way];
    }
    cut_short(world, from, to)
}

/// The straight line from `from` to `to` as the pieces of it that are dry (just dry, not clear of the shore),
/// each of at least two points.
fn cut_short(world: &World, from: Vec2, to: Vec2) -> Vec<Vec<Vec2>> {
    let steps = (from.distance(to) / CHECK_EVERY).ceil().max(1.0) as usize;
    let mut pieces: Vec<Vec<Vec2>> = Vec::new();
    let mut current: Vec<Vec2> = Vec::new();
    for k in 0..=steps {
        let at = from.lerp(to, k as f32 / steps as f32);
        if dry(world, at) {
            current.push(at);
        } else if !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces.retain(|piece| piece.len() >= 2);
    pieces
}

/// The grid of points the way round is looked for among, over the ground between the two ends and some way to
/// each side of it.
struct Grid<'a> {
    world: &'a World,
    low: Vec2,
    cols: usize,
    rows: usize,
    /// Whether each point is clear, found when first asked.
    clear: Vec<Option<bool>>,
}

impl Grid<'_> {
    fn at(&self, node: usize) -> Vec2 {
        self.low + vec2((node % self.cols) as f32, (node / self.cols) as f32) * GRID
    }

    fn is_clear(&mut self, node: usize) -> bool {
        let at = self.at(node);
        let world = self.world;
        *self.clear[node].get_or_insert_with(|| clear(world, at))
    }

    /// The clear point near `p` that a line from `p` can reach, and is nearest to it, if there is one.
    fn entry(&mut self, p: Vec2) -> Option<usize> {
        let (ci, cj) = (((p.x - self.low.x) / GRID).round() as i32, ((p.y - self.low.y) / GRID).round() as i32);
        let mut best: Option<(f32, usize)> = None;
        for dj in -2..=2 {
            for di in -2..=2 {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= self.cols as i32 || j >= self.rows as i32 {
                    continue;
                }
                let node = j as usize * self.cols + i as usize;
                let distance = self.at(node).distance(p);
                if self.is_clear(node) && best.is_none_or(|(d, _)| distance < d) && line_clear(self.world, p, self.at(node)) {
                    best = Some((distance, node));
                }
            }
        }
        best.map(|(_, node)| node)
    }
}

/// The shortest way from `from` to `to` over clear ground, going round the water between them, if there is one.
fn way_round(world: &World, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
    // Looked for over the ground between them and a margin around it: far enough to get round a lake.
    let margin = (from.distance(to) * 0.6).clamp(40.0, 250.0);
    let low = from.min(to) - Vec2::splat(margin);
    let high = from.max(to) + Vec2::splat(margin);
    let (cols, rows) = (((high.x - low.x) / GRID).ceil() as usize + 1, ((high.y - low.y) / GRID).ceil() as usize + 1);
    let mut grid = Grid { world, low, cols, rows, clear: vec![None; cols * rows] };
    let (start, goal) = (grid.entry(from)?, grid.entry(to)?);

    // A*, with a step along a row or column costing 10 and one across a diagonal 14.
    let goal_at = (goal % cols, goal / cols);
    let estimate = |node: usize| {
        let (dx, dz) = ((node % cols).abs_diff(goal_at.0) as u32, (node / cols).abs_diff(goal_at.1) as u32);
        10 * dx.max(dz) + 4 * dx.min(dz)
    };
    let mut cost = vec![u32::MAX; cols * rows];
    let mut came_from = vec![usize::MAX; cols * rows];
    let mut open = BinaryHeap::new();
    cost[start] = 0;
    open.push(Reverse((estimate(start), start)));
    while let Some(Reverse((_, node))) = open.pop() {
        if node == goal {
            break;
        }
        let (i, j) = ((node % cols) as i32, (node / cols) as i32);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (ni, nj) = (i + di, j + dj);
            if ni < 0 || nj < 0 || ni >= cols as i32 || nj >= rows as i32 {
                continue;
            }
            let next = nj as usize * cols + ni as usize;
            // A diagonal step may not cut the corner of water.
            let corners_clear = di == 0 || dj == 0 || (grid.is_clear((j as usize) * cols + ni as usize) && grid.is_clear(nj as usize * cols + i as usize));
            if !grid.is_clear(next) || !corners_clear {
                continue;
            }
            let through = cost[node] + if di == 0 || dj == 0 { 10 } else { 14 };
            if through < cost[next] {
                cost[next] = through;
                came_from[next] = node;
                open.push(Reverse((through + estimate(next), next)));
            }
        }
    }
    if cost[goal] == u32::MAX {
        return None;
    }
    let mut nodes = vec![goal];
    while *nodes.last()? != start {
        nodes.push(came_from[*nodes.last()?]);
    }
    let mut points: Vec<Vec2> = vec![from];
    points.extend(nodes.iter().rev().map(|&node| grid.at(node)));
    points.push(to);

    // Pulled tight: each point goes straight to the farthest one it can see over clear ground.
    let mut tight = vec![points[0]];
    let mut at = 0;
    while at < points.len() - 1 {
        let mut next = points.len() - 1;
        while next > at + 1 && !line_clear(world, points[at], points[next]) {
            next -= 1;
        }
        tight.push(points[next]);
        at = next;
    }
    Some(tight)
}

fn build(world: &World, lines: &[&Vec<Vec2>]) -> Vec<Mesh> {
    let mut meshes = Vec::new();
    let (mut vertices, mut indices): (Vec<Vertex>, Vec<u16>) = (Vec::new(), Vec::new());
    for line in lines {
        if vertices.len() > MAX_VERTICES {
            meshes.push(Mesh { vertices: std::mem::take(&mut vertices), indices: std::mem::take(&mut indices), texture: None });
        }
        ribbon(world, line, &mut vertices, &mut indices);
    }
    if !vertices.is_empty() {
        meshes.push(Mesh { vertices, indices, texture: None });
    }
    meshes
}

/// `line` with points added so that none are farther apart than a heightmap cell, for the ribbon to follow the
/// ground.
fn resample(line: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for pair in line.windows(2) {
        let steps = (pair[0].distance(pair[1]) / CELL_SIZE).ceil().max(1.0) as usize;
        for k in 0..steps {
            out.push(pair[0].lerp(pair[1], k as f32 / steps as f32));
        }
    }
    if let Some(last) = line.last() {
        out.push(*last);
    }
    out
}

/// Adds the ribbon along `line` to the mesh being made: three vertices across at each point of it, the middle one
/// solid and the outer two clear, so that it fades out at its edges.
fn ribbon(world: &World, line: &[Vec2], vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>) {
    let points = resample(line);
    if points.len() < 2 {
        return;
    }
    let start = vertices.len();
    for (k, &at) in points.iter().enumerate() {
        // Which way the path runs here, from the points either side, so that a bend turns smoothly.
        let along = (points[(k + 1).min(points.len() - 1)] - points[k.saturating_sub(1)]).normalize_or_zero();
        let across = vec2(-along.y, along.x) * (WIDTH * 0.5);
        // A little different from place to place, like ground that has been walked on.
        let shade = 1.0 + VARIATION * ((at.x * 0.37 + at.y * 0.23).sin() * 0.5 + (at.x * 0.11 - at.y * 0.17).cos() * 0.5);
        let color = |alpha: u8| [(DIRT[0] * shade * 255.0).min(255.0) as u8, (DIRT[1] * shade * 255.0).min(255.0) as u8, (DIRT[2] * shade * 255.0).min(255.0) as u8, alpha];
        for (offset, alpha) in [(across, 0), (Vec2::ZERO, 255), (-across, 0)] {
            let point = at + offset;
            vertices.push(Vertex {
                position: vec3(point.x, world.surface_height(point.x, point.y) + LIFT, point.y),
                uv: Vec2::ZERO,
                color: color(alpha),
                normal: Vec4::ZERO,
            });
        }
    }
    for step in 0..points.len() - 1 {
        let row = (start + step * 3) as u16;
        let (l0, m0, r0, l1, m1, r1) = (row, row + 1, row + 2, row + 3, row + 4, row + 5);
        indices.extend([l0, m0, m1, l0, m1, l1, m0, r0, r1, m0, r1, m1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map 512 m across of dry ground, 5 m above the sea.
    fn dry_world() -> World {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        world
    }

    /// Makes a lake of every sample for which `inside` is true, at the world position it gives.
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

    fn length(line: &[Vec2]) -> f32 {
        line.windows(2).map(|p| p[0].distance(p[1])).sum()
    }

    fn all_dry(world: &World, line: &[Vec2]) -> bool {
        line.windows(2).all(|p| {
            let steps = (p[0].distance(p[1]) / 1.0).ceil().max(1.0) as usize;
            (0..=steps).all(|k| dry(world, p[0].lerp(p[1], k as f32 / steps as f32)))
        })
    }

    #[test]
    fn a_path_over_dry_ground_is_the_straight_line() {
        let world = dry_world();
        let (from, to) = (vec2(-100.0, 0.0), vec2(100.0, 30.0));
        assert_eq!(route(&world, from, to), vec![vec![from, to]]);
    }

    #[test]
    fn a_path_goes_round_a_lake_in_its_way() {
        let mut world = dry_world();
        flood(&mut world, |p| p.distance(Vec2::ZERO) < 50.0);
        let (from, to) = (vec2(-150.0, 0.0), vec2(150.0, 0.0));
        assert!(!dry(&world, Vec2::ZERO), "there is a lake between them");

        let lines = route(&world, from, to);
        assert_eq!(lines.len(), 1, "one path, round it");
        let line = &lines[0];
        assert_eq!((line[0], *line.last().unwrap()), (from, to));
        assert!(all_dry(&world, line), "it never touches the water");
        // It keeps off the shore as well, so the width of the path stays on land.
        assert!(line.windows(2).all(|p| (0..=40).all(|k| p[0].lerp(p[1], k as f32 / 40.0).distance(Vec2::ZERO) > 50.0 + 2.5)));
        // And it goes round the nearest way: longer than straight, but not by much more than half the lake round.
        let straight = from.distance(to);
        assert!(length(line) > straight && length(line) < straight + 60.0, "{} vs {straight}", length(line));
    }

    #[test]
    fn a_path_goes_round_a_lake_off_its_line_too() {
        // The lake is to one side of the middle, so the way round is a small bend.
        let mut world = dry_world();
        flood(&mut world, |p| p.distance(vec2(0.0, 10.0)) < 30.0);
        let lines = route(&world, vec2(-120.0, 0.0), vec2(120.0, 0.0));
        assert_eq!(lines.len(), 1);
        assert!(all_dry(&world, &lines[0]));
        assert!(length(&lines[0]) < 240.0 + 40.0);
    }

    #[test]
    fn a_path_with_no_way_round_stops_at_the_water_on_each_side() {
        // A river across the whole map.
        let mut world = dry_world();
        flood(&mut world, |p| p.x.abs() < 20.0);
        let (from, to) = (vec2(-100.0, 0.0), vec2(100.0, 0.0));
        let lines = route(&world, from, to);
        assert_eq!(lines.len(), 2, "one piece each side");
        assert!(lines.iter().all(|line| all_dry(&world, line)), "none over the water");
        assert_eq!(lines[0][0], from);
        assert_eq!(*lines[1].last().unwrap(), to);
        assert!(lines[0].last().unwrap().x > -22.0 && lines[1][0].x < 22.0, "they go right up to the banks");
        assert!(lines[0].last().unwrap().x < -14.0 && lines[1][0].x > 14.0, "and stop there");
    }

    #[test]
    fn a_path_in_the_water_is_not_drawn() {
        let mut world = dry_world();
        flood(&mut world, |_| true);
        assert!(route(&world, vec2(-50.0, 0.0), vec2(50.0, 0.0)).is_empty());
    }

    #[test]
    fn the_ribbon_follows_every_bend_of_a_line() {
        let world = dry_world();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        ribbon(&world, &[vec2(0.0, 0.0), vec2(40.0, 0.0), vec2(40.0, 40.0)], &mut vertices, &mut indices);
        // Three vertices across each point, and four triangles between each pair of points.
        let points = vertices.len() / 3;
        assert_eq!(vertices.len(), points * 3);
        assert_eq!(indices.len(), (points - 1) * 12);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        // Nothing is made of a line with one point.
        let (mut v2, mut i2) = (Vec::new(), Vec::new());
        ribbon(&world, &[vec2(5.0, 5.0)], &mut v2, &mut i2);
        assert!(v2.is_empty() && i2.is_empty());
    }
}
