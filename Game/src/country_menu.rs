//! The countries menu. Clicking the pencil beside the country in the header shows what the country has (its money, cities, people
//! and stock of resources), with a way to rename it or change its color, and a button for the list of countries, where the player can
//! choose the country to play, edit a country's name, change its color, delete one that has no cities, or add a new country. Each of
//! those asks in a popup of its own, and comes back to where it was opened from when it is done. (The country in the header itself is a
//! drop-down for switching, see [`crate::country_dropdown`].)

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::color_picker::{ColorPicker, Outcome};
use crate::day::DaySave;
use nworld_map::world::Resource;

use crate::country::resource_name;
use crate::format::{money, thousands, thousands_u64};
use crate::hud;
use crate::ids::CountryId;
use crate::popup::{self, Icon, Popup};
use crate::rgb::Rgb;

const CARD_WIDTH: f32 = 480.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 34.0;
/// The most countries the list shows before it scrolls.
const MAX_ROWS: usize = 10;
const LIST_PADDING: f32 = 6.0;
/// The most countries listed to send a deleted country's coins and resources to, before the list scrolls.
const HEIR_ROWS: usize = 5;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the list starts, below the card's top: room for the title.
const LIST_TOP: f32 = 84.0;
/// How wide the card of what a country has is, and how tall each line of it.
const INFO_WIDTH: f32 = 400.0;
const INFO_ROW: f32 = 26.0;
/// How long the menu takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player chose in the menu, for the caller to do and save.
#[derive(Debug, PartialEq)]
pub enum Choice {
    /// Play this country.
    Select(CountryId),
    /// Found a country with this name.
    Add(String),
    /// Give this country this name.
    Rename(CountryId, String),
    /// Give this country this color.
    Recolor(CountryId, Rgb),
    /// Delete this country, which owns nothing, and send its coins and resources to the second, if it has any.
    Delete(CountryId, Option<CountryId>),
}

impl Choice {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Select(id) => day.select_country(*id),
            Self::Add(name) => day.found_country(name),
            Self::Rename(id, name) => day.rename_country(*id, name),
            Self::Recolor(id, color) => day.recolor_country(*id, *color),
            Self::Delete(id, heir) => day.delete_country(*id, *heir),
        }
    }
}

#[derive(Default)]
enum Screen {
    #[default]
    Closed,
    /// What this country has, with a way to the list.
    Info(CountryId),
    List,
    Adding(Popup),
    Renaming(CountryId, Popup),
    Coloring(ColorPicker),
    /// Asking whether to really delete this country.
    Deleting(CountryId),
}

#[derive(Default)]
pub struct CountryMenu {
    screen: Screen,
    opened_at: f64,
    /// Set when the list has just come up, so the click that did it isn't also a click on the list.
    fresh: bool,
    /// The row picked with the arrow keys or the mouse, and how far down the list is scrolled.
    highlight: usize,
    scroll: usize,
    /// Why the last choice couldn't be done, while the list is showing.
    error: Option<String>,
    /// The country whose card the player came from to rename it or change its color, which comes back when that is done, and not
    /// the list.
    from_card: Option<CountryId>,
    /// While asking whether to delete a country: the country picked to get what it has, and how far down the list of them is
    /// scrolled.
    heir: Option<CountryId>,
    heir_scroll: usize,
}

impl CountryMenu {
    pub fn is_open(&self) -> bool {
        !matches!(self.screen, Screen::Closed)
    }

    /// Opens what the country the player plays has, with a way to the list of countries to play another. Does nothing
    /// if there is no country.
    pub fn open_info(&mut self, day: &DaySave) {
        self.open_info_at(day, get_time());
    }

    /// The same, as of the time `now`, in seconds, which the menu fades in from.
    fn open_info_at(&mut self, day: &DaySave, now: f64) {
        if let Some(country) = day.player_country() {
            self.screen = Screen::Info(country.id);
            self.fresh = true;
            self.error = None;
            self.from_card = None;
            self.opened_at = now;
        }
    }

    /// From what a country has, on to the list of countries, with that country picked.
    fn switch_country(&mut self, day: &DaySave, id: CountryId) {
        self.show_list(day.countries.iter().position(|c| c.id == id).unwrap_or(0));
    }

