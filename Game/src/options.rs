//! The options: settings the player can change, and the button and panel to change them with.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::hud::FOOTER_PADDING;

/// The settings. They are for how the game looks and feels, and are not part of the saved game.
pub struct Options {
    /// Show the map's regions on the ground.
    pub show_regions: bool,
    /// Show the regions that countries own in their colors. Set in the overlays menu.
    pub show_country_areas: bool,
    /// Show the names of the regions, and click a region to name it. Set in the overlays menu.
    pub show_region_names: bool,
    /// Show the names of the named places (the rivers and the lakes), and click one to name it. Set in the overlays
    /// menu.
    pub show_named_places: bool,
    /// Show the buildings of the cities. Turned off in the overlays menu to see what is in them.
    pub show_cities: bool,
}

impl Default for Options {
    /// How the game starts: with the regions showing, and the countries' areas, and the cities.
    fn default() -> Self {
        Self { show_regions: true, show_country_areas: true, show_region_names: false, show_named_places: false, show_cities: true }
    }
}

pub const BUTTON_SIZE: f32 = 36.0;
const PANEL_WIDTH: f32 = 320.0;
const HEADER_HEIGHT: f32 = 48.0;
const ROW_HEIGHT: f32 = 56.0;
const GAP: f32 = 8.0;

/// The options button, and the panel it opens.
#[derive(Default)]
pub struct OptionsUi {
    open: bool,
    /// Where the footer and the panel were drawn last, so the mouse can be told it is over them.
    footer: Rect,
    panel: Option<Rect>,
}

impl OptionsUi {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the mouse is over the footer or the panel, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.footer.contains(mouse) || self.panel.is_some_and(|p| p.contains(mouse))
    }

    /// Draws the button at the right end of the `footer`, and the panel above it when it is open, and
    /// changes `options` when the player clicks a setting. `has_regions` is whether the map has
    /// any, since there is nothing to show without them.
    pub fn show(&mut self, style: &Style, footer: Rect, options: &mut Options, has_regions: bool) {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        self.footer = footer;
        let button = Rect::new(footer.right() - FOOTER_PADDING - BUTTON_SIZE, footer.center().y - BUTTON_SIZE * 0.5, BUTTON_SIZE, BUTTON_SIZE);
        let hovered = button.contains(mouse);
        if hovered && clicked {
            self.open = !self.open;
        }
        if self.open {
            style::rounded_rect(button, 10.0, style::ACCENT_SOFT);
        } else if hovered {
            style::rounded_rect(button, 10.0, style::HOVER);
        }
        draw_gear(button.center(), if self.open { style::ACCENT_HOVER } else { style::TEXT });

        if !self.open {
            self.panel = None;
            return;
        }
        let rows = 1;
        let height = HEADER_HEIGHT + ROW_HEIGHT * rows as f32 + GAP;
        let panel = Rect::new(footer.right() - FOOTER_PADDING - PANEL_WIDTH, footer.y - GAP - height, PANEL_WIDTH, height);
        self.panel = Some(panel);
        style::card(panel, 14.0, style::GLASS);
        style.text_mid("Options", panel.x + 16.0, panel.y + HEADER_HEIGHT * 0.5, 16, Weight::Semibold, style::TEXT);
        draw_line(panel.x + 16.0, panel.y + HEADER_HEIGHT - 1.0, panel.right() - 16.0, panel.y + HEADER_HEIGHT - 1.0, 1.0, style::BORDER);

        let row = Rect::new(panel.x + 8.0, panel.y + HEADER_HEIGHT, panel.w - 16.0, ROW_HEIGHT);
        let note = if has_regions { None } else { Some("This map has no regions.") };
        toggle(style, row, "Show regions", note, &mut options.show_regions, has_regions);
    }
}

/// One setting that is on or off: its label, a note under it, and a switch at the right. Clicking
/// the row flips `value` when it is `enabled`.
pub fn toggle(style: &Style, row: Rect, label: &str, note: Option<&str>, value: &mut bool, enabled: bool) {
    let hovered = enabled && row.contains(Vec2::from(mouse_position()));
    if hovered {
        style::rounded_rect(row, 8.0, style::HOVER);
        if is_mouse_button_pressed(MouseButton::Left) {
            *value = !*value;
        }
    }

    let text_color = if enabled { style::TEXT } else { style::TEXT_MUTED };
    match note {
        Some(note) => {
            style.text_mid(label, row.x + 12.0, row.center().y - 9.0, 15, Weight::Regular, text_color);
            style.text_mid(note, row.x + 12.0, row.center().y + 11.0, 12, Weight::Regular, style::TEXT_MUTED);
        }
        None => style.text_mid(label, row.x + 12.0, row.center().y, 15, Weight::Regular, text_color),
    }

    // The switch: a track with a knob that sits at the right when on.
    let track = Rect::new(row.right() - 12.0 - 44.0, row.center().y - 12.0, 44.0, 24.0);
    let on = *value && enabled;
    let track_color = match (on, enabled) {
        (true, _) => style::ACCENT,
        (false, true) => Color::new(1.0, 1.0, 1.0, 0.16),
        (false, false) => Color::new(1.0, 1.0, 1.0, 0.07),
    };
    style::rounded_rect(track, 12.0, track_color);
    let knob_x = if on { track.right() - 12.0 } else { track.x + 12.0 };
    draw_circle(knob_x, track.center().y, 9.0, if enabled { WHITE } else { Color::new(1.0, 1.0, 1.0, 0.4) });
}

/// A cog: a ring with eight teeth.
fn draw_gear(center: Vec2, color: Color) {
    draw_circle_lines(center.x, center.y, 6.0, 2.5, color);
    for k in 0..8 {
        let angle = std::f32::consts::TAU * k as f32 / 8.0;
        let tooth = center + vec2(angle.cos(), angle.sin()) * 8.5;
        draw_rectangle_ex(
            tooth.x,
            tooth.y,
            3.5,
            5.0,
            DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: angle + std::f32::consts::FRAC_PI_2, color },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regions_and_the_countries_areas_show_when_the_game_starts() {
        assert!(Options::default().show_cities);
        assert!(Options::default().show_regions);
        assert!(Options::default().show_country_areas);
        assert!(!Options::default().show_region_names && !Options::default().show_named_places, "the names start off: there are none to show");
    }
}
