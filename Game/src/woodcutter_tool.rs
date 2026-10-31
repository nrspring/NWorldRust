//! The woodcutter tool: a button in the footer that turns it on, and then the player clicks dry ground in a region
//! that has some forest, and a woodcutter is built there for the country being played. There can be only one to a
//! region, and it costs a lot.
//!
//! A marker follows the mouse over the map: green where a woodcutter could go, red where it can't, with the reason in
//! the prompt.

use std::collections::HashSet;

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};
use nworld_view::{mouse_ray, pick_terrain};

use crate::day::DaySave;
use crate::hud;
use crate::path_tool::Scene;
use crate::woodcutter::{site_region, Woodcutter, WOODCUTTER_COST};

/// How big the marker is, as a fraction of how far away it is, and the least it is, in meters.
const MARKER_SIZE: f32 = 0.009;
const MARKER_LEAST: f32 = 0.9;
/// How high above the ground the marker is held.
const HOLD: f32 = 1.5;
const BUTTON_HEIGHT: f32 = 36.0;

#[derive(Default)]
pub struct WoodcutterTool {
    active: bool,
    /// Why the last woodcutter couldn't be built.
    notice: Option<String>,
    /// Why one can't go where the mouse is, for the prompt to say.
    hover_problem: Option<String>,
    /// Where the prompt was drawn last, so the mouse can be told it is over it.
    prompt: Rect,
}

