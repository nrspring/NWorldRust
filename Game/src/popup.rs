//! The popups that ask the player for text: the name of a new country, the name of a city, and the name,
//! title and kind of title of a new person. One popup has one or more fields, and returns what was
//! typed in all of them.
//!
//! A field can be a drop-down: a text box with a list of choices under it. A choice is picked from the
//! list, or something that is not in it is typed, and the caller makes a new choice of it.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::{city, country, person, text, title_type};

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
/// The height of a row of a drop-down list, and the most rows it shows before it scrolls.
const ROW_HEIGHT: f32 = 34.0;
const LIST_PADDING: f32 = 6.0;
const MAX_ROWS: usize = 7;

/// Checks what was typed, one string for each field, and gives it back tidied up, or says what is
/// wrong with it.
type Check = fn(&[String]) -> Result<Vec<String>, String>;

#[derive(Clone, Copy)]
pub enum Icon {
    Flag,
    Person,
    City,
}

struct Field {
    label: &'static str,
    placeholder: &'static str,
    max_chars: usize,
    value: String,
    /// Whether it is a drop-down, a text field that also has a list of choices to pick from, and
    /// what is typed in it that isn't one of them is a new choice.
    drop_down: bool,
    /// The choices of a drop-down. There may be none yet.
    options: Vec<String>,
}

impl Field {
    fn new(label: &'static str, placeholder: &'static str, max_chars: usize) -> Self {
        Self { label, placeholder, max_chars, value: String::new(), drop_down: false, options: Vec::new() }
    }

    /// A drop-down with these choices, of which there may be none yet.
    fn choice(label: &'static str, placeholder: &'static str, max_chars: usize, options: Vec<String>) -> Self {
        Self { drop_down: true, options, ..Self::new(label, placeholder, max_chars) }
    }

    fn is_choice(&self) -> bool {
        self.drop_down
    }

    /// Whether there is a list to show: it is a drop-down with a choice in it.
    fn has_list(&self) -> bool {
        self.drop_down && !self.options.is_empty()
    }

    /// Whether what is typed is not one of the choices, so it would be a new one.
    fn is_new_choice(&self) -> bool {
        let typed = self.value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        self.is_choice() && !typed.is_empty() && !self.options.iter().any(|o| o.to_lowercase() == typed)
    }
}

/// The choices that have `typed` in them, whatever the capitals, as their places in `options`; all of
/// them if nothing is typed.
fn matching(options: &[String], typed: &str) -> Vec<usize> {
    let typed = typed.trim().to_lowercase();
    (0..options.len()).filter(|&i| options[i].to_lowercase().contains(&typed)).collect()
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
    /// Whether the list of the field being typed in, if it is a drop-down, is showing, which row of what
    /// it lists is chosen with the arrow keys, if one is, and how far down the list is scrolled.
    open: bool,
    highlight: Option<usize>,
    scroll: usize,
    error: Option<String>,
    opened_at: f64,
    /// How long Backspace has been held, for repeating it.
    backspace_held: f32,
}

fn check_country(typed: &[String]) -> Result<Vec<String>, String> {
    Ok(vec![country::clean_name(&typed[0])?])
}

fn check_city(typed: &[String]) -> Result<Vec<String>, String> {
    Ok(vec![city::clean_name(&typed[0])?])
}