    /// Opens the list of the countries of `day`, with `country` picked, which is in view.
    pub fn open_at(&mut self, day: &DaySave, country: Option<CountryId>) {
        let at = country.and_then(|id| day.countries.iter().position(|c| c.id == id)).unwrap_or(0);
        self.show_list(at);
        self.opened_at = get_time();
    }

    /// Back from a popup: to the card of the country it was opened from, if it was, and otherwise to the list.
    fn back(&mut self) {
        match self.from_card {
            Some(id) => {
                self.screen = Screen::Info(id);
                self.fresh = true;
            }
            None => self.show_list(self.highlight),
        }
    }

    fn show_list(&mut self, highlight: usize) {
        self.from_card = None;
        self.screen = Screen::List;
        self.fresh = true;
        self.error = None;
        self.highlight = highlight;
        self.scroll = highlight.saturating_sub(MAX_ROWS - 1);
    }

    /// What Esc does: back from a popup to the list, and from the list to the game.
    pub fn escape(&mut self) {
        match self.screen {
            Screen::Adding(_) | Screen::Renaming(..) | Screen::Coloring(_) | Screen::Deleting(_) => self.back(),
            Screen::Info(_) | Screen::List | Screen::Closed => self.screen = Screen::Closed,
        }
    }

    /// Shows why `choice`, which `show` gave, couldn't be done.
    pub fn fail(&mut self, message: String) {
        match &mut self.screen {
            Screen::Adding(popup) | Screen::Renaming(_, popup) => popup.fail(message),
            Screen::Coloring(_) | Screen::Deleting(_) => {
                self.back();
                self.error = Some(message);
            }
            _ => self.error = Some(message),
        }
    }

    /// Carries on after `choice`, which `show` gave, was done: choosing a country closes the menu, and
    /// the others come back to the list.
    pub fn done(&mut self, choice: &Choice, day: &DaySave) {
        match choice {
            Choice::Select(_) => self.screen = Screen::Closed,
            // With none left there is nothing to list, and the game asks for a new country.
            Choice::Delete(..) if day.countries.is_empty() => self.screen = Screen::Closed,
            Choice::Delete(..) => self.show_list(self.highlight.min(day.countries.len() - 1)),
            Choice::Add(_) => self.show_list(day.countries.len().saturating_sub(1)),
            Choice::Rename(id, _) | Choice::Recolor(id, _) => {
                self.highlight = day.countries.iter().position(|c| c.id == *id).unwrap_or(0);
                self.back();
            }
        }
    }

