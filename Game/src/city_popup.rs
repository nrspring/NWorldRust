//! The city popup, opened by clicking a city: what there is to know of it, and a way to change which
//! country it belongs to.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::ids::{CityId, CountryId};
use crate::militia;
use crate::options;
use crate::popup::{self, Icon};

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
/// The room for raising a militia: its heading, the slider for how many, and the button.
const MILITIA_HEIGHT: f32 = 36.0 + 30.0 + BUTTON_HEIGHT + 12.0;
const SLIDER_HEIGHT: f32 = 30.0;

/// What the player changed about the city in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The city now belongs to this country.
    Country(CityId, CountryId),
    /// The city is now under siege (or not).
    Siege(CityId, bool),
    /// The city raises a militia of this many members, camped at this place outside it.
    Militia { city: CityId, members: u32, camp: Vec2 },
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Country(city, country) => day.set_city_country(*city, *country),
            Self::Siege(city, under_siege) => day.set_city_under_siege(*city, *under_siege),
            Self::Militia { city, members, camp } => day.raise_militia(*city, *members, *camp),
        }
    }
}

/// `text` broken into lines no wider than `width` when drawn at `size`, at the spaces.
fn wrap(style: &Style, text: &str, width: f32, size: u16) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if style.measure(&format!("{line} {word}"), size, Weight::Regular).width <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

/// How many members the slider says for a mouse at `x` over a track from `left` for `width`, of 1 up to `most`: the far left is 1 and the
/// far right is all of them.
pub(crate) fn members_at(x: f32, left: f32, width: f32, most: u32) -> u32 {
    let along = ((x - left) / width).clamp(0.0, 1.0);
    1 + (along * most.saturating_sub(1) as f32).round() as u32
}

