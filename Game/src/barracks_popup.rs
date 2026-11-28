//! The barracks popup, opened by clicking a barracks: what there is to know of it, and the militia of its country that stand in its
//! region, each of which can be trained into soldiers with a click, for coins and iron for each member. A barracks belongs to whoever
//! owns the region it stands in, so there is no changing its country here.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::barracks::{rest_days, Barracks, MEMBERS_PER_REST_DAY};
use crate::day::DaySave;
use crate::country::resource_name;
use crate::format::{money, thousands, thousands_u64};
use crate::ids::{BarracksId, MilitiaId};
use crate::militia::Militia;
use crate::ship::days_text;

const CARD_WIDTH: f32 = 420.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// The height of a row of the list of militia, and the most it shows before it scrolls.
const LIST_ROW: f32 = 34.0;
const LIST_PADDING: f32 = 6.0;
const MAX_LIST_ROWS: usize = 5;
/// The room for the heading over the list, and for the note under it.
const HEADING_HEIGHT: f32 = 34.0;
const NOTE_HEIGHT: f32 = 36.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player changed about the barracks in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The barracks trains the militia; the number of the map region the militia stands in is given.
    Train { barracks: BarracksId, militia: MilitiaId, region: Option<u16> },
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Train { barracks, militia, region } => day.train_militia(*barracks, *militia, *region),
        }
    }
}

/// The map region a militia is in, as its number.
fn region_of(world: &World, militia: &Militia) -> Option<u16> {
    world.region_at(militia.x, militia.z).map(|r| r.raw())
}

/// The militia that could be trained at `barracks` if it were not resting and the country could pay: those of its country that stand
/// in its region and are not soldiers already.
pub fn candidates<'a>(day: &'a DaySave, barracks: &Barracks, region_of: &dyn Fn(&Militia) -> Option<u16>) -> Vec<&'a Militia> {
    day.militias.iter().filter(|m| m.country == barracks.country && !m.trained && m.in_fort.is_none() && barracks.region.is_some() && region_of(m) == barracks.region).collect()
}

