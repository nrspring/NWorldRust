//! The countries menu, opened by clicking the country in the header: choose the country to play,
//! edit a country's name, or add a new country. Editing and adding ask for a name in a popup of
//! their own, and come back to the list when they are done.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::ids::CountryId;
use crate::popup::{self, Icon, Popup};

const CARD_WIDTH: f32 = 420.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 34.0;
/// The most countries the list shows before it scrolls.
const MAX_ROWS: usize = 10;
const LIST_PADDING: f32 = 6.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the list starts, below the card's top: room for the title.
const LIST_TOP: f32 = 84.0;
/// How long the menu takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player chose in the menu, for the caller to do and save.
#[derive(Debug, PartialEq)]
pub enum Choice {
    /// Play this country.
    Select(CountryId),
    /// Found a country with this name.
    Add(String),
    /// Give this country this name.
    Rename(CountryId, String),
}

impl Choice {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Select(id) => day.select_country(*id),
            Self::Add(name) => day.found_country(name),
            Self::Rename(id, name) => day.rename_country(*id, name),
        }
    }
}

#[derive(Default)]
enum Screen {
    #[default]
    Closed,
    List,
    Adding(Popup),
    Renaming(CountryId, Popup),
}

#[derive(Default)]
pub struct CountryMenu {
    screen: Screen,
    opened_at: f64,
    /// Set when the list has just come up, so the click that did it isn't also a click on the list.
    fresh: bool,
    /// The row picked with the arrow keys or the mouse, and how far down the list is scrolled.
    highlight: usize,
    scroll: usize,
    /// Why the last choice couldn't be done, while the list is showing.
    error: Option<String>,
}

impl CountryMenu {
    pub fn is_open(&self) -> bool {
        !matches!(self.screen, Screen::Closed)
    }

    /// Opens the list of the countries of `day`, with the player's country picked.
    pub fn open(&mut self, day: &DaySave) {
        let current = day.player_country().and_then(|p| day.countries.iter().position(|c| c.id == p.id)).unwrap_or(0);
        self.show_list(current);
        self.opened_at = get_time();
    }

    fn show_list(&mut self, highlight: usize) {
        self.screen = Screen::List;
        self.fresh = true;
        self.error = None;
        self.highlight = highlight;
        self.scroll = highlight.saturating_sub(MAX_ROWS - 1);
    }

    /// What Esc does: back from a popup to the list, and from the list to the game.
    pub fn escape(&mut self) {
        match self.screen {
            Screen::Adding(_) | Screen::Renaming(..) => self.show_list(self.highlight),
            Screen::List | Screen::Closed => self.screen = Screen::Closed,
        }
    }

    /// Shows why `choice`, which `show` gave, couldn't be done.
    pub fn fail(&mut self, message: String) {
        match &mut self.screen {
            Screen::Adding(popup) | Screen::Renaming(_, popup) => popup.fail(message),
            _ => self.error = Some(message),
        }
    }

    /// Carries on after `choice`, which `show` gave, was done: choosing a country closes the menu, and
    /// the others come back to the list.
    pub fn done(&mut self, choice: &Choice, day: &DaySave) {
        match choice {
            Choice::Select(_) => self.screen = Screen::Closed,
            Choice::Add(_) => self.show_list(day.countries.len().saturating_sub(1)),
            Choice::Rename(id, _) => self.show_list(day.countries.iter().position(|c| c.id == *id).unwrap_or(0)),
        }
    }