fn check_person(typed: &[String]) -> Result<Vec<String>, String> {
    Ok(vec![person::clean_name(&typed[0])?, person::clean_title(&typed[1])?, title_type::clean_name(&typed[2])?])
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

    /// Asks for a new name for the country now called `current`, which is what the name starts as.
    pub fn rename_country(current: &str) -> Self {
        let mut popup = Self::new(
            "Edit country",
            format!("Change the name of {current}."),
            Icon::Flag,
            "Save name",
            vec![Field::new("COUNTRY NAME", "e.g. Aldoria", country::MAX_NAME_CHARS)],
            check_country,
        );
        popup.fields[0].value = current.to_owned();
        popup
    }

    /// Asks for the name of the city being founded.
    pub fn city() -> Self {
        Self::new(
            "Name your city",
            "Choose a name for the city you are founding.".to_owned(),
            Icon::City,
            "Found city",
            vec![Field::new("CITY NAME", "e.g. Port Aldor", city::MAX_NAME_CHARS)],
            check_city,
        )
    }

    /// Asks for the name, the title and the kind of title of a new person of `country`. The kind is
    /// chosen from `title_types`, or a new one is typed.
    pub fn person(country: &str, title_types: Vec<String>) -> Self {
        Self::new(
            "Introduce a person",
            format!("They will belong to {country}."),
            Icon::Person,
            "Add person",
            vec![
                Field::new("NAME", "e.g. Mara Voss", person::MAX_NAME_CHARS),
                Field::new("TITLE", "e.g. President", person::MAX_TITLE_CHARS),
                // There are none until the first is typed.
                Field::choice("TITLE TYPE", if title_types.is_empty() { "Type a title type, e.g. Monarch" } else { "Choose one, or type a new one" }, title_type::MAX_NAME_CHARS, title_types),
            ],
            check_person,
        )
    }

    fn new(heading: &'static str, subtitle: String, icon: Icon, button: &'static str, fields: Vec<Field>, check: Check) -> Self {
        // Keys pressed before the popup was up are not part of what is typed.
        while get_char_pressed().is_some() {}
        Self { heading, subtitle, icon, button, fields, check, focus: 0, open: false, highlight: None, scroll: 0, error: None, opened_at: get_time(), backspace_held: 0.0 }
    }

    /// Shows why what was typed couldn't be used, which is something only the caller finds out.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    fn typed(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.value.clone()).collect()
    }

    /// Closes the drop-down list if one is showing, and says whether one was. Esc does this before it
    /// does anything else.
    pub fn close_dropdown(&mut self) -> bool {
        let was_open = self.open;
        self.open = false;
        self.highlight = None;
        was_open
    }

    /// The choices of the field being typed in that are showing in its list, as their places in its
    /// options; none if it isn't a drop-down or its list is closed.
    fn listed(&self) -> Vec<usize> {
        let field = &self.fields[self.focus];
        if self.open && field.has_list() { matching(&field.options, &field.value) } else { Vec::new() }
    }

    /// Puts the choice in place `option` of the field being typed in as what is typed, and closes
    /// its list.
    fn pick(&mut self, option: usize) {
        let field = &mut self.fields[self.focus];
        field.value = field.options[option].clone();
        self.open = false;
        self.highlight = None;
        self.error = None;
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

        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let rects: Vec<Rect> = (0..self.fields.len()).map(|i| Rect::new(inner_x, card.y + 128.0 + FIELD_PITCH * i as f32, inner_w, FIELD_HEIGHT)).collect();

        // The list under a drop-down, if it is showing. A click on it picks a row, and nothing under
        // it gets that click.
        let listed = self.listed();
        let rows = listed.len().min(MAX_ROWS);
        let list = (rows > 0).then(|| {
            let field = rects[self.focus];
            Rect::new(field.x, field.bottom() + 4.0, field.w, rows as f32 * ROW_HEIGHT + LIST_PADDING * 2.0)
        });
        let over_list = list.is_some_and(|l| l.contains(mouse));
        if let Some(list) = list {
            // Keep the chosen row in view, and let the wheel scroll.
            if let Some(h) = self.highlight {
                self.scroll = self.scroll.clamp((h + 1).saturating_sub(rows), h);
            }
            if over_list && mouse_wheel().1 != 0.0 {
                self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
            }
            self.scroll = self.scroll.min(listed.len() - rows);
            if clicked && over_list {
                let row = ((mouse.y - list.y - LIST_PADDING) / ROW_HEIGHT).floor();
                if row >= 0.0 && (row as usize) < rows {
                    self.pick(listed[self.scroll + row as usize]);
                }
            }
        }

        // A click on a field makes it the one being typed in, and opens or closes the list of a
        // drop-down; a click anywhere else closes the list.
        if clicked && !over_list {
            if let Some(i) = rects.iter().position(|r| r.contains(mouse)) {
                let was_typing_here = i == self.focus;
                if !was_typing_here {
                    self.focus = i;
                    self.open = false;
                }
                if self.fields[i].has_list() {
                    self.open = if was_typing_here { !self.open } else { true };
                    self.highlight = None;
                    self.scroll = 0;
                }
            } else {
                self.open = false;
            }
        }

        for (i, field) in self.fields.iter().enumerate() {
            style.text(field.label, inner_x, rects[i].y - 10.0, 12, Weight::Semibold, style::TEXT_MUTED);
            draw_field(style, rects[i], field, i == self.focus, i == self.focus && self.open);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, card.y + fields_end + 24.0, 14, Weight::Regular, style::ERROR);
        }

        // The button.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let hovered = checked.is_ok() && button.contains(mouse) && !over_list;
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

        // The list goes over everything else.
        if let Some(list) = list {
            let options = &self.fields[self.focus].options;
            style::card(list, 10.0, style::SURFACE);
            style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
            for row in 0..rows {
                let place = self.scroll + row;
                let rect = Rect::new(list.x + 6.0, list.y + LIST_PADDING + row as f32 * ROW_HEIGHT, list.w - 12.0, ROW_HEIGHT);
                if self.highlight == Some(place) {
                    style::rounded_rect(rect, 7.0, style::ACCENT_SOFT);
                } else if rect.contains(mouse) {
                    style::rounded_rect(rect, 7.0, style::HOVER);
                }
                style.text_mid(&options[listed[place]], rect.x + 10.0, rect.center().y, 16, Weight::Regular, style::TEXT);
            }
            // More above or below than shows: a small mark at the edge.
            if self.scroll > 0 {
                draw_triangle(vec2(list.right() - 18.0, list.y + 10.0), vec2(list.right() - 10.0, list.y + 10.0), vec2(list.right() - 14.0, list.y + 5.0), style::TEXT_MUTED);
            }
            if self.scroll + rows < listed.len() {
                draw_triangle(vec2(list.right() - 18.0, list.bottom() - 10.0), vec2(list.right() - 10.0, list.bottom() - 10.0), vec2(list.right() - 14.0, list.bottom() - 5.0), style::TEXT_MUTED);
            }
        }

        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if enter && self.open {
            // Enter on a chosen row picks it, and doesn't also finish the form.
            if let Some(&option) = self.highlight.and_then(|h| listed.get(h)) {
                self.pick(option);
                return None;
            }
        }
        if (hovered && clicked) || (enter && checked.is_ok()) {
            return checked.ok();
        }
        if enter {
            // Not finished: go on to the next field, or say what is missing.
            if self.focus + 1 < self.fields.len() {
                self.focus += 1;
                self.open = false;
            } else if let Err(message) = checked {
                self.error = Some(message);
            }
        }
        None
    }

    /// Typing, Backspace, and moving between fields and through a list.
    fn type_in(&mut self) {
        let last = self.fields.len() - 1;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let (down, up, tab) = (is_key_pressed(KeyCode::Down), is_key_pressed(KeyCode::Up), is_key_pressed(KeyCode::Tab));

        // In a drop-down, the arrow keys open the list and go through it.
        let field = &self.fields[self.focus];
        let in_list = field.has_list() && (down || (up && self.open));
        if in_list {
            let count = matching(&field.options, &field.value).len();
            if !self.open {
                self.open = true;
                self.highlight = None;
                self.scroll = 0;
            } else if down {
                self.highlight = Some(self.highlight.map_or(0, |h| h + 1).min(count.saturating_sub(1)));
            } else {
                self.highlight = self.highlight.and_then(|h| h.checked_sub(1));
            }
        } else if down || (tab && !shift) {
            self.focus = if self.focus == last { 0 } else { self.focus + 1 };
            self.open = false;
        } else if up || (tab && shift) {
            self.focus = if self.focus == 0 { last } else { self.focus - 1 };
            self.open = false;
        }

        let field = &mut self.fields[self.focus];
        let mut changed = false;
        while let Some(c) = get_char_pressed() {
            if text::is_text_char(c) && field.value.chars().count() < field.max_chars {
                field.value.push(c);
                self.error = None;
                changed = true;
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
            changed = true;
        } else {
            self.backspace_held = 0.0;
        }
        // Typing in a drop-down narrows its list, so show it, with nothing chosen yet.
        if changed && field.has_list() {
            self.open = true;
            self.highlight = None;
            self.scroll = 0;
        }
    }
}

