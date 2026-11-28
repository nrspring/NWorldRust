//! The fort popup, opened by clicking a fort: what there is to know of it, its supplies, a switch to put it under siege, its garrison and the militia that could join it, each
//! with a button to march out or go in, and a button to upgrade it to the next size. Clicking a militia's name opens its own
//! popup, and clicking the fort's name renames it. A fort belongs to whoever owns the region it stands in, so there is no changing its country here.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::thousands;
use crate::fort::Fort;
use crate::ids::{FortId, MilitiaId};
use crate::militia::Militia;
use crate::newsreel::cost_text;
use crate::options;
use crate::ship::days_text;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// The room for upgrading: the reason it can't be done or what it costs, and the button.
const UPGRADE_HEIGHT: f32 = 12.0 + 34.0 + 6.0 + BUTTON_HEIGHT;
/// How many rows of information there are.
const ROW_COUNT: usize = 6;
/// The height of a militia in the list, the most that show before it scrolls, and the room for the list's heading and for the
/// line under it.
const LIST_ROW: f32 = 34.0;
const MAX_LIST_ROWS: usize = 5;
const LIST_HEADING: f32 = 30.0;
const LIST_NOTE: f32 = 22.0;
/// How wide the button of a militia in the list is.
const ACTION_WIDTH: f32 = 86.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player changed about the fort in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The fort is upgraded to the next size.
    Upgrade(FortId),
    /// The militia, which stands in the map region numbered `region`, goes into the fort.
    Enter { fort: FortId, militia: MilitiaId, region: Option<u16> },
    /// The militia comes out of the fort it is in.
    Leave(MilitiaId),
    /// The fort is now under siege (or not).
    Siege(FortId, bool),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Upgrade(id) => day.upgrade_fort(*id),
            Self::Enter { fort, militia, region } => day.enter_fort(*fort, *militia, *region),
            Self::Leave(militia) => day.leave_fort(*militia),
            Self::Siege(fort, under_siege) => day.set_fort_under_siege(*fort, *under_siege),
        }
    }

    /// The militia that is to go into a fort, which it can't while it is walking.
    pub fn entering(&self) -> Option<MilitiaId> {
        match self {
            Self::Enter { militia, .. } => Some(*militia),
            _ => None,
        }
    }
}

/// The map region a militia is in, as its number.
fn region_of(world: &World, militia: &Militia) -> Option<u16> {
    world.region_at(militia.x, militia.z).map(|r| r.raw())
}

/// The militia listed for `fort`: those in its garrison, and then those of its country in its region that are in no fort.
fn listed<'a>(day: &'a DaySave, fort: &Fort, region_of: &dyn Fn(&Militia) -> Option<u16>) -> Vec<&'a Militia> {
    let outside = day.militias.iter().filter(|m| m.country == fort.country && m.in_fort.is_none() && fort.region.is_some() && region_of(m) == fort.region);
    day.garrison_of(fort.id).into_iter().chain(outside).collect()
}

#[derive(Default)]
pub struct FortPopup {
    /// The fort being shown, while the popup is up.
    fort: Option<FortId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Why the last change couldn't be made.
    error: Option<String>,
    /// How far down the list of militia is scrolled.
    scroll: usize,
    /// The militia whose name was clicked, until `take_details` takes it.
    details: Option<MilitiaId>,
    /// Set when the fort's name was clicked, to be renamed: the fort, until `take_rename` takes it.
    rename: Option<FortId>,
}

impl FortPopup {
    pub fn is_open(&self) -> bool {
        self.fort.is_some()
    }

