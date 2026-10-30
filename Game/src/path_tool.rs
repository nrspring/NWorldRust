//! The path tool: a button in the footer that turns it on, and then the player clicks one end of a segment of
//! path, and then the other. An end is either the center of a region or the middle point of one of its edges,
//! and a segment joins a center to the middle of an edge of the same region.
//!
//! With the tool on, the points that can be clicked are shown on the map as markers: round ones for centers
//! and square ones for edges. At first those are the points of the region under the mouse. Once one end is
//! chosen, the markers left are the ones that can be the other end.

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::{RegionId, RegionKind};
use nworld_map::world::World;
use nworld_view::{mouse_ray, pick_terrain};
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::hud;
use crate::path::{PathProblem, PathSegment, PATH_COST};

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
}

impl Point {
    fn edge(a: RegionId, b: RegionId) -> Self {
        Self::Edge(a.raw().min(b.raw()), a.raw().max(b.raw()))
    }

    /// Where it is on the map, in world x and z.
    fn position(self, map: &RegionMap) -> Option<Vec2> {
        match self {
            Self::Center(region) => map.center(RegionId::from_raw(region)?),
            Self::Edge(a, b) => map.edge_midpoint(RegionId::from_raw(a)?, RegionId::from_raw(b)?),
        }
    }
}

/// The segment a center and an edge make, if they belong together: the edge is one of the center's region's.
fn segment(a: Point, b: Point) -> Option<PathSegment> {
    match (a, b) {
        (Point::Center(region), Point::Edge(x, y)) | (Point::Edge(x, y), Point::Center(region)) if region == x => Some(PathSegment { region, neighbor: y }),
        (Point::Center(region), Point::Edge(x, y)) | (Point::Edge(x, y), Point::Center(region)) if region == y => Some(PathSegment { region, neighbor: x }),
        _ => None,
    }
}

