//! The militia popup, opened by clicking a militia: what there is to know of it, a way to change which country it
//! belongs to, to combine it with another militia of its country in the same region, and to split some of its members
//! off into a militia of their own, and to disband it.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::city_popup::members_at;
use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::ids::{CountryId, MilitiaId};
use crate::militia::{self, Militia};
use crate::options;
use crate::movement::DAILY_POINTS;

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
/// The room for splitting a militia: its heading, the slider for how many, and the button.
const SPLIT_HEIGHT: f32 = 36.0 + SLIDER_HEIGHT + 6.0 + BUTTON_HEIGHT;
const SLIDER_HEIGHT: f32 = 30.0;
/// How many rows the information has.
const ROW_COUNT: usize = 8;

/// What the player changed about the militia in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The militia now belongs to this country.
    Country(MilitiaId, CountryId),
    /// The militia `absorb` joins the militia `keep`; the map regions they stand in are given.
    Merge { keep: MilitiaId, absorb: MilitiaId, regions: (Option<u16>, Option<u16>) },
    /// This many members of the militia split off into a militia that stands at this place.
    Split { id: MilitiaId, members: u32, at: Vec2 },
    /// The militia is now under siege (or not).
    Siege(MilitiaId, bool),
    /// The militia is disbanded.
    Disband(MilitiaId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Country(militia, country) => day.set_militia_country(*militia, *country),
            Self::Merge { keep, absorb, regions } => day.merge_militia(*keep, *absorb, *regions),
            Self::Split { id, members, at } => day.split_militia(*id, *members, *at),
            Self::Siege(id, under_siege) => day.set_militia_under_siege(*id, *under_siege),
            Self::Disband(id) => day.disband_militia(*id),
        }
    }

    /// The militia it is about, which can't be changed while they are walking.
    pub fn militia(&self) -> Vec<MilitiaId> {
        match self {
            Self::Country(id, _) | Self::Split { id, .. } | Self::Siege(id, _) | Self::Disband(id) => vec![*id],
            Self::Merge { keep, absorb, .. } => vec![*keep, *absorb],
        }
    }
}

/// The map region a militia is in, as its number.
fn region_of(world: &World, militia: &Militia) -> Option<u16> {
    world.region_at(militia.x, militia.z).map(|r| r.raw())
}

