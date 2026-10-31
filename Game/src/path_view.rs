//! Drawing the paths and the roads: each segment as a ribbon laid on the ground, soft at its edges. A path is packed
//! dirt, narrow and brown. A road is packed stone: wider, in gray, with a darker curb along each side and blocks
//! of slightly different shade one after another. One that has failed is drawn ruined: dark, and broken into pieces
//! with gaps between them. This is the game's own, and not part of drawing the map.
//!
//! A path or a road never crosses water. A segment is a straight line from the center of a region to the middle of one of
//! its edges, and where that line would cross a lake or a river, the path goes round instead, by the shortest
//! way over dry ground. If there is no way round, it is drawn as far as the water on each side and not over it.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
use nworld_map::world::{World, CELL_SIZE};

use crate::bridge::{Bridge, BridgeLink};
use crate::day::DaySave;
use crate::failure::Thing;
use crate::ground::{clear, dry, line_clear, CHECK_EVERY};
use crate::path::{Kind, PathPiece, PathSegment};

/// What a kind of way looks like.
#[derive(Clone, Copy)]
struct Look {
    /// How wide it is, in meters, counting its soft edges.
    width: f32,
    /// How far above the ground it lies, so that it isn't lost in it. A road lies a little above a path, so that it
    /// hides any path under it.
    lift: f32,
    /// The color of the middle of it, and how much its brightness varies from place to place.
    color: [f32; 3],
    variation: f32,
    /// How dark the curb along each side is (1 for none), and how far in from the edge it is as a fraction of half
    /// the width, if it has one.
    curb: Option<(f32, f32)>,
    /// How much brighter and darker the blocks one after another are, and how long each is, in meters.
    blocks: Option<(f32, f32)>,
    /// If it is broken into pieces, how long each piece is and how long the gap after it is, in meters.
    dashes: Option<(f32, f32)>,
}

impl Look {
    /// Packed dirt.
    const PATH: Look = Look { width: 7.0, lift: 0.35, color: [0.56, 0.42, 0.27], variation: 0.10, curb: None, blocks: None, dashes: None };
    /// Packed stone.
    const ROAD: Look = Look { width: 8.5, lift: 0.50, color: [0.60, 0.58, 0.54], variation: 0.06, curb: Some((0.70, 0.78)), blocks: Some((0.07, 2.0)), dashes: None };

    fn of(kind: Kind) -> &'static Look {
        match kind {
            Kind::Path => &Look::PATH,
            Kind::Road => &Look::ROAD,
        }
    }

    /// The same, ruined: darker, with the blocks gone, and broken into pieces.
    fn ruined(&self) -> Look {
        Look { color: [self.color[0] * 0.55, self.color[1] * 0.52, self.color[2] * 0.50], blocks: None, dashes: Some((5.0, 3.5)), ..*self }
    }
}
/// Most vertices in one mesh, which a mesh's 16 bit indices can't go past.
const MAX_VERTICES: usize = 60_000;
/// How far apart the points are that the way round is looked for among, in meters.
const GRID: f32 = 6.0;

/// The paths, or the roads, as meshes, made again when they change.
pub struct PathRenderer {
    kind: Kind,
    built_paths: Vec<PathSegment>,
    built_links: Vec<BridgeLink>,
    built_failed: Vec<Thing>,
    /// Where each piece goes, as lines of points over dry ground, so that the way round a lake is only
    /// worked out once.
    routes: HashMap<PathPiece, Vec<Vec<Vec2>>>,
    meshes: Vec<Mesh>,
}

impl PathRenderer {
    /// A renderer of the paths or of the roads of a day.
    pub fn new(kind: Kind) -> Self {
        Self { kind, built_paths: Vec::new(), built_links: Vec::new(), built_failed: Vec::new(), routes: HashMap::new(), meshes: Vec::new() }
    }

