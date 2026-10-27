//! Short messages that appear at the top of the view and fade away on their own.

use macroquad::prelude::*;

use crate::style::{self, Style, Weight};

const INFO_SECONDS: f64 = 3.5;
const ERROR_SECONDS: f64 = 8.0;

struct Toast {
    text: String,
    error: bool,
    shown_at: f64,
}

#[derive(Default)]
pub struct Toasts {
    current: Option<Toast>,
}

impl Toasts {
    pub fn info(&mut self, text: impl Into<String>) {
        self.show(text.into(), false);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.show(text.into(), true);
    }

    fn show(&mut self, text: String, error: bool) {
        self.current = Some(Toast {
            text,
            error,
            shown_at: get_time(),
        });
    }

    /// Draws the current message, centered along the top of `viewport`.
    pub fn draw(&mut self, style: &Style, viewport: Rect) {
        let Some(toast) = &self.current else {
            return;
        };
        let lifetime = if toast.error { ERROR_SECONDS } else { INFO_SECONDS };
        if get_time() - toast.shown_at > lifetime {
            self.current = None;
            return;
        }

        let text_w = style.measure(&toast.text, 13, Weight::Regular).width;
        let w = (text_w + 44.0).min(viewport.w - 24.0);
        let r = Rect::new(viewport.x + (viewport.w - w) * 0.5, 12.0, w, 32.0);
        style::card(r, 8.0, Color::new(0.11, 0.115, 0.13, 0.96));
        let dot = if toast.error { style::ERROR } else { style::ACCENT };
        draw_circle(r.x + 16.0, r.center().y, 4.0, dot);
        style.text_mid(&toast.text, r.x + 30.0, r.center().y, 13, Weight::Regular, style::TEXT);
    }
}
