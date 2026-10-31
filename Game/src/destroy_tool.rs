//! The destroy tool: a button in the footer that turns it on, and then the player points at a path, a road, a bridge
//! or a gatherer and clicks it, and it is gone for good. It costs nothing, and nothing is paid back. The one the
//! mouse is on is marked in red, so that it is clear which will go; if it is a bridge, so are the paths and roads that
//! start at its ends.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};
use nworld_view::{mouse_ray, pick_terrain};

use crate::costs;
use crate::day::DaySave;
use crate::failure::Thing;
use crate::hud;
use crate::ids::GathererId;
use crate::path::{Kind, PathPiece};
use crate::path_tool::Scene;
use crate::path_view::{distance_to_segment, PathRenderer};
use crate::gatherer_view;

/// What the tool can destroy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A path, a road or a bridge, or a path or road from the end of one.
    Built(Thing),
    Gatherer(GathererId),
}

impl Target {
    /// Destroys it in `day`. An error says why it can't be done.
    pub fn apply(self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Built(thing) => day.destroy(thing),
            Self::Gatherer(id) => day.destroy_gatherer(id),
        }
    }
}

/// The least distance, in meters, the mouse can be from something and still be on it.
const LEAST_REACH: f32 = 5.0;
/// How big a mark is, as a fraction of how far away it is, and the least it is, in meters.
const MARK_SIZE: f32 = 0.006;
const MARK_LEAST: f32 = 0.7;
/// How far apart the marks along the thing pointed at are, in meters.
const MARK_EVERY: f32 = 4.0;
/// How high above the ground the marks are held, and how much higher over a bridge, which arches.
const HOLD: f32 = 1.2;
const BRIDGE_HOLD: f32 = 3.0;
const BUTTON_HEIGHT: f32 = 36.0;
const RED: Color = Color::new(0.93, 0.26, 0.24, 1.0);

#[derive(Default)]
pub struct DestroyTool {
    active: bool,
    /// What the mouse is on, which a click destroys.
    hovered: Option<Target>,
    /// Why the last one couldn't be destroyed.
    notice: Option<String>,
    /// What has been clicked and is waiting to be confirmed.
    pending: Option<Target>,
    /// Where the prompt was drawn last, so the mouse can be told it is over it.
    prompt: Rect,
    /// Where the buttons that confirm and cancel were drawn last.
    confirm_button: Rect,
    cancel_button: Rect,
}

