//! The popups that ask the player for text: the name of a new country, and the name and title of a
//! new person. One popup has one or more fields, and returns what was typed in all of them.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::{country, person, text};

const CARD_WIDTH: f32 = 520.0;
const PADDING: f32 = 32.0;
const FIELD_HEIGHT: f32 = 52.0;
/// The distance from the top of one field to the top of the next.
const FIELD_PITCH: f32 = 92.0;
const BUTTON_HEIGHT: f32 = 52.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;
/// How long Backspace is held before it starts repeating, and the gap between repeats, in seconds.
const REPEAT_DELAY: f32 = 0.4;
const REPEAT_EVERY: f32 = 0.04;

/// Checks what was typed, one string for each field, and gives it back tidied up, or says what is
/// wrong with it.
type Check = fn(&[String]) -> Result<Vec<String>, String>;

#[derive(Clone, Copy)]
enum Icon {
    Flag,
    Person,
}

struct Field {
    label: &'static str,
    placeholder: &'static str,
    max_chars: usize,
    value: String,
}

impl Field {
    fn new(label: &'static str, placeholder: &'static str, max_chars: usize) -> Self {
        Self { label, placeholder, max_chars, value: String::new() }
    }
}

pub struct Popup {
    heading: &'static str,
    subtitle: String,
    icon: Icon,
    button: &'static str,
    fields: Vec<Field>,
    check: Check,
    /// The field being typed in.
    focus: usize,
    error: Option<String>,
    opened_at: f64,
    /// How long Backspace has been held, for repeating it.
    backspace_held: f32,
}

fn check_country(typed: &[String]) -> Result<Vec<String>, String> {
    Ok(vec![country::clean_name(&typed[0])?])
}

fn check_person(typed: &[String]) -> Result<Vec<String>, String> {
    Ok(vec![person::clean_name(&typed[0])?, person::clean_title(&typed[1])?])
}

impl Popup {
    /// Asks for the name of a new country.
    pub fn country() -> Self {
        Self::new(
            "Found your nation",
            "Choose a name for your new country.".to_owned(),
            Icon::Flag,
            "Found country",
            vec![Field::new("COUNTRY NAME", "e.g. Aldoria", country::MAX_NAME_CHARS)],
            check_country,
        )
    }

    /// Asks for the name and title of a new person of `country`.
    pub fn person(country: &str) -> Self {
        Self::new(
            "Introduce a person",
            format!("They will belong to {country}."),
            Icon::Person,
            "Add person",
            vec![
                Field::new("NAME", "e.g. Mara Voss", person::MAX_NAME_CHARS),
                Field::new("TITLE", "e.g. President", person::MAX_TITLE_CHARS),
            ],
            check_person,
        )
    }

    fn new(heading: &'static str, subtitle: String, icon: Icon, button: &'static str, fields: Vec<Field>, check: Check) -> Self {
        // Keys pressed before the popup was up are not part of what is typed.
        while get_char_pressed().is_some() {}
        Self { heading, subtitle, icon, button, fields, check, focus: 0, error: None, opened_at: get_time(), backspace_held: 0.0 }
    }

