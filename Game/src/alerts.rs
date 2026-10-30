//! Alerts: things in the game the player ought to know about or deal with. They are worked out from the
//! game as it is, again each frame, so they come and go as it changes and are never saved. They show in
//! a panel under the newsreel that is collapsed to an icon until the player opens it.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::failure::Thing;
use crate::hud;
use crate::ids::{BridgeId, CountryId};

/// The color of an alert's icon and marks.
const AMBER: Color = Color::new(0.96, 0.69, 0.20, 1.0);
/// Space between the newsreel and the alerts under it.
const GAP: f32 = 8.0;
const BUTTON_SIZE: f32 = 34.0;
const HEADER_HEIGHT: f32 = 36.0;
const LINE: f32 = 16.0;
const ITEM_GAP: f32 = 7.0;
const NOTE_HEIGHT: f32 = 26.0;

#[derive(Debug, PartialEq)]
pub enum Alert {
    /// A country that has no cities.
    CountryWithoutCities(CountryId),
    /// A path, a road or a bridge that has failed.
    Failed(Thing),
}

/// What clicking an alert shows: the thing the alert is about.
#[derive(Debug, PartialEq)]
pub enum AlertTarget {
    /// A country, in the countries menu.
    Country(CountryId),
    /// The region, by its number, that something failed in, which the camera goes to.
    Region(u16),
    /// A bridge, which the camera goes to the region of.
    Bridge(BridgeId),
}

impl Alert {
    /// Every alert there is in `day`, in a steady order: the countries that have no cities, then what has failed in
    /// the order it did.
    pub fn current(day: &DaySave) -> Vec<Alert> {
        let mut alerts: Vec<Alert> = day.countries.iter().filter(|c| !day.cities.iter().any(|city| city.country == c.id)).map(|c| Alert::CountryWithoutCities(c.id)).collect();
        alerts.extend(day.failed.iter().map(|thing| Alert::Failed(*thing)));
        alerts
    }

    /// What clicking the alert shows.
    pub fn target(&self) -> AlertTarget {
        match self {
            Alert::CountryWithoutCities(id) => AlertTarget::Country(*id),
            // A segment of path or road is in the region it is named for.
            Alert::Failed(Thing::Path(segment) | Thing::Road(segment)) => AlertTarget::Region(segment.region),
            Alert::Failed(Thing::PathLink(link) | Thing::RoadLink(link)) => AlertTarget::Region(link.region),
            Alert::Failed(Thing::Bridge(id)) => AlertTarget::Bridge(*id),
        }
    }

    /// What to tell the player.
    pub fn text(&self, day: &DaySave) -> String {
        match self {
            Alert::CountryWithoutCities(id) => format!("{} has no cities", day.country_name(*id)),
            Alert::Failed(thing) => format!("A {} has failed", thing.name()),
        }
    }
}

/// The alerts panel under the newsreel.
#[derive(Default)]
pub struct AlertsUi {
    expanded: bool,
    /// The first alert showing, when there are more than fit.
    first: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
}

