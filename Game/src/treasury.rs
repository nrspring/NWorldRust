//! The treasury. Clicking the coin in the header shows the money and the resources of the country the player plays. From
//! there the player can send some of them to another country, by clicking what to send or by the button, which asks in
//! a popup what, to which country and how much. The transfer is made, saved and reported in the newsreel by the caller.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::country_menu::fit;
use crate::day::DaySave;
use crate::format::{money, thousands_u64};
use crate::hud;
use crate::ids::CountryId;
use crate::popup::{self, Icon, Popup};
use crate::transfer::{Asset, Transfer};

const CARD_WIDTH: f32 = 400.0;
const PADDING: f32 = 24.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 76.0;
const ROW_HEIGHT: f32 = 32.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// How long it takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

enum Screen {
    Closed,
    /// What this country has.
    Overview(CountryId),
    /// Asking what this country sends, to whom and how much.
    Sending(CountryId, Popup),
}

pub struct Treasury {
    screen: Screen,
    opened_at: f64,
    /// Set when it has just come up, so the click that did it isn't also a click on it.
    fresh: bool,
    /// Why the last transfer couldn't be made, while what the country has is showing.
    error: Option<String>,
}

impl Default for Treasury {
    fn default() -> Self {
        Self { screen: Screen::Closed, opened_at: 0.0, fresh: false, error: None }
    }
}

/// The names of the countries other than `except`, for choosing who to send to.
fn others(day: &DaySave, except: CountryId) -> Vec<String> {
    day.countries.iter().filter(|c| c.id != except).map(|c| c.name.clone()).collect()
}

/// The transfer that `typed` (what, to whom, how much, as the popup checked them) asks for, from the country `from`. An
/// error says why it isn't one.
fn resolve(day: &DaySave, from: CountryId, typed: &[String]) -> Result<Transfer, String> {
    let asset = Asset::named(&typed[0]).ok_or("Choose what to send.")?;
    let to = day
        .countries
        .iter()
        .find(|c| c.id != from && c.name.to_lowercase() == typed[1].to_lowercase())
        .ok_or_else(|| format!("There is no other country called {}.", typed[1]))?;
    let amount = typed[2].parse().map_err(|_| "Enter how much to send, as a whole number.".to_owned())?;
    Ok(Transfer { from, to: to.id, asset, amount })
}

impl Treasury {
    pub fn is_open(&self) -> bool {
        !matches!(self.screen, Screen::Closed)
    }

    /// Opens what the country the player plays has. Does nothing if there is no country.
    pub fn open(&mut self, day: &DaySave) {
        self.open_at(day, get_time());
    }

    /// The same, as of the time `now`, in seconds, which it fades in from.
    fn open_at(&mut self, day: &DaySave, now: f64) {
        if let Some(country) = day.player_country() {
            self.screen = Screen::Overview(country.id);
            self.fresh = true;
            self.error = None;
            self.opened_at = now;
        }
    }

    /// What Esc does: it closes a drop-down that is open, then goes back from the popup to what the country has, and
    /// from there to the game.
    pub fn escape(&mut self) {
        match &mut self.screen {
            Screen::Sending(id, popup) => {
                let id = *id;
                if !popup.close_dropdown() {
                    self.screen = Screen::Overview(id);
                    self.fresh = true;
                }
            }
            Screen::Overview(_) | Screen::Closed => self.screen = Screen::Closed,
        }
    }

