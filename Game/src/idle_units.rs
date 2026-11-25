//! The units panel: the militia and ships of the country being played that still have movement points left today, so that the player
//! can see at a glance which units they haven't dealt with yet. It sits at the top left, under the header, collapsed to an icon with a
//! count on it until the player opens it. Clicking a unit in it selects the unit and takes the camera to it.
//!
//! The list is worked out from the game as it is, again each frame, and is never saved. A unit that has ended its moves for the day, or is a
//! garrison (see [`crate::militia::Militia::garrison`]), has none left and so is not in it.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::hud;
use crate::ids::{MilitiaId, ShipId};

const MARGIN: f32 = 16.0;
const PANEL_WIDTH: f32 = 280.0;
const BUTTON_SIZE: f32 = 34.0;
const HEADER_HEIGHT: f32 = 36.0;
const ROW_HEIGHT: f32 = 28.0;
const NOTE_HEIGHT: f32 = 26.0;
/// The most rows the panel shows however tall the window is, and the least it makes room for.
const MOST_ROWS: usize = 12;
const LEAST_ROWS: usize = 3;

/// A unit that has movement points left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    Militia(MilitiaId),
    Ship(ShipId),
}

/// A unit in the list, with what the list says of it.
#[derive(Debug, PartialEq)]
pub struct Entry {
    pub unit: Unit,
    pub name: String,
    /// What kind of unit it is, like "Militia" or "Small warship".
    pub kind: &'static str,
    /// The movement points it has left.
    pub points: u32,
}

/// The units of the country being played with movement points left, the militia first and then the ships, each in the order they
/// were raised or built. There are none if no country is being played.
pub fn current(day: &DaySave) -> Vec<Entry> {
    let Some(country) = day.player_country().map(|c| c.id) else {
        return Vec::new();
    };
    let militia = day.militias.iter().filter(|m| m.country == country && m.points > 0).map(|m| Entry { unit: Unit::Militia(m.id), name: m.title().to_owned(), kind: m.kind_label(), points: m.points });
    let ships = day.ships.iter().filter(|s| s.country == country && s.points > 0).map(|s| Entry { unit: Unit::Ship(s.id), name: s.title().to_owned(), kind: s.kind.label(), points: s.points });
    militia.chain(ships).collect()
}

/// The units panel.
#[derive(Default)]
pub struct IdleUnitsUi {
    expanded: bool,
    /// The first unit showing, when there are more than fit.
    first: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
}

impl IdleUnitsUi {
    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.rect.contains(mouse)
    }

    /// Draws it, and opens or closes it when the player clicks it. Returns the unit the player clicked, if they clicked one. When there
    /// are more units than fit, the mouse wheel over the panel scrolls through them. `covered` says that something is drawn over the
    /// mouse's place, which then isn't for this.
    pub fn show(&mut self, style: &Style, day: &DaySave, covered: bool) -> Option<Unit> {
        let mouse = if covered { vec2(f32::MIN, f32::MIN) } else { Vec2::from(mouse_position()) };
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !covered;
        let entries = current(day);
        let origin = vec2(MARGIN, hud::HEADER_HEIGHT + MARGIN);
        let color = if entries.is_empty() { style::TEXT_MUTED } else { style::ACCENT };

        if !self.expanded {
            let button = Rect::new(origin.x, origin.y, BUTTON_SIZE, BUTTON_SIZE);
            self.rect = button;
            let hovered = button.contains(mouse);
            style::card(button, 10.0, style::GLASS);
            if !entries.is_empty() {
                style::rounded_rect_lines(button, 10.0, 1.5, Color::new(style::ACCENT.r, style::ACCENT.g, style::ACCENT.b, 0.7));
            } else if hovered {
                style::rounded_rect_lines(button, 10.0, 1.0, style::BORDER_STRONG);
            }
            draw_pawn(button.center(), color);
            if !entries.is_empty() {
                let badge = vec2(button.right() - 3.0, button.y + 3.0);
                draw_circle(badge.x, badge.y, 8.0, style::ACCENT);
                let count = if entries.len() > 9 { "9+".to_owned() } else { entries.len().to_string() };
                style.text_centered(&count, Rect::new(badge.x - 8.0, badge.y - 8.0, 16.0, 16.0), 11, Weight::Semibold, WHITE);
            }
            if hovered && clicked {
                self.expanded = true;
            }
            return None;
        }

        // Open: a header that closes it again, and a row for each unit.
        let room = ((screen_height() - origin.y - hud::FOOTER_HEIGHT - MARGIN - HEADER_HEIGHT - 22.0 - NOTE_HEIGHT) / ROW_HEIGHT) as usize;
        let rows = room.clamp(LEAST_ROWS, MOST_ROWS);
        if self.rect.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.first = if mouse_wheel().1 > 0.0 { self.first.saturating_sub(1) } else { self.first + 1 };
        }
        self.first = self.first.min(entries.len().saturating_sub(rows));
        let shown = entries.len().saturating_sub(self.first).min(rows);
        let (before, after) = (self.first, entries.len() - self.first - shown);
        let body = if entries.is_empty() { ROW_HEIGHT } else { shown as f32 * ROW_HEIGHT };
        let note = if before + after > 0 { NOTE_HEIGHT } else { 0.0 };
        let panel = Rect::new(origin.x, origin.y, PANEL_WIDTH, HEADER_HEIGHT + 10.0 + body + note + 12.0);
        self.rect = panel;
        style::card(panel, 10.0, style::GLASS);

        let header = Rect::new(panel.x, panel.y, panel.w, HEADER_HEIGHT);
        let hovered = header.contains(mouse);
        if hovered {
            style::rounded_rect(Rect::new(header.x + 4.0, header.y + 4.0, header.w - 8.0, header.h - 8.0), 7.0, style::HOVER);
        }
        draw_pawn(vec2(panel.x + 22.0, header.center().y), color);
        style.text_mid("UNITS WITH MOVES LEFT", panel.x + 38.0, header.center().y, 10, Weight::Semibold, style::TEXT);
        // An arrow at the right that closes it.
        let arrow = vec2(panel.right() - 18.0, header.center().y);
        draw_triangle(vec2(arrow.x - 4.0, arrow.y - 2.0), vec2(arrow.x + 4.0, arrow.y - 2.0), vec2(arrow.x, arrow.y + 3.0), style::TEXT_MUTED);
        draw_line(panel.x + 12.0, header.bottom(), panel.right() - 12.0, header.bottom(), 1.0, style::BORDER);
        if hovered && clicked {
            self.expanded = false;
        }

        let top = header.bottom() + 10.0;
        if entries.is_empty() {
            let message = if day.player_country().is_some() { "Every unit has used its moves." } else { "No country is being played." };
            style.text_mid(message, panel.x + 12.0, top + ROW_HEIGHT * 0.5, 12, Weight::Regular, style::TEXT_MUTED);
            return None;
        }
        // Each unit can be clicked, which is for selecting it.
        let mut picked = None;
        for (k, entry) in entries[self.first..self.first + shown].iter().enumerate() {
            let row = Rect::new(panel.x + 6.0, top + k as f32 * ROW_HEIGHT, panel.w - 12.0, ROW_HEIGHT);
            let over = row.contains(mouse);
            if over {
                style::rounded_rect(row, 6.0, style::HOVER);
                if clicked {
                    picked = Some(entry.unit);
                }
            }
            let points = format!("{} pts", entry.points);
            let points_x = row.right() - 10.0 - style.measure(&points, 11, Weight::Regular).width;
            style.text_mid(&points, points_x, row.center().y, 11, Weight::Regular, style::TEXT_MUTED);
            let mut name = format!("{} · {}", entry.name, entry.kind);
            while name.chars().count() > 4 && style.measure(&name, 12, Weight::Regular).width > points_x - row.x - 22.0 {
                name.pop();
                name.pop();
                name.push('…');
            }
            style.text_mid(&name, row.x + 10.0, row.center().y, 12, Weight::Regular, style::TEXT);
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
        picked
    }
}

