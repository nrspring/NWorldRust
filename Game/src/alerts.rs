//! Alerts: things in the game the player ought to know about or deal with. They are worked out from the
//! game as it is, again each frame, so they come and go as it changes and are never saved. They show in
//! a panel under the newsreel that is collapsed to an icon until the player opens it.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::hud;
use crate::ids::CountryId;

/// The color of an alert's icon and marks.
const AMBER: Color = Color::new(0.96, 0.69, 0.20, 1.0);
/// Space between the newsreel and the alerts under it.
const GAP: f32 = 8.0;
const BUTTON_SIZE: f32 = 34.0;
const HEADER_HEIGHT: f32 = 36.0;
const LINE: f32 = 16.0;
const ITEM_GAP: f32 = 7.0;

#[derive(Debug, PartialEq)]
pub enum Alert {
    /// A country that has no cities.
    CountryWithoutCities(CountryId),
}

impl Alert {
    /// Every alert there is in `day`, in a steady order.
    pub fn current(day: &DaySave) -> Vec<Alert> {
        day.countries.iter().filter(|c| !day.cities.iter().any(|city| city.country == c.id)).map(|c| Alert::CountryWithoutCities(c.id)).collect()
    }

    /// The country the alert is about, which clicking it shows.
    pub fn country(&self) -> CountryId {
        match self {
            Alert::CountryWithoutCities(id) => *id,
        }
    }

    /// What to tell the player.
    pub fn text(&self, day: &DaySave) -> String {
        match self {
            Alert::CountryWithoutCities(id) => format!("{} has no cities", day.country_name(*id)),
        }
    }
}

/// The alerts panel under the newsreel.
#[derive(Default)]
pub struct AlertsUi {
    expanded: bool,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
}

impl AlertsUi {
    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.rect.contains(mouse)
    }

    /// Draws it under `newsreel`, at the right edge, and opens or closes it when the player clicks it.
    /// Returns the country of the alert the player clicked, if they clicked one.
    pub fn show(&mut self, style: &Style, day: &DaySave, newsreel: Rect) -> Option<CountryId> {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let alerts = Alert::current(day);
        let color = if alerts.is_empty() { style::TEXT_MUTED } else { AMBER };

        if !self.expanded {
            // Just an icon, with a count on it when there is something to see.
            let button = Rect::new(newsreel.right() - BUTTON_SIZE, newsreel.bottom() + GAP, BUTTON_SIZE, BUTTON_SIZE);
            self.rect = button;
            let hovered = button.contains(mouse);
            style::card(button, 10.0, style::GLASS);
            if !alerts.is_empty() {
                style::rounded_rect_lines(button, 10.0, 1.5, Color::new(AMBER.r, AMBER.g, AMBER.b, 0.7));
            } else if hovered {
                style::rounded_rect_lines(button, 10.0, 1.0, style::BORDER_STRONG);
            }
            draw_bell(button.center(), color);
            if !alerts.is_empty() {
                let badge = vec2(button.right() - 3.0, button.y + 3.0);
                draw_circle(badge.x, badge.y, 8.0, AMBER);
                let count = if alerts.len() > 9 { "9+".to_owned() } else { alerts.len().to_string() };
                style.text_centered(&count, Rect::new(badge.x - 8.0, badge.y - 8.0, 16.0, 16.0), 11, Weight::Semibold, Color::new(0.10, 0.08, 0.02, 1.0));
            }
            if hovered && clicked {
                self.expanded = true;
            }
            return None;
        }

        // Open: a header that closes it again, and each alert on lines of its own.
        let text_width = newsreel.w - 24.0 - 16.0;
        let items: Vec<Vec<String>> = alerts.iter().map(|a| hud::wrap(style, &a.text(day), 12, text_width)).collect();
        let body = if items.is_empty() {
            LINE
        } else {
            items.iter().map(|lines| lines.len() as f32 * LINE).sum::<f32>() + ITEM_GAP * (items.len() - 1) as f32
        };
        let panel = Rect::new(newsreel.x, newsreel.bottom() + GAP, newsreel.w, HEADER_HEIGHT + 10.0 + body + 12.0);
        self.rect = panel;
        style::card(panel, 10.0, style::GLASS);

        let header = Rect::new(panel.x, panel.y, panel.w, HEADER_HEIGHT);
        let hovered = header.contains(mouse);
        if hovered {
            style::rounded_rect(Rect::new(header.x + 4.0, header.y + 4.0, header.w - 8.0, header.h - 8.0), 7.0, style::HOVER);
        }
        draw_bell(vec2(panel.x + 22.0, header.center().y), color);
        style.text_mid("ALERTS", panel.x + 38.0, header.center().y, 10, Weight::Semibold, style::TEXT);
        // An arrow at the right that closes it.
        let arrow = vec2(panel.right() - 18.0, header.center().y);
        draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), style::TEXT_MUTED);
        draw_line(panel.x + 12.0, header.bottom(), panel.right() - 12.0, header.bottom(), 1.0, style::BORDER);
        if hovered && clicked {
            self.expanded = false;
        }

        let mut y = header.bottom() + 10.0;
        if items.is_empty() {
            style.text_mid("Nothing needs your attention.", panel.x + 12.0, y + LINE * 0.5, 12, Weight::Regular, style::TEXT_MUTED);
            return None;
        }
        // Each alert can be clicked, which is for showing what it is about.
        let mut opened = None;
        for (alert, lines) in alerts.iter().zip(&items) {
            let height = lines.len() as f32 * LINE;
            let row = Rect::new(panel.x + 6.0, y - 3.0, panel.w - 12.0, height + 6.0);
            let over = row.contains(mouse);
            if over {
                style::rounded_rect(row, 6.0, style::HOVER);
                if clicked {
                    opened = Some(alert.country());
                }
            }
            draw_circle(panel.x + 16.0, y + LINE * 0.5, 2.5, AMBER);
            for (k, line) in lines.iter().enumerate() {
                style.text_mid(line, panel.x + 28.0, y + LINE * (k as f32 + 0.5), 12, Weight::Regular, style::TEXT);
            }
            // A small arrow at the right, to say it can be clicked.
            let (ax, ay) = (panel.right() - 16.0, y + height * 0.5);
            let arrow = if over { style::TEXT } else { style::TEXT_MUTED };
            draw_line(ax - 2.0, ay - 4.0, ax + 2.0, ay, 1.5, arrow);
            draw_line(ax + 2.0, ay, ax - 2.0, ay + 4.0, 1.5, arrow);
            y += height + ITEM_GAP;
        }
        opened
    }
}

