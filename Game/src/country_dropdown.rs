//! The country drop-down: clicking the country in the header opens a short list under it, to switch to another country in one click. The
//! last row goes on to the full list of countries, to add or delete one. Editing the country played is the pencil beside it (see
//! [`crate::country_menu`]).

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::country_menu::fit;
use crate::day::DaySave;
use crate::format::money;
use crate::hud;
use crate::ids::CountryId;

const ROW_HEIGHT: f32 = 32.0;
const PADDING: f32 = 6.0;
const MIN_WIDTH: f32 = 260.0;
/// The most countries it shows before it scrolls.
const MAX_ROWS: usize = 8;

/// What the player picked in the drop-down.
#[derive(Debug, PartialEq)]
pub enum Pick {
    /// Play this country.
    Country(CountryId),
    /// Go on to the full list of countries.
    Manage,
}

#[derive(Default)]
pub struct CountryDropdown {
    open: bool,
    /// How far down the list is scrolled.
    scroll: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
}

impl CountryDropdown {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.open && self.rect.contains(mouse)
    }

    /// Opens it if it is closed, and closes it if it is open: for a click on the country in the header.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.scroll = 0;
        self.rect = Rect::new(0.0, 0.0, 0.0, 0.0);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Draws it under `anchor`, the country in the header, and returns what the player picked, if they picked something, which also closes
    /// it. A click anywhere else closes it. It closes by itself if there is no country to switch from.
    pub fn show(&mut self, style: &Style, day: &DaySave, anchor: Option<Rect>) -> Option<Pick> {
        let Some(anchor) = anchor.filter(|_| self.open) else {
            self.open = false;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let current = day.player_country().map(|c| c.id);
        let count = day.countries.len();
        let rows = count.min(MAX_ROWS);

        let width = MIN_WIDTH.max(anchor.w);
        let height = (rows + 1) as f32 * ROW_HEIGHT + PADDING * 2.0 + 9.0;
        let panel = Rect::new(anchor.x, anchor.bottom() + 6.0, width, height);
        self.rect = panel;
        style::card(panel, 10.0, style::PANEL);
        style::rounded_rect_lines(panel, 10.0, 1.0, style::BORDER_STRONG);

        if panel.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(count - rows);

        let mut picked = None;
        for (row, country) in day.countries.iter().skip(self.scroll).take(rows).enumerate() {
            let rect = Rect::new(panel.x + PADDING, panel.y + PADDING + row as f32 * ROW_HEIGHT, panel.w - PADDING * 2.0, ROW_HEIGHT);
            let hovered = rect.contains(mouse);
            if hovered {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }
            let played = current == Some(country.id);
            // A round mark that is filled for the country that is played, as in the full list.
            let mark = vec2(rect.x + 16.0, rect.center().y);
            if played {
                draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                draw_circle(mark.x, mark.y, 2.4, WHITE);
            } else {
                draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
            }
            style::rounded_rect(Rect::new(rect.x + 32.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, country.color.color());
            let amount = money(country.money);
            let amount_width = style.measure(&amount, 12, Weight::Regular).width;
            style.text_mid(&amount, rect.right() - 10.0 - amount_width, rect.center().y, 12, Weight::Regular, if country.money < 0 { style::ERROR } else { style::TEXT_MUTED });
            hud::draw_coin(vec2(rect.right() - 10.0 - amount_width - 11.0, rect.center().y), 5.0);
            let weight = if played { Weight::Semibold } else { Weight::Regular };
            let name = fit(style, &country.name, 14, weight, rect.right() - 10.0 - amount_width - 26.0 - (rect.x + 52.0));
            style.text_mid(&name, rect.x + 52.0, rect.center().y, 14, weight, style::TEXT);
            if clicked && hovered {
                // The country that is played already is no change.
                picked = Some(if played { None } else { Some(Pick::Country(country.id)) });
            }
        }
        // More above or below than shows: a small mark at the edge.
        if self.scroll > 0 {
            draw_triangle(vec2(panel.right() - 18.0, panel.y + 9.0), vec2(panel.right() - 10.0, panel.y + 9.0), vec2(panel.right() - 14.0, panel.y + 4.0), style::TEXT_MUTED);
        }
        if self.scroll + rows < count {
            draw_triangle(vec2(panel.right() - 18.0, panel.y + PADDING + rows as f32 * ROW_HEIGHT - 5.0), vec2(panel.right() - 10.0, panel.y + PADDING + rows as f32 * ROW_HEIGHT - 5.0), vec2(panel.right() - 14.0, panel.y + PADDING + rows as f32 * ROW_HEIGHT), style::TEXT_MUTED);
        }

        // A line, and the row for the full list.
        let divider = panel.y + PADDING + rows as f32 * ROW_HEIGHT + 4.0;
        draw_line(panel.x + 12.0, divider, panel.right() - 12.0, divider, 1.0, style::BORDER);
        let manage = Rect::new(panel.x + PADDING, divider + 4.0, panel.w - PADDING * 2.0, ROW_HEIGHT);
        let over_manage = manage.contains(mouse);
        if over_manage {
            style::rounded_rect(manage, 6.0, style::HOVER);
        }
        style.text_mid("Manage countries…", manage.x + 12.0, manage.center().y, 13, Weight::Regular, if over_manage { style::TEXT } else { style::TEXT_MUTED });
        if clicked && over_manage {
            picked = Some(Some(Pick::Manage));
        }

        // A pick closes it, and so does a click anywhere but on it or on the country that opens it (which toggles it).
        if picked.is_some() || (clicked && !panel.contains(mouse) && !anchor.contains(mouse)) {
            self.open = false;
        }
        picked.flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_closed_until_toggled_and_toggling_again_closes_it() {
        let mut dropdown = CountryDropdown::default();
        assert!(!dropdown.is_open() && !dropdown.has_mouse(vec2(1.0, 1.0)));
        dropdown.toggle();
        assert!(dropdown.is_open());
        dropdown.toggle();
        assert!(!dropdown.is_open());
        dropdown.toggle();
        dropdown.close();
        assert!(!dropdown.is_open());
    }

    #[test]
    fn it_is_only_over_the_mouse_while_open() {
        let mut dropdown = CountryDropdown { rect: Rect::new(0.0, 0.0, 100.0, 100.0), ..CountryDropdown::default() };
        assert!(!dropdown.has_mouse(vec2(50.0, 50.0)), "closed");
        dropdown.open = true;
        assert!(dropdown.has_mouse(vec2(50.0, 50.0)));
        assert!(!dropdown.has_mouse(vec2(150.0, 50.0)));
    }
}
