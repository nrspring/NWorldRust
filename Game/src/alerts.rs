//! Alerts: things in the game the player ought to know about or deal with. They are worked out from the
//! game as it is, again each frame, so they come and go as it changes and are never saved. They show in
//! a panel under the newsreel that is collapsed to an icon until the player opens it.
//!
//! Some are about how things are (a country has no cities), and last as long as that is so. Others are about what
//! happened on the last day to end (something failed), and are cleared when the next day ends: the things that failed
//! before stay ruined, but are no longer news.

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::bridge::End;
use crate::day::DaySave;
use crate::failure::Thing;
use crate::format;
use crate::hud;
use crate::ids::{FortId, BridgeId, CityId, CountryId, GathererId, MilitiaId, PersonId};
use crate::path::PathPiece;
use crate::pieces;
use crate::upkeep;
use crate::gatherer::GathererKind;
use crate::{gatherer_view, path_view, region_labels};

/// The color of an alert's icon and marks.
const AMBER: Color = Color::new(0.96, 0.69, 0.20, 1.0);
/// Space between the newsreel and the alerts under it.
const GAP: f32 = 8.0;
const BUTTON_SIZE: f32 = 34.0;
const HEADER_HEIGHT: f32 = 36.0;
const LINE: f32 = 16.0;
const ITEM_GAP: f32 = 7.0;
const NOTE_HEIGHT: f32 = 26.0;
/// How long the warning icon on the map takes to fade out and in again, in seconds.
const FLASH_PERIOD: f32 = 2.4;
/// How high above the ground the warning icon is held, in meters, and how big it is, in pixels.
const MARK_HOLD: f32 = 3.0;
/// How far over the top of a gatherer, which is tall, its warning icon is held.
const GATHERER_MARK_CLEARANCE: f32 = 3.0;
/// How high over the middle of a city under siege, which has buildings, and over a militia under siege, its warning icon is held.
const CITY_MARK_HOLD: f32 = 40.0;
const MILITIA_MARK_HOLD: f32 = 12.0;
/// How high over a fort its mark is held: over the top of its keep.
const FORT_MARK_HOLD: f32 = 20.0;
const MARK_WIDTH: f32 = 24.0;
const MARK_HEIGHT: f32 = 21.0;
/// The least a warning icon shrinks to, against its full size, from far out.
const LEAST_MARK: f32 = 0.4;

#[derive(Debug, PartialEq)]
pub enum Alert {
    /// A country that has no cities.
    CountryWithoutCities(CountryId),
    /// A person who has not been put on the map.
    PersonUnplaced(PersonId),
    /// A city that is under siege, and so is losing people.
    CityUnderSiege(CityId),
    /// A militia that is under siege, and so is losing members.
    MilitiaUnderSiege(MilitiaId),
    /// A fort that is under siege, and so is using up its supplies, or has none left.
    FortUnderSiege(FortId),
    /// A country that can't afford the upkeep of all its units, and will lose some unless it picks which to pay for.
    UpkeepShort(CountryId),
    /// A path, a road or a bridge that has failed.
    Failed(Thing),
    /// A gatherer that has no unbroken route to a city of its country, and so isn't operating.
    GathererCutOff(GathererId),
}

/// What clicking an alert shows: the thing the alert is about.
#[derive(Debug, PartialEq)]
pub enum AlertTarget {
    /// A country, in the countries menu.
    Country(CountryId),
    /// A person to put on the map, with a click.
    PlacePerson(PersonId),
    /// A country's upkeep, in the list of its units to pick which to pay for.
    Upkeep(CountryId),
    /// A gatherer, which the camera goes to.
    Gatherer(GathererId),
    /// A city, which the camera goes to.
    City(CityId),
    /// A militia, which the camera goes to.
    Militia(MilitiaId),
    Fort(FortId),
    /// The region, by its number, that something failed in, which the camera goes to.
    Region(u16),
    /// A bridge, which the camera goes to the region of.
    Bridge(BridgeId),
}

