//! The overlays menu: a button in the footer that opens a panel of layers that can be drawn over the
//! map, each turned on and off with a switch.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::hud::FOOTER_PADDING;
use crate::options::{self, Options};

const BUTTON_HEIGHT: f32 = 36.0;
const PANEL_WIDTH: f32 = 320.0;
const HEADER_HEIGHT: f32 = 48.0;
const ROW_HEIGHT: f32 = 56.0;
const GAP: f32 = 8.0;
const TEXT_SIZE: u16 = 13;

/// The overlays button, and the panel it opens.
#[derive(Default)]
pub struct OverlaysUi {
    open: bool,
    /// Where the footer and the panel were drawn last, so the mouse can be told it is over them.
    footer: Rect,
    panel: Option<Rect>,
    /// Where the button was drawn last.
    button: Rect,
}

impl OverlaysUi {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Where the button is, so that another can be put beside it.
    pub fn button(&self) -> Rect {
        self.button
    }

    /// Whether the mouse is over the footer or the panel, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.footer.contains(mouse) || self.panel.is_some_and(|p| p.contains(mouse))
    }

    /// Draws the button in the `footer`, just left of the options button, and the panel above it when it
    /// is open, and changes `options` when the player clicks a switch. `has_regions` is whether the map has
    /// any, since there is nothing to show without them.
    pub fn show(&mut self, style: &Style, footer: Rect, options: &mut Options, has_regions: bool) {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        self.footer = footer;
        let label = "Overlays";
        let label_width = style.measure(label, TEXT_SIZE, Weight::Semibold).width;
        let width = label_width + 14.0 + 12.0 + 8.0 + 8.0;
        let right = footer.right() - FOOTER_PADDING - options::BUTTON_SIZE - GAP;
        let button = Rect::new(right - width, footer.center().y - BUTTON_HEIGHT * 0.5, width, BUTTON_HEIGHT);
        self.button = button;
        let hovered = button.contains(mouse);
        if hovered && clicked {
            self.open = !self.open;
        }
        if self.open {
            style::rounded_rect(button, 10.0, style::ACCENT_SOFT);
        } else if hovered {
            style::rounded_rect(button, 10.0, style::HOVER);
        }
        let color = if self.open { style::ACCENT_HOVER } else { style::TEXT };
        style.text_mid(label, button.x + 14.0, button.center().y, TEXT_SIZE, Weight::Semibold, color);
        // A small arrow, pointing up as the panel opens upward from here.
        let arrow = vec2(button.right() - 16.0, button.center().y);
        draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), color);

        if !self.open {
            self.panel = None;
            return;
        }
        let rows = 3;
        let height = HEADER_HEIGHT + ROW_HEIGHT * rows as f32 + GAP;
        let panel = Rect::new(button.right() - PANEL_WIDTH, footer.y - GAP - height, PANEL_WIDTH, height);
        self.panel = Some(panel);
        style::card(panel, 14.0, style::GLASS);
        style.text_mid("Overlays", panel.x + 16.0, panel.y + HEADER_HEIGHT * 0.5, 16, Weight::Semibold, style::TEXT);
        draw_line(panel.x + 16.0, panel.y + HEADER_HEIGHT - 1.0, panel.right() - 16.0, panel.y + HEADER_HEIGHT - 1.0, 1.0, style::BORDER);

        let row = Rect::new(panel.x + 8.0, panel.y + HEADER_HEIGHT, panel.w - 16.0, ROW_HEIGHT);
        let note = if has_regions { None } else { Some("This map has no regions.") };
        options::toggle(style, row, "Country areas", note, &mut options.show_country_areas, has_regions);
        let names = Rect::new(row.x, row.y + ROW_HEIGHT, row.w, ROW_HEIGHT);
        let names_note = if has_regions { Some("Click a region to name it.") } else { note };
        options::toggle(style, names, "Region names", names_note, &mut options.show_region_names, has_regions);
        let places = Rect::new(row.x, row.y + ROW_HEIGHT * 2.0, row.w, ROW_HEIGHT);
        options::toggle(style, places, "Named places", Some("Click a river or lake to name it."), &mut options.show_named_places, true);
    }
}