/// A pawn, about 16 pixels tall, centered on `c`.
fn draw_pawn(c: Vec2, color: Color) {
    draw_circle(c.x, c.y - 4.5, 3.6, color);
    draw_triangle(vec2(c.x - 6.0, c.y + 7.0), vec2(c.x + 6.0, c.y + 7.0), vec2(c.x, c.y - 1.0), color);
    draw_rectangle(c.x - 6.5, c.y + 6.0, 13.0, 2.5, color);
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
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 100_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 5_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 2_000;
        day
    }

    #[test]
    fn there_are_none_when_there_are_no_units() {
        assert!(current(&day()).is_empty());
    }

    #[test]
    fn it_lists_the_militia_and_then_the_ships_of_the_country_played_that_have_points_left() {
        let mut day = day();
        let city = day.cities[0].id;
        day.raise_militia(city, 50, vec2(30.0, 0.0)).unwrap();
        day.build_shipyard_claiming(ShipyardKind::Small, 100.0, 0.0, Some(0x4005)).unwrap();
        let yard = day.shipyards[0].id;
        day.build_ship(yard, ShipKind::Small, vec2(140.0, 0.0)).unwrap();
        let (militia, ship) = (day.militias[0].id, day.ships[0].id);

        let listed = current(&day);
        assert_eq!(listed.iter().map(|e| e.unit).collect::<Vec<_>>(), [Unit::Militia(militia), Unit::Ship(ship)]);
        assert_eq!((listed[0].kind, listed[0].points), ("Militia", 200));
        assert_eq!(listed[1].kind, "Small warship");

        // A unit with no points left is not in it, and neither is one that belongs to another country.
        day.end_militia_moves(militia).unwrap();
        assert_eq!(current(&day).iter().map(|e| e.unit).collect::<Vec<_>>(), [Unit::Ship(ship)]);
        let brevon = day.countries[1].id;
        day.set_ship_country(ship, brevon).unwrap();
        assert!(current(&day).is_empty());

        // A unit that has spent only some of its points still has some left, and a new day brings it back.
        let mut day = day;
        day.set_ship_country(ship, day.countries[0].id).unwrap();
        day.move_ship(ship, vec2(150.0, 0.0), 120, None).unwrap();
        assert_eq!(current(&day).iter().map(|e| (e.unit, e.points)).collect::<Vec<_>>(), [(Unit::Ship(ship), 80)]);
        day.rest_militia();
        assert_eq!(current(&day).len(), 2);
    }

    #[test]
    fn a_garrison_is_not_in_it() {
        let mut day = day();
        let city = day.cities[0].id;
        day.raise_militia(city, 50, vec2(30.0, 0.0)).unwrap();
        let id = day.militias[0].id;
        day.set_militia_garrison(id, true).unwrap();
        day.rest_militia();
        assert!(current(&day).is_empty());
    }
}
