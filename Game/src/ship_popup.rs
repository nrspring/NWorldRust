//! The ship popup, opened by clicking a ship that is already selected: what there is to know of it, a way to rename it and to change
//! which country it belongs to, and to scrap it. A ship is one vessel, so there is no combining or splitting as there is for a militia.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::money;
use crate::ids::{CountryId, ShipId};
use crate::movement::DAILY_POINTS;
use crate::options;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// The height of a row of the list of countries, and the most it shows before it scrolls.
const LIST_ROW: f32 = 30.0;
const LIST_PADDING: f32 = 6.0;
const MAX_LIST_ROWS: usize = 6;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;
/// How many rows the information has.
const ROW_COUNT: usize = 9;

/// What the player changed about the ship in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The ship now belongs to this country.
    Country(ShipId, CountryId),
    /// The ship is now paid for before the country's other units (or not).
    PayFirst(ShipId, bool),
    /// The ship has no movement points left today.
    EndMoves(ShipId),
    /// The ship is now at anchor (or not).
    Garrison(ShipId, bool),
    /// The ship is scrapped.
    Scrap(ShipId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Country(ship, country) => day.set_ship_country(*ship, *country),
            Self::PayFirst(id, pay_first) => day.set_ship_pay_first(*id, *pay_first),
            Self::EndMoves(id) => day.end_ship_moves(*id),
            Self::Garrison(id, garrison) => day.set_ship_garrison(*id, *garrison),
            Self::Scrap(id) => day.scrap_ship(*id),
        }
    }

    /// The ship it is about, which can't be scrapped while it is sailing.
    pub fn ship(&self) -> ShipId {
        match self {
            Self::Country(id, _) | Self::PayFirst(id, _) | Self::EndMoves(id) | Self::Garrison(id, _) | Self::Scrap(id) => *id,
        }
    }
}

