//! The bridge tool: a button in the footer that turns it on, and then the player clicks dry ground on one side of
//! a river, and then dry ground on the other. The bridge goes between them, and costs a lot.
//!
//! With the first end chosen, a line to the place the mouse is on shows where the bridge would go: gold if it can
//! be built there, red if it can't, with the reason in the prompt.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};
use nworld_view::{mouse_ray, pick_terrain};

use crate::bridge::{check_bridge, Bridge, BridgeProblem};
use crate::costs::{self, Buildable};
use crate::day::DaySave;
use crate::ground::nearest_clear;
use crate::hud;
use crate::path_tool::Scene;

const MARKER_SIZE: f32 = 0.007;
const MARKER_LEAST: f32 = 0.6;
/// How high above the ground the markers and the line between the ends are held.
const HOLD: f32 = 1.2;
const BUTTON_HEIGHT: f32 = 36.0;

#[derive(Default)]
pub struct BridgeTool {
    active: bool,
    /// The first end of the bridge, once it has been chosen.
    first: Option<Vec2>,
    /// Why the last bridge couldn't be built.
    notice: Option<String>,
    /// Where the prompt was drawn last, so the mouse can be told it is over it.
    prompt: Rect,
    /// Why the bridge the mouse is on can't be built, for the prompt to say.
    hover_problem: Option<String>,
}

impl BridgeTool {
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

    /// Shows why the bridge that `update` gave couldn't be built.
    pub fn fail(&mut self, message: String) {
        self.first = None;
        self.notice = Some(message);
    }

    /// Carries on after a bridge was built, ready for the next.
    pub fn built(&mut self) {
        self.first = None;
        self.notice = None;
    }