impl WoodcutterTool {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        *self = Self::default();
    }

    /// What Esc does: turns the tool off.
    pub fn escape(&mut self) {
        self.deactivate();
    }

    /// Shows why the woodcutter that `update` gave couldn't be built.
    pub fn fail(&mut self, message: String) {
        self.notice = Some(message);
    }

    /// Carries on after a woodcutter was built, ready for the next.
    pub fn built(&mut self) {
        self.notice = None;
    }

    /// Whether the mouse is over the prompt, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.active && self.prompt.contains(mouse)
    }

    /// Why a woodcutter can't go at `at`, or the number of the region it would be in. `forest` is the regions that have
    /// some (see [`crate::woodcutter::forest_regions`]).
    fn site(scene: &Scene, day: &DaySave, forest: &HashSet<u16>, at: Vec2) -> Result<u16, String> {
        let region = site_region(scene.world, forest, at)?;
        match day.woodcutter_region_problem(region) {
            Some(problem) => Err(problem),
            None => Ok(region),
        }
    }

    /// Draws the marker where the mouse is, in the map's camera, and works out what the player is doing. `clicked` is
    /// whether the left button was just clicked on the map. Returns where a woodcutter is to be built and the number of
    /// its region when the player has just clicked a place fit for one, for the caller to build.
    pub fn update(&mut self, scene: &Scene, day: &DaySave, forest: &HashSet<u16>, clicked: bool) -> Option<(Vec2, u16)> {
        if !self.active {
            return None;
        }
        let under_mouse = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse) } else { None };
        let site = under_mouse.map(|at| (at, Self::site(scene, day, forest, at)));
        self.hover_problem = None;
        if let Some((at, result)) = &site {
            let lifted = vec3(at.x, scene.world.surface_height(at.x, at.y) + HOLD, at.y);
            let (origin, _) = mouse_ray(scene.eye, scene.mouse);
            let size = ((lifted - origin).length() * MARKER_SIZE).max(MARKER_LEAST);
            match result {
                Ok(_) => draw_sphere(lifted, size, None, Color::new(0.35, 0.80, 0.40, 1.0)),
                Err(problem) => {
                    self.hover_problem = Some(problem.clone());
                    draw_sphere(lifted, size, None, RED);
                }
            }
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

    /// Draws the button that turns the tool on and off, in the `footer`, with its right edge at `right`. When
    /// `problem` is something the tool can't be used, it is dimmed, and the whole reason shows when the mouse is over
    /// it. Returns where the button is.
    pub fn show_button(&mut self, style: &Style, footer: Rect, right: f32, problem: Option<&str>) -> Rect {
        let mouse = Vec2::from(mouse_position());
        let label = "Woodcutter";
        let label_width = style.measure(label, 13, Weight::Semibold).width;
        let width = 12.0 + 14.0 + 8.0 + label_width + 14.0;
        let button = Rect::new(right - width, footer.center().y - BUTTON_HEIGHT * 0.5, width, BUTTON_HEIGHT);
        let disabled = problem.is_some();
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
        // A little pine: a triangle on a trunk.
        let icon = vec2(button.x + 12.0 + 7.0, button.center().y);
        draw_triangle(vec2(icon.x, icon.y - 8.0), vec2(icon.x - 6.0, icon.y + 4.0), vec2(icon.x + 6.0, icon.y + 4.0), color);
        draw_line(icon.x, icon.y + 4.0, icon.x, icon.y + 8.0, 2.0, color);
        style.text_mid(label, button.x + 12.0 + 14.0 + 8.0, button.center().y, 13, Weight::Semibold, color);

        if let (Some(problem), true) = (problem, hovered) {
            let lines = hud::wrap(style, problem, 12, 260.0);
            let height = lines.len() as f32 * 17.0 + 20.0;
            let tip = Rect::new((button.right() - 300.0).max(8.0), button.y - 8.0 - height, 300.0, height);
            style::card(tip, 10.0, style::GLASS);
            for (k, line) in lines.iter().enumerate() {
                style.text_mid(line, tip.x + 14.0, tip.y + 10.0 + 17.0 * (k as f32 + 0.5), 12, Weight::Regular, style::TEXT);
            }
        }
        button
    }

    /// Draws the prompt over the footer while the tool is on: what to do next, what a woodcutter costs, and what the
    /// country being played has.
    pub fn show_prompt(&mut self, style: &Style, day: &DaySave) {
        if !self.active {
            return;
        }
        let width = 520.0;
        let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - 88.0, width, 88.0);
        self.prompt = panel;
        style::card(panel, 12.0, style::GLASS);
        style.text_mid("Build a woodcutter", panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
        match (&self.notice, &self.hover_problem) {
            (Some(notice), _) => style.text_mid(notice, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, Some(problem)) => style.text_mid(problem, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, None) => style.text_mid("Click dry ground in a region with forest. One to a region. Esc stops.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
        };
        let paying = day.player_country().map_or_else(|| "No country".to_owned(), |c| format!("{} has {}", c.name, crate::format::money(c.money)));
        style.text_mid(&format!("{} coins. {paying} coins.", crate::format::money(WOODCUTTER_COST)), panel.x + 20.0, panel.y + 70.0, 12, Weight::Regular, style::TEXT_MUTED);
    }
}

/// What there is to know of `woodcutter`, next to the mouse: whose it is, the region it stands in, and whether it is
/// `operating`, which it is when it has a route to a city.
pub fn tooltip(style: &Style, day: &DaySave, woodcutter: &Woodcutter, operating: bool) {
    let region = day.region_name(woodcutter.region).map_or_else(|| format!("Region {}", woodcutter.region & 0x3FFF), str::to_owned);
    let status = if operating { ("Operating", style::TEXT) } else { ("No route to a city", style::ERROR) };
    let rows = [("Country", day.country_name(woodcutter.country).to_owned(), style::TEXT), ("Region", region, style::TEXT), ("Status", status.0.to_owned(), status.1)];
    let label_w = rows.iter().map(|(label, _, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value, _)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let title = "Woodcutter";
    let title_w = style.measure(title, 17, Weight::Semibold).width;
    let width = title_w.max(label_w + 18.0 + value_w) + 32.0;
    let height = 16.0 + 24.0 + 12.0 + rows.len() as f32 * 22.0 + 6.0;

    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - 8.0), (mouse.y + 20.0).min(screen_height() - height - hud::FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    style.text_mid(title, panel.x + 16.0, panel.y + 16.0 + 12.0, 17, Weight::Semibold, style::TEXT);
    draw_line(panel.x + 16.0, panel.y + 16.0 + 24.0 + 2.0, panel.right() - 16.0, panel.y + 16.0 + 24.0 + 2.0, 1.0, style::BORDER);
    for (k, (label, value, color)) in rows.iter().enumerate() {
        let y = panel.y + 16.0 + 24.0 + 12.0 + 11.0 + 22.0 * k as f32;
        style.text_mid(label, panel.x + 16.0, y, 13, Weight::Regular, style::TEXT_MUTED);
        let value_x = panel.right() - 16.0 - style.measure(value, 14, Weight::Semibold).width;
        style.text_mid(value, value_x, y, 14, Weight::Semibold, *color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_is_off_until_turned_on_and_esc_turns_it_off() {
        let mut tool = WoodcutterTool::default();
        assert!(!tool.is_active());
        tool.active = true;
        tool.fail("no".to_owned());
        assert!(tool.notice.is_some());
        tool.built();
        assert!(tool.notice.is_none());
        tool.escape();
        assert!(!tool.is_active());
    }

    #[test]
    fn the_prompt_is_only_the_mouses_while_the_tool_is_on() {
        let mut tool = WoodcutterTool { prompt: Rect::new(0.0, 0.0, 10.0, 10.0), ..WoodcutterTool::default() };
        assert!(!tool.has_mouse(vec2(5.0, 5.0)));
        tool.active = true;
        assert!(tool.has_mouse(vec2(5.0, 5.0)) && !tool.has_mouse(vec2(50.0, 5.0)));
    }
}