#[derive(Default)]
pub struct CityPopup {
    /// The city being shown, while the popup is up.
    city: Option<CityId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Whether the list of countries to change to is showing, and how far down it is scrolled.
    choosing: bool,
    scroll: usize,
    /// How many members the slider is set to for the militia to raise, or `None` for all it can (until it is moved), and
    /// whether it is being dragged.
    members: Option<u32>,
    dragging: bool,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl CityPopup {
    pub fn is_open(&self) -> bool {
        self.city.is_some()
    }

    pub fn open(&mut self, city: CityId) {
        *self = Self { city: Some(city), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the list of countries if it is showing, and otherwise the popup.
    pub fn escape(&mut self) {
        if self.choosing {
            self.choosing = false;
        } else {
            self.city = None;
        }
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.choosing = false;
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A city that is not
    /// in `day` is not shown. `world` is the ground a militia can camp on. Returns what the player changed about the city, if
    /// they did.
    pub fn show(&mut self, style: &Style, day: &DaySave, world: &World) -> Option<Change> {
        let Some(city) = self.city.and_then(|id| day.city(id)) else {
            self.city = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let country = day.country(city.country);
        let error_lines = self.error.as_deref().map_or_else(Vec::new, |error| wrap(style, error, CARD_WIDTH - PADDING * 2.0, 12));
        let error_height = if error_lines.is_empty() { 0.0 } else { 10.0 + 15.0 * error_lines.len() as f32 };
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + 16.0 + MILITIA_HEIGHT + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The name, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::City, badge);
        let text_x = badge.right() + 14.0;
        style.text(&city.name, text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("City", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each. The country's can be clicked, to change it.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let country_row = Rect::new(inner_x, card.y + ROWS_TOP, inner_w, ROW_HEIGHT);
        let population_row = Rect::new(inner_x, country_row.bottom(), inner_w, ROW_HEIGHT);
        draw_line(inner_x, country_row.y, inner_x + inner_w, country_row.y, 1.0, style::BORDER);
        draw_line(inner_x, population_row.y, inner_x + inner_w, population_row.y, 1.0, style::BORDER);
        let siege_row = Rect::new(inner_x, population_row.bottom(), inner_w, ROW_HEIGHT);
        draw_line(inner_x, siege_row.y, inner_x + inner_w, siege_row.y, 1.0, style::BORDER);
        draw_line(inner_x, siege_row.bottom(), inner_x + inner_w, siege_row.bottom(), 1.0, style::BORDER);

        let over_country = country_row.contains(mouse);
        if over_country || self.choosing {
            style::rounded_rect(Rect::new(country_row.x - 8.0, country_row.y + 1.0, country_row.w + 16.0, country_row.h - 2.0), 8.0, style::HOVER);
        }
        let middle = country_row.center().y;
        style.text_mid("Country", inner_x, middle, 13, Weight::Regular, style::TEXT_MUTED);
        let name = country.map_or("an unknown country", |c| c.name.as_str());
        // At the right: the name, the country's color before it, and an arrow after it that says there is a
        // list to choose from.
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

        let population = thousands(city.population);
        style.text_mid("Population", inner_x, population_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let population_x = population_row.right() - style.measure(&population, 14, Weight::Semibold).width;
        style.text_mid(&population, population_x, population_row.center().y, 14, Weight::Semibold, style::TEXT);

        // The siege switch, which is for the list of countries to cover while it is open.
        let mut under_siege = city.under_siege;
        options::toggle(style, Rect::new(siege_row.x - 12.0, siege_row.y, siege_row.w + 24.0, siege_row.h), "Under siege", None, &mut under_siege, !self.choosing);
        if under_siege != city.under_siege {
            change = Some(Change::Siege(city.id, under_siege));
        }

        // Raising a militia: how many, on a slider that goes up to what the city can spare and its country can pay for,
        // and a button. When none can be raised, it says why.
        let militia_row = Rect::new(inner_x, siege_row.bottom() + 8.0, inner_w, 36.0);
        let slider = Rect::new(inner_x, militia_row.bottom(), inner_w, SLIDER_HEIGHT);
        let raise = Rect::new(inner_x, slider.bottom() + 6.0, inner_w, BUTTON_HEIGHT);
        let problem = day.militia_problem(city.id);
        let most = day.most_militia(city.id);
        let raising = problem.is_none() && most > 0;
        let track = Rect::new(slider.x + 8.0, slider.center().y - 3.0, slider.w - 16.0, 6.0);
        // The slider takes the mouse when it is pressed over it, until it is let go.
        if raising && clicked && slider.contains(mouse) && !self.choosing {
            self.dragging = true;
        }
        if !raising || !is_mouse_button_down(MouseButton::Left) {
            self.dragging = false;
        }
        if self.dragging {
            self.members = Some(members_at(mouse.x, track.x, track.w, most));
        }
        let members = self.members.unwrap_or(most).clamp(1, most.max(1));
        style.text_mid("Raise militia", inner_x, militia_row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        let summary = match &problem {
            None => format!("{} of up to {}", thousands(members), thousands(most)),
            Some(_) => "None today".to_owned(),
        };
        let summary_x = militia_row.right() - style.measure(&summary, 14, Weight::Semibold).width;
        style.text_mid(&summary, summary_x, militia_row.center().y, 14, Weight::Semibold, if raising { style::TEXT } else { style::TEXT_MUTED });
        if raising {
            let fraction = if most > 1 { (members - 1) as f32 / (most - 1) as f32 } else { 1.0 };
            let knob = vec2(track.x + track.w * fraction, track.center().y);
            style::rounded_rect(track, 3.0, style::BORDER_STRONG);
            style::rounded_rect(Rect::new(track.x, track.y, knob.x - track.x, track.h), 3.0, style::ACCENT);
            draw_circle(knob.x, knob.y, 9.0, style::ACCENT);
            draw_circle(knob.x, knob.y, 3.4, WHITE);
        } else {
            let lines = wrap(style, problem.as_deref().unwrap_or(""), slider.w, 12);
            for (n, line) in lines.iter().take(2).enumerate() {
                style.text_mid(line, slider.x, slider.center().y + (n as f32 - (lines.len().min(2) as f32 - 1.0) * 0.5) * 15.0, 12, Weight::Regular, style::TEXT_MUTED);
            }
        }
        let label = if raising { format!("Raise {} militia for {} coins", thousands(members), money(militia::raising_cost(members))) } else { "Raise militia".to_owned() };
        let over_raise = raise.contains(mouse) && !self.choosing && raising;
        let fill = if !raising { style::HOVER } else if over_raise { style::ACCENT_HOVER } else { style::ACCENT };
        style::rounded_rect(raise, 8.0, fill);
        style.text_centered(&label, raise, 14, Weight::Semibold, if raising { WHITE } else { style::TEXT_MUTED });
        if clicked && over_raise {
            match militia::camp_near(world, city, &day.camps_of(city.id)) {
                Some(camp) => {
                    self.error = None;
                    change = Some(Change::Militia { city: city.id, members, camp });
                }
                None => self.error = Some(format!("There is no dry ground near {} to camp a militia on.", city.name)),
            }
        }

        for (n, line) in error_lines.iter().enumerate() {
            style.text(line, inner_x, raise.bottom() + 20.0 + n as f32 * 15.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_button = button.contains(mouse) && !self.choosing;
        style::rounded_rect(button, 8.0, if over_button { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", button, 14, Weight::Semibold, style::TEXT);

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
                if listed.id == city.country {
                    draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                    draw_circle(mark.x, mark.y, 2.4, WHITE);
                } else {
                    draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
                }
                style::rounded_rect(Rect::new(rect.x + 30.0, rect.center().y - 6.0, 12.0, 12.0), 3.0, listed.color.color());
                style.text_mid(&listed.name, rect.x + 50.0, rect.center().y, 14, Weight::Regular, style::TEXT);
                if clicked && rect.contains(mouse) {
                    // The country it is already in is no change, so there is nothing to save.
                    if listed.id != city.country {
                        change = Some(Change::Country(city.id, listed.id));
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
            self.scroll = day.countries.iter().position(|c| c.id == city.country).unwrap_or(0).saturating_sub(MAX_LIST_ROWS - 1);
            used_click = true;
        }

        // A click on the button, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && !used_click && ((enter && !self.choosing) || (clicked && (over_button || !card.contains(mouse)))) {
            self.city = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    #[test]
    fn the_slider_goes_from_one_member_to_the_most() {
        // A track from 100 across 200 pixels, for up to 1,000 members.
        assert_eq!(members_at(100.0, 100.0, 200.0, 1_000), 1);
        assert_eq!(members_at(300.0, 100.0, 200.0, 1_000), 1_000);
        assert_eq!(members_at(200.0, 100.0, 200.0, 1_000), 501, "halfway");
        assert_eq!(members_at(-50.0, 100.0, 200.0, 1_000), 1, "past the left end");
        assert_eq!(members_at(900.0, 100.0, 200.0, 1_000), 1_000, "past the right end");
        assert_eq!(members_at(250.0, 100.0, 200.0, 1), 1, "only one to choose");
    }

    #[test]
    fn raising_a_militia_does_what_it_says_to_the_day() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 1_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 500;
        let city = day.cities[0].id;

        Change::Militia { city, members: 20, camp: vec2(30.0, 0.0) }.apply(&mut day).unwrap();
        assert_eq!((day.militias[0].members, day.cities[0].population), (20, 480));
        assert!(Change::Militia { city, members: 20, camp: vec2(30.0, 0.0) }.apply(&mut day).is_err(), "only one a day");
    }
}