    /// Draws the paths or roads of `day`, as seen by the camera that is set, which must still be the one the map was
    /// drawn with.
    pub fn draw(&mut self, world: &World, map: &RegionMap, day: &DaySave) {
        let (segments, links) = (day.segments(self.kind), day.links(self.kind));
        if self.built_paths != segments || self.built_links != links || self.built_failed != day.failed {
            let pieces: Vec<PathPiece> = segments.iter().map(|s| PathPiece::Segment(*s)).chain(links.iter().map(|l| PathPiece::Link(*l))).collect();
            for piece in &pieces {
                if !self.routes.contains_key(piece) {
                    let route = piece_ends(map, &day.bridges, *piece).map(|(from, to)| route(world, from, to)).unwrap_or_default();
                    self.routes.insert(*piece, route);
                }
            }
            self.routes.retain(|piece, _| pieces.contains(piece));
            // The ones that are all right, and the ones that have failed, which are drawn ruined.
            let (ruined, sound): (Vec<PathPiece>, Vec<PathPiece>) = pieces.iter().partition(|piece| day.is_failed(Thing::of(self.kind, **piece)));
            let lines_of = |of: &[PathPiece]| -> Vec<&Vec<Vec2>> { of.iter().filter_map(|piece| self.routes.get(piece)).flatten().collect() };
            let look = Look::of(self.kind);
            let mut meshes = build(world, &lines_of(&sound), look);
            meshes.extend(build(world, &lines_of(&ruined), &look.ruined()));
            self.meshes = meshes;
            self.built_paths = segments.to_vec();
            self.built_links = links.to_vec();
            self.built_failed = day.failed.clone();
        }
        for mesh in &self.meshes {
            draw_mesh(mesh);
        }
    }
}

