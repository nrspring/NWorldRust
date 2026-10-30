//! The path tool: a button in the footer that turns it on, and then the player clicks one end of a segment of
//! path, and then the other. An end is either the center of a region or the middle point of one of its edges.
//! A segment joins a center to the middle of one of its region's edges, or the middle of one edge of a region to
//! the middle of another of the same region, which doesn't go through the center. A path can also start at either
//! end of a bridge: an end of a bridge is a third kind of point, and it joins to the center of the region it is in or
//! the middle of one of that region's edges.
//!
//! With the tool on, the points that can be clicked are shown on the map as markers: round ones for centers
//! and square ones for edges. At first those are the points of the region under the mouse. Once one end is
//! chosen, the markers left are the ones that can be the other end.

use std::collections::HashMap;

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::{RegionId, RegionKind};
use nworld_map::world::World;
use nworld_view::{mouse_ray, pick_terrain};
use nworld_view::style::{self, Style, Weight};

use crate::bridge::{Bridge, BridgeLink, End};
use crate::day::DaySave;
use crate::hud;
use crate::ids::BridgeId;
use crate::path::{PathPiece, PathProblem, PathSegment, PATH_COST};
use crate::path_view;

/// Why a segment that crosses water is refused.
const CROSSES_WATER: &str = "A path can't cross a river or a lake.";
/// How close to a marker, as a fraction of how far away it is, the mouse has to be to point at it.
const POINT_TOLERANCE: f32 = 0.02;
/// How big a marker is, as a fraction of how far away it is, and the least it is, in meters.
const MARKER_SIZE: f32 = 0.007;
const MARKER_LEAST: f32 = 0.6;
/// How high above the ground the markers and the line between the ends are held.
const HOLD: f32 = 1.2;
const BUTTON_HEIGHT: f32 = 36.0;

/// A place a segment can start or end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    /// The center of the region with this number.
    Center(u16),
    /// The middle of the border between two regions, given with the lower number first.
    Edge(u16, u16),
    /// One end of a bridge.
    BridgeEnd(BridgeId, End),
}

impl Point {
    fn edge(a: RegionId, b: RegionId) -> Self {
        Self::Edge(a.raw().min(b.raw()), a.raw().max(b.raw()))
    }

    /// Where it is on the map, in world x and z.
    fn position(self, map: &RegionMap, bridges: &[Bridge]) -> Option<Vec2> {
        match self {
            Self::Center(region) => map.center(RegionId::from_raw(region)?),
            Self::Edge(a, b) => map.edge_midpoint(RegionId::from_raw(a)?, RegionId::from_raw(b)?),
            Self::BridgeEnd(id, end) => bridges.iter().find(|b| b.id == id).map(|b| b.end(end)),
        }
    }
}

/// The segment two ends make, if they belong together: a center and an edge of its region, or two different edges
/// of the same region (which share that region).
fn segment(a: Point, b: Point) -> Option<PathSegment> {
    match (a, b) {
        (Point::Center(region), Point::Edge(x, y)) | (Point::Edge(x, y), Point::Center(region)) if region == x => Some(PathSegment::to_edge(region, y)),
        (Point::Center(region), Point::Edge(x, y)) | (Point::Edge(x, y), Point::Center(region)) if region == y => Some(PathSegment::to_edge(region, x)),
        (Point::Edge(a1, a2), Point::Edge(b1, b2)) if (a1, a2) != (b1, b2) => {
            let shared = [a1, a2].into_iter().find(|region| *region == b1 || *region == b2)?;
            let (from, to) = (if a1 == shared { a2 } else { a1 }, if b1 == shared { b2 } else { b1 });
            (from != to).then(|| PathSegment::across(shared, from, to))
        }
        _ => None,
    }
}

