//! The city popup, opened by clicking a city: what there is to know of it, and a way to change which
//! country it belongs to.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::hud::thousands;
use crate::ids::{CityId, CountryId};
use crate::popup::{self, Icon};

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

#[derive(Default)]
pub struct CityPopup {
    /// The city being shown, while the popup is up.
    city: Option<CityId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Whether the list of countries to change to is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl CityPopup {
    pub fn is_open(&self) -> bool {
        self.city.is_some()
    }

    pub fn open(&mut self, city: CityId) {
        *self = Self { city: Some(city), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries if it is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if self.choosing {
            self.choosing = false;
        } else {
            self.city = None;
        }
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.choosing = false;
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A city that is not
    /// in `day` is not shown. Returns the city and the country the player chose to change it to.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<(CityId, CountryId)> {
        let Some(city) = self.city.and_then(|id| day.city(id)) else {
            self.city = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(city.country);
        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * 2.0 + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The name, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::City, badge);
        let text_x = badge.right() + 14.0;
        style.text(&city.name, text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("City", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each. The country's can be clicked, to change it.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let country_row = Rect::new(inner_x, card.y + ROWS_TOP, inner_w, ROW_HEIGHT);
        let population_row = Rect::new(inner_x, country_row.bottom(), inner_w, ROW_HEIGHT);
        draw_line(inner_x, country_row.y, inner_x + inner_w, country_row.y, 1.0, style::BORDER);
        draw_line(inner_x, population_row.y, inner_x + inner_w, population_row.y, 1.0, style::BORDER);
        draw_line(inner_x, population_row.bottom(), inner_x + inner_w, population_row.bottom(), 1.0, style::BORDER);

        let over_country = country_row.contains(mouse);
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(country_row.x - 8.0, country_row.y + 1.0, country_row.w + 16.0, country_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = country_row.center().y;
        style.text_mid("Country", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        let name = country.map_or("an unknown country", |c| c.name.as_str());
        // At the right: the name, the country's color before it, and an arrow after it that says there is a
        // list to choose from.
        let arrow = vec2(country_row.right() - 4.0, middle);
        if self.choosing {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), style::TEXT_MUTED);
        } else {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y - 2.0), vec2(arrow.x + 4.0, arrow.y - 2.0), vec2(arrow.x, arrow.y + 3.0), style::TEXT_MUTED);
        }
        let name_x = arrow.x - 14.0 - style.measure(name, 14, Weight::Semibold).width;
        style.text_mid(name, name_x, middle, 14, Weight::Semibold, style::TEXT);
        if let Some(country) = country {
            style::rounded_rect(Rect::new(name_x - 18.0, middle - 6.0, 12.0, 12.0), 3.0, country.color.color());
        }

        let population = thousands(city.population);
        style.text_mid("Population", inner_x, population_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let population_x = population_row.right() - style.measure(&population, 14, Weight::Semibold).width;
        style.text_mid(&population, population_x, population_row.center().y, 14, Weight::Semibold, style::TEXT);

        if let Some(error) = &self.error {
            style.text(error, inner_x, population_row.bottom() + 24.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_button = button.contains(mouse) && !self.choosing;
        style::rounded_rect(button, 8.0, if over_button { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", button, 14, Weight::Semibold, style::TEXT);

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
                let place = self.scroll + row;
                let listed = &day.countries[place];
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
                if rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                }
                let mark = vec2(rect.x + 14.0, rect.center().y);
                if listed.id == city.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, listed.color.color());
                style.text_mid(&listed.name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country it is already in is no change, so there is nothing to save.
                    if listed.id != city.country {
                        change = Some((city.id, listed.id));
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
        } else if clicked && over_country {
            self.choosing = true;
            self.error = None;
            self.scroll = day.countries.iter().position(|c| c.id == city.country).unwrap_or(0).saturating_sub(MAX_LIST_ROWS - 1);
            used_click = true;
        }

        // A click on the button, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && ((enter && !self.choosing) || (clicked && (over_button || !card.contains(mouse)))) {
            self.city = None;
        }
        change
    }
}