impl Alert {
    /// Every alert there is in `day`, in a steady order: the countries that have no cities, then the people who are not on the map,
    /// then the cities under siege,
    /// then the militia under siege, then the countries that can't pay their upkeep, then the gatherers that are cut off from a city, then what failed when the last day ended (`failures`, in the order it did), as long as
    /// it is still ruined.
    pub fn current(day: &DaySave, failures: &[Thing]) -> Vec<Alert> {
        let mut alerts: Vec<Alert> = day.countries.iter().filter(|c| !day.cities.iter().any(|city| city.country == c.id)).map(|c| Alert::CountryWithoutCities(c.id)).collect();
        alerts.extend(day.people.iter().filter(|p| p.at.is_none()).map(|p| Alert::PersonUnplaced(p.id)));
        alerts.extend(day.cities.iter().filter(|c| c.under_siege).map(|c| Alert::CityUnderSiege(c.id)));
        alerts.extend(day.militias.iter().filter(|m| day.is_besieged(m)).map(|m| Alert::MilitiaUnderSiege(m.id)));
        alerts.extend(day.forts.iter().filter(|f| f.under_siege).map(|f| Alert::FortUnderSiege(f.id)));
        alerts.extend(upkeep::short_countries(day).into_iter().map(Alert::UpkeepShort));
        // Whether a gatherer has a route is kept up to date on it (see `RouteWatch`), which is far cheaper than working the
        // routes out again, which this is asked for every frame.
        alerts.extend(day.gatherers.iter().filter(|g| !g.operating).map(|g| Alert::GathererCutOff(g.id)));
        alerts.extend(failures.iter().filter(|thing| day.is_failed(**thing)).map(|thing| Alert::Failed(*thing)));
        alerts
    }

    /// What failed in going from `before` to `after`, the day that ended and the day that followed it.
    pub fn new_failures(before: &DaySave, after: &DaySave) -> Vec<Thing> {
        after.failed.iter().filter(|thing| !before.is_failed(**thing)).copied().collect()
    }

    /// What clicking the alert shows.
    pub fn target(&self) -> AlertTarget {
        match self {
            Alert::CountryWithoutCities(id) => AlertTarget::Country(*id),
            Alert::PersonUnplaced(id) => AlertTarget::PlacePerson(*id),
            Alert::UpkeepShort(id) => AlertTarget::Upkeep(*id),
            Alert::GathererCutOff(id) => AlertTarget::Gatherer(*id),
            Alert::CityUnderSiege(id) => AlertTarget::City(*id),
            Alert::MilitiaUnderSiege(id) => AlertTarget::Militia(*id),
            Alert::FortUnderSiege(id) => AlertTarget::Fort(*id),
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
            Alert::PersonUnplaced(id) => match day.person(*id) {
                Some(person) => format!("{} ({}) is not on the map. Click to place them", person.name, day.person_country_name(person)),
                None => "A person is not on the map".to_owned(),
            },
            Alert::CityUnderSiege(id) => match day.city(*id) {
                Some(city) => format!("{} ({}) is under siege and is losing people", city.name, day.country_name(city.country)),
                None => "A city is under siege".to_owned(),
            },
            Alert::MilitiaUnderSiege(id) => match day.militia(*id) {
                Some(militia) => format!("{} ({}) is under siege and is losing members", militia.title(), day.country_name(militia.country)),
                None => "A militia is under siege".to_owned(),
            },
            Alert::FortUnderSiege(id) => match day.fort(*id) {
                Some(fort) if fort.starved() => format!("{} ({}) is under siege and out of supplies", fort.title(), day.country_name(fort.country)),
                Some(fort) => format!("{} ({}) is under siege, with {}% of its supplies left", fort.title(), day.country_name(fort.country), fort.supplies_percent()),
                None => "A fort is under siege".to_owned(),
            },
            Alert::UpkeepShort(id) => {
                let outlook = upkeep::outlook(day, *id);
                format!("{} can't pay to keep all its units: {} coins a day, and {} coins to pay it with. Click to pick which to pay for", day.country_name(*id), format::money(outlook.needed), format::money(outlook.available))
            }
            Alert::Failed(thing) => format!("A {} has failed", thing.name()),
            Alert::GathererCutOff(id) => match day.gatherer(*id) {
                Some(w) => format!("The {} in {} has no path, road or bridge to a city of {}", w.kind.noun(), day.region_label(w.region), day.country_name(w.country)),
                None => "A gatherer has no path to a city".to_owned(),
            },
        }
    }