/// A bell, about 16 pixels across, centered on `c`.
fn draw_bell(c: Vec2, color: Color) {
    draw_circle(c.x, c.y - 9.0 + 3.0, 1.5, color);
    draw_circle(c.x, c.y - 1.5, 5.5, color);
    draw_rectangle(c.x - 5.5, c.y - 1.5, 11.0, 5.5, color);
    draw_rectangle(c.x - 7.5, c.y + 3.0, 15.0, 2.5, color);
    draw_circle(c.x, c.y + 7.5, 2.0, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn there_are_no_alerts_when_there_are_no_countries() {
        assert!(Alert::current(&day()).is_empty());
    }

    #[test]
    fn an_alert_is_about_its_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        assert_eq!(Alert::current(&day)[0].country(), day.countries[0].id);
    }

    #[test]
    fn a_country_with_no_cities_is_an_alert() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert_eq!(Alert::current(&day), [Alert::CountryWithoutCities(aldoria), Alert::CountryWithoutCities(brevon)]);
        assert_eq!(Alert::current(&day)[0].text(&day), "Aldoria has no cities");
    }

    #[test]
    fn a_city_clears_its_countrys_alert_and_changing_hands_moves_it() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        assert_eq!(Alert::current(&day), [Alert::CountryWithoutCities(brevon)]);
        // Giving the only city to Brevon leaves Aldoria without one.
        day.set_city_country(day.cities[0].id, brevon).unwrap();
        assert_eq!(Alert::current(&day), [Alert::CountryWithoutCities(aldoria)]);
        day.set_city_country(day.cities[0].id, aldoria).unwrap();
        day.found_country("Cardell").unwrap();
        day.found_city("Highmark", 5.0, 0.0, None).unwrap();
        assert_eq!(Alert::current(&day).len(), 2, "Brevon and Cardell have none");
    }
}
