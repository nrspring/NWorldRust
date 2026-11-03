//! The number fields, sliders and buttons the panels are made of, and how they are drawn.

use super::*;

pub(super) struct NumberField {
    pub(super) label: &'static str,
    pub(super) text: String,
    /// Shown inside the field on the right, e.g. "m".
    pub(super) unit: &'static str,
    pub(super) min: u32,
    pub(super) max_digits: usize,
    /// The whole value is selected, so the next digit typed replaces it.
    pub(super) selected: bool,
    pub(super) invalid: bool,
}

impl NumberField {
    pub(super) fn new(label: &'static str, value: u32, unit: &'static str, min: u32, max_digits: usize) -> Self {
        Self {
            label,
            text: value.to_string(),
            unit,
            min,
            max_digits,
            selected: false,
            invalid: false,
        }
    }

    pub(super) fn value(&self) -> Option<u32> {
        self.text.parse().ok().filter(|&v| v >= self.min)
    }
}

/// A whole-number value picked by dragging along a track.
pub(super) struct Slider {
    pub(super) label: &'static str,
    pub(super) value: f32,
    pub(super) min: f32,
    pub(super) max: f32,
    pub(super) unit: &'static str,
    /// Short captions for the two ends of the track, for sliders whose direction isn't obvious.
    pub(super) ends: Option<(&'static str, &'static str)>,
}

impl Slider {
    pub(super) fn fraction(&self) -> f32 {
        (self.value - self.min) / (self.max - self.min)
    }

    pub(super) fn set_from_x(&mut self, track: Rect, x: f32) {
        let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
        self.value = (self.min + t * (self.max - self.min)).round();
    }
}

pub(super) fn draw_field(style: &Style, r: Rect, field: &NumberField, focused: bool, mouse: Vec2) {
    if focused {
        // Soft focus ring around the field.
        let ring = Rect::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0);
        style::rounded_rect(ring, CONTROL_RADIUS + 3.0, style::ACCENT_SOFT);
    }
    style::rounded_rect(r, CONTROL_RADIUS, style::FIELD);
    let border = if field.invalid {
        style::ERROR
    } else if focused {
        style::ACCENT
    } else if r.contains(mouse) {
        style::BORDER_STRONG
    } else {
        style::BORDER
    };
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, border);

    let text_x = r.x + 12.0;
    let mid = r.center().y;
    let text_w = style.measure(&field.text, 14, Weight::Regular).width;
    if focused && field.selected && !field.text.is_empty() {
        let sel = Rect::new(text_x - 3.0, mid - 10.0, text_w + 6.0, 20.0);
        style::rounded_rect(sel, 3.0, Color::new(0.302, 0.553, 1.0, 0.45));
    }
    style.text_mid(&field.text, text_x, mid, 14, Weight::Regular, style::TEXT);
    let caret_on = (get_time() * 2.0) as i64 % 2 == 0;
    if focused && !field.selected && caret_on {
        let cx = (text_x + text_w + 1.5).round();
        draw_line(cx, mid - 8.0, cx, mid + 8.0, 1.5, style::TEXT);
    }
    if !field.unit.is_empty() {
        let unit_w = style.measure(field.unit, 13, Weight::Regular).width;
        style.text_mid(field.unit, r.right() - 12.0 - unit_w, mid, 13, Weight::Regular, style::TEXT_MUTED);
    }
}

