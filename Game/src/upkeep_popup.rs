//! The upkeep popup, opened by clicking the alert that says a country can't pay to keep all its units: a list of the country's units, the
//! ships first and then the militia, the biggest first, for the player to pick which to pay for (see [`crate::upkeep`]). Under the list it
//! says what the units cost, what the country has to pay with, and what the picked ones cost.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::money;
use crate::ids::CountryId;
use crate::upkeep::{self, Unit};

const CARD_WIDTH: f32 = 480.0;
const PADDING: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 40.0;
const SUMMARY_ROW: f32 = 26.0;
const LIST_ROW: f32 = 34.0;
const LIST_PADDING: f32 = 6.0;
const MAX_LIST_ROWS: usize = 8;
const FADE_IN: f64 = 0.22;
/// How wide the columns at the right of a row are: the upkeep, and the size before it.
const UPKEEP_WIDTH: f32 = 84.0;
const SIZE_WIDTH: f32 = 116.0;

/// What the player changed in the list.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The unit is now picked to be paid for (or not).
    Pick(Unit, bool),
    /// Every unit of the country is no longer picked.
    Clear(CountryId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Pick(unit, picked) => day.set_unit_picked(*unit, *picked),
            Self::Clear(country) => {
                day.clear_picked(*country);
                Ok(())
            }
        }
    }
}