impl DestroyTool {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        *self = Self::default();
    }

    /// What Esc does: calls off what was waiting to be confirmed, and failing that turns the tool off.
    pub fn escape(&mut self) {
        if self.pending.take().is_none() {
            self.deactivate();
        }
    }

    /// Shows why what `update` gave couldn't be destroyed.
    pub fn fail(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// Carries on after something was destroyed, ready for the next.
    pub fn destroyed(&mut self) {
        self.hovered = None;
        self.notice = None;
    }

    /// Whether the mouse is over the prompt, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.active && self.prompt.contains(mouse)
    }

    /// What is nearest `at` (world x and z) within `reach` meters, if anything: a bridge, then a road, then a path
    /// when they are as near as each other, since the one on top is the one being pointed at.
    fn thing_near(day: &DaySave, paths: &PathRenderer, roads: &PathRenderer, at: Vec2, reach: f32) -> Option<Thing> {
        let bridges = day.bridges.iter().map(|b| (Thing::Bridge(b.id), distance_to_segment(b.first.into(), b.second.into(), at)));
        let road = roads.piece_near(at, reach).map(|(piece, distance)| (Thing::of(Kind::Road, piece), distance));
        let path = paths.piece_near(at, reach).map(|(piece, distance)| (Thing::of(Kind::Path, piece), distance));
        bridges.chain(road).chain(path).filter(|(_, distance)| *distance <= reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(thing, _)| thing)
    }

    /// Marks the thing the mouse is on, in the map's camera, and works out what the player is doing. `reach` is how
    /// close, in meters, the mouse has to be to a thing to be on it, which grows as the camera moves away. `clicked` is
    /// whether the left button was just clicked on the map. Returns what the player has just clicked, for the caller to
    /// destroy. A gatherer is pointed at where it is seen, and is first, since it stands over what is round it.
    pub fn update(&mut self, scene: &Scene, day: &DaySave, paths: &PathRenderer, roads: &PathRenderer, reach: f32, clicked: bool) -> Option<Target> {
        if !self.active {
            return None;
        }
        let ray = mouse_ray(scene.eye, scene.mouse);
        let origin = ray.0;
        // While it is being confirmed, what is to go stays marked, and the mouse can't point at anything else.
        self.hovered = if self.pending.is_some() {
            self.pending
        } else {
            let gatherer = if scene.over_map { gatherer_view::under_ray(ray, scene.world, &day.gatherers) } else { None };
            match gatherer {
                Some(id) => Some(Target::Gatherer(id)),
                None => {
                    let under_mouse = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse) } else { None };
                    under_mouse.and_then(|at| Self::thing_near(day, paths, roads, at, reach.max(LEAST_REACH))).map(Target::Built)
                }
            }
        };

        if let Some(Target::Gatherer(id)) = self.hovered {
            if let Some(gatherer) = day.gatherer(id) {
                gatherer_view::outline(scene.world, gatherer, RED);
            }
        }
        if let Some(Target::Built(thing)) = self.hovered {
            for (line, hold) in lines_of(day, paths, roads, thing) {
                for pair in line.windows(2) {
                    let steps = (pair[0].distance(pair[1]) / MARK_EVERY).ceil().max(1.0) as usize;
                    for k in 0..=steps {
                        let at = pair[0].lerp(pair[1], k as f32 / steps as f32);
                        let lifted = vec3(at.x, scene.world.surface_height(at.x, at.y) + hold, at.y);
                        draw_sphere(lifted, ((lifted - origin).length() * MARK_SIZE).max(MARK_LEAST), None, RED);
                    }
                }
            }
        }

        if self.pending.is_some() {
            // Waiting to be confirmed: Destroy goes ahead, and Cancel, or a click anywhere else, thinks better of it.
            if is_mouse_button_pressed(MouseButton::Left) {
                let mouse = Vec2::from(mouse_position());
                if self.confirm_button.contains(mouse) {
                    return self.pending.take();
                }
                if self.cancel_button.contains(mouse) || !self.prompt.contains(mouse) {
                    self.pending = None;
                }
            }
            return None;
        }
        if clicked && self.hovered.is_some() {
            self.notice = None;
            self.pending = self.hovered;
        }
        None
    }

    /// Draws the button that turns the tool on and off, in the `footer`, with its right edge at `right`. Returns where
    /// the button is.
    pub fn show_button(&mut self, style: &Style, footer: Rect, right: f32) -> Rect {
        let mouse = Vec2::from(mouse_position());
        let label = "Destroy";
        let label_width = style.measure(label, 13, Weight::Semibold).width;
        let width = 12.0 + 14.0 + 8.0 + label_width + 14.0;
        let button = Rect::new(right - width, footer.center().y - BUTTON_HEIGHT * 0.5, width, BUTTON_HEIGHT);
        let hovered = button.contains(mouse);
        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            if self.active {
                self.deactivate();
            } else {
                *self = Self { active: true, ..Self::default() };
            }
        }
        if self.active {
            style::rounded_rect(button, 10.0, style::ACCENT_SOFT);
        } else if hovered {
            style::rounded_rect(button, 10.0, style::HOVER);
        }
        let color = if self.active { style::ACCENT_HOVER } else { style::TEXT };
        // A cross.
        let icon = vec2(button.x + 12.0 + 7.0, button.center().y);
        draw_line(icon.x - 5.0, icon.y - 5.0, icon.x + 5.0, icon.y + 5.0, 2.0, color);
        draw_line(icon.x - 5.0, icon.y + 5.0, icon.x + 5.0, icon.y - 5.0, 2.0, color);
        style.text_mid(label, button.x + 12.0 + 14.0 + 8.0, button.center().y, 13, Weight::Semibold, color);
        if hovered {
            costs::draw_tip(style, button, &[("Destroy: free.".to_owned(), style::TEXT), ("Removes a path, road, bridge or gatherer for good. Nothing is paid back.".to_owned(), style::TEXT_MUTED)]);
        }
        button
    }

    /// Draws the prompt over the footer while the tool is on: what to do, and what the mouse is on.
    pub fn show_prompt(&mut self, style: &Style, day: &DaySave) {
        if !self.active {
            return;
        }
        let width = 520.0;
        if let Some(target) = self.pending {
            let height = 132.0;
            let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - height, width, height);
            self.prompt = panel;
            style::card(panel, 12.0, style::GLASS);
            let (name, more) = match target {
                Target::Built(thing) => (thing.name(), if matches!(thing, Thing::Bridge(_)) { " The paths and roads from its ends go too." } else { "" }),
                Target::Gatherer(id) => (day.gatherer(id).map_or("gatherer", |g| g.kind.noun()), ""),
            };
            style.text_mid(&format!("Destroy this {name}?"), panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
            style.text_mid(&format!("It is gone for good, and nothing is paid back.{more}"), panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR);
            let (button_width, button_height) = (110.0, 32.0);
            let top = panel.y + height - button_height - 16.0;
            self.cancel_button = Rect::new(panel.x + width - 20.0 - button_width, top, button_width, button_height);
            self.confirm_button = Rect::new(self.cancel_button.x - 10.0 - button_width, top, button_width, button_height);
            let mouse = Vec2::from(mouse_position());
            let fill = if self.confirm_button.contains(mouse) { Color::new(1.0, 0.36, 0.34, 1.0) } else { RED };
            style::rounded_rect(self.confirm_button, 8.0, fill);
            style.text_centered("Destroy", self.confirm_button, 14, Weight::Semibold, WHITE);
            let fill = if self.cancel_button.contains(mouse) { Color::new(1.0, 1.0, 1.0, 0.10) } else { style::FIELD };
            style::rounded_rect(self.cancel_button, 8.0, fill);
            style::rounded_rect_lines(self.cancel_button, 8.0, 1.0, style::BORDER_STRONG);
            style.text_centered("Cancel", self.cancel_button, 14, Weight::Semibold, style::TEXT);
            return;
        }
        let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - 88.0, width, 88.0);
        self.prompt = panel;
        style::card(panel, 12.0, style::GLASS);
        style.text_mid("Destroy a path, a road, a bridge or a gatherer", panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
        match (&self.notice, self.hovered) {
            (Some(notice), _) => style.text_mid(notice, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, Some(Target::Built(thing))) => {
                let failed = if day.is_failed(thing) { " (it has failed)" } else { "" };
                let more = if matches!(thing, Thing::Bridge(_)) { " The paths and roads from its ends go too." } else { "" };
                style.text_mid(&format!("Click to destroy this {}{failed}.{more}", thing.name()), panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR)
            }
            (None, Some(Target::Gatherer(id))) => style.text_mid(&format!("Click to destroy this {}.", day.gatherer(id).map_or("gatherer", |g| g.kind.noun())), panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, None) => style.text_mid("Point at one and click it. Esc stops.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
        };
        style.text_mid("Destroying is free, and nothing is paid back.", panel.x + 20.0, panel.y + 70.0, 12, Weight::Regular, style::TEXT_MUTED);
    }
}