    /// Shows why what was typed couldn't be used, which is something only the caller finds out.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    fn typed(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.value.clone()).collect()
    }

    /// Takes the keyboard and mouse for this frame and draws the popup over everything. Returns
    /// what was typed in each field when it is given.
    pub fn show(&mut self, style: &Style) -> Option<Vec<String>> {
        self.type_in();
        let checked = (self.check)(&self.typed());

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let fields_end = 180.0 + FIELD_PITCH * (self.fields.len() - 1) as f32;
        let height = fields_end + 130.0;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 48.0, 48.0);
        style::rounded_rect(badge, 12.0, style::ACCENT_SOFT);
        draw_icon(self.icon, badge);
        let text_x = badge.right() + 16.0;
        style.text(self.heading, text_x, card.y + PADDING + 22.0, 26, Weight::Semibold, style::TEXT);
        style.text(&self.subtitle, text_x, card.y + PADDING + 46.0, 15, Weight::Regular, style::TEXT_MUTED);

        // The fields.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let mouse = Vec2::from(mouse_position());
        for (i, field) in self.fields.iter().enumerate() {
            let top = card.y + 128.0 + FIELD_PITCH * i as f32;
            style.text(field.label, inner_x, top - 10.0, 12, Weight::Semibold, style::TEXT_MUTED);
            let rect = Rect::new(inner_x, top, inner_w, FIELD_HEIGHT);
            if is_mouse_button_pressed(MouseButton::Left) && rect.contains(mouse) {
                self.focus = i;
            }
            draw_field(style, rect, field, i == self.focus);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, card.y + fields_end + 24.0, 14, Weight::Regular, style::ERROR);
        }

        // The button.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let hovered = checked.is_ok() && button.contains(mouse);
        let (fill, label) = match (checked.is_ok(), hovered) {
            (false, _) => (Color::new(1.0, 1.0, 1.0, 0.06), style::TEXT_MUTED),
            (true, false) => (style::ACCENT, WHITE),
            (true, true) => (style::ACCENT_HOVER, WHITE),
        };
        style::rounded_rect(button, 10.0, fill);
        style.text_centered(self.button, button, 17, Weight::Semibold, label);
        if checked.is_ok() {
            let cap = Rect::new(button.right() - 68.0, button.center().y - 10.0, 48.0, 20.0);
            style::rounded_rect(cap, 5.0, Color::new(1.0, 1.0, 1.0, 0.22));
            style.text_centered("Enter", cap, 12, Weight::Semibold, WHITE);
        }

        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if (hovered && is_mouse_button_pressed(MouseButton::Left)) || (enter && checked.is_ok()) {
            return checked.ok();
        }
        if enter {
            // Not finished: go on to the next field, or say what is missing.
            if self.focus + 1 < self.fields.len() {
                self.focus += 1;
            } else if let Err(message) = checked {
                self.error = Some(message);
            }
        }
        None
    }

    /// Typing, Backspace, and moving between fields.
    fn type_in(&mut self) {
        let last = self.fields.len() - 1;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Down) || (is_key_pressed(KeyCode::Tab) && !shift) {
            self.focus = if self.focus == last { 0 } else { self.focus + 1 };
        } else if is_key_pressed(KeyCode::Up) || (is_key_pressed(KeyCode::Tab) && shift) {
            self.focus = if self.focus == 0 { last } else { self.focus - 1 };
        }

        let field = &mut self.fields[self.focus];
        while let Some(c) = get_char_pressed() {
            if text::is_text_char(c) && field.value.chars().count() < field.max_chars {
                field.value.push(c);
                self.error = None;
            }
        }
        if is_key_down(KeyCode::Backspace) {
            let before = self.backspace_held;
            self.backspace_held += get_frame_time();
            let presses = |held: f32| if held < REPEAT_DELAY { 0 } else { 1 + ((held - REPEAT_DELAY) / REPEAT_EVERY) as u32 };
            let deletes = if before == 0.0 { 1 } else { presses(self.backspace_held) - presses(before) };
            for _ in 0..deletes {
                field.value.pop();
            }
            self.error = None;
        } else {
            self.backspace_held = 0.0;
        }
    }
}

/// A field: what was typed, or a hint when there is nothing, with a blinking caret when it has the
/// keyboard.
fn draw_field(style: &Style, rect: Rect, field: &Field, focused: bool) {
    style::rounded_rect(rect, 10.0, style::FIELD);
    if focused {
        style::rounded_rect_lines(rect, 10.0, 2.0, style::ACCENT);
    } else {
        style::rounded_rect_lines(rect, 10.0, 1.0, style::BORDER_STRONG);
    }

    let inner = Rect::new(rect.x + 16.0, rect.y, rect.w - 32.0, rect.h);
    if field.value.is_empty() {
        style.text_mid(field.placeholder, inner.x, inner.center().y, 20, Weight::Regular, Color::new(1.0, 1.0, 1.0, 0.28));
    }
    // Text wider than the field shows its end, where the typing is.
    let mut shown: &str = &field.value;
    while style.measure(shown, 20, Weight::Regular).width > inner.w - 4.0 {
        let mut chars = shown.chars();
        chars.next();
        shown = chars.as_str();
    }
    style.text_mid(shown, inner.x, inner.center().y, 20, Weight::Regular, style::TEXT);
    if focused && (get_time() * 1.6) as i64 % 2 == 0 {
        let end = inner.x + style.measure(shown, 20, Weight::Regular).width + 2.0;
        draw_rectangle(end, rect.center().y - 11.0, 2.0, 22.0, style::ACCENT);
    }
}

/// A small picture inside `badge`.
fn draw_icon(icon: Icon, badge: Rect) {
    let c = badge.center();
    match icon {
        Icon::Flag => {
            let (pole_x, top, bottom) = (c.x - 8.0, c.y - 13.0, c.y + 13.0);
            draw_line(pole_x, top, pole_x, bottom, 2.5, style::ACCENT);
            draw_triangle(vec2(pole_x, top), vec2(pole_x + 19.0, top + 6.0), vec2(pole_x, top + 12.0), style::ACCENT);
        }
        Icon::Person => {
            draw_circle(c.x, c.y - 6.0, 7.0, style::ACCENT);
            style::rounded_rect(Rect::new(c.x - 12.0, c.y + 4.0, 24.0, 14.0), 7.0, style::ACCENT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn the_country_check_tidies_the_name() {
        assert_eq!(check_country(&typed(&["  Old  Aldoria "])).unwrap(), typed(&["Old Aldoria"]));
        assert!(check_country(&typed(&[" "])).is_err());
    }

    #[test]
    fn the_person_check_needs_both_a_name_and_a_title() {
        assert_eq!(check_person(&typed(&[" Mara  Voss ", "President"])).unwrap(), typed(&["Mara Voss", "President"]));
        assert!(check_person(&typed(&["Mara Voss", ""])).unwrap_err().contains("title"));
        assert!(check_person(&typed(&["", "President"])).unwrap_err().contains("name"));
    }
}
