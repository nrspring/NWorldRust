//! The gatherer popup, opened by clicking a gatherer: what there is to know of it, a way to change which country
//! it belongs to.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::gatherer::GathererKind;
use crate::ids::{CountryId, GathererId};

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

/// What the player changed about the gatherer in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The gatherer now belongs to this country.
    Country(GathererId, CountryId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Country(gatherer, country) => day.set_gatherer_country(*gatherer, *country),
        }
    }
}

#[derive(Default)]
pub struct GathererPopup {
    /// The gatherer being shown, while the popup is up.
    gatherer: Option<GathererId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Whether the list of countries to change to is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl GathererPopup {
    pub fn is_open(&self) -> bool {
        self.gatherer.is_some()
    }

    pub fn open(&mut self, gatherer: GathererId) {
        *self = Self { gatherer: Some(gatherer), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries if it is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if self.choosing {
            self.choosing = false;
        } else {
            self.gatherer = None;
        }
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.choosing = false;
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A gatherer that is not in `day`
    /// is not shown. Returns what the player changed about the gatherer, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Change> {
        let Some(gatherer) = self.gatherer.and_then(|id| day.gatherer(id)) else {
            self.gatherer = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(gatherer.country);
        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little pine.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        let c = badge.center();
        match gatherer.kind {
            // A little pine, or a little rock.
            GathererKind::Woodcutter => {
                draw_triangle(vec2(c.x, c.y - 12.0), vec2(c.x - 9.0, c.y + 5.0), vec2(c.x + 9.0, c.y + 5.0), style::ACCENT_HOVER);
                draw_line(c.x, c.y + 5.0, c.x, c.y + 12.0, 2.5, style::ACCENT_HOVER);
            }
            GathererKind::Stonecutter => {
                draw_poly(c.x, c.y + 1.0, 6, 12.0, 0.0, style::ACCENT_HOVER);
                draw_line(c.x - 4.0, c.y - 6.0, c.x + 1.0, c.y + 1.0, 2.0, style::ACCENT_SOFT);
                draw_line(c.x + 1.0, c.y + 1.0, c.x - 2.0, c.y + 8.0, 2.0, style::ACCENT_SOFT);
            }
        }
        let text_x = badge.right() + 14.0;
        style.text(gatherer.kind.label(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&day.region_label(gatherer.region), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each. The country's can be clicked, to change it.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let country_row = Rect::new(inner_x, card.y + ROWS_TOP, inner_w, ROW_HEIGHT);
        let region_row = Rect::new(inner_x, country_row.bottom(), inner_w, ROW_HEIGHT);
        let status_row = Rect::new(inner_x, region_row.bottom(), inner_w, ROW_HEIGHT);
        for y in [country_row.y, region_row.y, status_row.y, status_row.bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }

        let over_country = country_row.contains(mouse);
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(country_row.x - 8.0, country_row.y + 1.0, country_row.w + 16.0, country_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = country_row.center().y;
        style.text_mid("Country", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        let name = country.map_or("an unknown country", |c| c.name.as_str());
        // At the right: the name, the country's color before it, and an arrow after it that says there is a list to
        // choose from.
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

        let region = day.region_name(gatherer.region).map_or_else(|| format!("Region {}", gatherer.region & 0x3FFF), str::to_owned);
        style.text_mid("Region", inner_x, region_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let region_x = region_row.right() - style.measure(&region, 14, Weight::Semibold).width;
        style.text_mid(&region, region_x, region_row.center().y, 14, Weight::Semibold, style::TEXT);

        let (status, status_color) = if day.gatherer_operating(gatherer.id) { ("Operating", style::TEXT) } else { ("No route to a city", style::ERROR) };
        style.text_mid("Status", inner_x, status_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let status_x = status_row.right() - style.measure(status, 14, Weight::Semibold).width;
        style.text_mid(status, status_x, status_row.center().y, 14, Weight::Semibold, status_color);

        if let Some(error) = &self.error {
            style.text(error, inner_x, status_row.bottom() + 24.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse) && !self.choosing;
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
                let place = self.scroll + row;
                let listed = &day.countries[place];
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
                if rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                }
                let mark = vec2(rect.x + 14.0, rect.center().y);
                if listed.id == gatherer.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, listed.color.color());
                style.text_mid(&listed.name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country it is already in is no change, so there is nothing to save.
                    if listed.id != gatherer.country {
                        change = Some(Change::Country(gatherer.id, listed.id));
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
            self.scroll = day.countries.iter().position(|c| c.id == gatherer.country).unwrap_or(0).saturating_sub(MAX_LIST_ROWS - 1);
            used_click = true;
        }

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && ((enter && !self.choosing) || (clicked && (over_close || !card.contains(mouse)))) {
            self.gatherer = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatherer::GathererKind;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        day
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_the_list_first() {
        assert!(!GathererPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = GathererPopup { gatherer: Some(GathererId::new()), choosing: true, ..GathererPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(popup.is_open() && !popup.choosing);
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_failure_closes_the_list_and_is_kept_to_show() {
        let mut popup = GathererPopup { gatherer: Some(GathererId::new()), choosing: true, ..GathererPopup::default() };
        popup.fail("no".to_owned());
        assert!(!popup.choosing && popup.error.as_deref() == Some("no"));
    }

    #[test]
    fn a_change_does_what_it_says_to_the_day() {
        let mut day = day();
        let (id, brevon) = (day.gatherers[0].id, day.countries[1].id);
        Change::Country(id, brevon).apply(&mut day).unwrap();
        assert_eq!(day.gatherer(id).unwrap().country, brevon);
        assert!(Change::Country(id, CountryId::new()).apply(&mut day).is_err());
        assert!(Change::Country(GathererId::new(), brevon).apply(&mut day).is_err(), "it is not there");
    }
}