    /// Takes the keyboard and mouse for this frame and draws the menu over everything. Returns what the
    /// player chose, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Choice> {
        match &mut self.screen {
            Screen::Closed => None,
            Screen::Adding(popup) => popup.show(style).map(|typed| Choice::Add(typed.into_iter().next().unwrap_or_default())),
            Screen::Renaming(id, popup) => {
                let id = *id;
                popup.show(style).map(|typed| Choice::Rename(id, typed.into_iter().next().unwrap_or_default()))
            }
            Screen::Coloring(picker) => match picker.show(style) {
                Some(Outcome::Save(color)) => Some(Choice::Recolor(picker.country(), color)),
                Some(Outcome::Cancel) => {
                    self.back();
                    None
                }
                None => None,
            },
            Screen::Deleting(id) => {
                let id = *id;
                self.confirm_delete(style, day, id)
            }
            Screen::Info(id) => {
                let id = *id;
                self.info(style, day, id)
            }
            Screen::List => self.list(style, day),
        }
    }

    /// Shows what the country `id` has, and a button to go on to the list of countries.
    fn info(&mut self, style: &Style, day: &DaySave, id: CountryId) -> Option<Choice> {
        let Some(country) = day.country(id) else {
            self.screen = Screen::Closed;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let totals = day.country_totals(id);
        let facts = [
            ("Money", money(country.money), true),
            ("Cities", thousands(totals.cities as u32), false),
            ("Population", thousands_u64(totals.population), false),
            ("Regions owned", thousands(totals.regions as u32), false),
            ("People", thousands(totals.people as u32), false),
        ];
        let resource_rows = Resource::COUNT.div_ceil(2);
        let height = LIST_TOP - 8.0 + facts.len() as f32 * INFO_ROW + 14.0 + 22.0 + resource_rows as f32 * INFO_ROW + 18.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - INFO_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, INFO_WIDTH, height);
        style::card(card, 18.0, style::PANEL);
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;

        // The country's name and color, beside the icon.
        let badge = Rect::new(inner_x, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        // The color, which opens the color picker, and a chip that renames the country.
        let swatch = Rect::new(card.right() - PADDING - 18.0, card.y + PADDING + 3.0, 18.0, 18.0);
        let over_swatch = swatch.contains(mouse);
        style::rounded_rect(swatch, 4.0, country.color.color());
        style::rounded_rect_lines(swatch, 4.0, if over_swatch { 2.0 } else { 1.0 }, if over_swatch { WHITE } else { style::BORDER_STRONG });
        let rename = Rect::new(swatch.x - 8.0 - 56.0, card.y + PADDING + 0.0, 56.0, 24.0);
        let over_rename = rename.contains(mouse);
        style::rounded_rect(rename, 6.0, if over_rename { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Rename", rename, 12, Weight::Semibold, if over_rename { style::TEXT } else { style::TEXT_MUTED });
        let name = fit(style, &country.name, 20, Weight::Semibold, rename.x - 12.0 - text_x);
        style.text(&name, text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("The country you play", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What it has: a label at the left and the amount at the right of each line.
        let mut y = card.y + LIST_TOP - 8.0;
        for (label, value, is_money) in &facts {
            let mid = y + INFO_ROW * 0.5;
            style.text_mid(label, inner_x, mid, 13, Weight::Regular, style::TEXT_MUTED);
            let width = style.measure(value, 14, Weight::Semibold).width;
            let color = if *is_money && country.money < 0 { style::ERROR } else { style::TEXT };
            style.text_mid(value, inner_x + inner_w - width, mid, 14, Weight::Semibold, color);
            if *is_money {
                hud::draw_coin(vec2(inner_x + inner_w - width - 12.0, mid), 5.5);
            }
            y += INFO_ROW;
        }

        // Its stock of each resource, two to a line.
        y += 14.0;
        draw_line(inner_x, y - 7.0, inner_x + inner_w, y - 7.0, 1.0, style::BORDER);
        style.text_mid("RESOURCES", inner_x, y + 4.0, 11, Weight::Semibold, style::TEXT_MUTED);
        y += 22.0;
        let cell = (inner_w - 20.0) * 0.5;
        for (i, resource) in Resource::ALL.into_iter().enumerate() {
            let x = inner_x + (i % 2) as f32 * (cell + 20.0);
            let mid = y + (i / 2) as f32 * INFO_ROW + INFO_ROW * 0.5;
            let amount = country.stock.get(resource);
            draw_circle(x + 5.0, mid, 4.5, hud::resource_color(resource));
            let name = resource_name(resource);
            let label: String = name.chars().next().map(|c| c.to_uppercase().chain(name.chars().skip(1)).collect()).unwrap_or_default();
            style.text_mid(&label, x + 18.0, mid, 13, Weight::Regular, style::TEXT_MUTED);
            let text = thousands_u64(amount);
            let width = style.measure(&text, 14, Weight::Semibold).width;
            style.text_mid(&text, x + cell - width, mid, 14, Weight::Semibold, if amount == 0 { style::TEXT_MUTED } else { style::TEXT });
        }

        // Close, and on to the other countries.
        let top = card.bottom() - PADDING - BUTTON_HEIGHT;
        let close = Rect::new(inner_x, top, 110.0, BUTTON_HEIGHT);
        let switch = Rect::new(close.right() + 12.0, top, inner_w - 110.0 - 12.0, BUTTON_HEIGHT);
        let (over_close, over_switch) = (close.contains(mouse), switch.contains(mouse));
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);
        style::rounded_rect(switch, 8.0, if over_switch { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("Switch country", switch, 14, Weight::Semibold, WHITE);

        if clicked && over_switch {
            self.switch_country(day, id);
        } else if clicked && over_rename {
            self.from_card = Some(id);
            self.screen = Screen::Renaming(id, Popup::rename_country(&country.name));
        } else if clicked && over_swatch {
            self.from_card = Some(id);
            self.screen = Screen::Coloring(ColorPicker::new(id, &country.name, country.color));
        } else if clicked && (over_close || !card.contains(mouse)) {
            self.screen = Screen::Closed;
        }
        None
    }

    /// Asks whether to really delete the country `id`, and what goes with it.
    fn confirm_delete(&mut self, style: &Style, day: &DaySave, id: CountryId) -> Option<Choice> {
        let Some(country) = day.country(id) else {
            self.show_list(self.highlight);
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        // What it has has to be sent to another country, if there is one, which is picked from a list.
        let wealth = day.wealth(id);
        let others: Vec<_> = day.countries.iter().filter(|c| c.id != id).collect();
        let sending = !wealth.is_empty() && !others.is_empty();
        let heir = self.heir.filter(|heir| others.iter().any(|c| c.id == *heir));
        let shown = others.len().min(HEIR_ROWS);
        let held: Vec<String> = wealth.iter().map(|(asset, amount)| format!("{} {}", thousands_u64(*amount), asset.name())).collect();
        let held = held.join(", ");
        let held_lines = if wealth.is_empty() { Vec::new() } else { hud::wrap(style, &format!("It has {held}."), 13, 400.0 - PADDING * 2.0) };
        let wealth_height = if wealth.is_empty() { 0.0 } else { held_lines.len() as f32 * 18.0 + 8.0 };
        let list_height = if sending { 26.0 + shown as f32 * ROW_HEIGHT + LIST_PADDING * 2.0 + 14.0 } else { 0.0 };
        let height = 84.0 + 44.0 + wealth_height + list_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - 400.0) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, 400.0, height);
        style::card(card, 18.0, style::PANEL);
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;

        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, Color::new(ERROR_SOFT.0, ERROR_SOFT.1, ERROR_SOFT.2, 0.18));
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        let name = fit(style, &country.name, 20, Weight::Semibold, card.right() - PADDING - text_x - 60.0);
        style.text(&format!("Delete {name}?"), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        // What goes with it.
        let people = day.people.iter().filter(|p| p.country == Some(id)).count();
        let regions = day.region_owners.iter().filter(|o| o.country == id).count();
        let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let goes = match (people, regions) {
            (0, 0) => "Nothing else belongs to it.".to_owned(),
            (p, 0) => format!("Its {} will become {}.", plural(p, "person", "people"), if p == 1 { "a nomad" } else { "nomads" }),
            (0, r) => format!("Its {} will be unowned.", plural(r, "region", "regions")),
            // (Shorter, so that it fits beside the badge.)
            (p, r) => format!("{} to be {}, {} unowned.", plural(p, "person", "people"), if p == 1 { "a nomad" } else { "nomads" }, plural(r, "region", "regions")),
        };
        style.text(&goes, text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text("This can't be undone.", inner_x, card.y + 84.0 + 14.0, 13, Weight::Regular, style::TEXT);
        let mut y = card.y + 84.0 + 36.0;
        for line in &held_lines {
            style.text(line, inner_x, y + 12.0, 13, Weight::Regular, style::TEXT);
            y += 18.0;
        }
        if !wealth.is_empty() && others.is_empty() {
            style.text("There is no other country, so it is lost.", inner_x, y + 12.0, 12, Weight::Regular, style::ERROR);
        }
        if sending {
            style.text("Send it all to", inner_x, y + 26.0, 12, Weight::Semibold, style::TEXT_MUTED);
            let list = Rect::new(inner_x, y + 34.0, inner_w, shown as f32 * ROW_HEIGHT + LIST_PADDING * 2.0);
            if list.contains(mouse) && mouse_wheel().1 != 0.0 {
                self.heir_scroll = if mouse_wheel().1 > 0.0 { self.heir_scroll.saturating_sub(1) } else { self.heir_scroll + 1 };
            }
            self.heir_scroll = self.heir_scroll.min(others.len() - shown);
            style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
            for (row, other) in others.iter().skip(self.heir_scroll).take(shown).enumerate() {
                let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * ROW_HEIGHT, list.w - LIST_PADDING * 2.0, ROW_HEIGHT);
                if rect.contains(mouse) {
                    style::rounded_rect(rect, 6.0, style::HOVER);
                    if clicked {
                        self.heir = Some(other.id);
                    }
                }
                let mark = vec2(rect.x + 14.0, rect.center().y);
                if heir == Some(other.id) {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, other.color.color());
                let name = fit(style, &other.name, 14, Weight::Regular, rect.w - 60.0);
                style.text_mid(&name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
            }
        }

        // Cancel and Delete, side by side.
        let top = card.bottom() - PADDING - BUTTON_HEIGHT;
        let cancel = Rect::new(inner_x, top, (inner_w - 12.0) * 0.5, BUTTON_HEIGHT);
        let delete = Rect::new(cancel.right() + 12.0, top, cancel.w, BUTTON_HEIGHT);
        // Delete waits for a country to be picked, if there is something to send.
        let can_delete = !sending || heir.is_some();
        let (over_cancel, over_delete) = (cancel.contains(mouse), can_delete && delete.contains(mouse));
        style::rounded_rect(cancel, 8.0, if over_cancel { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Cancel", cancel, 14, Weight::Semibold, style::TEXT);
        let fill = if !can_delete { style::HOVER } else if over_delete { style::ERROR } else { Color::new(ERROR_SOFT.0, ERROR_SOFT.1, ERROR_SOFT.2, 0.85) };
        style::rounded_rect(delete, 8.0, fill);
        style.text_centered("Delete", delete, 14, Weight::Semibold, if can_delete { WHITE } else { style::TEXT_MUTED });

        if clicked && over_delete {
            return Some(Choice::Delete(id, heir.filter(|_| sending)));
        }
        // Cancel, or a click on the dimmed window outside the card, goes back to the list.
        if clicked && (over_cancel || !card.contains(mouse)) {
            self.show_list(self.highlight);
        }
        None
    }

    fn list(&mut self, style: &Style, day: &DaySave) -> Option<Choice> {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let current = day.player_country().map(|c| c.id);
        let count = day.countries.len();
        let rows = count.min(MAX_ROWS);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let list_height = rows as f32 * ROW_HEIGHT + LIST_PADDING * 2.0;
        let height = LIST_TOP + list_height + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        style.text("Countries", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("Choose the country you play, edit one, or add a new one.", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // The list, which the mouse wheel scrolls when there are more countries than rows.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let list = Rect::new(inner_x, card.y + LIST_TOP, inner_w, list_height);
        style::rounded_rect(list, 10.0, style::FIELD);
        style::rounded_rect_lines(list, 10.0, 1.0, style::BORDER_STRONG);
        if list.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(count - rows);

        // The arrow keys move the pick, the mouse picks what it is over, and Enter chooses it.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if is_key_pressed(KeyCode::Down) {
            self.highlight = (self.highlight + 1).min(count.saturating_sub(1));
        }
        if is_key_pressed(KeyCode::Up) {
            self.highlight = self.highlight.saturating_sub(1);
        }
        self.scroll = self.scroll.clamp((self.highlight + 1).saturating_sub(rows), self.highlight);

        let mut chosen = None;
        let mut renaming = None;
        let mut coloring = None;
        let mut deleting = None;
        for row in 0..rows {
            let place = self.scroll + row;
            let country = &day.countries[place];
            let rect = Rect::new(list.x + LIST_PADDING, list.y + LIST_PADDING + row as f32 * ROW_HEIGHT, list.w - LIST_PADDING * 2.0, ROW_HEIGHT);
            let edit = Rect::new(rect.right() - 8.0 - 46.0, rect.center().y - 12.0, 46.0, 24.0);
            let delete = Rect::new(edit.x - 4.0 - 24.0, edit.y, 24.0, 24.0);
            let can_delete = day.delete_problem(country.id).is_none();
            let over_row = rect.contains(mouse);
            if over_row && mouse_delta_position() != Vec2::ZERO {
                self.highlight = place;
            }
            if self.highlight == place {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }

            // A round mark that is filled for the country that is played.
            let mark = vec2(rect.x + 18.0, rect.center().y);
            if current == Some(country.id) {
                draw_circle(mark.x, mark.y, 7.0, style::ACCENT);
                draw_circle(mark.x, mark.y, 2.8, WHITE);
            } else {
                draw_circle_lines(mark.x, mark.y, 6.5, 1.5, style::BORDER_STRONG);
            }
            // The country's color, which opens the color picker.
            let swatch = Rect::new(rect.x + 34.0, rect.center().y - 8.0, 16.0, 16.0);
            let over_swatch = swatch.contains(mouse);
            style::rounded_rect(swatch, 4.0, country.color.color());
            style::rounded_rect_lines(swatch, 4.0, if over_swatch { 2.0 } else { 1.0 }, if over_swatch { WHITE } else { style::BORDER_STRONG });
            let weight = if current == Some(country.id) { Weight::Semibold } else { Weight::Regular };
            // The country's money, in a column of its own before the buttons.
            let money_right = delete.x - 14.0;
            let money_text = money(country.money);
            let money_width = style.measure(&money_text, 13, Weight::Regular).width;
            let money_color = if country.money < 0 { style::ERROR } else { style::TEXT_MUTED };
            style.text_mid(&money_text, money_right - money_width, rect.center().y, 13, Weight::Regular, money_color);
            hud::draw_coin(vec2(money_right - money_width - 12.0, rect.center().y), 5.5);
            // The name has room up to the money.
            let name_end = money_right - 90.0;
            let name = fit(style, &country.name, 15, weight, name_end - 8.0 - (rect.x + 62.0));
            style.text_mid(&name, rect.x + 62.0, rect.center().y, 15, weight, style::TEXT);

            let over_edit = edit.contains(mouse);
            style::rounded_rect(edit, 6.0, if over_edit { style::BORDER_STRONG } else { style::HOVER });
            style.text_centered("Edit", edit, 12, Weight::Semibold, if over_edit { style::TEXT } else { style::TEXT_MUTED });
            // Delete, a cross, only for a country that owns nothing.
            let over_delete = can_delete && delete.contains(mouse);
            if can_delete {
                if over_delete {
                    style::rounded_rect(delete, 6.0, Color::new(ERROR_SOFT.0, ERROR_SOFT.1, ERROR_SOFT.2, 0.25));
                }
                let cross = if over_delete { style::ERROR } else { style::TEXT_MUTED };
                let c = delete.center();
                draw_line(c.x - 4.0, c.y - 4.0, c.x + 4.0, c.y + 4.0, 1.6, cross);
                draw_line(c.x - 4.0, c.y + 4.0, c.x + 4.0, c.y - 4.0, 1.6, cross);
            }

            if clicked && over_delete {
                deleting = Some((country.id, place));
            } else if clicked && over_edit {
                renaming = Some((country.id, country.name.clone(), place));
            } else if clicked && over_swatch {
                coloring = Some((country.id, country.name.clone(), country.color, place));
            } else if clicked && over_row {
                chosen = Some(country.id);
            }
        }
        if enter {
            chosen = day.countries.get(self.highlight).map(|c| c.id);
        }
        // More above or below than shows: a small mark at the edge.
        if self.scroll > 0 {
            draw_triangle(vec2(list.right() - 18.0, list.y + 10.0), vec2(list.right() - 10.0, list.y + 10.0), vec2(list.right() - 14.0, list.y + 5.0), style::TEXT_MUTED);
        }
        if self.scroll + rows < count {
            draw_triangle(vec2(list.right() - 18.0, list.bottom() - 10.0), vec2(list.right() - 10.0, list.bottom() - 10.0), vec2(list.right() - 14.0, list.bottom() - 5.0), style::TEXT_MUTED);
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, list.bottom() + 22.0, 12, Weight::Regular, style::ERROR);
        }

        // The button for a new country.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let hovered = button.contains(mouse);
        style::rounded_rect(button, 8.0, if hovered { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("+  Add a new country", button, 14, Weight::Semibold, WHITE);

        if clicked && hovered {
            self.screen = Screen::Adding(Popup::country());
            return None;
        }
        if let Some((id, place)) = deleting {
            self.highlight = place;
            self.fresh = true;
            self.heir = None;
            self.heir_scroll = 0;
            self.screen = Screen::Deleting(id);
            return None;
        }
        if let Some((id, name, place)) = renaming {
            self.highlight = place;
            self.screen = Screen::Renaming(id, Popup::rename_country(&name));
            return None;
        }
        if let Some((id, name, color, place)) = coloring {
            self.highlight = place;
            self.screen = Screen::Coloring(ColorPicker::new(id, &name, color));
            return None;
        }
        if let Some(id) = chosen {
            // Choosing the country that is already played changes nothing, so there is nothing to save.
            if current == Some(id) {
                self.screen = Screen::Closed;
                return None;
            }
            return Some(Choice::Select(id));
        }
        // A click on the dimmed window outside the card closes the menu.
        if clicked && !card.contains(mouse) {
            self.screen = Screen::Closed;
        }
        None
    }
}

/// A red for the Delete button and its icon, a little softer than the one for errors.
const ERROR_SOFT: (f32, f32, f32) = (0.80, 0.27, 0.27);

/// `text` cut short, with "..." at the end, so that it is no wider than `width`.
pub fn fit(style: &Style, text: &str, size: u16, weight: Weight, width: f32) -> String {
    if style.measure(text, size, weight).width <= width {
        return text.to_owned();
    }
    let mut shown: String = text.to_owned();
    while !shown.is_empty() && style.measure(&format!("{shown}..."), size, weight).width > width {
        shown.pop();
    }
    format!("{}...", shown.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day
    }

    #[test]
    fn what_a_country_has_opens_for_the_country_played_and_goes_on_to_the_list_with_it_picked() {
        let mut day = day();
        day.select_country(day.countries[1].id).unwrap();
        let mut menu = CountryMenu::default();
        assert!(!menu.is_open());
        menu.open_info_at(&day, 0.0);
        assert!(menu.is_open());
        assert!(matches!(menu.screen, Screen::Info(id) if id == day.countries[1].id));
        menu.switch_country(&day, day.countries[1].id);
        assert!(matches!(menu.screen, Screen::List));
        assert_eq!(menu.highlight, 1, "the country played is the one picked");
    }

    #[test]
    fn escape_closes_what_a_country_has_and_nothing_opens_without_a_country() {
        let day = day();
        let mut menu = CountryMenu::default();
        menu.open_info_at(&day, 0.0);
        menu.escape();
        assert!(!menu.is_open());
        menu.open_info_at(&DaySave::new(crate::calendar::Date::FIRST, "Assets/map.nworld"), 0.0);
        assert!(!menu.is_open(), "there is no country to tell about");
    }

    #[test]
    fn renaming_or_coloring_from_the_card_of_a_country_comes_back_to_it_and_from_the_list_comes_back_to_the_list() {
        let mut day = day();
        let brevon = day.countries[1].id;
        let mut menu = CountryMenu::default();

        // From the card: when it is done, or given up, the card is back. (A popup that asks for something needs the game's window, so
        // the screens here are ones that don't.)
        menu.from_card = Some(brevon);
        menu.screen = Screen::Deleting(brevon);
        let choice = Choice::Rename(brevon, "New Brevon".to_owned());
        choice.apply(&mut day).unwrap();
        menu.done(&choice, &day);
        assert!(matches!(menu.screen, Screen::Info(id) if id == brevon));
        menu.from_card = Some(brevon);
        menu.screen = Screen::Deleting(brevon);
        menu.escape();
        assert!(matches!(menu.screen, Screen::Info(id) if id == brevon), "Esc goes back to the card");

        // From the list, it is the list.
        menu.screen = Screen::Deleting(brevon);
        menu.from_card = None;
        menu.escape();
        assert!(matches!(menu.screen, Screen::List));
        assert_eq!(menu.from_card, None);

        // Going on to the list from the card forgets where it came from.
        menu.from_card = Some(brevon);
        menu.switch_country(&day, brevon);
        assert_eq!(menu.from_card, None);
    }

    #[test]
    fn a_choice_does_what_it_says_to_the_day() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);

        Choice::Select(brevon).apply(&mut day).unwrap();
        assert_eq!(day.player, Some(brevon));
        Choice::Rename(aldoria, "Old Aldoria".to_owned()).apply(&mut day).unwrap();
        assert_eq!(day.country_name(aldoria), "Old Aldoria");
        Choice::Add("Cardell".to_owned()).apply(&mut day).unwrap();
        assert_eq!(day.countries.len(), 3);
        assert_eq!(day.player, Some(brevon), "adding a country doesn't change the one that is played");
    }

    #[test]
    fn deleting_a_country_is_a_choice() {
        let mut day = day();
        let brevon = day.countries[1].id;
        Choice::Delete(brevon, None).apply(&mut day).unwrap();
        assert_eq!(day.countries.len(), 1);
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        assert!(Choice::Delete(day.countries[0].id, None).apply(&mut day).unwrap_err().contains("owns 1 city"));
    }

    #[test]
    fn a_choice_that_cannot_be_done_says_why_and_changes_nothing() {
        let mut day = day();
        let before = day.clone();
        assert!(Choice::Add("brevon".to_owned()).apply(&mut day).unwrap_err().contains("already"));
        assert!(Choice::Rename(day.countries[0].id, "BREVON".to_owned()).apply(&mut day).is_err());
        assert!(Choice::Select(CountryId::new()).apply(&mut day).is_err());
        assert_eq!(day, before);
    }
}
