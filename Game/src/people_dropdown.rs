//! The people drop-down: beside the country in the header, a list of the people of the country being played. Picking one selects them on
//! the map, and the camera goes to them; one who is not on the map yet is put there next. Nobody is picked until one is, and when another country is played nobody is picked again. The
//! last row introduces another person to the country.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::country_menu::fit;
use crate::day::DaySave;
use crate::ids::{CountryId, PersonId};
use crate::person::Person;

const ROW_HEIGHT: f32 = 32.0;
const PADDING: f32 = 6.0;
const WIDTH: f32 = 340.0;
/// The most people it shows before it scrolls.
const MAX_ROWS: usize = 8;

/// What the player picked in the drop-down.
#[derive(Debug, PartialEq)]
pub enum PeoplePick {
    /// Select this person.
    Person(PersonId),
    /// Introduce another person to the country.
    Introduce,
}

/// The people of the country being played, in the order they joined.
pub fn people_of(day: &DaySave) -> impl Iterator<Item = &Person> {
    let country = day.player_country().map(|c| c.id);
    day.people.iter().filter(move |p| Some(p.country) == country)
}

/// The person the drop-down shows as picked: the one `selected` on the map, if they belong to the country being played.
pub fn shown(day: &DaySave, selected: Option<PersonId>) -> Option<&Person> {
    people_of(day).find(|p| Some(p.id) == selected)
}

/// What the header says in the drop-down's place.
pub fn label(day: &DaySave, selected: Option<PersonId>) -> &str {
    match shown(day, selected) {
        Some(person) => &person.name,
        None if people_of(day).next().is_none() => "No people",
        None => "Select a person",
    }
}

#[derive(Default)]
pub struct PeopleDropdown {
    open: bool,
    /// How far down the list is scrolled.
    scroll: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
    /// The country that was being played when last looked.
    country: Option<CountryId>,
}