    /// Where the thing the alert is about is on the map, in world x and z, and how high above the ground to hold the
    /// icon, in meters: the middle of a failed path, road or bridge, a city or militia under siege, or a gatherer, which is held high to clear it.
    /// A country with no cities and a person who is not on the map are nowhere on it, so they have none.
    pub fn place(&self, day: &DaySave, map: &RegionMap) -> Option<(Vec2, f32)> {
        let piece = match self {
            Alert::CountryWithoutCities(_) | Alert::PersonUnplaced(_) | Alert::UpkeepShort(_) => return None,
            Alert::CityUnderSiege(id) => return day.city(*id).map(|c| (vec2(c.x, c.z), CITY_MARK_HOLD)),
            Alert::MilitiaUnderSiege(id) => return day.militia(*id).map(|m| (vec2(m.x, m.z), MILITIA_MARK_HOLD)),
            Alert::FortUnderSiege(id) => return day.fort(*id).map(|f| (vec2(f.x, f.z), FORT_MARK_HOLD)),
            Alert::GathererCutOff(id) => return day.gatherer(*id).map(|w| (vec2(w.x, w.z), gatherer_mark_hold(w.kind))),
            Alert::Failed(Thing::Path(segment) | Thing::Road(segment)) => PathPiece::Segment(*segment),
            Alert::Failed(Thing::PathLink(link) | Thing::RoadLink(link)) => PathPiece::Link(*link),
            Alert::Failed(Thing::Bridge(id)) => {
                let bridge = day.bridge(*id)?;
                return Some(((bridge.end(End::First) + bridge.end(End::Second)) * 0.5, MARK_HOLD));
            }
        };
        path_view::piece_ends(map, &day.bridges, piece).map(|(from, to)| ((from + to) * 0.5, MARK_HOLD))
    }
}

/// The regions that have an alert about something in them, each once, in order of their numbers: where something failed, a city or
/// militia is under siege, or a gatherer is cut off. An alert that is not about a place (a country with no cities) is in none. The map
/// makes them glow while the player holds the mouse over "Alerts" in the footer.
pub fn regions(day: &DaySave, failures: &[Thing], map: &RegionMap, world: &World) -> Vec<RegionId> {
    let mut regions: Vec<RegionId> = Alert::current(day, failures)
        .iter()
        .filter_map(|alert| match alert.target() {
            AlertTarget::Region(number) => RegionId::from_raw(number),
            _ => alert.place(day, map).and_then(|(at, _)| world.region_at(at.x, at.y)),
        })
        .collect();
    regions.sort_by_key(|r| r.raw());
    regions.dedup();
    regions
}

/// How high above the ground to hold the warning icon over a gatherer of `kind`: above the top of its flag pole, which is
/// higher on a greater one.
fn gatherer_mark_hold(kind: GathererKind) -> f32 {
    gatherer_view::height(kind) + GATHERER_MARK_CLEARANCE
}

/// Draws a red warning icon, flashing slowly, over each alert there is that is about a place on the map, as `camera`
/// sees it. `time` is the seconds the game has run.
pub fn draw_marks(day: &DaySave, failures: &[Thing], map: &RegionMap, world: &World, camera: &Camera3D, time: f32) {
    // Fading in and out, never quite gone, so that it is always there to be found.
    let flash = 0.5 + 0.5 * (time * std::f32::consts::TAU / FLASH_PERIOD).sin();
    let strength = 0.25 + 0.75 * flash;
    for (at, hold) in Alert::current(day, failures).iter().filter_map(|alert| alert.place(day, map)) {
        let point = vec3(at.x, world.surface_height(at.x, at.y) + hold, at.y);
        let Some(on_screen) = region_labels::to_screen(camera, point) else { continue };
        if on_screen.x < -MARK_WIDTH || on_screen.x > screen_width() + MARK_WIDTH || on_screen.y < 0.0 || on_screen.y > screen_height() + MARK_HEIGHT {
            continue;
        }
        let scale = mark_scale(camera.position.distance(point));
        draw_warning(vec2(on_screen.x, on_screen.y - 4.0 * scale), strength, scale);
    }
}