/// Whether a path can go through the region: it is dry land, which includes the mountains.
fn walkable(region: RegionId) -> bool {
    matches!(region.kind(), RegionKind::Land | RegionKind::Mountain)
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
    /// chosen, the edges of its region; with an edge chosen, the centers of the two regions it divides.
    fn candidates(&self, map: &RegionMap, hovered: Option<RegionId>) -> Vec<Point> {
        let edges_of = |region: RegionId| map.neighbors(region).iter().filter(|n| walkable(n.region)).map(move |n| Point::edge(region, n.region)).collect::<Vec<_>>();
        match self.first {
            None => hovered.filter(|r| walkable(*r)).map_or(Vec::new(), |region| {
                let mut points = vec![Point::Center(region.raw())];
                points.extend(edges_of(region));
                points
            }),
            Some(Point::Center(region)) => RegionId::from_raw(region).map_or(Vec::new(), edges_of),
            Some(Point::Edge(a, b)) => vec![Point::Center(a), Point::Center(b)],
        }
    }

    /// Draws the markers and the line to the end the mouse is on, in the map's camera, and works out what the
    /// player is doing. `clicked` is whether the left button was just clicked on the map. Returns the segment
    /// the player has just finished choosing, for the caller to build.
    pub fn update(&mut self, scene: &Scene, existing: &[PathSegment], clicked: bool) -> Option<PathSegment> {
        if !self.active {
            return None;
        }
        let hovered_region = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse).and_then(|at| scene.world.region_at(at.x, at.y)) } else { None };
        let points: Vec<(Point, Vec3)> = self
            .candidates(scene.regions, hovered_region)
            .into_iter()
            .filter_map(|p| p.position(scene.regions).map(|at| (p, vec3(at.x, scene.world.surface_height(at.x, at.y) + HOLD, at.y))))
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

        // The markers, and a line from the chosen end to the one the mouse is on.
        let first_at = self.first.and_then(|f| points.iter().find(|(p, _)| *p == f).map(|(_, at)| *at).or_else(|| f.position(scene.regions).map(|a| vec3(a.x, scene.world.surface_height(a.x, a.y) + HOLD, a.y))));
        if let (Some(from), Some(point)) = (first_at, under_mouse) {
            if let Some((_, to)) = points.iter().find(|(p, _)| *p == point) {
                let exists = self.first.and_then(|f| segment(f, point)).is_some_and(|s| existing.contains(&s));
                draw_line_3d(from, *to, if exists { RED } else { Color::new(1.0, 0.85, 0.35, 1.0) });
            }
        }
        for (point, at) in &points {
            let size = ((*at - origin).length() * MARKER_SIZE).max(MARKER_LEAST) * if Some(*point) == under_mouse { 1.5 } else { 1.0 };
            let color = if Some(*point) == under_mouse { Color::new(1.0, 0.85, 0.2, 1.0) } else { Color::new(0.96, 0.96, 0.96, 1.0) };
            match point {
                Point::Center(_) => draw_sphere(*at, size, None, color),
                Point::Edge(..) => draw_cube(*at, vec3(size * 1.7, size * 1.7, size * 1.7), None, color),
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
                segment(first, point)
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

        // Why it can't be used, beside it, and in full over it.
        let reason = if no_regions { Some(("This map has no regions".to_owned(), "Paths join the regions of the map, and this map has none.".to_owned())) } else { problem.map(|p| (p.short(), p.long())) };
        if let Some((short, long)) = reason {
            let short_width = style.measure(&short, 12, Weight::Regular).width;
            style.text_mid(&short, button.x - 10.0 - short_width, button.center().y, 12, Weight::Regular, style::ERROR);
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
            (None, None) => style.text_mid("Click the center of a region, or the middle of one of its edges.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, Some(Point::Center(_))) => style.text_mid("Now click the middle of one of its edges. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, Some(Point::Edge(..))) => style.text_mid("Now click the center of one of the two regions. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
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
        assert_eq!(segment(Point::Center(a.raw()), edge), Some(PathSegment { region: a.raw(), neighbor: b.raw() }));
        assert_eq!(segment(edge, Point::Center(a.raw())), Some(PathSegment { region: a.raw(), neighbor: b.raw() }));
        // From the other side of the same edge it is the other segment.
        assert_eq!(segment(edge, Point::Center(b.raw())), Some(PathSegment { region: b.raw(), neighbor: a.raw() }));
        assert_eq!(segment(Point::Center(b.raw()), edge), Some(PathSegment { region: b.raw(), neighbor: a.raw() }));
    }

    #[test]
    fn two_ends_of_the_same_kind_or_of_different_regions_make_no_segment() {
        let (a, b, c) = (region(1, RegionKind::Land), region(2, RegionKind::Land), region(3, RegionKind::Land));
        assert_eq!(segment(Point::Center(a.raw()), Point::Center(b.raw())), None);
        assert_eq!(segment(Point::edge(a, b), Point::edge(b, c)), None);
        assert_eq!(segment(Point::Center(c.raw()), Point::edge(a, b)), None, "an edge of other regions");
    }

    #[test]
    fn an_edge_is_the_same_point_whichever_region_it_is_named_from() {
        let (a, b) = (region(1, RegionKind::Land), region(2, RegionKind::Mountain));
        assert_eq!(Point::edge(a, b), Point::edge(b, a));
    }

    #[test]
    fn only_dry_land_can_have_a_path() {
        assert!(walkable(region(1, RegionKind::Land)));
        assert!(walkable(region(2, RegionKind::Mountain)));
        assert!(!walkable(region(3, RegionKind::Shallow)));
        assert!(!walkable(region(4, RegionKind::Deep)));
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
        let tool = PathTool { active: true, first: Some(Point::Edge(0xC001, 0xC002)), ..PathTool::default() };
        // After an edge, the two centers; the map is not needed for that.
        assert_eq!(tool.candidates(&map, None), [Point::Center(0xC001), Point::Center(0xC002)]);
        // Before anything, with nothing under the mouse there is nothing to choose from.
        assert!(PathTool { active: true, ..PathTool::default() }.candidates(&map, None).is_empty());
    }
}