impl PeopleDropdown {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.open && self.rect.contains(mouse)
    }

    /// Opens it if it is closed, and closes it if it is open: for a click on it in the header.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.scroll = 0;
        self.rect = Rect::new(0.0, 0.0, 0.0, 0.0);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Looks at which country is being played, and says whether it is another one than when last looked, in which case it closes and
    /// whoever was picked is to be let go.
    pub fn country_changed(&mut self, day: &DaySave) -> bool {
        let now = day.player_country().map(|c| c.id);
        let changed = self.country.is_some() && self.country != now;
        self.country = now;
        if changed {
            self.open = false;
        }
        changed
    }

    /// Draws it under `anchor`, its place in the header, with `selected` marked, and returns what the player picked, if they picked
    /// something, which also closes it. A click anywhere else closes it.
    pub fn show(&mut self, style: &Style, day: &DaySave, selected: Option<PersonId>, anchor: Option<Rect>) -> Option<PeoplePick> {
        let Some(anchor) = anchor.filter(|_| self.open) else {
            self.open = false;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let people: Vec<&Person> = people_of(day).collect();
        let rows = people.len().min(MAX_ROWS);

        // A country with nobody in it still has a row, to say so.
        let height = (rows.max(1) + 1) as f32 * ROW_HEIGHT + PADDING * 2.0 + 9.0;
        let panel = Rect::new(anchor.x, anchor.bottom() + 6.0, WIDTH, height);
        self.rect = panel;
        style::card(panel, 10.0, style::PANEL);
        style::rounded_rect_lines(panel, 10.0, 1.0, style::BORDER_STRONG);

        if panel.contains(mouse) && mouse_wheel().1 != 0.0 {
            self.scroll = if mouse_wheel().1 > 0.0 { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
        }
        self.scroll = self.scroll.min(people.len() - rows);

        if people.is_empty() {
            let country = day.player_country().map_or("this country", |c| c.name.as_str());
            let text = fit(style, &format!("Nobody belongs to {country} yet."), 13, Weight::Regular, panel.w - 36.0);
            style.text_mid(&text, panel.x + 18.0, panel.y + PADDING + ROW_HEIGHT * 0.5, 13, Weight::Regular, style::TEXT_MUTED);
        }
        let mut picked = None;
        for (row, person) in people.iter().skip(self.scroll).take(rows).enumerate() {
            let rect = Rect::new(panel.x + PADDING, panel.y + PADDING + row as f32 * ROW_HEIGHT, panel.w - PADDING * 2.0, ROW_HEIGHT);
            let hovered = rect.contains(mouse);
            if hovered {
                style::rounded_rect(rect, 6.0, style::HOVER);
            }
            let chosen = selected == Some(person.id);
            // A round mark that is filled for the person that is picked, as in the list of countries.
            let mark = vec2(rect.x + 16.0, rect.center().y);
            if chosen {
                draw_circle(mark.x, mark.y, 6.0, style::ACCENT);
                draw_circle(mark.x, mark.y, 2.4, WHITE);
            } else {
                draw_circle_lines(mark.x, mark.y, 5.5, 1.5, style::BORDER_STRONG);
            }
            // The movement they have left, at the right, or that they are still to be put on the map.
            let moves = if person.at.is_none() { "Click to place".to_owned() } else { format!("{} moves", person.points) };
            let moves_width = style.measure(&moves, 12, Weight::Regular).width;
            style.text_mid(&moves, rect.right() - 10.0 - moves_width, rect.center().y, 12, Weight::Regular, style::TEXT_MUTED);
            // The name, and after it the title in what room is left.
            let weight = if chosen { Weight::Semibold } else { Weight::Regular };
            let room = rect.right() - 10.0 - moves_width - 12.0 - (rect.x + 32.0);
            let name = fit(style, &person.name, 14, weight, room);
            style.text_mid(&name, rect.x + 32.0, rect.center().y, 14, weight, style::TEXT);
            let after = style.measure(&name, 14, weight).width + 8.0;
            if room - after > 40.0 {
                let title = fit(style, &person.title, 12, Weight::Regular, room - after);
                style.text_mid(&title, rect.x + 32.0 + after, rect.center().y, 12, Weight::Regular, style::TEXT_MUTED);
            }
            if clicked && hovered {
                picked = Some(PeoplePick::Person(person.id));
            }
        }
        // More above or below than shows: a small mark at the edge.
        if self.scroll > 0 {
            draw_triangle(vec2(panel.right() - 18.0, panel.y + 9.0), vec2(panel.right() - 10.0, panel.y + 9.0), vec2(panel.right() - 14.0, panel.y + 4.0), style::TEXT_MUTED);
        }
        if self.scroll + rows < people.len() {
            let bottom = panel.y + PADDING + rows as f32 * ROW_HEIGHT;
            draw_triangle(vec2(panel.right() - 18.0, bottom - 3.0), vec2(panel.right() - 10.0, bottom - 3.0), vec2(panel.right() - 14.0, bottom + 2.0), style::TEXT_MUTED);
        }

        // A line, and the row for introducing somebody.
        let divider = panel.y + PADDING + rows.max(1) as f32 * ROW_HEIGHT + 4.0;
        draw_line(panel.x + 12.0, divider, panel.right() - 12.0, divider, 1.0, style::BORDER);
        let introduce = Rect::new(panel.x + PADDING, divider + 4.0, panel.w - PADDING * 2.0, ROW_HEIGHT);
        let over_introduce = introduce.contains(mouse);
        if over_introduce {
            style::rounded_rect(introduce, 6.0, style::HOVER);
        }
        style.text_mid("Introduce a person…", introduce.x + 12.0, introduce.center().y, 13, Weight::Regular, if over_introduce { style::TEXT } else { style::TEXT_MUTED });
        if clicked && over_introduce {
            picked = Some(PeoplePick::Introduce);
        }

        // A pick closes it, and so does a click anywhere but on it or on the place in the header that opens it (which toggles it).
        if picked.is_some() || (clicked && !panel.contains(mouse) && !anchor.contains(mouse)) {
            self.open = false;
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    /// Aldoria, which is played and has Mara Voss, and Brevon, which has nobody.
    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day
    }

    #[test]
    fn only_the_people_of_the_country_played_are_listed_and_shown_as_picked() {
        let mut day = day();
        let mara = day.people[0].id;
        assert_eq!(people_of(&day).map(|p| p.id).collect::<Vec<_>>(), vec![mara]);
        assert_eq!(label(&day, None), "Select a person");
        assert_eq!(label(&day, Some(mara)), "Mara Voss");
        assert_eq!(shown(&day, Some(PersonId::new())), None);
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        assert_eq!(people_of(&day).count(), 0);
        assert_eq!(shown(&day, Some(mara)), None, "they belong to another country");
        assert_eq!(label(&day, Some(mara)), "No people");
    }

    #[test]
    fn somebody_introduced_belongs_to_the_country_played_and_is_listed_for_it_only() {
        let mut day = day();
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        day.found_person("Ilse Brandt", "General", "Military").unwrap();
        assert_eq!(people_of(&day).map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Ilse Brandt"]);
        assert_eq!(label(&day, None), "Select a person");
        let aldoria = day.countries[0].id;
        day.select_country(aldoria).unwrap();
        assert_eq!(people_of(&day).map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Mara Voss"]);
    }

    #[test]
    fn playing_another_country_is_noticed_once_and_closes_it() {
        let mut day = day();
        let mut dropdown = PeopleDropdown::default();
        assert!(!dropdown.country_changed(&day), "the first look");
        dropdown.toggle();
        assert!(!dropdown.country_changed(&day));
        assert!(dropdown.is_open());
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        assert!(dropdown.country_changed(&day));
        assert!(!dropdown.is_open());
        assert!(!dropdown.country_changed(&day));
    }

    #[test]
    fn it_is_only_over_the_mouse_while_open() {
        let mut dropdown = PeopleDropdown { rect: Rect::new(0.0, 0.0, 100.0, 100.0), ..PeopleDropdown::default() };
        assert!(!dropdown.has_mouse(vec2(50.0, 50.0)), "closed");
        dropdown.open = true;
        assert!(dropdown.has_mouse(vec2(50.0, 50.0)));
        dropdown.close();
        assert!(!dropdown.is_open());
    }
}