/// How big a warning icon is, against its full size, for a point `distance` meters from the camera: it shrinks as the camera
/// goes out, as the pieces on the map do, but never to less than a share of its size that can still be read.
pub fn mark_scale(distance: f32) -> f32 {
    pieces::screen_scale(distance).max(LEAST_MARK)
}

/// A warning triangle with an exclamation mark in it, standing with the middle of its base at `base`, `scale` times its full size.
fn draw_warning(base: Vec2, strength: f32, scale: f32) {
    let (half, height) = (MARK_WIDTH * 0.5 * scale, MARK_HEIGHT * scale);
    let top = vec2(base.x, base.y - height);
    let (left, right) = (vec2(base.x - half, base.y), vec2(base.x + half, base.y));
    let edge = 3.0 * scale;
    // A dark edge first, so that it shows on any ground.
    draw_triangle(top + vec2(0.0, -edge * 0.8), left + vec2(-edge, edge * 0.5), right + vec2(edge, edge * 0.5), Color::new(0.10, 0.02, 0.02, 0.7 * strength));
    draw_triangle(top, left, right, Color::new(0.88, 0.10, 0.10, strength));
    // The exclamation mark is left out when the icon is too small for it to be read.
    if scale >= 0.6 {
        let white = Color::new(1.0, 1.0, 1.0, strength);
        draw_rectangle(base.x - 1.3 * scale, base.y - 15.0 * scale, 2.6 * scale, 8.0 * scale, white);
        draw_circle(base.x, base.y - 3.8 * scale, 1.5 * scale, white);
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

    /// Goes back to the first alert, for when the alerts have been cleared by a new day.
    pub fn reset(&mut self) {
        self.first = 0;
    }

    /// Draws it under `newsreel`, at the right edge, and opens or closes it when the player clicks it.
    /// Returns what the alert the player clicked is about, if they clicked one. When there are more alerts than fit
    /// under the newsreel, the mouse wheel over the panel scrolls through them.
    pub fn show(&mut self, style: &Style, day: &DaySave, failures: &[Thing], newsreel: Rect) -> Option<AlertTarget> {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let alerts = Alert::current(day, failures);
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
    use crate::gatherer::GathererKind;
    use crate::bridge::Spot;
    use crate::calendar::Date;
    use crate::path::PathSegment;

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn a_besieged_fort_is_an_alert_and_its_garrison_one_only_once_the_supplies_are_gone() {
        use nworld_map::world::Resource;
        let mut day = DaySave::new(crate::calendar::Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let country = day.countries[0].id;
        day.add_money(country, 20_000).unwrap();
        day.add_resource(country, Resource::Stone, 500).unwrap();
        day.add_resource(country, Resource::Forest, 500).unwrap();
        day.claim_region(0x4005).unwrap();
        day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
        day.found_city("Port Aldor", 500.0, 0.0, Some(0x4005)).unwrap();
        day.cities[0].population = 2_000;
        day.raise_militia(day.cities[0].id, 100, macroquad::math::vec2(30.0, 0.0)).unwrap();
        let (fort, militia) = (day.forts[0].id, day.militias[0].id);
        day.enter_fort(fort, militia, Some(0x4005)).unwrap();
        day.set_fort_under_siege(fort, true).unwrap();
        let sieges = |day: &DaySave| Alert::current(day, &[]).into_iter().filter(|a| matches!(a, Alert::FortUnderSiege(_) | Alert::MilitiaUnderSiege(_))).collect::<Vec<_>>();
        assert_eq!(sieges(&day), [Alert::FortUnderSiege(fort)]);
        assert_eq!(sieges(&day)[0].text(&day), "Small fort (Aldoria) is under siege, with 100% of its supplies left");
        assert_eq!(sieges(&day)[0].target(), AlertTarget::Fort(fort));
        day.forts[0].supplies_used = 500;
        assert_eq!(sieges(&day), [Alert::MilitiaUnderSiege(militia), Alert::FortUnderSiege(fort)]);
        assert_eq!(sieges(&day)[1].text(&day), "Small fort (Aldoria) is under siege and out of supplies");
    }

    #[test]
    fn a_country_that_cannot_pay_to_keep_all_its_units_is_an_alert_that_opens_the_list_of_its_units() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 10_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 1_000;
        day.raise_militia(day.cities[0].id, 100, vec2(30.0, 0.0)).unwrap();
        assert!(Alert::current(&day, &[]).is_empty(), "it can pay");

        // With no money and 100 people it earns 100 coins, and the 100 members cost 100: just enough. Then one more member is too many.
        let money = day.country(aldoria).unwrap().money;
        day.add_money(aldoria, -money).unwrap();
        day.cities[0].population = 99;
        let alerts = Alert::current(&day, &[]);
        assert_eq!(alerts, [Alert::UpkeepShort(aldoria)]);
        assert_eq!(alerts[0].target(), AlertTarget::Upkeep(aldoria));
        assert_eq!(alerts[0].text(&day), "Aldoria can't pay to keep all its units: 100 coins a day, and 99 coins to pay it with. Click to pick which to pay for");
        assert_eq!(alerts[0].place(&day, &RegionMap::default()), None, "it is not at a place on the map");
        day.cities[0].population = 100;
        assert!(Alert::current(&day, &[]).is_empty());
    }

    #[test]
    fn the_regions_with_alerts_are_those_a_city_or_militia_under_siege_stands_in_and_not_a_country_with_no_cities() {
        let world = crate::test_support::regioned_continent();
        let map = RegionMap::build(&world);
        let spots = crate::test_support::land_spots(&world, 8);
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 1_000_000).unwrap();
        assert!(regions(&day, &[], &map, &world).is_empty(), "Brevon has no cities, but that is not a place");
        // (Aldoria has no cities yet either: neither country is anywhere.)
        day.found_city("Port Aldor", spots[0].x, spots[0].y, None).unwrap();
        day.cities[0].population = 2_000;
        assert!(regions(&day, &[], &map, &world).is_empty(), "a city that is not under siege has none");
        day.cities[0].under_siege = true;
        assert_eq!(regions(&day, &[], &map, &world), [world.region_at(spots[0].x, spots[0].y).unwrap()]);
        // A militia under siege in the same region adds nothing.
        day.raise_militia(day.cities[0].id, 50, spots[0]).unwrap();
        let id = day.militias[0].id;
        day.set_militia_under_siege(id, true).unwrap();
        assert_eq!(regions(&day, &[], &map, &world).len(), 1);
    }

    #[test]
    fn the_warning_icon_shrinks_as_the_camera_goes_out_but_stays_big_enough_to_see() {
        assert_eq!(mark_scale(100.0), 1.0);
        assert!(mark_scale(2_000.0) < 0.7 && mark_scale(5_000.0) < mark_scale(2_000.0));
        assert_eq!(mark_scale(1_000_000.0), LEAST_MARK);
    }

    #[test]
    fn there_are_no_alerts_when_there_are_no_countries() {
        assert!(Alert::current(&day(), &[]).is_empty());
    }

    #[test]
    fn a_failed_bridge_is_marked_at_its_middle_and_a_country_with_no_cities_is_marked_nowhere() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: -30.0, z: 10.0 }, Spot { x: 30.0, z: 20.0 }).unwrap();
        let map = RegionMap::default();
        assert_eq!(Alert::CountryWithoutCities(day.countries[0].id).place(&day, &map), None);
        assert_eq!(Alert::Failed(Thing::Bridge(day.bridges[0].id)).place(&day, &map), Some((vec2(0.0, 15.0), MARK_HOLD)));
        // A path on a map with no regions is nowhere either.
        assert_eq!(Alert::Failed(Thing::Path(PathSegment::to_edge(1, 2))).place(&day, &map), None);
    }

    #[test]
    fn a_gatherer_with_no_route_to_a_city_is_an_alert_held_high_over_it_until_a_route_is_built() {
        use crate::path::{Kind, PathSegment};
        const CITY: u16 = 0xC001;
        const CUTTER: u16 = 0xC002;
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(CITY)).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 100.0, 0.0, CUTTER).unwrap();
        let id = day.gatherers[0].id;

        let alerts = Alert::current(&day, &[]);
        assert_eq!(alerts, [Alert::GathererCutOff(id)]);
        assert_eq!(alerts[0].text(&day), "The woodcutter in region 2 has no path, road or bridge to a city of Aldoria");
        assert_eq!(alerts[0].target(), AlertTarget::Gatherer(id));
        assert_eq!(alerts[0].place(&day, &RegionMap::default()), Some((vec2(100.0, 0.0), gatherer_mark_hold(GathererKind::Woodcutter))));
        assert!(gatherer_mark_hold(GathererKind::Woodcutter) > gatherer_view::height(GathererKind::Woodcutter), "above the top of it");
        assert!(gatherer_mark_hold(GathererKind::Quarry) > gatherer_mark_hold(GathererKind::Stonecutter), "the icon is higher over a bigger one");
        day.set_region_name(CUTTER, "The Marches").unwrap();
        assert_eq!(Alert::current(&day, &[])[0].text(&day), "The woodcutter in The Marches has no path, road or bridge to a city of Aldoria");

        // A stonecutter is an alert of its own, which says what it is.
        day.build_gatherer_claiming(GathererKind::Stonecutter, 130.0, 0.0, CUTTER).unwrap();
        let stone = day.gatherers[1].id;
        assert_eq!(Alert::current(&day, &[]), [Alert::GathererCutOff(id), Alert::GathererCutOff(stone)]);
        assert_eq!(Alert::GathererCutOff(stone).text(&day), "The stonecutter in The Marches has no path, road or bridge to a city of Aldoria");
        assert_eq!(Alert::GathererCutOff(stone).place(&day, &RegionMap::default()), Some((vec2(130.0, 0.0), gatherer_mark_hold(GathererKind::Stonecutter))));
        day.destroy_gatherer(stone).unwrap();

        // After the countries that have no city, and before what failed.
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(brevon), Alert::GathererCutOff(id)]);

        // A route from the gatherer's region to the city's, of path in part and road in part, ends the alert.
        day.select_country(day.countries[0].id).unwrap();
        // Through a region between them, ending at the border of the city's.
        const BETWEEN: u16 = 0xC003;
        day.build_segment(Kind::Path, PathSegment::to_edge(CUTTER, BETWEEN)).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(BETWEEN, CUTTER)).unwrap();
        day.build_segment(Kind::Road, PathSegment::to_edge(BETWEEN, CITY)).unwrap();
        // (The game brings the gatherers up to date with their routes every frame.)
        day.update_gatherer_operation();
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(brevon)]);
        // And it is back when part of that fails.
        let failed = Thing::Path(PathSegment::to_edge(BETWEEN, CUTTER));
        day.fail(failed);
        day.update_gatherer_operation();
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(brevon), Alert::GathererCutOff(id)]);
    }

    #[test]
    fn a_city_or_militia_under_siege_is_an_alert_marked_where_it_stands_until_the_siege_is_lifted() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.found_city("Port Aldor", 10.0, 20.0, None).unwrap();
        day.found_city("Highmark", 300.0, 0.0, None).unwrap();
        day.cities[0].population = 2_000;
        day.raise_militia(day.cities[0].id, 100, vec2(50.0, 20.0)).unwrap();
        assert!(Alert::current(&day, &[]).is_empty(), "nothing is under siege to begin with");

        let (port, highmark, guard) = (day.cities[0].id, day.cities[1].id, day.militias[0].id);
        day.set_city_under_siege(highmark, true).unwrap();
        day.set_city_under_siege(port, true).unwrap();
        day.set_militia_under_siege(guard, true).unwrap();
        // The cities in the order they were founded, then the militia.
        let alerts = Alert::current(&day, &[]);
        assert_eq!(alerts, [Alert::CityUnderSiege(port), Alert::CityUnderSiege(highmark), Alert::MilitiaUnderSiege(guard)]);
        assert_eq!(alerts[0].text(&day), "Port Aldor (Aldoria) is under siege and is losing people");
        assert_eq!(alerts[2].text(&day), format!("{} (Aldoria) is under siege and is losing members", day.militias[0].name));
        assert_eq!((alerts[0].target(), alerts[2].target()), (AlertTarget::City(port), AlertTarget::Militia(guard)));
        let map = RegionMap::default();
        assert_eq!(alerts[0].place(&day, &map), Some((vec2(10.0, 20.0), CITY_MARK_HOLD)));
        assert_eq!(alerts[2].place(&day, &map), Some((vec2(50.0, 20.0), MILITIA_MARK_HOLD)));

        // They come after the countries that have no cities, and before the gatherers and what failed.
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        assert_eq!(Alert::current(&day, &[])[0], Alert::CountryWithoutCities(brevon));
        assert_eq!(Alert::current(&day, &[]).len(), 4);

        // Lifting a siege takes its alert away; a militia moved is marked where it is now.
        day.set_city_under_siege(port, false).unwrap();
        day.set_militia_under_siege(guard, false).unwrap();
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(brevon), Alert::CityUnderSiege(highmark)]);
        // One that is gone is no alert.
        day.set_militia_under_siege(guard, true).unwrap();
        day.disband_militia(guard).unwrap();
        assert_eq!(Alert::current(&day, &[]).len(), 2);
    }

    #[test]
    fn a_person_who_is_not_on_the_map_is_an_alert_for_placing_them_until_they_are_placed() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        let alerts = Alert::current(&day, &[]);
        assert_eq!(alerts, [Alert::PersonUnplaced(id)]);
        assert_eq!(alerts[0].target(), AlertTarget::PlacePerson(id));
        assert_eq!(alerts[0].text(&day), "Mara Voss (Aldoria) is not on the map. Click to place them");
        assert_eq!(alerts[0].place(&day, &RegionMap::default()), None);
        day.place_person(id, vec2(600.0, 0.0), None).unwrap();
        assert!(Alert::current(&day, &[]).is_empty());
    }

    #[test]
    fn an_alert_is_about_its_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        assert_eq!(Alert::current(&day, &[])[0].target(), AlertTarget::Country(day.countries[0].id));
    }

    #[test]
    fn a_country_with_no_cities_is_an_alert() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(aldoria), Alert::CountryWithoutCities(brevon)]);
        assert_eq!(Alert::current(&day, &[])[0].text(&day), "Aldoria has no cities");
    }

    #[test]
    fn a_city_clears_its_countrys_alert_and_changing_hands_moves_it() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(brevon)]);
        // Giving the only city to Brevon leaves Aldoria without one.
        day.set_city_country(day.cities[0].id, brevon).unwrap();
        assert_eq!(Alert::current(&day, &[]), [Alert::CountryWithoutCities(aldoria)]);
        day.set_city_country(day.cities[0].id, aldoria).unwrap();
        day.found_country("Cardell").unwrap();
        day.found_city("Highmark", 5.0, 0.0, None).unwrap();
        assert_eq!(Alert::current(&day, &[]).len(), 2, "Brevon and Cardell have none");
    }

    #[test]
    fn something_that_has_failed_is_an_alert_after_the_countries_without_cities() {
        use crate::path::PathSegment;
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let (road, path) = (Thing::Road(PathSegment::to_edge(3, 4)), Thing::Path(PathSegment::to_edge(1, 2)));
        day.fail(road);
        day.fail(path);
        let failures = [road, path];
        let alerts = Alert::current(&day, &failures);
        assert_eq!(alerts, [Alert::CountryWithoutCities(day.countries[0].id), Alert::Failed(road), Alert::Failed(path)], "in the order they failed");
        assert_eq!(alerts[1].text(&day), "A road has failed");
        assert_eq!(alerts[2].text(&day), "A path has failed");
        // Built again, it is mended, and the alert goes.
        day.repair(road);
        assert_eq!(Alert::current(&day, &failures).len(), 2);
    }

    #[test]
    fn what_failed_on_an_earlier_day_is_still_ruined_but_no_longer_an_alert() {
        use crate::path::PathSegment;
        let mut yesterday = day();
        let old = Thing::Path(PathSegment::to_edge(1, 2));
        yesterday.fail(old);
        // The next day begins with the same ruins, and one more thing fails as it begins.
        let mut today = yesterday.clone();
        let new = Thing::Road(PathSegment::to_edge(3, 4));
        today.fail(new);
        let failures = Alert::new_failures(&yesterday, &today);
        assert_eq!(failures, [new], "only what failed in going from one day to the next");
        assert_eq!(Alert::current(&today, &failures), [Alert::Failed(new)]);
        assert!(today.is_failed(old), "the old ruin is still there");
        // A day on which nothing fails clears the lot.
        let tomorrow = today.clone();
        assert!(Alert::current(&tomorrow, &Alert::new_failures(&today, &tomorrow)).is_empty());
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