    /// Whether the mouse is over the prompt, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.active && self.prompt.contains(mouse)
    }

    /// Draws a marker at each end of the bridge being planned, and the line between them, in the map's camera,
    /// and works out what the player is doing. `clicked` is whether the left button was just clicked on the map.
    /// Returns the two ends of a bridge the player has just finished choosing, for the caller to build.
    pub fn update(&mut self, scene: &Scene, bridges: &[Bridge], clicked: bool) -> Option<(Vec2, Vec2)> {
        if !self.active {
            return None;
        }
        let under_mouse = if scene.over_map { pick_terrain(scene.eye, scene.world, scene.mouse) } else { None };
        let (origin, _) = mouse_ray(scene.eye, scene.mouse);
        let lifted = |at: Vec2| vec3(at.x, scene.world.surface_height(at.x, at.y) + HOLD, at.y);
        let marker = |at: Vec2, color: Color, grow: f32| {
            let size = ((lifted(at) - origin).length() * MARKER_SIZE).max(MARKER_LEAST) * grow;
            draw_sphere(lifted(at), size, None, color);
        };

        // What the bridge would be, if the first end is chosen and the mouse is on the second.
        self.hover_problem = None;
        let planned = self.first.zip(under_mouse).map(|(first, here)| check_bridge(scene.world, bridges, first, here));
        match (&planned, self.first, under_mouse) {
            (Some(Ok((a, b))), _, _) => {
                draw_line_3d(lifted(*a), lifted(*b), Color::new(1.0, 0.85, 0.35, 1.0));
                marker(*a, Color::new(0.30, 0.55, 1.0, 1.0), 1.6);
                marker(*b, Color::new(1.0, 0.85, 0.2, 1.0), 1.4);
            }
            (Some(Err(message)), Some(first), Some(here)) => {
                self.hover_problem = Some(message.clone());
                draw_line_3d(lifted(first), lifted(here), RED);
                marker(first, Color::new(0.30, 0.55, 1.0, 1.0), 1.6);
            }
            (None, Some(first), _) => marker(first, Color::new(0.30, 0.55, 1.0, 1.0), 1.6),
            (None, None, Some(here)) => {
                // Before the first end, a marker where one could go.
                if let Some(at) = nearest_clear(scene.world, here) {
                    marker(at, Color::new(0.96, 0.96, 0.96, 1.0), 1.0);
                }
            }
            _ => {}
        }

        if !clicked {
            return None;
        }
        self.notice = None;
        let here = under_mouse?;
        match (self.first, planned) {
            (None, _) => {
                match nearest_clear(scene.world, here) {
                    Some(at) => self.first = Some(at),
                    None => self.notice = Some("A bridge has to start on dry ground by the water.".to_owned()),
                }
                None
            }
            (Some(_), Some(Ok(ends))) => {
                self.first = None;
                Some(ends)
            }
            (Some(_), Some(Err(message))) => {
                self.notice = Some(message);
                None
            }
            (Some(_), None) => None,
        }
    }

    /// Draws the button that turns the tool on and off, in the `footer`, with its right edge at `right`. When
    /// `problem` is something the tool can't be used, it is dimmed, and the whole reason shows when the mouse is over
    /// it. `inline` is a line of text to put to the left of it, which is for the reason when one of the buttons is
    /// off. Returns where the button is.
    pub fn show_button(&mut self, style: &Style, footer: Rect, right: f32, problem: Option<&BridgeProblem>, inline: Option<&str>) -> Rect {
        let mouse = Vec2::from(mouse_position());
        let label = "Bridge";
        let label_width = style.measure(label, 13, Weight::Semibold).width;
        let width = 12.0 + 18.0 + 8.0 + label_width + 14.0;
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
        // A little arch: a curve with a line under it.
        let icon = vec2(button.x + 12.0 + 9.0, button.center().y + 3.0);
        let mut last = vec2(icon.x - 8.0, icon.y);
        for k in 1..=8 {
            let t = k as f32 / 8.0;
            let next = vec2(icon.x - 8.0 + 16.0 * t, icon.y - 8.0 * (4.0 * t * (1.0 - t)));
            draw_line(last.x, last.y, next.x, next.y, 1.8, color);
            last = next;
        }
        draw_line(icon.x - 9.0, icon.y + 1.0, icon.x + 9.0, icon.y + 1.0, 1.8, color);
        style.text_mid(label, button.x + 12.0 + 18.0 + 8.0, button.center().y, 13, Weight::Semibold, color);

        if let Some(text) = inline {
            let text_width = style.measure(text, 12, Weight::Regular).width;
            style.text_mid(text, button.x - 10.0 - text_width, button.center().y, 12, Weight::Regular, style::ERROR);
        }
        if hovered {
            // What it is and costs and does, and first of all why it can't be used, if it can't.
            let mut lines = Buildable::Bridge.button_lines();
            if let Some(problem) = problem {
                lines.insert(0, (problem.long(), style::ERROR));
            }
            costs::draw_tip(style, button, &lines);
        }
        button
    }

    /// Draws the prompt over the footer while the tool is on: what to do next, what a bridge costs, and what the
    /// country being played has.
    pub fn show_prompt(&mut self, style: &Style, day: &DaySave) {
        if !self.active {
            return;
        }
        let width = 520.0;
        let panel = Rect::new((screen_width() - width) * 0.5, screen_height() - hud::FOOTER_HEIGHT - 16.0 - 112.0, width, 112.0);
        self.prompt = panel;
        style::card(panel, 12.0, style::GLASS);
        style.text_mid("Build a bridge", panel.x + 20.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
        match (&self.notice, &self.hover_problem, self.first) {
            (Some(notice), _, _) => style.text_mid(notice, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, Some(problem), Some(_)) => style.text_mid(problem, panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::ERROR),
            (None, _, None) => style.text_mid("Click dry ground by the water, on one side of a river.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
            (None, _, Some(_)) => style.text_mid("Now click dry ground on the other side. Esc lets go.", panel.x + 20.0, panel.y + 46.0, 13, Weight::Regular, style::TEXT_MUTED),
        };
        style.text_mid(&Buildable::Bridge.output(), panel.x + 20.0, panel.y + 70.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text_mid(&format!("Costs {}. {}.", Buildable::Bridge.cost_text(), Buildable::Bridge.has_text(day)), panel.x + 20.0, panel.y + 92.0, 12, Weight::Regular, style::TEXT_MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_starts_off_and_escape_steps_back_before_turning_it_off() {
        let mut tool = BridgeTool { active: true, first: Some(vec2(1.0, 2.0)), ..BridgeTool::default() };
        assert!(!BridgeTool::default().is_active());
        tool.escape();
        assert!(tool.is_active() && tool.first.is_none(), "the first Esc lets go of the end");
        tool.fail("No.".to_owned());
        tool.escape();
        assert!(tool.is_active() && tool.notice.is_none(), "then it clears the message");
        tool.escape();
        assert!(!tool.is_active());
    }

    #[test]
    fn a_failed_or_built_bridge_lets_go_of_the_first_end() {
        let mut tool = BridgeTool { active: true, first: Some(vec2(1.0, 2.0)), ..BridgeTool::default() };
        tool.fail("Not enough coin.".to_owned());
        assert!(tool.first.is_none() && tool.notice.as_deref() == Some("Not enough coin."));
        tool.first = Some(vec2(3.0, 4.0));
        tool.built();
        assert!(tool.first.is_none() && tool.notice.is_none() && tool.is_active(), "ready for the next");
    }
}