/// The path from an end of a bridge that two points make, if they belong together: the end of the bridge, and the
/// center of the region the end is in or the middle of one of that region's edges.
fn link_between(world: &World, bridges: &[Bridge], a: Point, b: Point) -> Option<BridgeLink> {
    let ((bridge, end), other) = match (a, b) {
        (Point::BridgeEnd(id, end), other) | (other, Point::BridgeEnd(id, end)) if !matches!(other, Point::BridgeEnd(..)) => ((id, end), other),
        _ => return None,
    };
    let at = bridges.iter().find(|x| x.id == bridge)?.end(end);
    let region = world.region_at(at.x, at.y)?.raw();
    match other {
        Point::Center(r) if r == region => Some(BridgeLink { bridge, end, region, to: None }),
        Point::Edge(x, y) if x == region => Some(BridgeLink { bridge, end, region, to: Some(y) }),
        Point::Edge(x, y) if y == region => Some(BridgeLink { bridge, end, region, to: Some(x) }),
        _ => None,
    }
}

/// The piece of path two points make, of either kind, if they belong together.
fn piece_between(world: &World, bridges: &[Bridge], a: Point, b: Point) -> Option<PathPiece> {
    segment(a, b).map(PathPiece::Segment).or_else(|| link_between(world, bridges, a, b).map(PathPiece::Link))
}

/// The bridge ends that are in the region numbered `region`.
fn bridge_ends_in(world: &World, bridges: &[Bridge], region: u16) -> Vec<Point> {
    let mut ends = Vec::new();
    for bridge in bridges {
        for end in [End::First, End::Second] {
            let at = bridge.end(end);
            if world.region_at(at.x, at.y).is_some_and(|r| r.raw() == region) {
                ends.push(Point::BridgeEnd(bridge.id, end));
            }
        }
    }
    ends
}

/// Whether a path can go through the region: it is dry land, which includes the mountains.
fn walkable(region: RegionId) -> bool {
    matches!(region.kind(), RegionKind::Land | RegionKind::Mountain)
}

/// Whether `piece` would have to cross water: a river or a lake with no way round it, or an end in the water.
/// A piece that goes round a lake doesn't.
fn crosses_water(world: &World, map: &RegionMap, bridges: &[Bridge], piece: PathPiece) -> bool {
    path_view::piece_ends(map, bridges, piece).is_none_or(|(from, to)| !path_view::connects(world, from, to))
}

/// What the tool needs to know about the map and the mouse each frame.
pub struct Scene<'a> {
    pub world: &'a World,
    pub regions: &'a RegionMap,
    pub eye: &'a Camera3D,
    pub mouse: Vec2,
    /// Whether the mouse is over the map, and not the interface.
    pub over_map: bool,
}

#[derive(Default)]
pub struct PathTool {
    active: bool,
    /// The end of the segment that has been chosen, if one has.
    first: Option<Point>,
    /// Why the last segment couldn't be built.
    notice: Option<String>,
    /// Where the prompt was drawn last, so the mouse can be told it is over it.
    prompt: Rect,
    /// Whether each piece looked at would cross water, so that it is only worked out once.
    crossing: HashMap<PathPiece, bool>,
    /// Whether the segment the mouse is on would cross water, for the prompt to say so.
    hover_blocked: bool,
}