#[derive(Default)]
pub struct MilitiaPopup {
    /// The militia being shown, while the popup is up.
    militia: Option<MilitiaId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Whether the list of countries to change to is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
    /// Whether the list of militia to combine with is showing, and how far down it is scrolled.
    combining: bool,
    combine_scroll: usize,
    /// How many members the slider is set to for the group to split off, or `None` for half (until it is moved), and
    /// whether it is being dragged.
    split: Option<u32>,
    dragging: bool,
    /// Whether Disband has been clicked once, and is waiting for a second click to be sure.
    disbanding: bool,
    /// Set when the name was clicked, to be renamed: the militia whose name it is, until `take_rename` takes it.
    rename: Option<MilitiaId>,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl MilitiaPopup {
    pub fn is_open(&self) -> bool {
        self.militia.is_some()
    }

    pub fn open(&mut self, militia: MilitiaId) {
        *self = Self { militia: Some(militia), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries or of militia if one is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if self.choosing || self.combining || self.disbanding {
            self.choosing = false;
            self.combining = false;
            self.disbanding = false;
        } else {
            self.militia = None;
        }
    }

    /// The militia whose name was clicked since this was last called, if one was. The popup closes when it is, for the name
    /// to be asked for.
    pub fn take_rename(&mut self) -> Option<MilitiaId> {
        self.rename.take()
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.choosing = false;
        self.combining = false;
        self.disbanding = false;
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A militia that is not in `day` is
    /// not shown. `world` is the ground a split-off group can stand on, and the regions the militia are in. Returns what
    /// the player changed about the militia, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave, world: &World) -> Option<Change> {
        let Some(unit) = self.militia.and_then(|id| day.militia(id)) else {
            self.militia = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(unit.country);
        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * ROW_COUNT as f32 + 8.0 + SPLIT_HEIGHT + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let listing = self.choosing || self.combining;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a flag on a pole in the country's color.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        let c = badge.center();
        draw_line(c.x - 7.0, c.y - 12.0, c.x - 7.0, c.y + 12.0, 2.0, style::ACCENT_HOVER);
        let flag = country.map_or(style::TEXT_MUTED, |c| c.color.color());
        draw_rectangle(c.x - 6.0, c.y - 12.0, 17.0, 11.0, flag);
        let text_x = badge.right() + 14.0;
        style.text(unit.title(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        let origin = day.city(unit.city).map_or("a city", |c| c.name.as_str());
        style.text(&format!("{} raised in {origin}", unit.kind_label()), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each. The country's can be clicked, to change it.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..ROW_COUNT).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        let [name_row, country_row, members_row, raised_row, upkeep_row, moves_row, siege_row, combine_row] = [rows[0], rows[1], rows[2], rows[3], rows[4], rows[5], rows[6], rows[7]];
        for y in [name_row.y, country_row.y, members_row.y, raised_row.y, upkeep_row.y, moves_row.y, siege_row.y, combine_row.y, combine_row.bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }

        // The name, which can be clicked to change it.
        let over_name = name_row.contains(mouse) && !listing;
        if over_name {
            style::rounded_rect(Rect::new(name_row.x - 8.0, name_row.y + 1.0, name_row.w + 16.0, name_row.h - 2.0), 8.0, style::HOVER);
        }
        style.text_mid("Name", inner_x, name_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let edit = "Rename";
        let edit_x = name_row.right() - style.measure(edit, 12, Weight::Regular).width;
        style.text_mid(edit, edit_x, name_row.center().y, 12, Weight::Regular, if over_name { style::ACCENT } else { style::TEXT_MUTED });
        let mut name = unit.title().to_owned();
        let room = edit_x - inner_x - 70.0;
        while name.chars().count() > 4 && style.measure(&name, 14, Weight::Semibold).width > room {
            name.pop();
            name.pop();
            name.push('…');
        }
        let name_x = edit_x - 12.0 - style.measure(&name, 14, Weight::Semibold).width;
        style.text_mid(&name, name_x, name_row.center().y, 14, Weight::Semibold, style::TEXT);

        let over_country = country_row.contains(mouse) && !self.combining;
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(country_row.x - 8.0, country_row.y + 1.0, country_row.w + 16.0, country_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = country_row.center().y;
        style.text_mid("Country", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        let country_name = country.map_or("an unknown country", |c| c.name.as_str());
        // At the right: the name, the country's color before it, and an arrow after it that says there is a list to
        // choose from.
        let arrow = vec2(country_row.right() - 4.0, middle);
        if self.choosing {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), style::TEXT_MUTED);
        } else {
            draw_triangle(vec2(arrow.x - 4.0, arrow.y - 2.0), vec2(arrow.x + 4.0, arrow.y - 2.0), vec2(arrow.x, arrow.y + 3.0), style::TEXT_MUTED);
        }
        let country_x = arrow.x - 14.0 - style.measure(country_name, 14, Weight::Semibold).width;
        style.text_mid(country_name, country_x, middle, 14, Weight::Semibold, style::TEXT);
        if let Some(country) = country {
            style::rounded_rect(Rect::new(country_x - 18.0, middle - 6.0, 12.0, 12.0), 3.0, country.color.color());
        }

        let value_row = |label: &str, value: &str, row: Rect| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, style::TEXT);
        };
        value_row(if unit.trained { "Soldiers" } else { "Members" }, &thousands(unit.members), members_row);
        value_row("Raised", &unit.raised.to_string(), raised_row);
        value_row("Upkeep", &format!("{} coins a day", money(unit.upkeep())), upkeep_row);
        value_row("Movement", &format!("{} of {} points left", unit.points, DAILY_POINTS), moves_row);

        // The siege switch, which is for the lists to cover while one is open. A militia under siege loses members each day.
        let mut under_siege = unit.under_siege;
        options::toggle(style, Rect::new(siege_row.x - 12.0, siege_row.y, siege_row.w + 24.0, siege_row.h), "Under siege", None, &mut under_siege, !listing);
        if under_siege != unit.under_siege {
            change = Some(Change::Siege(unit.id, under_siege));
        }

        // Combining with another militia of the country that stands in the same region: a row that opens the list of them.
        let partners = day.merge_partners(unit.id, &|m| region_of(world, m));
        let over_combine = combine_row.contains(mouse) && !self.choosing && !partners.is_empty();
        if over_combine || self.combining {
            style::rounded_rect(Rect::new(combine_row.x - 8.0, combine_row.y + 1.0, combine_row.w + 16.0, combine_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = combine_row.center().y;
        style.text_mid("Combine with", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        if partners.is_empty() {
            let none = "None in this region";
            style.text_mid(none, combine_row.right() - style.measure(none, 14, Weight::Semibold).width, middle, 14, Weight::Semibold, style::TEXT_MUTED);
        } else {
            let arrow = vec2(combine_row.right() - 4.0, middle);
            if self.combining {
                draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), style::TEXT_MUTED);
            } else {
                draw_triangle(vec2(arrow.x - 4.0, arrow.y - 2.0), vec2(arrow.x + 4.0, arrow.y - 2.0), vec2(arrow.x, arrow.y + 3.0), style::TEXT_MUTED);
            }
            let count = format!("{} here", partners.len());
            style.text_mid(&count, arrow.x - 14.0 - style.measure(&count, 14, Weight::Semibold).width, middle, 14, Weight::Semibold, style::TEXT);
        }

        // Splitting some members off: how many, on a slider that goes up to all but one, and a button.
        let split_row = Rect::new(inner_x, combine_row.bottom() + 8.0, inner_w, 36.0);
        let slider = Rect::new(inner_x, split_row.bottom(), inner_w, SLIDER_HEIGHT);
        let split_button = Rect::new(inner_x, slider.bottom() + 6.0, inner_w, BUTTON_HEIGHT);
        let most = unit.members.saturating_sub(1);
        let splitting = most >= 1;
        let track = Rect::new(slider.x + 8.0, slider.center().y - 3.0, slider.w - 16.0, 6.0);
        // The slider takes the mouse when it is pressed over it, until it is let go.
        if splitting && clicked && slider.contains(mouse) && !listing {
            self.dragging = true;
        }
        if !splitting || !is_mouse_button_down(MouseButton::Left) {
            self.dragging = false;
        }
        if self.dragging {
            self.split = Some(members_at(mouse.x, track.x, track.w, most));
        }
        let members = self.split.unwrap_or(unit.members / 2).clamp(1, most.max(1));
        style.text_mid("Split off", inner_x, split_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let summary = if splitting { format!("{} of {}", thousands(members), thousands(unit.members)) } else { "Too few to split".to_owned() };
        style.text_mid(&summary, split_row.right() - style.measure(&summary, 14, Weight::Semibold).width, split_row.center().y, 14, Weight::Semibold, if splitting { style::TEXT } else { style::TEXT_MUTED });
        if splitting {
            let fraction = if most > 1 { (members - 1) as f32 / (most - 1) as f32 } else { 1.0 };
            let knob = vec2(track.x + track.w * fraction, track.center().y);
            style::rounded_rect(track, 3.0, style::BORDER_STRONG);
            style::rounded_rect(Rect::new(track.x, track.y, knob.x - track.x, track.h), 3.0, style::ACCENT);
            draw_circle(knob.x, knob.y, 9.0, style::ACCENT);
            draw_circle(knob.x, knob.y, 3.4, WHITE);
        }
        let over_split = split_button.contains(mouse) && !listing && splitting;
        let fill = if !splitting { style::HOVER } else if over_split { style::ACCENT_HOVER } else { style::ACCENT };
        style::rounded_rect(split_button, 8.0, fill);
        let label = if splitting { format!("Split off {} members", thousands(members)) } else { "Split off".to_owned() };
        style.text_centered(&label, split_button, 14, Weight::Semibold, if splitting { WHITE } else { style::TEXT_MUTED });
        if clicked && over_split {
            let taken: Vec<Vec2> = day.militias.iter().map(|m| vec2(m.x, m.z)).collect();
            match militia::split_spot(world, vec2(unit.x, unit.z), &taken, &|at| world.region_at(at.x, at.y).map(|r| r.raw())) {
                Some(at) => {
                    self.error = None;
                    self.split = None;
                    change = Some(Change::Split { id: unit.id, members, at });
                }
                None => self.error = Some(format!("There is no room near {} for another militia to stand.", unit.title())),
            }
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, split_button.bottom() + 20.0, 12, Weight::Regular, style::ERROR);
        }

        // The buttons at the bottom: one that disbands it, which has to be clicked twice to be sure, and one that closes it.
        let button_w = (inner_w - 10.0) * 0.5;
        let disband = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, button_w, BUTTON_HEIGHT);
        let close = Rect::new(disband.right() + 10.0, disband.y, button_w, BUTTON_HEIGHT);
        let over_disband = disband.contains(mouse) && !listing;
        let over_close = close.contains(mouse) && !listing;
        let (fill, label, color) = match (self.disbanding, over_disband) {
            (true, _) => (style::ERROR, "Click again to disband", WHITE),
            (false, true) => (style::BORDER_STRONG, "Disband", style::ERROR),
            (false, false) => (style::HOVER, "Disband", style::ERROR),
        };
        style::rounded_rect(disband, 8.0, fill);
        style.text_centered(label, disband, 14, Weight::Semibold, color);
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
                if listed.id == unit.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, listed.color.color());
                style.text_mid(&listed.name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country it is already in is no change, so there is nothing to save.
                    if listed.id != unit.country {
                        change = Some(Change::Country(unit.id, listed.id));
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
        } else if self.combining {
            // The militia that can be combined with, each with its members. One that would make more than a militia can
            // have is dimmed, and does nothing.
            let count = partners.len();
            let rows = count.min(MAX_LIST_ROWS);
            let list = Rect::new(inner_x, combine_row.bottom() + 2.0, inner_w, rows as f32 * LIST_ROW + LIST_PADDING * 2.0);
            if list.contains(mouse) && mouse_wheel().1 != 0.0 {
                self.combine_scroll = if mouse_wheel().1 > 0.0 { self.combine_scroll.saturating_sub(1) } else { self.combine_scroll + 1 };
            }
            self.combine_scroll = self.combine_scroll.min(count - rows);
            style::card(list, 10.0, style::SURFACE);
            style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
            for row in 0..rows {
                let other = partners[self.combine_scroll + row];
                let fits = unit.members + other.members <= militia::MAX_MEMBERS;
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * LIST_ROW, list.w - LIST_PADDING * 2.0, LIST_ROW);
                if fits && rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                }
                let color = if fits { style::TEXT } else { style::TEXT_MUTED };
                let members = thousands(other.members);
                let members_x = rect.right() - 10.0 - style.measure(&members, 13, Weight::Semibold).width;
                style.text_mid(&members, members_x, rect.center().y, 13, Weight::Semibold, color);
                let mut name = other.title().to_owned();
                while name.chars().count() > 4 && style.measure(&name, 14, Weight::Regular).width > members_x - rect.x - 24.0 {
                    name.pop();
                    name.pop();
                    name.push('…');
                }
                style.text_mid(&name, rect.x + 10.0, rect.center().y, 14, Weight::Regular, color);
                if clicked && fits && rect.contains(mouse) {
                    change = Some(Change::Merge { keep: unit.id, absorb: other.id, regions: (region_of(world, unit), region_of(world, other)) });
                    self.combining = false;
                }
            }
            if self.combine_scroll > 0 {
                draw_triangle(vec2(list.right() - 18.0, list.y + 9.0), vec2(list.right() - 10.0, list.y + 9.0), vec2(list.right() - 14.0, list.y + 4.0), style::TEXT_MUTED);
            }
            if self.combine_scroll + rows < count {
                draw_triangle(vec2(list.right() - 18.0, list.bottom() - 9.0), vec2(list.right() - 10.0, list.bottom() - 9.0), vec2(list.right() - 14.0, list.bottom() - 4.0), style::TEXT_MUTED);
            }
            // A click on the list was for it; a click anywhere else closes it, and goes no further.
            if clicked {
                self.combining = false;
                used_click = true;
            }
        } else if clicked && over_name {
            // The name is asked for in a popup of its own, which takes this one's place.
            self.rename = Some(unit.id);
            self.militia = None;
            used_click = true;
        } else if clicked && over_country {
            self.choosing = true;
            self.error = None;
            self.scroll = day.countries.iter().position(|c| c.id == unit.country).unwrap_or(0).saturating_sub(MAX_LIST_ROWS - 1);
            used_click = true;
        } else if clicked && over_combine {
            self.combining = true;
            self.combine_scroll = 0;
            self.error = None;
            used_click = true;
        }

        // A click on Disband asks to be sure, and the next one disbands; a click anywhere else takes the question back.
        if clicked && !used_click && over_disband {
            if self.disbanding {
                change = Some(Change::Disband(unit.id));
            }
            self.disbanding = !self.disbanding;
            used_click = true;
        } else if clicked {
            self.disbanding = false;
        }

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && ((enter && !listing) || (clicked && (over_close || !card.contains(mouse)))) {
            self.militia = None;
        }
        change
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
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 1_000;
        day.raise_militia(day.cities[0].id, 50, vec2(30.0, 0.0)).unwrap();
        day
    }

    #[test]
    fn a_click_on_the_name_asks_for_a_rename_once() {
        let mut popup = MilitiaPopup { militia: Some(MilitiaId::new()), ..MilitiaPopup::default() };
        assert_eq!(popup.take_rename(), None);
        let id = MilitiaId::new();
        popup.rename = Some(id);
        assert_eq!(popup.take_rename(), Some(id));
        assert_eq!(popup.take_rename(), None, "it is taken");
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_the_list_first() {
        assert!(!MilitiaPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = MilitiaPopup { militia: Some(MilitiaId::new()), choosing: true, ..MilitiaPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(popup.is_open() && !popup.choosing);
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_failure_closes_the_list_and_is_kept_to_show() {
        let mut popup = MilitiaPopup { militia: Some(MilitiaId::new()), choosing: true, ..MilitiaPopup::default() };
        popup.fail("no".to_owned());
        assert!(!popup.choosing && popup.error.as_deref() == Some("no"));
    }

    #[test]
    fn a_change_does_what_it_says_to_the_day() {
        let mut day = day();
        let (id, brevon) = (day.militias[0].id, day.countries[1].id);
        Change::Country(id, brevon).apply(&mut day).unwrap();
        assert_eq!(day.militia(id).unwrap().country, brevon);
        assert!(Change::Country(id, CountryId::new()).apply(&mut day).is_err());
        assert!(Change::Country(MilitiaId::new(), brevon).apply(&mut day).is_err(), "it is not there");
    }

    #[test]
    fn disbanding_removes_the_militia_and_asking_twice_is_the_players_to_do() {
        let mut day = day();
        let id = day.militias[0].id;
        assert_eq!(Change::Disband(id).militia(), [id]);
        Change::Disband(id).apply(&mut day).unwrap();
        assert!(day.militias.is_empty());
        assert!(Change::Disband(id).apply(&mut day).is_err(), "it is already gone");

        // Esc takes back the question before it closes the popup.
        let mut popup = MilitiaPopup { militia: Some(MilitiaId::new()), disbanding: true, ..MilitiaPopup::default() };
        popup.escape();
        assert!(popup.is_open() && !popup.disbanding);
        popup.escape();
        assert!(!popup.is_open());
    }
}
