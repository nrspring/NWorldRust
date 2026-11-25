//! The barracks tools (one for a barracks and one for an archery range): a row in the Buildings menu that turns it on, and then the
//! player clicks dry ground, and one is built there for the country being played, which pays for it.
//!
//! A marker follows the mouse over the map: green where a barracks could go, red where it can't, with the reason in the prompt.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};
use nworld_view::{mouse_ray, pick_terrain};

use crate::barracks::{site_region, Barracks, BarracksKind};
use crate::barracks_view;
use crate::costs::{self, Buildable};
use crate::day::DaySave;
use crate::hud;
use crate::path_tool::Scene;
use crate::toolbar;

/// How big the marker is, as a fraction of how far away it is, and the least it is, in meters.
const MARKER_SIZE: f32 = 0.009;
const MARKER_LEAST: f32 = 0.9;
/// How high above the ground the marker is held.
const HOLD: f32 = 1.5;

#[derive(Default)]
pub struct BarracksTool {
    /// Which kind of building it builds.
    kind: BarracksKind,
    active: bool,
    /// Why the last barracks couldn't be built.
    notice: Option<String>,
    /// Why one can't go where the mouse is, for the prompt to say.
    hover_problem: Option<String>,
    /// Where the prompt was drawn last, so the mouse can be told it is over it.
    prompt: Rect,
}

impl BarracksTool {
    pub fn new(kind: BarracksKind) -> Self {
        Self { kind, ..Self::default() }
    }

    /// The kind of building it builds.
    pub fn kind(&self) -> BarracksKind {
        self.kind
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        *self = Self::new(self.kind);
    }

    /// What Esc does: turns the tool off.
    pub fn escape(&mut self) {
        self.deactivate();
    }

    /// Shows why the barracks that `update` gave couldn't be built.
    pub fn fail(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// Carries on after a barracks was built, ready for the next.
    pub fn built(&mut self) {
        self.notice = None;
    }

    /// Whether the mouse is over the prompt, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.active && self.prompt.contains(mouse)
    }

    /// Why a barracks can't go at `at`, or the number of the region it would be in (`None` if the map has none).
    fn site(scene: &Scene, day: &DaySave, at: Vec2) -> Result<Option<u16>, String> {
        let region = site_region(scene.world, at)?;
        match day.barracks_place_problem(at.x, at.y, region) {
            Some(problem) => Err(problem),
            None => Ok(region),
        }
    }