/// Label and value above, track and knob below; `r` is the draggable area.
pub(super) fn draw_slider(style: &Style, r: Rect, slider: &Slider, dragging: bool, mouse: Vec2) {
    let label_y = r.y - 6.0;
    style.text(slider.label, r.x, label_y, 13, Weight::Regular, style::TEXT);
    let value = format!("{:.0}{}", slider.value, slider.unit);
    let value_w = style.measure(&value, 13, Weight::Semibold).width;
    style.text(&value, r.right() - value_w, label_y, 13, Weight::Semibold, style::TEXT);

    let mid = r.center().y;
    let track = Rect::new(r.x, mid - 2.0, r.w, 4.0);
    style::rounded_rect(track, 2.0, Color::new(1.0, 1.0, 1.0, 0.12));
    let knob_x = r.x + r.w * slider.fraction();
    style::rounded_rect(Rect::new(r.x, track.y, knob_x - r.x, 4.0), 2.0, style::ACCENT);

    let hovered = dragging || r.contains(mouse);
    if hovered {
        draw_circle(knob_x, mid, 11.0, style::ACCENT_SOFT);
    }
    draw_circle(knob_x, mid, 7.0, WHITE);
    draw_circle_lines(knob_x, mid, 7.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.25));

    if let Some((left, right)) = slider.ends {
        // Right under the track, well clear of the next slider's label.
        let y = r.bottom() + 5.0;
        style.text(left, r.x, y, 11, Weight::Regular, style::TEXT_MUTED);
        let w = style.measure(right, 11, Weight::Regular).width;
        style.text(right, r.right() - w, y, 11, Weight::Regular, style::TEXT_MUTED);
    }
}

/// A "−  value  +" control for small whole numbers.
pub(super) fn draw_stepper(style: &Style, l: &ContinentsLayout, value: u32, mouse: Vec2) {
    let r = l.count;
    style::rounded_rect(r, CONTROL_RADIUS, style::FIELD);
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, style::BORDER);
    for (b, enabled) in [(l.minus, value > 1), (l.plus, value < MAX_CONTINENTS)] {
        if enabled && b.contains(mouse) {
            style::rounded_rect(b, CONTROL_RADIUS, style::HOVER);
        }
        let color = if enabled { style::TEXT } else { Color::new(1.0, 1.0, 1.0, 0.2) };
        let c = b.center();
        draw_line(c.x - 5.0, c.y, c.x + 5.0, c.y, 1.6, color);
        if b == l.plus {
            draw_line(c.x, c.y - 5.0, c.x, c.y + 5.0, 1.6, color);
        }
    }
    style.text_centered(&value.to_string(), r, 14, Weight::Semibold, style::TEXT);
}

pub(super) fn draw_primary_button(style: &Style, r: Rect, label: &str, enabled: bool, mouse: Vec2) {
    let (fill, text) = if !enabled {
        (Color::new(1.0, 1.0, 1.0, 0.08), Color::new(1.0, 1.0, 1.0, 0.35))
    } else if r.contains(mouse) {
        (style::ACCENT_HOVER, WHITE)
    } else {
        (style::ACCENT, WHITE)
    };
    style::rounded_rect(r, CONTROL_RADIUS, fill);
    style.text_centered(label, r, 14, Weight::Semibold, text);
}

pub(super) fn draw_secondary_button(style: &Style, r: Rect, label: &str, enabled: bool, mouse: Vec2) {
    let text = if enabled { style::TEXT } else { Color::new(1.0, 1.0, 1.0, 0.35) };
    let fill = if enabled && r.contains(mouse) { Color::new(1.0, 1.0, 1.0, 0.10) } else { style::FIELD };
    style::rounded_rect(r, CONTROL_RADIUS, fill);
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, style::BORDER_STRONG);
    style.text_centered(label, r, 14, Weight::Semibold, text);
}

pub(super) fn draw_icon_button(r: Rect, mouse: Vec2) {
    let fill = if r.contains(mouse) { Color::new(1.0, 1.0, 1.0, 0.10) } else { style::FIELD };
    style::rounded_rect(r, CONTROL_RADIUS, fill);
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, style::BORDER);
}

pub(super) fn draw_tooltip(style: &Style, text: &str, x: f32, center_y: f32) {
    let w = style.measure(text, 13, Weight::Regular).width + 20.0;
    let r = Rect::new(x, center_y - 14.0, w, 28.0);
    style::card(r, 6.0, Color::new(0.07, 0.07, 0.08, 1.0));
    style.text_mid(text, r.x + 10.0, r.center().y, 13, Weight::Regular, style::TEXT);
}