/// A field: what was typed, or a hint when there is nothing, with a blinking caret when it has the
/// keyboard.
fn draw_field(style: &Style, rect: Rect, field: &Field, focused: bool, open: bool) {
    style::rounded_rect(rect, 10.0, style::FIELD);
    if focused {
        style::rounded_rect_lines(rect, 10.0, 2.0, style::ACCENT);
    } else {
        style::rounded_rect_lines(rect, 10.0, 1.0, style::BORDER_STRONG);
    }

    let mut inner = Rect::new(rect.x + 16.0, rect.y, rect.w - 32.0, rect.h);
    if field.is_choice() {
        // A mark at the right that it has a list, pointing up when the list is open, and, when what is
        // typed is not in the list, a tag that it would be a new choice.
        let (cx, cy) = (rect.right() - 26.0, rect.center().y);
        let tag_right = if field.has_list() {
            if open {
                draw_triangle(vec2(cx - 6.0, cy + 3.0), vec2(cx + 6.0, cy + 3.0), vec2(cx, cy - 4.0), style::TEXT_MUTED);
            } else {
                draw_triangle(vec2(cx - 6.0, cy - 3.0), vec2(cx + 6.0, cy - 3.0), vec2(cx, cy + 4.0), style::TEXT_MUTED);
            }
            inner.w -= 36.0;
            rect.right() - 36.0
        } else {
            rect.right() - 12.0
        };
        if field.is_new_choice() {
            let tag = Rect::new(tag_right - 58.0, cy - 11.0, 54.0, 22.0);
            style::rounded_rect(tag, 11.0, style::ACCENT_SOFT);
            style.text_centered("New", tag, 12, Weight::Semibold, style::ACCENT_HOVER);
            inner.w -= 62.0;
        }
    }
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
pub fn draw_icon(icon: Icon, badge: Rect) {
    let c = badge.center();
    match icon {
        Icon::Flag => {
            let (pole_x, top, bottom) = (c.x - 8.0, c.y - 13.0, c.y + 13.0);
            draw_line(pole_x, top, pole_x, bottom, 2.5, style::ACCENT);
            draw_triangle(vec2(pole_x, top), vec2(pole_x + 19.0, top + 6.0), vec2(pole_x, top + 12.0), style::ACCENT);
        }
        Icon::City => {
            // A little town: three houses of different heights on a line.
            for (dx, w, h) in [(-11.0, 9.0, 12.0), (-1.5, 10.0, 19.0), (9.0, 8.0, 9.0)] {
                style::rounded_rect(Rect::new(c.x + dx, c.y + 13.0 - h, w, h), 1.5, style::ACCENT);
            }
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
    fn the_city_check_tidies_the_name() {
        assert_eq!(check_city(&typed(&["  Port  Aldor "])).unwrap(), typed(&["Port Aldor"]));
        assert!(check_city(&typed(&[""])).is_err());
    }

    #[test]
    fn the_person_check_needs_a_name_a_title_and_a_kind_of_title() {
        assert_eq!(check_person(&typed(&[" Mara  Voss ", "President", "Head  of State"])).unwrap(), typed(&["Mara Voss", "President", "Head of State"]));
        assert!(check_person(&typed(&["Mara Voss", "", "Head of State"])).unwrap_err().contains("title"));
        assert!(check_person(&typed(&["", "President", "Head of State"])).unwrap_err().contains("name"));
        assert!(check_person(&typed(&["Mara Voss", "President", " "])).unwrap_err().contains("title type"));
    }

    fn kinds() -> Vec<String> {
        typed(&["Monarch", "Nobility", "Head of State", "Head of Government"])
    }

    #[test]
    fn a_drop_down_lists_everything_until_something_is_typed() {
        assert_eq!(matching(&kinds(), ""), [0, 1, 2, 3]);
        assert_eq!(matching(&kinds(), "  "), [0, 1, 2, 3]);
    }

    #[test]
    fn typing_narrows_the_list_to_what_has_it_in_whatever_the_capitals() {
        assert_eq!(matching(&kinds(), "head"), [2, 3]);
        assert_eq!(matching(&kinds(), "OF ST"), [2]);
        assert_eq!(matching(&kinds(), "xyz"), Vec::<usize>::new());
    }

    #[test]
    fn what_is_typed_is_a_new_choice_only_if_it_is_not_in_the_list() {
        let field = |value: &str| Field { value: value.to_owned(), ..Field::choice("KIND", "", 32, kinds()) };
        assert!(!field("").is_new_choice());
        assert!(!field("monarch").is_new_choice());
        assert!(!field("  Head   of  State ").is_new_choice());
        assert!(field("Sea Priest").is_new_choice());
        assert!(field("Head").is_new_choice(), "part of a choice is not a choice");
        // A plain text field has no choices to be new among.
        assert!(!Field::new("NAME", "", 32).is_new_choice());
    }

    #[test]
    fn a_drop_down_with_nothing_to_choose_from_yet_still_takes_a_new_choice() {
        let field = Field { value: "Monarch".to_owned(), ..Field::choice("KIND", "", 32, Vec::new()) };
        assert!(!field.has_list(), "there is no list to open");
        assert!(field.is_new_choice());
        assert!(!Field { value: "".to_owned(), ..field }.is_new_choice());
    }
}