    /// Shows why the transfer `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        match &mut self.screen {
            Screen::Sending(_, popup) => popup.fail(message),
            _ => self.error = Some(message),
        }
    }

    /// Carries on after the transfer `show` gave was made: back to what the country has.
    pub fn done(&mut self) {
        if let Screen::Sending(id, _) = self.screen {
            self.screen = Screen::Overview(id);
            self.fresh = true;
            self.error = None;
        }
    }

    /// Takes the keyboard and mouse for this frame and draws it over everything. Returns the transfer the player asked
    /// for, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Transfer> {
        match &mut self.screen {
            Screen::Closed => None,
            Screen::Sending(id, popup) => {
                let id = *id;
                let typed = popup.show(style)?;
                match resolve(day, id, &typed) {
                    Ok(transfer) => Some(transfer),
                    Err(message) => {
                        popup.fail(message);
                        None
                    }
                }
            }
            Screen::Overview(id) => {
                let id = *id;
                self.overview(style, day, id);
                None
            }
        }
    }

    fn overview(&mut self, style: &Style, day: &DaySave, id: CountryId) {
        let Some(country) = day.country(id) else {
            self.screen = Screen::Closed;
            return;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + Asset::ALL.len() as f32 * ROW_HEIGHT + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;

        // The title, beside its icon, with the name of the country.
        let badge = Rect::new(inner_x, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Coin, badge);
        let text_x = badge.right() + 14.0;
        style.text("Treasury", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        let name = fit(style, &country.name, 12, Weight::Regular, card.right() - PADDING - text_x);
        style.text(&name, text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What it has, a row each for the money and the resources. A click on a row is for sending some of it.
        let others = others(day, id);
        let can_send = !others.is_empty();
        let mut sending = None;
        for (i, asset) in Asset::ALL.into_iter().enumerate() {
            let row = Rect::new(inner_x - 8.0, card.y + ROWS_TOP + i as f32 * ROW_HEIGHT, inner_w + 16.0, ROW_HEIGHT);
            let mid = row.center().y;
            let over = can_send && row.contains(mouse);
            if over {
                style::rounded_rect(row, 8.0, style::HOVER);
            }
            let (text, amount_is_none, debt) = match asset.resource() {
                Some(resource) => {
                    let amount = country.stock.get(resource);
                    (thousands_u64(amount), amount == 0, false)
                }
                None => (money(country.money), country.money == 0, country.money < 0),
            };
            match asset.resource() {
                Some(resource) => draw_circle(inner_x + 5.0, mid, 4.5, hud::resource_color(resource)),
                None => hud::draw_coin(vec2(inner_x + 5.0, mid), 5.5),
            }
            style.text_mid(&asset.label(), inner_x + 20.0, mid, 14, Weight::Regular, style::TEXT_MUTED);
            let width = style.measure(&text, 15, Weight::Semibold).width;
            let color = if debt { style::ERROR } else if amount_is_none { style::TEXT_MUTED } else { style::TEXT };
            style.text_mid(&text, inner_x + inner_w - width, mid, 15, Weight::Semibold, color);
            if over {
                let hint = "Send";
                let hint_w = style.measure(hint, 12, Weight::Semibold).width;
                style.text_mid(hint, inner_x + inner_w - width - 14.0 - hint_w, mid, 12, Weight::Semibold, style::ACCENT_HOVER);
            }
            if clicked && over {
                sending = Some(asset);
            }
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, card.y + ROWS_TOP + Asset::ALL.len() as f32 * ROW_HEIGHT + 30.0, 12, Weight::Regular, style::ERROR);
        }

        // Close, and send.
        let top = card.bottom() - PADDING - BUTTON_HEIGHT;
        let close = Rect::new(inner_x, top, 110.0, BUTTON_HEIGHT);
        let send = Rect::new(close.right() + 12.0, top, inner_w - 110.0 - 12.0, BUTTON_HEIGHT);
        let (over_close, over_send) = (close.contains(mouse), can_send && send.contains(mouse));
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);
        if can_send {
            style::rounded_rect(send, 8.0, if over_send { style::ACCENT_HOVER } else { style::ACCENT });
            style.text_centered("Send to another country", send, 14, Weight::Semibold, WHITE);
        } else {
            style::rounded_rect(send, 8.0, Color::new(1.0, 1.0, 1.0, 0.06));
            style.text_centered("No other country to send to", send, 14, Weight::Semibold, style::TEXT_MUTED);
        }

        if clicked && over_send {
            self.screen = Screen::Sending(id, Popup::transfer(&country.name, others, None));
        } else if let Some(asset) = sending {
            self.screen = Screen::Sending(id, Popup::transfer(&country.name, others, Some(asset)));
        } else if clicked && (over_close || !card.contains(mouse)) {
            self.screen = Screen::Closed;
        }
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
        day.found_country("Cardell").unwrap();
        day
    }

    fn typed(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn the_countries_to_send_to_are_all_but_the_sender() {
        let day = day();
        assert_eq!(others(&day, day.countries[1].id), ["Aldoria", "Cardell"]);
        assert!(others(&DaySave::new(Date::FIRST, "Assets/map.nworld"), CountryId::new()).is_empty());
    }

    #[test]
    fn what_is_typed_becomes_a_transfer_to_the_country_of_that_name() {
        let day = day();
        let (aldoria, cardell) = (day.countries[0].id, day.countries[2].id);
        let transfer = resolve(&day, aldoria, &typed(&["Wood", "cardell", "250"])).unwrap();
        assert_eq!(transfer, Transfer { from: aldoria, to: cardell, asset: Asset::Wood, amount: 250 });
    }

    #[test]
    fn a_transfer_to_nobody_or_to_oneself_is_not_made_from_what_is_typed() {
        let day = day();
        let aldoria = day.countries[0].id;
        assert!(resolve(&day, aldoria, &typed(&["Coins", "Narnia", "5"])).unwrap_err().contains("no other country called Narnia"));
        assert!(resolve(&day, aldoria, &typed(&["Coins", "Aldoria", "5"])).is_err());
        assert!(resolve(&day, aldoria, &typed(&["Gold", "Brevon", "5"])).is_err());
    }

    #[test]
    fn it_opens_for_the_country_played_and_esc_goes_back_a_step() {
        let mut day = day();
        day.select_country(day.countries[1].id).unwrap();
        let mut treasury = Treasury::default();
        treasury.open_at(&DaySave::new(Date::FIRST, "Assets/map.nworld"), 0.0);
        assert!(!treasury.is_open(), "no country, nothing to show");
        treasury.open_at(&day, 0.0);
        assert!(matches!(treasury.screen, Screen::Overview(id) if id == day.countries[1].id));
        treasury.escape();
        assert!(!treasury.is_open());
    }
}