    pub fn open(&mut self, fort: FortId) {
        *self = Self { fort: Some(fort), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the popup.
    pub fn escape(&mut self) {
        self.fort = None;
    }

    /// The militia whose name was clicked since this was last called, if one was. The popup closes when it is, for the
    /// militia's own popup to take its place.
    pub fn take_details(&mut self) -> Option<MilitiaId> {
        self.details.take()
    }

    /// The fort whose name was clicked since this was last called, if one was. The popup closes when it is, for the name to be
    /// asked for.
    pub fn take_rename(&mut self) -> Option<FortId> {
        self.rename.take()
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A fort that is not in `day` is not shown.
    /// `world` says which regions the militia are in. Returns what the player changed about the fort, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave, world: &World) -> Option<Change> {
        let Some(fort) = self.fort.and_then(|id| day.fort(id)) else {
            self.fort = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let militia = listed(day, fort, &|m| region_of(world, m));
        let shown = militia.len().min(MAX_LIST_ROWS);
        // A line is left to say there are none, when there are none.
        let list_height = LIST_HEADING + LIST_ROW * shown as f32 + LIST_NOTE;
        let height = ROWS_TOP + ROW_HEIGHT * ROW_COUNT as f32 + list_height + UPGRADE_HEIGHT + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little shield.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        fort.size.draw_icon(badge.center(), 1.5, style::ACCENT_HOVER, style::ACCENT_SOFT);
        let text_x = badge.right() + 14.0;
        style.text(fort.title(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(fort.size.label(), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..ROW_COUNT).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in rows.iter().map(|row| row.y).chain([rows[ROW_COUNT - 1].bottom()]) {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }
        let [name_row, country_row, region_row, garrison_row, supplies_row, siege_row] = [rows[0], rows[1], rows[2], rows[3], rows[4], rows[5]];
        let country = day.country(fort.country);
        let region = fort.region.map_or_else(|| "None".to_owned(), |region| day.region_name(region).map_or_else(|| format!("Region {}", region & 0x3FFF), str::to_owned));
        let value_row = |label: &str, value: &str, row: Rect, swatch: Option<Color>| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, style::TEXT);
            if let Some(swatch) = swatch {
                style::rounded_rect(Rect::new(x - 18.0, row.center().y - 6.0, 12.0, 12.0), 3.0, swatch);
            }
        };
        value_row("Country", country.map_or("an unknown country", |c| c.name.as_str()), country_row, country.map(|c| c.color.color()));
        value_row("Region", &region, region_row, None);
        let garrisoned = day.garrisoned_members(fort.id);
        value_row("Garrison", &format!("{} of {} members", thousands(garrisoned), thousands(fort.size.capacity())), garrison_row, None);
        // What is left to live on, and how long it would last the garrison there is, if the fort is under siege.
        let supplies = match fort.days_of_supplies(garrisoned).filter(|_| fort.under_siege) {
            Some(0) => "None left".to_owned(),
            Some(days) => format!("{}%, {} left", fort.supplies_percent(), days_text(days)),
            None => format!("{}% ({} of {})", fort.supplies_percent(), thousands(fort.supplies_left()), thousands(fort.size.supplies())),
        };
        value_row("Supplies", &supplies, supplies_row, None);
        let mut under_siege = fort.under_siege;
        options::toggle(style, Rect::new(siege_row.x - 12.0, siege_row.y, siege_row.w + 24.0, siege_row.h), "Under siege", None, &mut under_siege, true);
        if under_siege != fort.under_siege {
            self.error = None;
            change = Some(Change::Siege(fort.id, under_siege));
        }

        // The name, which can be clicked to change it.
        let over_name_row = name_row.contains(mouse);
        if over_name_row {
            style::rounded_rect(Rect::new(name_row.x - 8.0, name_row.y + 1.0, name_row.w + 16.0, name_row.h - 2.0), 8.0, style::HOVER);
        }
        style.text_mid("Name", inner_x, name_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let edit = "Rename";
        let edit_x = name_row.right() - style.measure(edit, 12, Weight::Regular).width;
        style.text_mid(edit, edit_x, name_row.center().y, 12, Weight::Regular, if over_name_row { style::ACCENT } else { style::TEXT_MUTED });
        let mut name = if fort.name.is_empty() { "None".to_owned() } else { fort.name.clone() };
        while name.chars().count() > 4 && style.measure(&name, 14, Weight::Semibold).width > edit_x - inner_x - 70.0 {
            name.pop();
            name.pop();
            name.push('…');
        }
        let name_x = edit_x - 12.0 - style.measure(&name, 14, Weight::Semibold).width;
        style.text_mid(&name, name_x, name_row.center().y, 14, Weight::Semibold, style::TEXT);

        // The militia in it, and those that could go in, each with a button.
        let list = Rect::new(inner_x, garrison_row.bottom() + LIST_HEADING, inner_w, LIST_ROW * shown as f32);
        style.text_mid("Militia here", inner_x, garrison_row.bottom() + LIST_HEADING * 0.6, 13, Weight::Semibold, style::TEXT);
        if list.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(militia.len() - shown);
        // Why the one the mouse is on can't go in.
        let mut why = None;
        let mut used_click = false;
        for (n, unit) in militia.iter().skip(self.scroll).take(shown).enumerate() {
            let row = Rect::new(list.x, list.y + LIST_ROW * n as f32, list.w, LIST_ROW);
            let inside = unit.in_fort.is_some();
            let action = Rect::new(row.right() - ACTION_WIDTH, row.y + 4.0, ACTION_WIDTH, row.h - 8.0);
            let region = region_of(world, unit);
            let problem = if inside { None } else { day.enter_fort_problem(fort.id, unit.id, region) };
            let name_area = Rect::new(row.x, row.y, row.w - ACTION_WIDTH - 8.0, row.h);
            let over_name = name_area.contains(mouse);
            if over_name {
                style::rounded_rect(name_area, 6.0, style::HOVER);
            }
            let members = thousands(unit.members);
            let members_x = name_area.right() - 8.0 - style.measure(&members, 13, Weight::Semibold).width;
            style.text_mid(&members, members_x, row.center().y, 13, Weight::Semibold, style::TEXT_MUTED);
            let mut name = unit.title().to_owned();
            while name.chars().count() > 4 && style.measure(&name, 13, Weight::Regular).width > members_x - row.x - 22.0 {
                name.pop();
                name.pop();
                name.push('…');
            }
            // A filled dot for one that is inside.
            if inside {
                draw_circle(row.x + 7.0, row.center().y, 3.5, style::ACCENT);
            } else {
                draw_circle_lines(row.x + 7.0, row.center().y, 3.5, 1.2, style::BORDER_STRONG);
            }
            style.text_mid(&name, row.x + 16.0, row.center().y, 13, Weight::Regular, style::TEXT);
            let over_action = action.contains(mouse);
            let usable = problem.is_none();
            let fill = if !usable { style::HOVER } else if over_action { style::ACCENT_HOVER } else if inside { style::BORDER_STRONG } else { style::ACCENT };
            style::rounded_rect(action, 6.0, fill);
            style.text_centered(if inside { "March out" } else { "Garrison" }, action, 12, Weight::Semibold, if usable { WHITE } else { style::TEXT_MUTED });
            if over_action || over_name {
                why = problem.clone();
            }
            if clicked && over_action && usable {
                self.error = None;
                used_click = true;
                change = Some(if inside { Change::Leave(unit.id) } else { Change::Enter { fort: fort.id, militia: unit.id, region } });
            } else if clicked && over_name {
                used_click = true;
                self.details = Some(unit.id);
            }
        }
        let note = match why {
            Some(why) => why,
            None if militia.is_empty() => "No militia of this country is in this region.".to_owned(),
            None if militia.len() > shown => format!("Scroll for more: {} in all.", militia.len()),
            None => "Click a name for more about it.".to_owned(),
        };
        if let Some(line) = crate::hud::wrap(style, &note, 11, inner_w).first() {
            style.text_mid(line, inner_x, list.bottom() + LIST_NOTE * 0.5, 11, Weight::Regular, style::TEXT_MUTED);
        }

        // Upgrading it: what it costs, or why it can't be done, and the button. A large fort is as big as it gets.
        let problem = day.fort_upgrade_problem(fort.id);
        let next = fort.size.upgrade();
        let upgradable = next.is_some() && problem.is_none();
        let note_rect = Rect::new(inner_x, list.bottom() + LIST_NOTE + 12.0, inner_w, 34.0);
        let button = Rect::new(inner_x, note_rect.bottom() + 6.0, inner_w, BUTTON_HEIGHT);
        let note = match (&problem, next) {
            (_, None) => "It is the largest size.".to_owned(),
            (Some(problem), _) => problem.clone(),
            (None, Some(to)) => format!("Costs {}.", cost_text(&to.full_cost())),
        };
        for (n, line) in crate::hud::wrap(style, &note, 12, inner_w).iter().take(2).enumerate() {
            style.text_mid(line, note_rect.x, note_rect.y + 9.0 + n as f32 * 15.0, 12, Weight::Regular, style::TEXT_MUTED);
        }
        let over_upgrade = button.contains(mouse) && upgradable;
        let fill = if !upgradable { style::HOVER } else if over_upgrade { style::ACCENT_HOVER } else { style::ACCENT };
        style::rounded_rect(button, 8.0, fill);
        let label = next.map_or_else(|| "Fully upgraded".to_owned(), |to| format!("Upgrade to {}", to.a_noun()));
        style.text_centered(&label, button, 14, Weight::Semibold, if upgradable { WHITE } else { style::TEXT_MUTED });
        if clicked && over_upgrade {
            self.error = None;
            change = Some(Change::Upgrade(fort.id));
        }

        if clicked && over_name_row {
            self.rename = Some(fort.id);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, button.bottom() + 20.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if self.details.is_some() || self.rename.is_some() || (change.is_none() && !used_click && (enter || (clicked && (over_close || !card.contains(mouse))))) {
            self.fort = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use nworld_map::world::Resource;

    #[test]
    fn the_list_has_the_garrison_first_and_then_who_could_join_it_and_changes_move_them() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let country = day.countries[0].id;
        day.add_money(country, 20_000).unwrap();
        day.add_resource(country, Resource::Stone, 500).unwrap();
        day.add_resource(country, Resource::Forest, 500).unwrap();
        day.claim_region(0x4005).unwrap();
        day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
        day.found_city("Port Aldor", 500.0, 0.0, Some(0x4005)).unwrap();
        day.cities[0].population = 1_000;
        day.raise_militia(day.cities[0].id, 50, vec2(30.0, 0.0)).unwrap();
        let (fort, militia) = (day.forts[0].id, day.militias[0].id);
        let here = |_: &Militia| Some(0x4005);
        assert_eq!(listed(&day, &day.forts[0], &here).len(), 1);
        assert!(listed(&day, &day.forts[0], &|_| Some(0x4006)).is_empty(), "not in the region");

        let enter = Change::Enter { fort, militia, region: Some(0x4005) };
        assert_eq!(enter.entering(), Some(militia));
        enter.apply(&mut day).unwrap();
        assert_eq!(listed(&day, &day.forts[0], &|_| None).len(), 1, "one inside is listed wherever it is said to be");
        assert_eq!(Change::Leave(militia).entering(), None);
        Change::Leave(militia).apply(&mut day).unwrap();
        assert!(day.militias[0].in_fort.is_none());
        Change::Siege(fort, true).apply(&mut day).unwrap();
        assert!(day.forts[0].under_siege);
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_it_and_a_failure_is_kept_to_show() {
        assert!(!FortPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = FortPopup { fort: Some(FortId::new()), ..FortPopup::default() };
        assert!(popup.is_open());
        popup.fail("no".to_owned());
        assert_eq!(popup.error.as_deref(), Some("no"));
        popup.escape();
        assert!(!popup.is_open());
    }
}