#[derive(Default)]
pub struct ShipPopup {
    /// The ship being shown, while the popup is up.
    ship: Option<ShipId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Whether the list of countries to change to is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
    /// Whether Scrap has been clicked once, and is waiting for a second click to be sure.
    scrapping: bool,
    /// Set when the name was clicked, to be renamed: the ship whose name it is, until `take_rename` takes it.
    rename: Option<ShipId>,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl ShipPopup {
    pub fn is_open(&self) -> bool {
        self.ship.is_some()
    }

    pub fn open(&mut self, ship: ShipId) {
        *self = Self { ship: Some(ship), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries or the question about scrapping if one is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if self.choosing || self.scrapping {
            self.choosing = false;
            self.scrapping = false;
        } else {
            self.ship = None;
        }
    }

    /// The ship whose name was clicked since this was last called, if one was. The popup closes when it is, for the name to be asked
    /// for.
    pub fn take_rename(&mut self) -> Option<ShipId> {
        self.rename.take()
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.choosing = false;
        self.scrapping = false;
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A ship that is not in `day` is not shown.
    /// Returns what the player changed about the ship, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Change> {
        let Some(unit) = self.ship.and_then(|id| day.ship(id)) else {
            self.ship = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(unit.country);
        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * ROW_COUNT as f32 + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little ship in the country's color.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        let flag = country.map_or(style::TEXT_MUTED, |c| c.color.color());
        crate::shipyard::ShipyardKind::Small.draw_icon(badge.center(), 1.5, flag, style::ACCENT_SOFT);
        let text_x = badge.right() + 14.0;
        style.text(unit.title(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&format!("{} built on {}", unit.kind.label(), unit.built), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each. The name and the country can be clicked, to change them.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..ROW_COUNT).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        let [name_row, country_row, size_row, sails_row, built_row, upkeep_row, moves_row, pay_row, anchor_row] = [rows[0], rows[1], rows[2], rows[3], rows[4], rows[5], rows[6], rows[7], rows[8]];
        for y in [name_row.y, country_row.y, size_row.y, sails_row.y, built_row.y, upkeep_row.y, moves_row.y, pay_row.y, anchor_row.y, anchor_row.bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }

        let over_name = name_row.contains(mouse) && !self.choosing;
        if over_name {
            style::rounded_rect(Rect::new(name_row.x - 8.0, name_row.y + 1.0, name_row.w + 16.0, name_row.h - 2.0), 8.0, style::HOVER);
        }
        style.text_mid("Name", inner_x, name_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let edit = "Rename";
        let edit_x = name_row.right() - style.measure(edit, 12, Weight::Regular).width;
        style.text_mid(edit, edit_x, name_row.center().y, 12, Weight::Regular, if over_name { style::ACCENT } else { style::TEXT_MUTED });
        let mut name = unit.title().to_owned();
        let room = edit_x - inner_x - 70.0;
        while name.chars().count() > 4 && style.measure(&name, 14, Weight::Semibold).width > room {
            name.pop();
            name.pop();
            name.push('…');
        }
        let name_x = edit_x - 12.0 - style.measure(&name, 14, Weight::Semibold).width;
        style.text_mid(&name, name_x, name_row.center().y, 14, Weight::Semibold, style::TEXT);

        let over_country = country_row.contains(mouse);
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(country_row.x - 8.0, country_row.y + 1.0, country_row.w + 16.0, country_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = country_row.center().y;
        style.text_mid("Country", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        let country_name = country.map_or("an unknown country", |c| c.name.as_str());
        // At the right: the name, the country's color before it, and an arrow after it that says there is a list to choose from.
        let arrow = vec2(country_row.right() - 4.0, middle);
        if self.choosing {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), style::TEXT_MUTED);
        } else {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y - 2.0), vec2(arrow.x + 4.0, arrow.y - 2.0), vec2(arrow.x, arrow.y + 3.0), style::TEXT_MUTED);
        }
        let country_x = arrow.x - 14.0 - style.measure(country_name, 14, Weight::Semibold).width;
        style.text_mid(country_name, country_x, middle, 14, Weight::Semibold, style::TEXT);
        if let Some(country) = country {
            style::rounded_rect(Rect::new(country_x - 18.0, middle - 6.0, 12.0, 12.0), 3.0, country.color.color());
        }

        let value_row = |label: &str, value: &str, row: Rect| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, style::TEXT);
        };
        value_row("Size", unit.kind.label(), size_row);
        value_row("Sails", unit.kind.waters_text(), sails_row);
        value_row("Built", &unit.built.to_string(), built_row);
        value_row("Upkeep", &format!("{} coins a day", money(unit.kind.upkeep())), upkeep_row);
        value_row("Movement", &format!("{} of {} points left", unit.points, DAILY_POINTS), moves_row);

        // A button that ends its moves for the day, in the movement row, while it has any left.
        let end_label = "End moves";
        let end_rect = Rect::new(inner_x + 84.0, moves_row.y + 6.0, style.measure(end_label, 12, Weight::Semibold).width + 18.0, moves_row.h - 12.0);
        let over_end = unit.points > 0 && !self.choosing && end_rect.contains(mouse);
        if unit.points > 0 {
            style::rounded_rect(end_rect, 6.0, if over_end { style::BORDER_STRONG } else { style::HOVER });
            style.text_centered(end_label, end_rect, 12, Weight::Semibold, style::TEXT);
        }
        if clicked && over_end {
            change = Some(Change::EndMoves(unit.id));
        }

        // Whether it is paid for first, which matters when the country has too little money for all its units.
        let mut pay_first = unit.pay_first;
        options::toggle(style, Rect::new(pay_row.x - 12.0, pay_row.y, pay_row.w + 24.0, pay_row.h), "Pay first", None, &mut pay_first, !self.choosing);
        if pay_first != unit.pay_first {
            change = Some(Change::PayFirst(unit.id, pay_first));
        }

        // Whether it is at anchor, which stays put: it has no movement points today, and gets none on any day after.
        let mut garrison = unit.garrison;
        options::toggle(style, Rect::new(anchor_row.x - 12.0, anchor_row.y, anchor_row.w + 24.0, anchor_row.h), "At anchor", None, &mut garrison, !self.choosing);
        if garrison != unit.garrison && change.is_none() {
            change = Some(Change::Garrison(unit.id, garrison));
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, pay_row.bottom() + 24.0, 12, Weight::Regular, style::ERROR);
        }

        // The buttons at the bottom: one that scraps it, which has to be clicked twice to be sure, and one that closes it.
        let button_w = (inner_w - 10.0) * 0.5;
        let scrap = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, button_w, BUTTON_HEIGHT);
        let close = Rect::new(scrap.right() + 10.0, scrap.y, button_w, BUTTON_HEIGHT);
        let over_scrap = scrap.contains(mouse) && !self.choosing;
        let over_close = close.contains(mouse) && !self.choosing;
        let (fill, label, color) = match (self.scrapping, over_scrap) {
            (true, _) => (style::ERROR, "Click again to scrap", WHITE),
            (false, true) => (style::BORDER_STRONG, "Scrap", style::ERROR),
            (false, false) => (style::HOVER, "Scrap", style::ERROR),
        };
        style::rounded_rect(scrap, 8.0, fill);
        style.text_centered(label, scrap, 14, Weight::Semibold, color);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // The list of countries, over everything else while it is showing.
        let mut used_click = false;
        if self.choosing {
            let count = day.countries.len();
            let rows = count.min(MAX_LIST_ROWS);
            let list = Rect::new(inner_x, country_row.bottom() + 2.0, inner_w, rows as f32 * LIST_ROW + LIST_PADDING * 2.0);
            if list.contains(mouse) && mouse_wheel().1 != 0.0 {
                self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
            }
            self.scroll = self.scroll.min(count - rows);
            style::card(list, 10.0, style::SURFACE);
            style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
            for row in 0..rows {
                let listed = &day.countries[self.scroll + row];
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
                if rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                }
                let mark = vec2(rect.x + 14.0, rect.center().y);
                if listed.id == unit.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, listed.color.color());
                style.text_mid(&listed.name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country it is already in is no change, so there is nothing to save.
                    if listed.id != unit.country {
                        change = Some(Change::Country(unit.id, listed.id));
                    }
                    self.choosing = false;
                }
            }
            if self.scroll > 0 {
                draw_triangle(vec2(list.right() - 18.0, list.y + 9.0), vec2(list.right() - 10.0, list.y + 9.0), vec2(list.right() - 14.0, list.y + 4.0), style::TEXT_MUTED);
            }
            if self.scroll + rows < count {
                draw_triangle(vec2(list.right() - 18.0, list.bottom() - 9.0), vec2(list.right() - 10.0, list.bottom() - 9.0), vec2(list.right() - 14.0, list.bottom() - 4.0), style::TEXT_MUTED);
            }
            // A click on the list was for it; a click anywhere else closes it, and goes no further.
            if clicked {
                self.choosing = false;
                used_click = true;
            }
        } else if clicked && over_name {
            // The name is asked for in a popup of its own, which takes this one's place.
            self.rename = Some(unit.id);
            self.ship = None;
            used_click = true;
        } else if clicked && over_country {
            self.choosing = true;
            self.error = None;
            self.scroll = day.countries.iter().position(|c| c.id == unit.country).unwrap_or(0).saturating_sub(MAX_LIST_ROWS - 1);
            used_click = true;
        }

        // A click on Scrap asks to be sure, and the next one scraps; a click anywhere else takes the question back.
        if clicked && !used_click && over_scrap {
            if self.scrapping {
                change = Some(Change::Scrap(unit.id));
            }
            self.scrapping = !self.scrapping;
            used_click = true;
        } else if clicked {
            self.scrapping = false;
        }

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && ((enter && !self.choosing) || (clicked && (over_close || !card.contains(mouse)))) {
            self.ship = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ship::ShipKind;
    use crate::shipyard::ShipyardKind;
    use nworld_map::world::Resource;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, 20_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(id, resource, 2_000).unwrap();
        }
        day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        day.build_ship(day.shipyards[0].id, ShipKind::Small, vec2(30.0, 0.0)).unwrap();
        day
    }

    #[test]
    fn a_click_on_the_name_asks_for_a_rename_once() {
        let mut popup = ShipPopup { ship: Some(ShipId::new()), ..ShipPopup::default() };
        assert_eq!(popup.take_rename(), None);
        let id = ShipId::new();
        popup.rename = Some(id);
        assert_eq!(popup.take_rename(), Some(id));
        assert_eq!(popup.take_rename(), None, "it is taken");
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_the_list_and_the_question_before_the_popup() {
        assert!(!ShipPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = ShipPopup { ship: Some(ShipId::new()), choosing: true, scrapping: true, ..ShipPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(popup.is_open() && !popup.choosing && !popup.scrapping);
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_failure_closes_the_list_and_is_kept_to_show() {
        let mut popup = ShipPopup { ship: Some(ShipId::new()), choosing: true, scrapping: true, ..ShipPopup::default() };
        popup.fail("no".to_owned());
        assert!(!popup.choosing && !popup.scrapping && popup.error.as_deref() == Some("no"));
    }

    #[test]
    fn a_change_does_what_it_says_to_the_day() {
        let mut day = day();
        let (id, brevon) = (day.ships[0].id, day.countries[1].id);
        assert_eq!(Change::Scrap(id).ship(), id);
        Change::Country(id, brevon).apply(&mut day).unwrap();
        assert_eq!(day.ship(id).unwrap().country, brevon);
        assert!(Change::Country(id, CountryId::new()).apply(&mut day).is_err());
        assert!(Change::Country(ShipId::new(), brevon).apply(&mut day).is_err(), "it is not there");
        Change::Scrap(id).apply(&mut day).unwrap();
        assert!(day.ships.is_empty());
        assert!(Change::Scrap(id).apply(&mut day).is_err(), "it is already gone");
    }
}