    /// Draws the marker where the mouse is, in the map's camera, and works out what the player is doing. `clicked` is whether the
    /// left button was just clicked on the map. Returns where a barracks is to be built and the number of its region when the player
    /// has just clicked a place fit for one, for the caller to build.
    pub fn update(&mut self, scene: &Scene, day: &DaySave, clicked: bool) -> Option<(Vec2, Option<u16>)> {
        if !self.active {
            return None;
        }
        let under_mouse = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse) } else { None };
        let site = under_mouse.map(|at| (at, Self::site(scene, day, at)));
        self.hover_problem = None;
        if let Some((at, result)) = &site {
            let lifted = vec3(at.x, scene.world.surface_height(at.x, at.y) + HOLD, at.y);
            let (origin, _) = mouse_ray(scene.eye, scene.mouse);
            let size = ((lifted - origin).length() * MARKER_SIZE).max(MARKER_LEAST);
            let color = match result {
                Ok(_) => Color::new(0.35, 0.80, 0.40, 1.0),
                Err(problem) => {
                    self.hover_problem = Some(problem.clone());
                    RED
                }
            };
            draw_sphere(lifted, size, None, color);
            // And the barracks itself, in outline.
            barracks_view::preview(scene.world, self.kind, *at, scene.eye.position, scene.time, color);
        }

        if !clicked {
            return None;
        }
        self.notice = None;
        match site? {
            (at, Ok(region)) => Some((at, region)),
            (_, Err(problem)) => {
                self.notice = Some(problem);
                None
            }
        }
    }

    /// Draws the tool's row in the Buildings menu, in `rect`, which turns the tool on and off when clicked. `menu` is where the
    /// menu's panel is, for the tip to go above. When `problem` is something the tool can't be used, the row is dimmed, and the
    /// whole reason shows when the mouse is over it.
    pub fn show_row(&mut self, style: &Style, rect: Rect, menu: Rect, problem: Option<&str>) {
        let (buildable, kind) = (Buildable::Barracks(self.kind), self.kind);
        let row = toolbar::draw_row(style, rect, buildable.label(), buildable.cost(), self.active, problem.is_some(), |icon, color| crate::barracks::draw_icon(kind, icon, 1.0, color, style::GLASS));
        if row.pressed {
            if self.active {
                self.deactivate();
            } else {
                *self = Self { active: true, ..Self::new(self.kind) };
            }
        }
        if row.hovered {
            // What it is and costs and does, and first of all why it can't be used, if it can't.
            let mut lines = buildable.button_lines();
            if let Some(problem) = problem {
                lines.insert(0, (problem.to_owned(), style::ERROR));
            }
            costs::draw_tip(style, menu, &lines);
        }
    }

    /// Draws the prompt over the footer while the tool is on: what to do next, what a barracks costs, and what the country being
    /// played has.
    pub fn show_prompt(&mut self, style: &Style, day: &DaySave) {
        if !self.active {
            return;
        }
        let width = 600.0;
        let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - 112.0, width, 112.0);
        self.prompt = panel;
        style::card(panel, 12.0, style::GLASS);
        let buildable = Buildable::Barracks(self.kind);
        style.text_mid(&format!("Build {}", self.kind.a_noun()), panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
        match (&self.notice, &self.hover_problem) {
            (Some(notice), _) => style.text_mid(notice, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, Some(problem)) => style.text_mid(problem, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, None) => style.text_mid("Click dry ground in a region you own.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
        };
        style.text_mid(&buildable.output(), panel.x + 20.0, panel.y + 70.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text_mid(&format!("Costs {}. {}.", buildable.cost_text(), buildable.has_text(day)), panel.x + 20.0, panel.y + 92.0, 12, Weight::Regular, style::TEXT_MUTED);
    }
}

/// What there is to know of `barracks`, next to the mouse: whose it is, the region it stands in, and whether it is ready to train.
pub fn tooltip(style: &Style, day: &DaySave, barracks: &Barracks) {
    let region = barracks.region.map_or_else(|| "None".to_owned(), |region| day.region_name(region).map_or_else(|| format!("Region {}", region & 0x3FFF), str::to_owned));
    let rest = barracks.rest_left(day.date);
    let status = if rest == 0 { "Ready to train".to_owned() } else { format!("Resting for {}", crate::ship::days_text(rest)) };
    let rows = [("Country", day.country_name(barracks.country).to_owned()), ("Region", region), ("Training", status)];
    let label_w = rows.iter().map(|(label, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let title = barracks.kind.label();
    let title_w = style.measure(title, 17, Weight::Semibold).width;
    let width = title_w.max(label_w + 18.0 + value_w) + 32.0;
    let height = 16.0 + 24.0 + 12.0 + rows.len() as f32 * 22.0 + 6.0;

    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - 8.0), (mouse.y + 20.0).min(screen_height() - height - hud::FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    style.text_mid(title, panel.x + 16.0, panel.y + 16.0 + 12.0, 17, Weight::Semibold, style::TEXT);
    draw_line(panel.x + 16.0, panel.y + 16.0 + 24.0 + 2.0, panel.right() - 16.0, panel.y + 16.0 + 24.0 + 2.0, 1.0, style::BORDER);
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = panel.y + 16.0 + 24.0 + 12.0 + 11.0 + 22.0 * k as f32;
        style.text_mid(label, panel.x + 16.0, y, 13, Weight::Regular, style::TEXT_MUTED);
        let value_x = panel.right() - 16.0 - style.measure(value, 14, Weight::Semibold).width;
        style.text_mid(value, value_x, y, 14, Weight::Semibold, style::TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_is_off_until_turned_on_and_esc_turns_it_off() {
        let mut tool = BarracksTool::new(BarracksKind::ArcheryRange);
        assert!(!tool.is_active());
        tool.active = true;
        tool.fail("no".to_owned());
        assert!(tool.notice.is_some());
        tool.built();
        assert!(tool.notice.is_none());
        tool.escape();
        assert!(!tool.is_active());
        assert_eq!(tool.kind(), BarracksKind::ArcheryRange, "turning it off keeps what it builds");
    }

    #[test]
    fn the_prompt_is_only_the_mouses_while_the_tool_is_on() {
        let mut tool = BarracksTool { prompt: Rect::new(0.0, 0.0, 10.0, 10.0), ..BarracksTool::default() };
        assert!(!tool.has_mouse(vec2(5.0, 5.0)));
        tool.active = true;
        assert!(tool.has_mouse(vec2(5.0, 5.0)) && !tool.has_mouse(vec2(50.0, 5.0)));
    }
}
