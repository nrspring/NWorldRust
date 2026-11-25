//! The person popup, opened by clicking a person who is already picked: what there is to know of them (their name, their title, the
//! country they belong to, and the movement points they have left today), and a button to end their moves for the day. They belong to
//! a country for good, so there is no changing that here.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::ids::PersonId;
use crate::movement::DAILY_POINTS;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player changed about the person in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The person spends what is left of their movement points today.
    EndMoves(PersonId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::EndMoves(id) => day.end_person_moves(*id),
        }
    }
}

#[derive(Default)]
pub struct PersonPopup {
    /// The person being shown, while the popup is up.
    person: Option<PersonId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl PersonPopup {
    pub fn is_open(&self) -> bool {
        self.person.is_some()
    }

    pub fn open(&mut self, person: PersonId) {
        *self = Self { person: Some(person), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the popup.
    pub fn escape(&mut self) {
        self.person = None;
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A person who is not in `day` is not shown. Returns
    /// what the player changed, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Change> {
        let Some(person) = self.person.and_then(|id| day.person(id)) else {
            self.person = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + 16.0 + error_height + BUTTON_HEIGHT + 10.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little badge in the color of the country.
        let country = day.country(person.country);
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, country.map_or(style::ACCENT_SOFT, |c| c.color.color()));
        let text_x = badge.right() + 14.0;
        style.text(&person.name, text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&person.title, text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..3).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in [rows[0].y, rows[1].y, rows[2].y, rows[2].bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }
        let value_row = |label: &str, value: &str, row: Rect, swatch: Option<Color>, color: Color| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, color);
            if let Some(swatch) = swatch {
                style::rounded_rect(Rect::new(x - 18.0, row.center().y - 6.0, 12.0, 12.0), 3.0, swatch);
            }
        };
        value_row("Country", country.map_or("an unknown country", |c| c.name.as_str()), rows[0], country.map(|c| c.color.color()), style::TEXT);
        value_row("Title", &person.title, rows[1], None, style::TEXT);
        let moves = format!("{} of {} points left", person.points, DAILY_POINTS);
        value_row("Movement", &moves, rows[2], None, if person.points == 0 { style::TEXT_MUTED } else { style::TEXT });

        if let Some(error) = &self.error {
            style.text(error, inner_x, rows[2].bottom() + 22.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that ends their moves for the day, which only works while they have some left.
        let end = Rect::new(inner_x, rows[2].bottom() + 16.0 + error_height, inner_w, BUTTON_HEIGHT);
        let can_end = person.points > 0;
        let over_end = can_end && end.contains(mouse);
        style::rounded_rect(end, 8.0, if over_end { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("End moves for the day", end, 14, Weight::Semibold, if can_end { style::TEXT } else { style::TEXT_MUTED });
        if clicked && over_end {
            self.error = None;
            change = Some(Change::EndMoves(person.id));
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && (enter || (clicked && (over_close || !card.contains(mouse)))) {
            self.person = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_it() {
        assert!(!PersonPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = PersonPopup { person: Some(PersonId::new()), ..PersonPopup::default() };
        assert!(popup.is_open());
        popup.fail("no".to_owned());
        assert_eq!(popup.error.as_deref(), Some("no"));
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn ending_their_moves_does_what_it_says_to_the_day() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        Change::EndMoves(id).apply(&mut day).unwrap();
        assert_eq!(day.people[0].points, 0);
        assert!(Change::EndMoves(PersonId::new()).apply(&mut day).is_err(), "they are not there");
    }
}