impl PathTool {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        *self = Self::default();
    }

    /// What Esc does: lets go of the end that was chosen, and then turns the tool off.
    pub fn escape(&mut self) {
        if self.first.is_some() || self.notice.is_some() {
            self.first = None;
            self.notice = None;
        } else {
            self.deactivate();
        }
    }

    /// Shows why the segment that `update` gave couldn't be built.
    pub fn fail(&mut self, message: String) {
        self.first = None;
        self.notice = Some(message);
    }

    /// Carries on after a segment was built, ready for the next.
    pub fn built(&mut self) {
        self.first = None;
        self.notice = None;
    }

    /// Whether the mouse is over the prompt, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.active && self.prompt.contains(mouse)
    }

    /// The points that can be clicked now: with none chosen, those of the region at `hovered`; with a center
    /// chosen, the edges of its region; with an edge chosen, the centers of the two regions it divides and their
    /// other edges. The ends of bridges that are in those regions are offered too, and with an end of a bridge
    /// chosen, the center and the edges of the region it is in.
    fn candidates(&self, world: &World, map: &RegionMap, bridges: &[Bridge], hovered: Option<RegionId>) -> Vec<Point> {
        let edges_of = |region: RegionId| map.neighbors(region).iter().filter(|n| walkable(n.region)).map(move |n| Point::edge(region, n.region)).collect::<Vec<_>>();
        match self.first {
            None => hovered.filter(|r| walkable(*r)).map_or(Vec::new(), |region| {
                let mut points = vec![Point::Center(region.raw())];
                points.extend(edges_of(region));
                points.extend(bridge_ends_in(world, bridges, region.raw()));
                points
            }),
            Some(Point::Center(region)) => {
                let mut points = RegionId::from_raw(region).map_or(Vec::new(), edges_of);
                points.extend(bridge_ends_in(world, bridges, region));
                points
            }
            Some(Point::Edge(a, b)) => {
                let mut points = vec![Point::Center(a), Point::Center(b)];
                for region in [a, b].into_iter().filter_map(RegionId::from_raw) {
                    points.extend(edges_of(region).into_iter().filter(|p| *p != Point::Edge(a, b)));
                    points.extend(bridge_ends_in(world, bridges, region.raw()));
                }
                points
            }
            Some(Point::BridgeEnd(id, end)) => {
                let region = bridges.iter().find(|b| b.id == id).map(|b| b.end(end)).and_then(|at| world.region_at(at.x, at.y)).filter(|r| walkable(*r));
                region.map_or(Vec::new(), |region| {
                    let mut points = vec![Point::Center(region.raw())];
                    points.extend(edges_of(region));
                    points
                })
            }
        }
    }

    /// Draws the markers and the line to the end the mouse is on, in the map's camera, and works out what the
    /// player is doing. `clicked` is whether the left button was just clicked on the map. Returns the segment
    /// the player has just finished choosing, for the caller to build.
    pub fn update(&mut self, scene: &Scene, day: &DaySave, clicked: bool) -> Option<PathPiece> {
        if !self.active {
            return None;
        }
        let hovered_region = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse).and_then(|at| scene.world.region_at(at.x, at.y)) } else { None };
        let points: Vec<(Point, Vec3)> = self
            .candidates(scene.world, scene.regions, &day.bridges, hovered_region)
            .into_iter()
            .filter_map(|p| p.position(scene.regions, &day.bridges).map(|at| (p, vec3(at.x, scene.world.surface_height(at.x, at.y) + HOLD, at.y))))
            .collect();

        // The marker the mouse is on: the one the view ray passes nearest, if it passes close enough.
        let (origin, direction) = mouse_ray(scene.eye, scene.mouse);
        let under_mouse = if scene.over_map {
            points
                .iter()
                .filter_map(|(point, at)| {
                    let ahead = (*at - origin).dot(direction);
                    (ahead > 0.0).then(|| ((*at - origin).cross(direction).length() / ahead, *point))
                })
                .filter(|(closeness, _)| *closeness < POINT_TOLERANCE)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, point)| point)
        } else {
            None
        };

        // Whether the segment the mouse is on is one that can't be built.
        let hover_piece = self.first.zip(under_mouse).and_then(|(first, point)| piece_between(scene.world, &day.bridges, first, point));
        let wet = hover_piece.is_some_and(|piece| *self.crossing.entry(piece).or_insert_with(|| crosses_water(scene.world, scene.regions, &day.bridges, piece)));
        self.hover_blocked = wet;

        // The markers, and a line from the chosen end to the one the mouse is on.
        let first_at = self.first.and_then(|f| points.iter().find(|(p, _)| *p == f).map(|(_, at)| *at).or_else(|| f.position(scene.regions, &day.bridges).map(|a| vec3(a.x, scene.world.surface_height(a.x, a.y) + HOLD, a.y))));
        if let (Some(from), Some(point)) = (first_at, under_mouse) {
            if let Some((_, to)) = points.iter().find(|(p, _)| *p == point) {
                let exists = hover_piece.is_some_and(|piece| match piece {
                    PathPiece::Segment(s) => day.paths.contains(&s),
                    PathPiece::Link(l) => day.bridge_links.contains(&l),
                });
                draw_line_3d(from, *to, if exists || wet { RED } else { Color::new(1.0, 0.85, 0.35, 1.0) });
            }
        }
        for (point, at) in &points {
            let size = ((*at - origin).length() * MARKER_SIZE).max(MARKER_LEAST) * if Some(*point) == under_mouse { 1.5 } else { 1.0 };
            let color = if Some(*point) == under_mouse { Color::new(1.0, 0.85, 0.2, 1.0) } else { Color::new(0.96, 0.96, 0.96, 1.0) };
            match point {
                Point::Center(_) => draw_sphere(*at, size, None, color),
                Point::Edge(..) => draw_cube(*at, vec3(size * 1.7, size * 1.7, size * 1.7), None, color),
                // The end of a bridge is a stone gray block, bigger than the rest.
                Point::BridgeEnd(..) => draw_cube(*at, vec3(size * 2.4, size * 1.2, size * 2.4), None, if Some(*point) == under_mouse { color } else { Color::new(0.62, 0.64, 0.70, 1.0) }),
            }
        }
        if let Some(at) = first_at {
            let size = ((at - origin).length() * MARKER_SIZE).max(MARKER_LEAST) * 1.7;
            match self.first {
                Some(Point::Center(_)) => draw_sphere(at, size, None, Color::new(0.30, 0.55, 1.0, 1.0)),
                _ => draw_cube(at, vec3(size * 1.7, size * 1.7, size * 1.7), None, Color::new(0.30, 0.55, 1.0, 1.0)),
            }
        }

        if !clicked {
            return None;
        }
        self.notice = None;
        match (self.first, under_mouse) {
            (None, Some(point)) => {
                self.first = Some(point);
                None
            }
            (Some(first), Some(point)) => {
                self.first = None;
                let chosen = piece_between(scene.world, &day.bridges, first, point)?;
                if wet {
                    self.notice = Some(CROSSES_WATER.to_owned());
                    return None;
                }
                Some(chosen)
            }
            // A click on nothing lets go of the end that was chosen.
            (Some(_), None) => {
                self.first = None;
                None
            }
            (None, None) => None,
        }
    }

    /// Draws the button that turns the tool on and off, in the `footer`, with its right edge at `right`. When
    /// `problem` is something the tool can't be used, it is dimmed, the reason is beside it, and the whole reason
    /// shows when the mouse is over it. Returns where the button is.
    pub fn show_button(&mut self, style: &Style, footer: Rect, right: f32, problem: Option<&PathProblem>, no_regions: bool) -> Rect {
        let mouse = Vec2::from(mouse_position());
        let label = "Paths";
        let label_width = style.measure(label, 13, Weight::Semibold).width;
        let width = 12.0 + 14.0 + 8.0 + label_width + 14.0;
        let button = Rect::new(right - width, footer.center().y - BUTTON_HEIGHT * 0.5, width, BUTTON_HEIGHT);
        let disabled = problem.is_some() || no_regions;
        let hovered = button.contains(mouse);
        if hovered && is_mouse_button_pressed(MouseButton::Left) && !disabled {
            if self.active {
                self.deactivate();
            } else {
                *self = Self { active: true, ..Self::default() };
            }
        }
        if self.active {
            style::rounded_rect(button, 10.0, style::ACCENT_SOFT);
        } else if hovered && !disabled {
            style::rounded_rect(button, 10.0, style::HOVER);
        }
        let color = match (disabled, self.active) {
            (true, _) => Color::new(1.0, 1.0, 1.0, 0.30),
            (false, true) => style::ACCENT_HOVER,
            (false, false) => style::TEXT,
        };
        // A little path: two dots joined by a line that bends.
        let icon = vec2(button.x + 12.0 + 7.0, button.center().y);
        draw_line(icon.x - 6.0, icon.y + 4.0, icon.x, icon.y - 1.0, 1.8, color);
        draw_line(icon.x, icon.y - 1.0, icon.x + 6.0, icon.y - 4.0, 1.8, color);
        draw_circle(icon.x - 6.0, icon.y + 4.0, 2.2, color);
        draw_circle(icon.x + 6.0, icon.y - 4.0, 2.2, color);
        style.text_mid(label, button.x + 12.0 + 14.0 + 8.0, button.center().y, 13, Weight::Semibold, color);

        // Why it can't be used, in full over it. (The short reason goes beside the leftmost of the footer's tool
        // buttons: see `BridgeTool::show_button`.)
        let reason = if no_regions { Some(("This map has no regions".to_owned(), "Paths join the regions of the map, and this map has none.".to_owned())) } else { problem.map(|p| (p.short(), p.long())) };
        if let Some((_, long)) = reason {
            if hovered {
                let lines = hud::wrap(style, &long, 12, 260.0);
                let height = lines.len() as f32 * 17.0 + 20.0;
                let tip = Rect::new((button.right() - 300.0).max(8.0), button.y - 8.0 - height, 300.0, height);
                style::card(tip, 10.0, style::GLASS);
                for (k, line) in lines.iter().enumerate() {
                    style.text_mid(line, tip.x + 14.0, tip.y + 10.0 + 17.0 * (k as f32 + 0.5), 12, Weight::Regular, style::TEXT);
                }
            }
        }
        button
    }

    /// Draws the prompt over the footer while the tool is on: what to do next, what a segment costs, and what
    /// the country being played has.
    pub fn show_prompt(&mut self, style: &Style, day: &DaySave) {
        if !self.active {
            return;
        }
        let width = 520.0;
        let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - 88.0, width, 88.0);
        self.prompt = panel;
        style::card(panel, 12.0, style::GLASS);
        style.text_mid("Build a path", panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
        match (&self.notice, self.first) {
            (Some(notice), _) => style.text_mid(notice, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, Some(_)) if self.hover_blocked => style.text_mid(CROSSES_WATER, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, None) => style.text_mid("Click the center of a region, or the middle of one of its edges.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, Some(Point::Center(_))) => style.text_mid("Now click the middle of one of its edges. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, Some(Point::Edge(..))) => style.text_mid("Now click the center of one of the two regions, or another of their edges. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, Some(Point::BridgeEnd(..))) => style.text_mid("Now click the center of its region, or the middle of one of its edges. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
        };
        let paying = day.player_country().map_or_else(|| "No country".to_owned(), |c| format!("{} has {}", c.name, crate::format::money(c.money)));
        style.text_mid(&format!("{PATH_COST} coins a segment. {paying} coins."), panel.x + 20.0, panel.y + 70.0, 12, Weight::Regular, style::TEXT_MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(number: u16, kind: RegionKind) -> RegionId {
        RegionId::from_raw((kind as u16) << 14 | number).unwrap()
    }

    #[test]
    fn a_center_and_an_edge_of_its_region_make_a_segment_whichever_is_first() {
        let (a, b) = (region(1, RegionKind::Land), region(2, RegionKind::Land));
        let edge = Point::edge(a, b);
        assert_eq!(segment(Point::Center(a.raw()), edge), Some(PathSegment::to_edge(a.raw(), b.raw())));
        assert_eq!(segment(edge, Point::Center(a.raw())), Some(PathSegment::to_edge(a.raw(), b.raw())));
        // From the other side of the same edge it is the other segment.
        assert_eq!(segment(edge, Point::Center(b.raw())), Some(PathSegment::to_edge(b.raw(), a.raw())));
        assert_eq!(segment(Point::Center(b.raw()), edge), Some(PathSegment::to_edge(b.raw(), a.raw())));
    }

    #[test]
    fn two_ends_of_the_same_kind_or_of_different_regions_make_no_segment() {
        let (a, b, c) = (region(1, RegionKind::Land), region(2, RegionKind::Land), region(3, RegionKind::Land));
        assert_eq!(segment(Point::Center(a.raw()), Point::Center(b.raw())), None);
        assert_eq!(segment(Point::edge(a, b), Point::edge(a, b)), None, "the same edge");
        assert_eq!(segment(Point::Center(c.raw()), Point::edge(a, b)), None, "an edge of other regions");
    }

    #[test]
    fn an_edge_is_the_same_point_whichever_region_it_is_named_from() {
        let (a, b) = (region(1, RegionKind::Land), region(2, RegionKind::Mountain));
        assert_eq!(Point::edge(a, b), Point::edge(b, a));
    }

    #[test]
    fn two_edges_of_the_same_region_make_a_segment_across_it() {
        let (a, b, c, d) = (region(1, RegionKind::Land), region(2, RegionKind::Land), region(3, RegionKind::Land), region(4, RegionKind::Land));
        // The edges a|b and b|c are both b's, so the segment crosses b, whichever is chosen first.
        let across = Some(PathSegment::across(b.raw(), a.raw(), c.raw()));
        assert_eq!(segment(Point::edge(a, b), Point::edge(b, c)), across);
        assert_eq!(segment(Point::edge(b, c), Point::edge(a, b)), across);
        // The edges a|b and a|c are both a's.
        assert_eq!(segment(Point::edge(a, b), Point::edge(a, c)), Some(PathSegment::across(a.raw(), b.raw(), c.raw())));
        // Edges that share no region don't make one.
        assert_eq!(segment(Point::edge(a, b), Point::edge(c, d)), None);
    }

    #[test]
    fn after_choosing_an_edge_the_other_edges_of_its_two_regions_can_be_chosen_too() {
        // Three regions in a row, A | B | C, and the edge chosen is A|B: the other edge of A has nowhere to go
        // (A is at the end), and the other edge of B is B|C.
        let (world, map) = three_regions(|_| false);
        let tool = PathTool { active: true, first: Some(Point::Edge(0xC001, 0xC002)), ..PathTool::default() };
        let points = tool.candidates(&world, &map, &[], None);
        assert_eq!(points, [Point::Center(0xC001), Point::Center(0xC002), Point::Edge(0xC002, 0xC003)]);
    }

    /// Three regions side by side over dry ground, A | B | C, with `flood` turned to water first.
    fn three_regions(flood: impl Fn(Vec2) -> bool) -> (World, RegionMap) {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        let mut regions = vec![0u16; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                let at = vec2(i as f32 * 4.0 - half.x, j as f32 * 4.0 - half.y);
                regions[j * nx + i] = if i < nx / 3 { 0xC001 } else if i < 2 * nx / 3 { 0xC002 } else { 0xC003 };
                if flood(at) {
                    world.water_mut()[j * nx + i] = 10.0;
                }
            }
        }
        world.set_regions(regions);
        let map = RegionMap::build(&world);
        (world, map)
    }

    #[test]
    fn a_segment_across_a_region_runs_between_the_middles_of_two_borders_not_through_the_center() {
        let (world, map) = three_regions(|_| false);
        let across = PathSegment::across(0xC002, 0xC001, 0xC003);
        let (from, to) = path_view::ends(&map, across).unwrap();
        let (a_b, b_c) = (map.edge_midpoint(region_id(0xC001), region_id(0xC002)).unwrap(), map.edge_midpoint(region_id(0xC002), region_id(0xC003)).unwrap());
        assert_eq!((from, to), (a_b, b_c));
        assert!(from.distance(to) > 100.0, "{}", from.distance(to));
        assert!(!crosses_water(&world, &map, &[], PathPiece::Segment(across)));
        // A river between the two edges, inside B, stops it like any other.
        let (river_world, river_map) = three_regions(|p| p.x.abs() < 10.0 && p.y.abs() < 250.0);
        assert!(crosses_water(&river_world, &river_map, &[], PathPiece::Segment(across)));
    }

    fn region_id(raw: u16) -> RegionId {
        RegionId::from_raw(raw).unwrap()
    }

    /// A bridge with its first end in the middle region B of `three_regions` and its second end in the right-hand one, C.
    fn bridge_in_b_and_c() -> Bridge {
        Bridge { id: BridgeId::new(), first: vec2(0.0, 0.0).into(), second: vec2(120.0, 0.0).into() }
    }

    #[test]
    fn a_path_can_go_from_an_end_of_a_bridge_to_the_center_or_an_edge_of_the_region_it_is_in() {
        let (world, _map) = three_regions(|_| false);
        let bridge = bridge_in_b_and_c();
        let bridges = [bridge];
        let first = Point::BridgeEnd(bridge.id, End::First);
        let second = Point::BridgeEnd(bridge.id, End::Second);
        // The first end is in B: its center and both its edges, whichever is chosen first.
        let link = |to| Some(BridgeLink { bridge: bridge.id, end: End::First, region: 0xC002, to });
        assert_eq!(link_between(&world, &bridges, first, Point::Center(0xC002)), link(None));
        assert_eq!(link_between(&world, &bridges, Point::Center(0xC002), first), link(None));
        assert_eq!(link_between(&world, &bridges, first, Point::Edge(0xC001, 0xC002)), link(Some(0xC001)));
        assert_eq!(link_between(&world, &bridges, Point::Edge(0xC002, 0xC003), first), link(Some(0xC003)));
        // But not to another region's center, or an edge it isn't on, or another end.
        assert_eq!(link_between(&world, &bridges, first, Point::Center(0xC001)), None);
        assert_eq!(link_between(&world, &bridges, first, Point::Edge(0xC001, 0xC003)), None);
        assert_eq!(link_between(&world, &bridges, first, second), None);
        // The second end is in C, so it joins to C's center.
        assert_eq!(
            link_between(&world, &bridges, second, Point::Center(0xC003)),
            Some(BridgeLink { bridge: bridge.id, end: End::Second, region: 0xC003, to: None })
        );
        // A piece of path is whichever kind the two points make.
        assert!(matches!(piece_between(&world, &bridges, first, Point::Center(0xC002)), Some(PathPiece::Link(_))));
        assert!(matches!(piece_between(&world, &bridges, Point::Center(0xC002), Point::Edge(0xC002, 0xC003)), Some(PathPiece::Segment(_))));
    }

    #[test]
    fn the_ends_of_bridges_are_offered_as_points_in_their_regions_and_lead_to_the_region_they_are_in() {
        let (world, map) = three_regions(|_| false);
        let bridge = bridge_in_b_and_c();
        let first = Point::BridgeEnd(bridge.id, End::First);
        let second = Point::BridgeEnd(bridge.id, End::Second);
        let tool = PathTool { active: true, ..PathTool::default() };
        // Over B, the first end is there along with B's center and edges, and the second is not.
        let over_b = tool.candidates(&world, &map, &[bridge], Some(region_id(0xC002)));
        assert!(over_b.contains(&first) && !over_b.contains(&second) && over_b.contains(&Point::Center(0xC002)), "{over_b:?}");
        // With B's center chosen, its edges and the end are what is left.
        let after_center = PathTool { active: true, first: Some(Point::Center(0xC002)), ..PathTool::default() };
        let points = after_center.candidates(&world, &map, &[bridge], None);
        assert_eq!(points, [Point::Edge(0xC001, 0xC002), Point::Edge(0xC002, 0xC003), first]);
        // With the end chosen, the center and edges of the region it is in.
        let from_end = PathTool { active: true, first: Some(first), ..PathTool::default() };
        assert_eq!(from_end.candidates(&world, &map, &[bridge], None), [Point::Center(0xC002), Point::Edge(0xC001, 0xC002), Point::Edge(0xC002, 0xC003)]);
        // And where they are, the end is at the bridge's end.
        assert_eq!(first.position(&map, &[bridge]), Some(vec2(0.0, 0.0)));
        assert_eq!(second.position(&map, &[bridge]), Some(vec2(120.0, 0.0)));
    }

    #[test]
    fn a_path_from_a_bridge_cannot_cross_water_either() {
        let bridge = bridge_in_b_and_c();
        // A river inside B, between the first end of the bridge and the border with A.
        let (dry_world, dry_map) = three_regions(|_| false);
        let link = BridgeLink { bridge: bridge.id, end: End::First, region: 0xC002, to: Some(0xC001) };
        assert!(!crosses_water(&dry_world, &dry_map, &[bridge], PathPiece::Link(link)));
        let (river_world, river_map) = three_regions(|p| (-60.0..-45.0).contains(&p.x));
        assert!(crosses_water(&river_world, &river_map, &[bridge], PathPiece::Link(link)), "a river between the end and the edge");
        // A link to a bridge that isn't there can't be checked, so it counts as crossing.
        assert!(crosses_water(&dry_world, &dry_map, &[], PathPiece::Link(link)));
    }

    #[test]
    fn only_dry_land_can_have_a_path() {
        assert!(walkable(region(1, RegionKind::Land)));
        assert!(walkable(region(2, RegionKind::Mountain)));
        assert!(!walkable(region(3, RegionKind::Shallow)));
        assert!(!walkable(region(4, RegionKind::Deep)));
    }

    /// Two regions side by side, A on the left (x below -2) and B on the right, over dry ground, with `flood`
    /// turned to water first.
    fn two_regions(flood: impl Fn(Vec2) -> bool) -> (World, RegionMap) {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        let mut regions = vec![0u16; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                let at = vec2(i as f32 * 4.0 - half.x, j as f32 * 4.0 - half.y);
                regions[j * nx + i] = if i < nx / 2 { 0xC001 } else { 0xC002 };
                if flood(at) {
                    world.water_mut()[j * nx + i] = 10.0;
                }
            }
        }
        world.set_regions(regions);
        let map = RegionMap::build(&world);
        (world, map)
    }

    #[test]
    fn a_segment_that_crosses_a_river_is_refused_and_one_that_does_not_is_allowed() {
        let a_to_edge = PathSegment::to_edge(0xC001, 0xC002);
        let (dry, map) = two_regions(|_| false);
        assert!(!crosses_water(&dry, &map, &[], PathPiece::Segment(a_to_edge)));
        // A river across all of A, between its center and the edge it goes to.
        let (river, map) = two_regions(|p| (-90.0..-70.0).contains(&p.x));
        assert!(crosses_water(&river, &map, &[], PathPiece::Segment(a_to_edge)));
        // The other side of the border has no river, so its segment is fine.
        assert!(!crosses_water(&river, &map, &[], PathPiece::Segment(PathSegment::to_edge(0xC002, 0xC001))));
        // A lake that can be got round doesn't count.
        let (lake, map) = two_regions(|p| p.distance(vec2(-65.0, 0.0)) < 30.0);
        assert!(!crosses_water(&lake, &map, &[], PathPiece::Segment(a_to_edge)));
    }

    #[test]
    fn a_segment_that_is_not_a_real_one_counts_as_crossing() {
        let (world, map) = two_regions(|_| false);
        assert!(crosses_water(&world, &map, &[], PathPiece::Segment(PathSegment::to_edge(0xC001, 0xC009))), "no such neighbor");
    }

    #[test]
    fn the_tool_starts_off_and_escape_steps_back_before_turning_it_off() {
        let mut tool = PathTool { active: true, first: Some(Point::Center(0xC001)), ..PathTool::default() };
        assert!(!PathTool::default().is_active());
        tool.escape();
        assert!(tool.is_active() && tool.first.is_none(), "the first Esc lets go of the end");
        tool.fail("No.".to_owned());
        tool.escape();
        assert!(tool.is_active() && tool.notice.is_none(), "then it clears the message");
        tool.escape();
        assert!(!tool.is_active());
    }

    #[test]
    fn the_points_to_choose_from_follow_what_has_been_chosen() {
        let map = RegionMap::default();
        let world = World::new(512.0, 512.0);
        let tool = PathTool { active: true, first: Some(Point::Edge(0xC001, 0xC002)), ..PathTool::default() };
        // After an edge, the two centers; the map is not needed for that.
        assert_eq!(tool.candidates(&world, &map, &[], None), [Point::Center(0xC001), Point::Center(0xC002)]);
        // Before anything, with nothing under the mouse there is nothing to choose from.
        assert!(PathTool { active: true, ..PathTool::default() }.candidates(&world, &map, &[], None).is_empty());
    }
}
