//! The person popup, opened by clicking a person who is already picked: what there is to know of them (their name, their title, the
//! country they belong to, and the movement points they have left today), and a button to end their moves for the day. Clicking
//! their country lists the countries, and "Nomad" for none, to move them to.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::ids::{CountryId, PersonId};
use crate::movement::DAILY_POINTS;
use crate::person;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// The list of countries: the height of a row, the room round them, and the most rows before it scrolls.
const LIST_ROW: f32 = 32.0;
const LIST_PADDING: f32 = 6.0;
const MAX_LIST_ROWS: usize = 8;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player changed about the person in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The person spends what is left of their movement points today.
    EndMoves(PersonId),
    /// The person belongs to this country from now on, or to none, as a nomad.
    Country(PersonId, Option<CountryId>),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::EndMoves(id) => day.end_person_moves(*id),
            Self::Country(id, country) => day.set_person_country(*id, *country),
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
    /// Whether the list of countries is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
}

impl PersonPopup {
    pub fn is_open(&self) -> bool {
        self.person.is_some()
    }

    pub fn open(&mut self, person: PersonId) {
        *self = Self { person: Some(person), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries if it is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if !std::mem::take(&mut self.choosing) {
            self.person = None;
        }
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
        let height = ROWS_TOP + ROW_HEIGHT * 4.0 + 16.0 + error_height + BUTTON_HEIGHT + 10.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little badge in the color of the country.
        let country = person.country.and_then(|id| day.country(id));
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, country.map_or(style::ACCENT_SOFT, |c| c.color.color()));
        let text_x = badge.right() + 14.0;
        style.text(&person.name, text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&person.title, text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..4).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in [rows[0].y, rows[1].y, rows[2].y, rows[3].y, rows[3].bottom()] {
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
        // The country is a button for the list of them, which an arrow says.
        let over_country = rows[0].contains(mouse);
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(rows[0].x - 8.0, rows[0].y + 1.0, rows[0].w + 16.0, rows[0].h - 2.0), 8.0, style::HOVER);
        }
        let arrow = vec2(rows[0].right() - 4.0, rows[0].center().y);
        let tip = if self.choosing { -3.0 } else { 3.0 };
        draw_triangle(vec2(arrow.x - 4.0, arrow.y - tip * 0.66), vec2(arrow.x + 4.0, arrow.y - tip * 0.66), vec2(arrow.x, arrow.y + tip), style::TEXT_MUTED);
        let beside_arrow = Rect::new(rows[0].x, rows[0].y, rows[0].w - 14.0, rows[0].h);
        style.text_mid("Country", inner_x, rows[0].center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let name = day.person_country_name(person);
        let name_x = beside_arrow.right() - style.measure(name, 14, Weight::Semibold).width;
        style.text_mid(name, name_x, rows[0].center().y, 14, Weight::Semibold, style::TEXT);
        if let Some(country) = country {
            style::rounded_rect(Rect::new(name_x - 18.0, rows[0].center().y - 6.0, 12.0, 12.0), 3.0, country.color.color());
        }
        value_row("Title", &person.title, rows[1], None, style::TEXT);
        let moves = format!("{} of {} points left", person.points, DAILY_POINTS);
        value_row("Age", &person.age_words(), rows[2], None, style::TEXT);
        value_row("Movement", &moves, rows[3], None, if person.points == 0 { style::TEXT_MUTED } else { style::TEXT });

        if let Some(error) = &self.error {
            style.text(error, inner_x, rows[3].bottom() + 22.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that ends their moves for the day, which only works while they have some left.
        let end = Rect::new(inner_x, rows[3].bottom() + 16.0 + error_height, inner_w, BUTTON_HEIGHT);
        let can_end = person.points > 0;
        let over_end = can_end && end.contains(mouse) && !self.choosing;
        style::rounded_rect(end, 8.0, if over_end { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("End moves for the day", end, 14, Weight::Semibold, if can_end { style::TEXT } else { style::TEXT_MUTED });
        if clicked && over_end {
            self.error = None;
            change = Some(Change::EndMoves(person.id));
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse) && !self.choosing;
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // The list of countries, over everything else while it is showing: each country, and then "Nomad".
        let mut used_click = false;
        if self.choosing {
            let choices: Vec<(Option<CountryId>, &str, Option<Color>)> = day.countries.iter().map(|c| (Some(c.id), c.name.as_str(), Some(c.color.color()))).chain([(None, person::NOMAD, None)]).collect();
            let shown = choices.len().min(MAX_LIST_ROWS);
            let list = Rect::new(inner_x, rows[0].bottom() + 2.0, inner_w, shown as f32 * LIST_ROW + LIST_PADDING * 2.0);
            if list.contains(mouse) && mouse_wheel().1 != 0.0 {
                self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
            }
            self.scroll = self.scroll.min(choices.len() - shown);
            style::card(list, 10.0, style::SURFACE);
            style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
            for (row, (id, name, color)) in choices.iter().skip(self.scroll).take(shown).enumerate() {
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
                if rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                }
                let mark = vec2(rect.x + 14.0, rect.center().y);
                if *id == person.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                if let Some(color) = color {
                    style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, *color);
                }
                style.text_mid(name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country they are in already is no change, so there is nothing to save.
                    if *id != person.country {
                        self.error = None;
                        change = Some(Change::Country(person.id, *id));
                    }
                    self.choosing = false;
                }
            }
            if self.scroll > 0 {
                draw_triangle(vec2(list.right() - 18.0, list.y + 9.0), vec2(list.right() - 10.0, list.y + 9.0), vec2(list.right() - 14.0, list.y + 4.0), style::TEXT_MUTED);
            }
            if self.scroll + shown < choices.len() {
                draw_triangle(vec2(list.right() - 18.0, list.bottom() - 9.0), vec2(list.right() - 10.0, list.bottom() - 9.0), vec2(list.right() - 14.0, list.bottom() - 4.0), style::TEXT_MUTED);
            }
            // A click anywhere while it is showing was for it: it closes it, and goes no further.
            if clicked {
                self.choosing = false;
                used_click = true;
            }
        } else if clicked && over_country {
            self.choosing = true;
            self.scroll = 0;
            self.error = None;
            used_click = true;
        }

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && (enter || (clicked && (over_close || !card.contains(mouse)))) {
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
        // With the list of countries showing, Esc closes that first.
        let mut popup = PersonPopup { person: Some(PersonId::new()), choosing: true, ..PersonPopup::default() };
        popup.escape();
        assert!(popup.is_open() && !popup.choosing);
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
        Change::Country(id, None).apply(&mut day).unwrap();
        assert_eq!(day.people[0].country, None);
    }
}