/// The lines of points that `thing` lies along, each with how high above the ground to mark it.
fn lines_of(day: &DaySave, paths: &PathRenderer, roads: &PathRenderer, thing: Thing) -> Vec<(Vec<Vec2>, f32)> {
    let along = |renderer: &PathRenderer, piece: PathPiece| renderer.route_of(piece).iter().map(|line| (line.clone(), HOLD)).collect();
    match thing {
        Thing::Path(segment) => along(paths, PathPiece::Segment(segment)),
        Thing::PathLink(link) => along(paths, PathPiece::Link(link)),
        Thing::Road(segment) => along(roads, PathPiece::Segment(segment)),
        Thing::RoadLink(link) => along(roads, PathPiece::Link(link)),
        Thing::Bridge(id) => day.bridge(id).map(|b| vec![(vec![b.first.into(), b.second.into()], BRIDGE_HOLD)]).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatherer::GathererKind;
    use crate::bridge::Spot;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day
    }

    #[test]
    fn the_tool_is_off_until_turned_on_and_esc_turns_it_off() {
        let mut tool = DestroyTool::default();
        assert!(!tool.is_active());
        tool.active = true;
        tool.notice = Some("x".to_owned());
        tool.escape();
        assert!(!tool.is_active() && tool.notice.is_none());
    }

    #[test]
    fn esc_calls_off_a_destroy_waiting_to_be_confirmed_before_it_turns_the_tool_off() {
        let mut tool = DestroyTool { active: true, pending: Some(Target::Gatherer(GathererId::from_u128(1))), ..DestroyTool::default() };
        tool.escape();
        assert!(tool.is_active() && tool.pending.is_none());
        tool.escape();
        assert!(!tool.is_active());
    }

    #[test]
    fn the_prompt_is_only_the_mouses_while_the_tool_is_on() {
        let mut tool = DestroyTool { prompt: Rect::new(0.0, 0.0, 10.0, 10.0), ..DestroyTool::default() };
        assert!(!tool.has_mouse(vec2(5.0, 5.0)));
        tool.active = true;
        assert!(tool.has_mouse(vec2(5.0, 5.0)) && !tool.has_mouse(vec2(50.0, 5.0)));
    }

    #[test]
    fn a_target_destroys_what_it_is_in_the_day() {
        let mut day = day();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: 0.0, z: 0.0 }, Spot { x: 40.0, z: 0.0 }).unwrap();
        day.build_gatherer(GathererKind::Woodcutter, 5.0, 5.0, 0xC001).unwrap();
        let (bridge, gatherer) = (day.bridges[0].id, day.gatherers[0].id);
        Target::Gatherer(gatherer).apply(&mut day).unwrap();
        assert!(day.gatherers.is_empty() && day.bridges.len() == 1);
        assert!(Target::Gatherer(gatherer).apply(&mut day).is_err(), "it is already gone");
        Target::Built(Thing::Bridge(bridge)).apply(&mut day).unwrap();
        assert!(day.bridges.is_empty());
    }

    #[test]
    fn what_is_pointed_at_is_a_bridge_when_the_mouse_is_near_enough_to_it() {
        let mut day = day();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: 0.0, z: 0.0 }, Spot { x: 40.0, z: 0.0 }).unwrap();
        let bridge = day.bridges[0].id;
        let (paths, roads) = (PathRenderer::new(Kind::Path), PathRenderer::new(Kind::Road));
        assert_eq!(DestroyTool::thing_near(&day, &paths, &roads, vec2(20.0, 2.0), 5.0), Some(Thing::Bridge(bridge)));
        assert_eq!(DestroyTool::thing_near(&day, &paths, &roads, vec2(20.0, 9.0), 5.0), None, "out of reach");
        assert_eq!(DestroyTool::thing_near(&day, &paths, &roads, vec2(60.0, 0.0), 5.0), None, "past the end");
    }
}