    /// Takes the keyboard and mouse for this frame and draws the menu over everything. Returns what the
    /// player chose, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Choice> {
        match &mut self.screen {
            Screen::Closed => None,
            Screen::Adding(popup) => popup.show(style).map(|typed| Choice::Add(typed.into_iter().next().unwrap_or_default())),
            Screen::Renaming(id, popup) => {
                let id = *id;
                popup.show(style).map(|typed| Choice::Rename(id, typed.into_iter().next().unwrap_or_default()))
            }
            Screen::List => self.list(style, day),
        }
    }

    fn list(&mut self, style: &Style, day: &DaySave) -> Option<Choice> {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let current = day.player_country().map(|c| c.id);
        let count = day.countries.len();
        let rows = count.min(MAX_ROWS);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let list_height = rows as f32 * ROW_HEIGHT + LIST_PADDING * 2.0;
        let height = LIST_TOP + list_height + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        style.text("Countries", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("Choose the country you play, edit one, or add a new one.", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // The list, which the mouse wheel scrolls when there are more countries than rows.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let list = Rect::new(inner_x, card.y + LIST_TOP, inner_w, list_height);
        style::rounded_rect(list, 10.0, style::FIELD);
        style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
        if list.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(count - rows);

        // The arrow keys move the pick, the mouse picks what it is over, and Enter chooses it.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if is_key_pressed(KeyCode::Down) {
            self.highlight = (self.highlight + 1).min(count.saturating_sub(1));
        }
        if is_key_pressed(KeyCode::Up) {
            self.highlight = self.highlight.saturating_sub(1);
        }
        self.scroll = self.scroll.clamp((self.highlight + 1).saturating_sub(rows), self.highlight);

        let mut chosen = None;
        let mut renaming = None;
        for row in 0..rows {
            let place = self.scroll + row;
            let country = &day.countries[place];
            let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * ROW_HEIGHT, list.w - LIST_PADDING * 2.0, ROW_HEIGHT);
            let edit = Rect::new(rect.right() - 8.0 - 46.0, rect.center().y - 12.0, 46.0, 24.0);
            let over_row = rect.contains(mouse);
            if over_row && mouse_delta_position() != Vec2::ZERO {
                self.highlight = place;
            }
            if self.highlight == place {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }

            // A round mark that is filled for the country that is played.
            let mark = vec2(rect.x + 18.0, rect.center().y);
            if current == Some(country.id) {
                draw_circle(mark.x, mark.y, 7.0, style::ACCENT);
                draw_circle(mark.x, mark.y, 2.8, WHITE);
            } else {
                draw_circle_lines(mark.x, mark.y, 6.5, 1.5, style::BORDER_STRONG);
            }
            let weight = if current == Some(country.id) { Weight::Semibold } else { Weight::Regular };
            style.text_mid(&country.name, rect.x + 36.0, rect.center().y, 15, weight, style::TEXT);

            let over_edit = edit.contains(mouse);
            style::rounded_rect(edit, 6.0, if over_edit { style::BORDER_STRONG } else { style::HOVER });
            style.text_centered("Edit", edit, 12, Weight::Semibold, if over_edit { style::TEXT } else { style::TEXT_MUTED });
            if clicked && over_edit {
                renaming = Some((country.id, country.name.clone(), place));
            } else if clicked && over_row {
                chosen = Some(country.id);
            }
        }
        if enter {
            chosen = day.countries.get(self.highlight).map(|c| c.id);
        }
        // More above or below than shows: a small mark at the edge.
        if self.scroll > 0 {
            draw_triangle(vec2(list.right() - 18.0, list.y + 10.0), vec2(list.right() - 10.0, list.y + 10.0), vec2(list.right() - 14.0, list.y + 5.0), style::TEXT_MUTED);
        }
        if self.scroll + rows < count {
            draw_triangle(vec2(list.right() - 18.0, list.bottom() - 10.0), vec2(list.right() - 10.0, list.bottom() - 10.0), vec2(list.right() - 14.0, list.bottom() - 5.0), style::TEXT_MUTED);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, list.bottom() + 22.0, 12, Weight::Regular, style::ERROR);
        }

        // The button for a new country.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let hovered = button.contains(mouse);
        style::rounded_rect(button, 8.0, if hovered { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("+  Add a new country", button, 14, Weight::Semibold, WHITE);

        if clicked && hovered {
            self.screen = Screen::Adding(Popup::country());
            return None;
        }
        if let Some((id, name, place)) = renaming {
            self.highlight = place;
            self.screen = Screen::Renaming(id, Popup::rename_country(&name));
            return None;
        }
        if let Some(id) = chosen {
            // Choosing the country that is already played changes nothing, so there is nothing to save.
            if current == Some(id) {
                self.screen = Screen::Closed;
                return None;
            }
            return Some(Choice::Select(id));
        }
        // A click on the dimmed window outside the card closes the menu.
        if clicked && !card.contains(mouse) {
            self.screen = Screen::Closed;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day
    }

    #[test]
    fn a_choice_does_what_it_says_to_the_day() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);

        Choice::Select(brevon).apply(&mut day).unwrap();
        assert_eq!(day.player, Some(brevon));
        Choice::Rename(aldoria, "Old Aldoria".to_owned()).apply(&mut day).unwrap();
        assert_eq!(day.country_name(aldoria), "Old Aldoria");
        Choice::Add("Cardell".to_owned()).apply(&mut day).unwrap();
        assert_eq!(day.countries.len(), 3);
        assert_eq!(day.player, Some(brevon), "adding a country doesn't change the one that is played");
    }

    #[test]
    fn a_choice_that_cannot_be_done_says_why_and_changes_nothing() {
        let mut day = day();
        let before = day.clone();
        assert!(Choice::Add("brevon".to_owned()).apply(&mut day).unwrap_err().contains("already"));
        assert!(Choice::Rename(day.countries[0].id, "BREVON".to_owned()).apply(&mut day).is_err());
        assert!(Choice::Select(CountryId::new()).apply(&mut day).is_err());
        assert_eq!(day, before);
    }
}
