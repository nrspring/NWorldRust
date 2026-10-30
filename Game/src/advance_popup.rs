//! The popup that asks whether to end the day and start the next, opened by clicking the day in the header.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::calendar::Date;
use crate::popup::{self, Icon};

const CARD_WIDTH: f32 = 420.0;
const PADDING: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

#[derive(Default)]
pub struct AdvancePopup {
    /// The day that would end, while the popup is up.
    day: Option<Date>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Why the last try couldn't be done.
    error: Option<String>,
}

impl AdvancePopup {
    pub fn is_open(&self) -> bool {
        self.day.is_some()
    }

    /// Asks about ending `day`.
    pub fn open(&mut self, day: Date) {
        *self = Self { day: Some(day), opened_at: get_time(), fresh: true, error: None };
    }

    pub fn close(&mut self) {
        self.day = None;
    }

    /// Shows why the day couldn't be ended.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse for this frame and draws the popup over everything. Returns whether the player
    /// clicked Start day. Esc, which the caller handles, gives up.
    pub fn show(&mut self, style: &Style) -> bool {
        let Some(day) = self.day else {
            return false;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let inner_w = CARD_WIDTH - PADDING * 2.0;
        let error_height = if self.error.is_some() { 22.0 } else { 0.0 };
        let height = 84.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);
        let inner_x = card.x + PADDING;

        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        style.text("Start the next day?", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&format!("From day {day} to day {}", day.next()), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);
        if let Some(error) = &self.error {
            style.text(error, inner_x, card.y + 84.0 - 6.0, 12, Weight::Regular, style::ERROR);
        }

        // Cancel and Start, side by side.
        let top = card.bottom() - PADDING - BUTTON_HEIGHT;
        let cancel = Rect::new(inner_x, top, (inner_w - 12.0) * 0.5, BUTTON_HEIGHT);
        let start = Rect::new(cancel.right() + 12.0, top, cancel.w, BUTTON_HEIGHT);
        let (over_cancel, over_start) = (cancel.contains(mouse), start.contains(mouse));
        style::rounded_rect(cancel, 8.0, if over_cancel { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Cancel", cancel, 14, Weight::Semibold, style::TEXT);
        style::rounded_rect(start, 8.0, if over_start { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("Start day", start, 14, Weight::Semibold, WHITE);

        // Only a click on Start day goes ahead: no key does it.
        if clicked && over_start {
            return true;
        }
        // Cancel, or a click on the dimmed window outside the card, gives up.
        if clicked && (over_cancel || !card.contains(mouse)) {
            self.day = None;
        }
        false
    }
}