impl PathRenderer {
    /// The piece nearest `at` (world x and z) that is no more than `reach` meters from it, and how far it is: for
    /// pointing at one with the mouse. Only what has been drawn is known, which is everything there is by the time the
    /// mouse is used.
    pub fn piece_near(&self, at: Vec2, reach: f32) -> Option<(PathPiece, f32)> {
        self.routes
            .iter()
            .filter_map(|(piece, lines)| lines.iter().map(|line| distance_to_line(line, at)).min_by(f32::total_cmp).map(|d| (*piece, d)))
            .filter(|(_, distance)| *distance <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The lines of points that `piece` is drawn along, none if it isn't one of this renderer's.
    pub fn route_of(&self, piece: PathPiece) -> &[Vec<Vec2>] {
        self.routes.get(&piece).map_or(&[], Vec::as_slice)
    }
}

/// How far `at` is from the straight line from `a` to `b`, which ends at them.
pub fn distance_to_segment(a: Vec2, b: Vec2, at: Vec2) -> f32 {
    let along = b - a;
    let length_squared = along.length_squared();
    let t = if length_squared > 0.0 { ((at - a).dot(along) / length_squared).clamp(0.0, 1.0) } else { 0.0 };
    at.distance(a + along * t)
}

/// How far `at` is from the line through `points`, or infinitely far if there are none.
fn distance_to_line(points: &[Vec2], at: Vec2) -> f32 {
    match points {
        [] => f32::INFINITY,
        [only] => at.distance(*only),
        _ => points.windows(2).map(|pair| distance_to_segment(pair[0], pair[1], at)).fold(f32::INFINITY, f32::min),
    }
}

/// The ends of `segment` on the map, in world x and z: the center of its region (or, for a segment across the
/// region, the middle of its border with one neighbor) and the middle of its border with another.
pub fn ends(map: &RegionMap, segment: PathSegment) -> Option<(Vec2, Vec2)> {
    let (region, neighbor) = (RegionId::from_raw(segment.region)?, RegionId::from_raw(segment.neighbor)?);
    let to = map.edge_midpoint(region, neighbor)?;
    match segment.other {
        None => Some((map.center(region)?, to)),
        Some(other) => Some((to, map.edge_midpoint(region, RegionId::from_raw(other)?)?)),
    }
}

/// The ends of a path from the end of a bridge: the end itself, and the center of its region or the middle of one
/// of that region's borders.
pub fn link_ends(map: &RegionMap, bridges: &[Bridge], link: BridgeLink) -> Option<(Vec2, Vec2)> {
    let from = bridges.iter().find(|b| b.id == link.bridge)?.end(link.end);
    let region = RegionId::from_raw(link.region)?;
    let to = match link.to {
        None => map.center(region)?,
        Some(neighbor) => map.edge_midpoint(region, RegionId::from_raw(neighbor)?)?,
    };
    Some((from, to))
}

/// The ends of a piece of path of either kind.
pub fn piece_ends(map: &RegionMap, bridges: &[Bridge], piece: PathPiece) -> Option<(Vec2, Vec2)> {
    match piece {
        PathPiece::Segment(segment) => ends(map, segment),
        PathPiece::Link(link) => link_ends(map, bridges, link),
    }
}

/// Whether a path from `from` to `to` can be drawn all the way over dry ground, straight or going round the
/// water, and so doesn't have to cross a river or a lake.
pub fn connects(world: &World, from: Vec2, to: Vec2) -> bool {
    let lines = route(world, from, to);
    lines.len() == 1 && lines[0].first() == Some(&from) && lines[0].last() == Some(&to)
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

fn build(world: &World, lines: &[&Vec<Vec2>], look: &Look) -> Vec<Mesh> {
    let mut meshes = Vec::new();
    let (mut vertices, mut indices): (Vec<Vertex>, Vec<u16>) = (Vec::new(), Vec::new());
    for line in lines {
        if vertices.len() > MAX_VERTICES {
            meshes.push(Mesh { vertices: std::mem::take(&mut vertices), indices: std::mem::take(&mut indices), texture: None });
        }
        ribbon(world, line, look, &mut vertices, &mut indices);
    }
    if !vertices.is_empty() {
        meshes.push(Mesh { vertices, indices, texture: None });
    }
    meshes
}

/// `line` with points added so that none are farther apart than `step`, for the ribbon to follow the ground.
fn resample(line: &[Vec2], step: f32) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::new();
    for pair in line.windows(2) {
        let steps = (pair[0].distance(pair[1]) / step).ceil().max(1.0) as usize;
        for k in 0..steps {
            out.push(pair[0].lerp(pair[1], k as f32 / steps as f32));
        }
    }
    if let Some(last) = line.last() {
        out.push(*last);
    }
    out
}

/// Adds the ribbon along `line` to the mesh being made, looking as `look` says. Across it there are three vertices at
/// each point (the middle one solid and the outer two clear, so that it fades out at its edges), or five if it has
/// a curb: clear at the edge, the darker curb, the middle, the curb on the other side, and clear again.
fn ribbon(world: &World, line: &[Vec2], look: &Look, vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>) {
    // Blocks of stone and gaps are told apart by rows of points as often as they change, and dirt only needs one for
    // each cell.
    let step = look.blocks.map_or(CELL_SIZE, |(_, length)| length.min(CELL_SIZE)).min(if look.dashes.is_some() { 2.0 } else { CELL_SIZE });
    let points = resample(line, step);
    if points.len() < 2 {
        return;
    }
    // Across the ribbon, as a fraction of half its width, how bright and how solid it is there.
    let across_points: Vec<(f32, f32, u8)> = match look.curb {
        None => vec![(1.0, 1.0, 0), (0.0, 1.0, 255), (-1.0, 1.0, 0)],
        Some((dark, at)) => vec![(1.0, 1.0, 0), (at, dark, 255), (0.0, 1.0, 255), (-at, dark, 255), (-1.0, 1.0, 0)],
    };
    let width = across_points.len();
    let start = vertices.len();
    let mut length_so_far = 0.0;
    for (k, &at) in points.iter().enumerate() {
        if k > 0 {
            length_so_far += points[k - 1].distance(at);
        }
        // Which way the way runs here, from the points either side, so that a bend turns smoothly.
        let along = (points[(k + 1).min(points.len() - 1)] - points[k.saturating_sub(1)]).normalize_or_zero();
        let across = vec2(-along.y, along.x) * (look.width * 0.5);
        // A little different from place to place, like ground that has been walked on or stone that was cut
        // from different blocks, and for a road each block a little lighter or darker than the one before.
        let mut shade = 1.0 + look.variation * ((at.x * 0.37 + at.y * 0.23).sin() * 0.5 + (at.x * 0.11 - at.y * 0.17).cos() * 0.5);
        if let Some((contrast, length)) = look.blocks {
            let block = (length_so_far / length).floor() as i64;
            shade *= 1.0 + contrast * if block.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        }
        // In the gap between two pieces of a broken way, it is clear.
        let gap = look.dashes.is_some_and(|(piece, gap)| length_so_far % (piece + gap) >= piece);
        for &(fraction, dim, alpha) in &across_points {
            let alpha = if gap { 0 } else { alpha };
            let point = at + across * fraction;
            let c = |k: usize| (look.color[k] * shade * dim * 255.0).clamp(0.0, 255.0) as u8;
            vertices.push(Vertex {
                position: vec3(point.x, world.surface_height(point.x, point.y) + look.lift, point.y),
                uv: Vec2::ZERO,
                color: [c(0), c(1), c(2), alpha],
                normal: Vec4::ZERO,
            });
        }
    }
    for row in 0..points.len() - 1 {
        for column in 0..width - 1 {
            let a = (start + row * width + column) as u16;
            let (b, c, d) = (a + 1, a + width as u16, a + width as u16 + 1);
            indices.extend([a, b, d, a, d, c]);
        }
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
    fn a_path_connects_if_it_is_straight_or_goes_round_and_not_if_water_cuts_it() {
        let (from, to) = (vec2(-100.0, 0.0), vec2(100.0, 0.0));
        let mut world = dry_world();
        assert!(connects(&world, from, to), "dry ground");
        flood(&mut world, |p| p.distance(Vec2::ZERO) < 40.0);
        assert!(connects(&world, from, to), "a lake it goes round");
        let mut river = dry_world();
        flood(&mut river, |p| p.x.abs() < 20.0);
        assert!(!connects(&river, from, to), "a river across the whole map");
        // One that ends in the water doesn't connect either, even though the part before it is dry.
        assert!(!connects(&river, from, vec2(0.0, 0.0)));
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
        ribbon(&world, &[vec2(0.0, 0.0), vec2(40.0, 0.0), vec2(40.0, 40.0)], &Look::PATH, &mut vertices, &mut indices);
        // Three vertices across each point, and four triangles between each pair of points.
        let points = vertices.len() / 3;
        assert_eq!(vertices.len(), points * 3);
        assert_eq!(indices.len(), (points - 1) * 12);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        // Nothing is made of a line with one point.
        let (mut v2, mut i2) = (Vec::new(), Vec::new());
        ribbon(&world, &[vec2(5.0, 5.0)], &Look::PATH, &mut v2, &mut i2);
        assert!(v2.is_empty() && i2.is_empty());
    }

    #[test]
    fn a_road_is_wider_than_a_path_lies_above_it_and_is_gray_stone() {
        assert!(Look::ROAD.width > Look::PATH.width && Look::ROAD.lift > Look::PATH.lift, "a road hides a path under it");
        let world = dry_world();
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        ribbon(&world, &[vec2(-60.0, 0.0), vec2(60.0, 0.0)], &Look::ROAD, &mut vertices, &mut indices);
        // Five vertices across, and every one an index in range.
        assert_eq!(vertices.len() % 5, 0);
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()) && indices.len() % 3 == 0);
        let solid: Vec<&Vertex> = vertices.iter().filter(|v| v.color[3] == 255).collect();
        assert!(!solid.is_empty() && solid.iter().all(|v| {
            let [r, g, b, _] = v.color;
            r.abs_diff(g) < 20 && g.abs_diff(b) < 20 && r > 70
        }), "stone is gray, and not brown like dirt");
        // The path next to it is brown: more red than blue.
        let (mut dirt, mut i2) = (Vec::new(), Vec::new());
        ribbon(&world, &[vec2(-60.0, 0.0), vec2(60.0, 0.0)], &Look::PATH, &mut dirt, &mut i2);
        assert!(dirt.iter().filter(|v| v.color[3] == 255).all(|v| v.color[0] > v.color[2] + 40));
    }

    #[test]
    fn a_road_has_a_darker_curb_along_each_side_and_blocks_that_alternate() {
        let world = dry_world();
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        ribbon(&world, &[vec2(0.0, 0.0), vec2(40.0, 0.0)], &Look::ROAD, &mut vertices, &mut indices);
        // At each point across: clear edge, curb, middle, curb, clear edge. The curb is darker than the middle.
        for row in vertices.chunks(5) {
            assert!(row[1].color[0] < row[2].color[0] && row[3].color[0] < row[2].color[0], "{:?}", row.iter().map(|v| v.color).collect::<Vec<_>>());
            assert_eq!((row[0].color[3], row[1].color[3], row[2].color[3], row[3].color[3], row[4].color[3]), (0, 255, 255, 255, 0));
        }
        // Along the road the middle is lighter and darker in turn, a block at a time.
        let middles: Vec<u8> = vertices.chunks(5).map(|row| row[2].color[0]).collect();
        let distinct: std::collections::HashSet<u8> = middles.iter().copied().collect();
        assert!(distinct.len() >= 2 && middles.windows(2).any(|w| w[0] != w[1]));
    }

    #[test]
    fn the_distance_to_a_line_is_to_the_nearest_part_of_it() {
        let line = [vec2(0.0, 0.0), vec2(10.0, 0.0), vec2(10.0, 10.0)];
        assert_eq!(distance_to_line(&line, vec2(5.0, 3.0)), 3.0);
        assert_eq!(distance_to_line(&line, vec2(13.0, 5.0)), 3.0);
        assert_eq!(distance_to_line(&line, vec2(-4.0, 3.0)), 5.0, "past the end it is the distance to the end");
        assert_eq!(distance_to_line(&[vec2(1.0, 1.0)], vec2(4.0, 5.0)), 5.0);
        assert_eq!(distance_to_line(&[], Vec2::ZERO), f32::INFINITY);
        assert_eq!(distance_to_segment(vec2(2.0, 2.0), vec2(2.0, 2.0), vec2(5.0, 6.0)), 5.0, "a segment of no length");
    }

    #[test]
    fn a_piece_is_found_by_how_near_it_is_and_the_nearest_wins() {
        let mut paths = PathRenderer::new(Kind::Path);
        let (near, far) = (PathPiece::Segment(PathSegment::to_edge(1, 2)), PathPiece::Segment(PathSegment::to_edge(3, 4)));
        paths.routes.insert(near, vec![vec![vec2(0.0, 0.0), vec2(20.0, 0.0)]]);
        paths.routes.insert(far, vec![vec![vec2(0.0, 6.0), vec2(20.0, 6.0)]]);
        assert_eq!(paths.piece_near(vec2(10.0, 2.0), 5.0), Some((near, 2.0)));
        assert_eq!(paths.piece_near(vec2(10.0, 4.5), 5.0), Some((far, 1.5)));
        assert_eq!(paths.piece_near(vec2(10.0, 20.0), 5.0), None, "out of reach");
        assert_eq!(paths.route_of(near).len(), 1);
        assert!(paths.route_of(PathPiece::Segment(PathSegment::to_edge(9, 8))).is_empty());
    }

    #[test]
    fn each_kind_is_drawn_by_its_own_renderer() {
        let paths = PathRenderer::new(Kind::Path);
        let roads = PathRenderer::new(Kind::Road);
        assert_eq!((paths.kind, roads.kind), (Kind::Path, Kind::Road));
    }
}