#[derive(Default)]
pub struct BarracksPopup {
    /// The barracks being shown, while the popup is up.
    barracks: Option<BarracksId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// How far down the list of militia is scrolled.
    scroll: usize,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl BarracksPopup {
    pub fn is_open(&self) -> bool {
        self.barracks.is_some()
    }

    pub fn open(&mut self, barracks: BarracksId) {
        *self = Self { barracks: Some(barracks), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the popup.
    pub fn escape(&mut self) {
        self.barracks = None;
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A barracks that is not in `day` is not shown.
    /// `world` is where the militia are found to be in the barracks' region. Returns what the player changed, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave, world: &World) -> Option<Change> {
        let Some(yard) = self.barracks.and_then(|id| day.find_barracks(id)) else {
            self.barracks = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let militia = candidates(day, yard, &|m| region_of(world, m));
        let list_rows = militia.len().clamp(1, MAX_LIST_ROWS);
        let list_height = list_rows as f32 * LIST_ROW + LIST_PADDING * 2.0;
        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + 12.0 + HEADING_HEIGHT + list_height + NOTE_HEIGHT + error_height + 8.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little fort.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        crate::barracks::draw_icon(yard.kind, badge.center(), 1.5, style::ACCENT_HOVER, style::ACCENT_SOFT);
        let text_x = badge.right() + 14.0;
        style.text(yard.kind.label(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&format!("Trains militia into {}", yard.kind.trains_into()), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..3).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in [rows[0].y, rows[1].y, rows[2].y, rows[2].bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }
        let country = day.country(yard.country);
        let region = yard.region.map_or_else(|| "None".to_owned(), |region| day.region_name(region).map_or_else(|| format!("Region {}", region & 0x3FFF), str::to_owned));
        let rest = yard.rest_left(day.date);
        let status = if rest == 0 { "Ready to train".to_owned() } else { format!("Resting for {}", days_text(rest)) };
        let value_row = |label: &str, value: &str, row: Rect, swatch: Option<Color>, color: Color| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, color);
            if let Some(swatch) = swatch {
                style::rounded_rect(Rect::new(x - 18.0, row.center().y - 6.0, 12.0, 12.0), 3.0, swatch);
            }
        };
        value_row("Country", country.map_or("an unknown country", |c| c.name.as_str()), rows[0], country.map(|c| c.color.color()), style::TEXT);
        value_row("Region", &region, rows[1], None, style::TEXT);
        value_row("Training", &status, rows[2], None, if rest == 0 { style::TEXT } else { style::TEXT_MUTED });

        // The militia that can be trained: each with its members and what training it costs, and a click trains it. One that can't be
        // trained now (the barracks is resting, or the country can't pay) is dimmed, and a click says why.
        let heading = Rect::new(inner_x, rows[2].bottom() + 12.0, inner_w, HEADING_HEIGHT);
        style.text_mid("Militia in this region", heading.x, heading.center().y + 4.0, 13, Weight::Semibold, style::TEXT);
        let list = Rect::new(inner_x, heading.bottom(), inner_w, list_height);
        if list.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(militia.len().saturating_sub(list_rows));
        style::card(list, 10.0, style::SURFACE);
        style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
        if militia.is_empty() {
            style.text_mid("None of your militia stand here. Move one into this region.", list.x + 14.0, list.center().y, 12, Weight::Regular, style::TEXT_MUTED);
        }
        for row in 0..list_rows.min(militia.len()) {
            let unit = militia[self.scroll + row];
            let problem = day.train_problem(yard.id, unit.id, yard.region);
            let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
            let over = rect.contains(mouse);
            if over {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }
            let color = if problem.is_none() { style::TEXT } else { style::TEXT_MUTED };
            let cost = format!("{} coins, {} {}", money(yard.kind.training_coins(unit.members)), thousands_u64(yard.kind.training_materials(unit.members)), resource_name(yard.kind.training_resource()));
            let cost_x = rect.right() - 10.0 - style.measure(&cost, 12, Weight::Regular).width;
            style.text_mid(&cost, cost_x, rect.center().y + 8.0, 12, Weight::Regular, style::TEXT_MUTED);
            let members = format!("{} members", thousands(unit.members));
            style.text_mid(&members, cost_x.max(rect.x + 200.0), rect.center().y - 8.0, 12, Weight::Semibold, color);
            let mut name = unit.title().to_owned();
            while name.chars().count() > 4 && style.measure(&name, 14, Weight::Regular).width > 190.0 {
                name.pop();
                name.pop();
                name.push('…');
            }
            style.text_mid(&name, rect.x + 10.0, rect.center().y, 14, Weight::Regular, color);
            if clicked && over {
                match problem {
                    None => {
                        self.error = None;
                        change = Some(Change::Train { barracks: yard.id, militia: unit.id, region: yard.region });
                    }
                    Some(problem) => self.error = Some(problem),
                }
            }
        }
        if self.scroll > 0 {
            draw_triangle(vec2(list.right() - 18.0, list.y + 9.0), vec2(list.right() - 10.0, list.y + 9.0), vec2(list.right() - 14.0, list.y + 4.0), style::TEXT_MUTED);
        }
        if self.scroll + list_rows < militia.len() {
            draw_triangle(vec2(list.right() - 18.0, list.bottom() - 9.0), vec2(list.right() - 10.0, list.bottom() - 9.0), vec2(list.right() - 14.0, list.bottom() - 4.0), style::TEXT_MUTED);
        }

        // What it costs, and how long it rests after.
        let upkeep = yard.kind.class().upkeep_per_member();
        let note = format!(
            "Each member costs {} coins and {} {}. The {} then rests a day for every {MEMBERS_PER_REST_DAY} it trained (1 to {}). {} cost {upkeep} coins a day each to keep. To train only some, split the militia first.",
            yard.kind.training_coins_each(),
            yard.kind.training_each(),
            resource_name(yard.kind.training_resource()),
            yard.kind.noun(),
            rest_days(crate::militia::MAX_MEMBERS),
            crate::format::capitalized(yard.kind.trains_into())
        );
        let note_rect = Rect::new(inner_x, list.bottom() + 6.0, inner_w, NOTE_HEIGHT);
        for (n, line) in crate::hud::wrap(style, &note, 11, inner_w).iter().take(3).enumerate() {
            style.text_mid(line, note_rect.x, note_rect.y + 7.0 + n as f32 * 13.0, 11, Weight::Regular, style::TEXT_MUTED);
        }
        if let Some(error) = &self.error {
            style.text(error, inner_x, note_rect.bottom() + 16.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && (enter || (clicked && (over_close || !card.contains(mouse)))) {
            self.barracks = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use nworld_map::world::Resource;

    const HERE: Option<u16> = Some(0x4005);

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, 100_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(id, resource, 5_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, HERE).unwrap();
        day.cities[0].population = 5_000;
        day.build_barracks(crate::barracks::BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap();
        day.raise_militia(day.cities[0].id, 100, vec2(40.0, 0.0)).unwrap();
        day
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_it() {
        assert!(!BarracksPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = BarracksPopup { barracks: Some(BarracksId::new()), ..BarracksPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_failure_is_kept_to_show() {
        let mut popup = BarracksPopup { barracks: Some(BarracksId::new()), ..BarracksPopup::default() };
        popup.fail("no".to_owned());
        assert_eq!(popup.error.as_deref(), Some("no"));
    }

    #[test]
    fn the_militia_that_can_be_trained_are_those_of_its_country_in_its_region_that_are_not_soldiers() {
        let mut day = day();
        let yard = day.barracks[0];
        let here = |_: &Militia| HERE;
        let elsewhere = |_: &Militia| Some(0x4006u16);
        assert_eq!(candidates(&day, &yard, &here).len(), 1);
        assert!(candidates(&day, &yard, &elsewhere).is_empty(), "not in its region");
        let brevon = day.countries[1].id;
        let mut other = day.clone();
        other.militias[0].country = brevon;
        assert!(candidates(&other, &yard, &here).is_empty(), "not of its country");
        day.militias[0].trained = true;
        assert!(candidates(&day, &yard, &here).is_empty(), "soldiers already");
        assert!(candidates(&day, &Barracks { region: None, ..yard }, &|_| None).is_empty(), "a map with no regions has none");
    }

    #[test]
    fn training_does_what_it_says_to_the_day() {
        let mut day = day();
        let (barracks, militia) = (day.barracks[0].id, day.militias[0].id);
        Change::Train { barracks, militia, region: HERE }.apply(&mut day).unwrap();
        assert!(day.militia(militia).unwrap().trained);
        assert!(Change::Train { barracks, militia, region: HERE }.apply(&mut day).is_err(), "it is soldiers already, and the barracks is resting");
        assert!(Change::Train { barracks: BarracksId::new(), militia, region: HERE }.apply(&mut day).is_err(), "it is not there");
    }
}
