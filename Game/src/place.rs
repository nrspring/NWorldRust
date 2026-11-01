//! Places: labels the player puts on the map at a point of their own choosing, for things the map doesn't name on
//! its own, such as a strait, a plain, a bay or a range. A place is only a name at a point; it has no effect on
//! anything else. Places are kept in each day's file and carry forward.

use macroquad::prelude::Vec2;
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::newsreel::{Event, Named, News};
use crate::text;

/// The longest a place's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 40;

/// One named place.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Place {
    /// Tells it from the other places. Not shown.
    pub id: u32,
    /// Where it is on the map, in world x and z.
    pub x: f32,
    pub z: f32,
    pub name: String,
}

/// A place's name as typed, tidied up. Blank is allowed: it means no name. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    if typed.trim().is_empty() {
        return Ok(String::new());
    }
    text::clean(typed, MAX_NAME_CHARS, "a name for the place")
}

impl DaySave {
    /// The place numbered `id`, if there is one.
    pub fn place(&self, id: u32) -> Option<&Place> {
        self.places.iter().find(|p| p.id == id)
    }

    /// Puts a place called `typed` at `at` (world x and z) and returns its number. An error says why it can't be done.
    pub fn add_place(&mut self, at: Vec2, typed: &str) -> Result<u32, String> {
        let name = text::clean(typed, MAX_NAME_CHARS, "a name for the place")?;
        let id = self.places.iter().map(|p| p.id + 1).max().unwrap_or(1);
        self.places.push(Place { id, x: at.x, z: at.y, name: name.clone() });
        self.newsreel.push(News::happening(Event::NameChanged { what: Named::Place, id, from: String::new(), to: name }));
        Ok(id)
    }

    /// Renames the place numbered `id` to `typed`, or removes it if that is blank. An error says why it can't be done.
    pub fn set_place_name(&mut self, id: u32, typed: &str) -> Result<(), String> {
        let from = self.place(id).ok_or("There is no such place.")?.name.clone();
        let name = clean_name(typed)?;
        if name.is_empty() {
            self.places.retain(|p| p.id != id);
        } else if let Some(place) = self.places.iter_mut().find(|p| p.id == id) {
            place.name = name.clone();
        }
        if from != name {
            self.newsreel.push(News::happening(Event::NameChanged { what: Named::Place, id, from, to: name }));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use macroquad::prelude::vec2;

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn there_are_no_places_to_begin_with() {
        let day = day();
        assert!(day.places.is_empty());
        assert_eq!(day.place(1), None);
    }

    #[test]
    fn a_place_is_a_tidied_name_at_a_point_and_needs_a_name() {
        let mut day = day();
        let id = day.add_place(vec2(120.5, -40.0), "  The   Gallus Strait ").unwrap();
        let place = day.place(id).unwrap();
        assert_eq!((place.name.as_str(), place.x, place.z), ("The Gallus Strait", 120.5, -40.0));
        assert!(day.add_place(vec2(0.0, 0.0), "   ").unwrap_err().contains("name"));
        assert!(day.add_place(vec2(0.0, 0.0), "Bad!").is_err());
        assert!(day.add_place(vec2(0.0, 0.0), &"a".repeat(MAX_NAME_CHARS + 1)).unwrap_err().contains("too long"));
        assert_eq!(day.places.len(), 1, "the refused ones were not added");
    }

    #[test]
    fn places_get_their_own_numbers_even_at_the_same_point_and_after_one_is_removed() {
        let mut day = day();
        let a = day.add_place(vec2(1.0, 1.0), "Great Plain").unwrap();
        let b = day.add_place(vec2(1.0, 1.0), "Great Plain").unwrap();
        assert_ne!(a, b);
        day.set_place_name(a, "").unwrap();
        let c = day.add_place(vec2(5.0, 5.0), "The Narrows").unwrap();
        assert_ne!(b, c);
        assert_eq!(day.places.len(), 2);
    }

    #[test]
    fn a_place_can_be_renamed_and_a_blank_name_removes_it() {
        let mut day = day();
        let id = day.add_place(vec2(10.0, 20.0), "Great Plain").unwrap();
        day.set_place_name(id, "The Wide Plain").unwrap();
        assert_eq!(day.place(id).unwrap().name, "The Wide Plain");
        assert_eq!((day.place(id).unwrap().x, day.place(id).unwrap().z), (10.0, 20.0), "it stays where it is");
        let before = day.clone();
        assert!(day.set_place_name(id, "Bad!").is_err());
        assert!(day.set_place_name(99, "Nowhere").is_err());
        assert_eq!(day, before, "a refused change changes nothing");
        day.set_place_name(id, "  ").unwrap();
        assert_eq!(day.place(id), None);
    }

    #[test]
    fn places_are_saved_and_carry_forward_and_a_day_without_any_has_none() {
        let mut today = day();
        let id = today.add_place(vec2(10.0, 20.0), "Great Plain").unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["places"][0], serde_json::json!({ "id": id, "x": 10.0, "z": 20.0, "name": "Great Plain" }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        assert_eq!(today.following(Date { year: 1, month: 1, day: 2 }).places, today.places);
        let old = serde_json::json!({ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" });
        assert!(serde_json::from_value::<DaySave>(old).unwrap().places.is_empty());
    }
}