impl AlertsUi {
    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.rect.contains(mouse)
    }

    /// Draws it under `newsreel`, at the right edge, and opens or closes it when the player clicks it.
    /// Returns what the alert the player clicked is about, if they clicked one. When there are more alerts than fit
    /// under the newsreel, the mouse wheel over the panel scrolls through them.
    pub fn show(&mut self, style: &Style, day: &DaySave, newsreel: Rect) -> Option<AlertTarget> {
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
        let heights: Vec<f32> = items.iter().map(|lines| lines.len() as f32 * LINE).collect();
        // As much room as there is under the newsreel, above the footer; past that, the wheel scrolls.
        let top = newsreel.bottom() + GAP;
        let room = screen_height() - top - hud::FOOTER_HEIGHT - 16.0 - HEADER_HEIGHT - 10.0 - 12.0 - NOTE_HEIGHT;
        let max_body = room.max(60.0);
        if self.rect.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.first = if mouse_wheel().1 > 0.0 { self.first.saturating_sub(1) } else { self.first + 1 };
        }
        // Scrolled no further than where the last alert is at the bottom of a full panel.
        let mut used = 0.0;
        let mut last_page = items.len();
        for i in (0..items.len()).rev() {
            let need = heights[i] + if i + 1 < items.len() { ITEM_GAP } else { 0.0 };
            if used + need > max_body {
                break;
            }
            used += need;
            last_page = i;
        }
        self.first = self.first.min(last_page.min(items.len().saturating_sub(1)));
        // The alerts that fit from there.
        let mut shown = 0;
        let mut body = 0.0;
        for h in &heights[self.first.min(heights.len())..] {
            let need = h + if shown > 0 { ITEM_GAP } else { 0.0 };
            if body + need > max_body && shown > 0 {
                break;
            }
            body += need;
            shown += 1;
        }
        let (before, after) = (self.first, items.len().saturating_sub(self.first + shown));
        if items.is_empty() {
            body = LINE;
        }
        let note = if before + after > 0 { NOTE_HEIGHT } else { 0.0 };
        let panel = Rect::new(newsreel.x, top, newsreel.w, HEADER_HEIGHT + 10.0 + body + note + 12.0);
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
        let range = self.first..self.first + shown;
        for (alert, lines) in alerts[range.clone()].iter().zip(&items[range]) {
            let height = lines.len() as f32 * LINE;
            let row = Rect::new(panel.x + 6.0, y - 3.0, panel.w - 12.0, height + 6.0);
            let over = row.contains(mouse);
            if over {
                style::rounded_rect(row, 6.0, style::HOVER);
                if clicked {
                    opened = Some(alert.target());
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
        // What is out of sight, and that the wheel brings it in.
        if before + after > 0 {
            let more = match (before, after) {
                (0, after) => format!("{after} more below. Scroll for them."),
                (before, 0) => format!("{before} earlier above. Scroll up for them."),
                (before, after) => format!("{before} above and {after} below. Scroll for them."),
            };
            style.text_mid(&more, panel.x + 14.0, panel.bottom() - 12.0 - NOTE_HEIGHT * 0.5, 10, Weight::Regular, style::TEXT_MUTED);
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
        assert_eq!(Alert::current(&day)[0].target(), AlertTarget::Country(day.countries[0].id));
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

    #[test]
    fn something_that_has_failed_is_an_alert_after_the_countries_without_cities() {
        use crate::path::PathSegment;
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let (road, path) = (Thing::Road(PathSegment::to_edge(3, 4)), Thing::Path(PathSegment::to_edge(1, 2)));
        day.fail(road);
        day.fail(path);
        let alerts = Alert::current(&day);
        assert_eq!(alerts, [Alert::CountryWithoutCities(day.countries[0].id), Alert::Failed(road), Alert::Failed(path)], "in the order they failed");
        assert_eq!(alerts[1].text(&day), "A road has failed");
        assert_eq!(alerts[2].text(&day), "A path has failed");
        // Built again, it is mended, and the alert goes.
        day.repair(road);
        assert_eq!(Alert::current(&day).len(), 2);
    }

    #[test]
    fn clicking_what_has_failed_goes_to_the_region_it_failed_in() {
        use crate::bridge::{BridgeLink, End};
        use crate::path::PathSegment;
        // A segment is in the region it is named for, whichever kind it is.
        assert_eq!(Alert::Failed(Thing::Path(PathSegment::to_edge(7, 8))).target(), AlertTarget::Region(7));
        assert_eq!(Alert::Failed(Thing::Road(PathSegment::across(9, 2, 3))).target(), AlertTarget::Region(9));
        let link = BridgeLink { bridge: BridgeId::new(), end: End::Second, region: 11, to: Some(12) };
        assert_eq!(Alert::Failed(Thing::PathLink(link)).target(), AlertTarget::Region(11));
        assert_eq!(Alert::Failed(Thing::RoadLink(link)).target(), AlertTarget::Region(11));
        // A bridge is found by its id, and its region found from where it stands.
        assert_eq!(Alert::Failed(Thing::Bridge(link.bridge)).target(), AlertTarget::Bridge(link.bridge));
    }
}