#[derive(Default)]
pub struct UpkeepPopup {
    /// The country whose units are listed, while the popup is up.
    country: Option<CountryId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// How far down the list is scrolled.
    scroll: usize,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl UpkeepPopup {
    pub fn is_open(&self) -> bool {
        self.country.is_some()
    }

    pub fn open(&mut self, country: CountryId) {
        *self = Self { country: Some(country), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    pub fn close(&mut self) {
        self.country = None;
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. Returns what the player changed, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Change> {
        let country = self.country?;
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let listed = upkeep::payables(day, country);
        let outlook = upkeep::outlook(day, country);
        let picked_cost: i64 = listed.iter().filter(|p| p.picked).map(|p| p.upkeep).sum();

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let rows = listed.len().clamp(1, MAX_LIST_ROWS);
        let error_height = if self.error.is_some() { 22.0 } else { 0.0 };
        let list_height = rows as f32 * LIST_ROW + LIST_PADDING * 2.0;
        let height = PADDING + 52.0 + SUMMARY_ROW * 3.0 + 12.0 + list_height + 14.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        style.text("Choose which units to pay for", inner_x, card.y + PADDING + 18.0, 20, Weight::Semibold, style::TEXT);
        let flag = day.country(country).map_or(style::TEXT_MUTED, |c| c.color.color());
        style::rounded_rect(Rect::new(inner_x, card.y + PADDING + 30.0, 12.0, 12.0), 3.0, flag);
        style.text_mid(day.country_name(country), inner_x + 20.0, card.y + PADDING + 36.0, 13, Weight::Regular, style::TEXT_MUTED);

        // What it costs, what there is to pay it with, and what the picked ones cost.
        let top = card.y + PADDING + 52.0;
        let over = picked_cost > outlook.available;
        let summary = [
            ("All the units cost", format!("{} coins a day", money(outlook.needed)), if outlook.is_short() { style::ERROR } else { style::TEXT }),
            ("To pay with", format!("{} coins", money(outlook.available)), style::TEXT),
            ("The ones picked cost", format!("{} coins a day", money(picked_cost)), if over { style::ERROR } else { style::TEXT }),
        ];
        for (k, (label, value, color)) in summary.iter().enumerate() {
            let y = top + k as f32 * SUMMARY_ROW + SUMMARY_ROW * 0.5;
            style.text_mid(label, inner_x, y, 13, Weight::Regular, style::TEXT_MUTED);
            style.text_mid(value, inner_x + inner_w - style.measure(value, 14, Weight::Semibold).width, y, 14, Weight::Semibold, *color);
        }

        // The list, scrolled with the wheel when there are more units than fit.
        let list = Rect::new(inner_x, top + SUMMARY_ROW * 3.0 + 12.0, inner_w, list_height);
        if list.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(listed.len().saturating_sub(rows));
        style::card(list, 10.0, style::SURFACE);
        style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
        if listed.is_empty() {
            style.text_mid("This country has no units.", list.x + 14.0, list.y + LIST_PADDING + LIST_ROW * 0.5, 13, Weight::Regular, style::TEXT_MUTED);
        }
        for (row, payable) in listed.iter().skip(self.scroll).take(rows).enumerate() {
            let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
            let hovered = rect.contains(mouse);
            if hovered {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }
            // A box that is ticked when the unit is picked.
            let tick = Rect::new(rect.x + 8.0, rect.center().y - 8.0, 16.0, 16.0);
            if payable.picked {
                style::rounded_rect(tick, 4.0, style::ACCENT);
                draw_line(tick.x + 3.5, tick.center().y + 0.5, tick.x + 7.0, tick.bottom() - 4.5, 2.0, WHITE);
                draw_line(tick.x + 7.0, tick.bottom() - 4.5, tick.right() - 3.0, tick.y + 4.0, 2.0, WHITE);
            } else {
                style::rounded_rect_lines(tick, 4.0, 1.5, style::BORDER_STRONG);
            }
            let upkeep = format!("{} coins", money(payable.upkeep));
            style.text_mid(&upkeep, rect.right() - 10.0 - style.measure(&upkeep, 13, Weight::Semibold).width, rect.center().y, 13, Weight::Semibold, style::TEXT);
            let size_right = rect.right() - UPKEEP_WIDTH;
            style.text_mid(&payable.size, size_right - style.measure(&payable.size, 12, Weight::Regular).width, rect.center().y, 12, Weight::Regular, style::TEXT_MUTED);
            let mut name = payable.name.clone();
            let room = size_right - SIZE_WIDTH - (tick.right() + 10.0);
            while name.chars().count() > 4 && style.measure(&name, 13, Weight::Regular).width > room {
                name.pop();
                name.pop();
                name.push('…');
            }
            style.text_mid(&name, tick.right() + 10.0, rect.center().y, 13, Weight::Regular, style::TEXT);
            if clicked && hovered {
                change = Some(Change::Pick(payable.unit, !payable.picked));
                self.error = None;
            }
        }
        // The arrows that say the list scrolls.
        if self.scroll > 0 {
            draw_triangle(vec2(list.right() - 18.0, list.y + 9.0), vec2(list.right() - 10.0, list.y + 9.0), vec2(list.right() - 14.0, list.y + 4.0), style::TEXT_MUTED);
        }
        if self.scroll + rows < listed.len() {
            draw_triangle(vec2(list.right() - 18.0, list.bottom() - 9.0), vec2(list.right() - 10.0, list.bottom() - 9.0), vec2(list.right() - 14.0, list.bottom() - 4.0), style::TEXT_MUTED);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, list.bottom() + 28.0, 12, Weight::Regular, style::ERROR);
        }

        // The buttons: one that takes back every pick, and one that closes it.
        let button_w = (inner_w - 10.0) * 0.5;
        let clear = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, button_w, BUTTON_HEIGHT);
        let done = Rect::new(clear.right() + 10.0, clear.y, button_w, BUTTON_HEIGHT);
        let any_picked = listed.iter().any(|p| p.picked);
        let over_clear = clear.contains(mouse) && any_picked;
        let over_done = done.contains(mouse);
        style::rounded_rect(clear, 8.0, if over_clear { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Clear picks", clear, 14, Weight::Semibold, if any_picked { style::TEXT } else { style::TEXT_MUTED });
        style::rounded_rect(done, 8.0, if over_done { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("Done", done, 14, Weight::Semibold, WHITE);
        if clicked && over_clear {
            change = Some(Change::Clear(country));
        }

        // A click on Done, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && (enter || (clicked && (over_done || !card.contains(mouse)))) {
            self.country = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::MilitiaId;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 1_000;
        day.raise_militia(day.cities[0].id, 50, vec2(30.0, 0.0)).unwrap();
        day
    }

    #[test]
    fn it_is_closed_until_opened() {
        let mut popup = UpkeepPopup::default();
        assert!(!popup.is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        popup.country = Some(CountryId::new());
        assert!(popup.is_open());
        popup.close();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_change_picks_and_clears_units_of_the_day() {
        let mut day = day();
        let (aldoria, id) = (day.countries[0].id, day.militias[0].id);
        Change::Pick(Unit::Militia(id), true).apply(&mut day).unwrap();
        assert!(day.militia(id).unwrap().pay_first);
        Change::Pick(Unit::Militia(id), false).apply(&mut day).unwrap();
        assert!(!day.militia(id).unwrap().pay_first);
        Change::Pick(Unit::Militia(id), true).apply(&mut day).unwrap();
        Change::Clear(aldoria).apply(&mut day).unwrap();
        assert!(!day.militia(id).unwrap().pay_first);
        assert!(Change::Pick(Unit::Militia(MilitiaId::new()), true).apply(&mut day).is_err(), "it is not there");
    }

    #[test]
    fn a_failure_is_kept_to_show() {
        let mut popup = UpkeepPopup { country: Some(CountryId::new()), ..UpkeepPopup::default() };
        popup.fail("no".to_owned());
        assert_eq!(popup.error.as_deref(), Some("no"));
    }
}
