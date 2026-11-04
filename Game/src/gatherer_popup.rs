//! The gatherer popup, opened by clicking a gatherer: what there is to know of it. A gatherer belongs to whoever owns the
//! region it stands in, so there is no changing its country here.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::ids::GathererId;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

#[derive(Default)]
pub struct GathererPopup {
    /// The gatherer being shown, while the popup is up.
    gatherer: Option<GathererId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
}

impl GathererPopup {
    pub fn is_open(&self) -> bool {
        self.gatherer.is_some()
    }

    pub fn open(&mut self, gatherer: GathererId) {
        *self = Self { gatherer: Some(gatherer), opened_at: get_time(), fresh: true };
    }

    /// What Esc does: closes the popup.
    pub fn escape(&mut self) {
        self.gatherer = None;
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A gatherer that is not in `day` is not
    /// shown.
    pub fn show(&mut self, style: &Style, day: &DaySave) {
        let Some(gatherer) = self.gatherer.and_then(|id| day.gatherer(id)) else {
            self.gatherer = None;
            return;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(gatherer.country);
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + 16.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        gatherer.kind.draw_icon(badge.center(), 1.5, style::ACCENT_HOVER, style::ACCENT_SOFT);
        let text_x = badge.right() + 14.0;
        style.text(gatherer.kind.label(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&day.region_label(gatherer.region), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..3).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in [rows[0].y, rows[1].y, rows[2].y, rows[2].bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }
        let value_row = |label: &str, value: &str, row: Rect, color: Color, swatch: Option<Color>| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, color);
            if let Some(swatch) = swatch {
                style::rounded_rect(Rect::new(x - 18.0, row.center().y - 6.0, 12.0, 12.0), 3.0, swatch);
            }
        };
        value_row("Country", country.map_or("an unknown country", |c| c.name.as_str()), rows[0], style::TEXT, country.map(|c| c.color.color()));
        let region = day.region_name(gatherer.region).map_or_else(|| format!("Region {}", gatherer.region & 0x3FFF), str::to_owned);
        value_row("Region", &region, rows[1], style::TEXT, None);
        let (status, status_color) = if gatherer.operating { ("Operating", style::TEXT) } else { ("No route to a city", style::ERROR) };
        value_row("Status", status, rows[2], status_color, None);

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // A click on the button, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if enter || (clicked && (over_close || !card.contains(mouse))) {
            self.gatherer = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_it() {
        assert!(!GathererPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = GathererPopup { gatherer: Some(GathererId::new()), ..GathererPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(!popup.is_open());
    }
}
