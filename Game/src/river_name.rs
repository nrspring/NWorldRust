//! Names for the rivers of the map. A river here is a patch of inland water that is not a lake (see
//! `nworld_map::water_bodies`), which is how the map tells one from another: it doesn't know which rivers were dug, so
//! a river with branches is one river. A river has no name until the player gives it one. The names are kept in each
//! day's file and carry forward.

use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::newsreel::{Event, Named, News};
use crate::region_name;

/// The name of one river.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RiverName {
    /// Which river: the number of the first sample of its water, as `BodyId` has it.
    pub river: u32,
    pub name: String,
}

impl DaySave {
    /// The name of the river with this number, if it has one.
    pub fn river_name(&self, river: u32) -> Option<&str> {
        self.river_names.iter().find(|n| n.river == river).map(|n| n.name.as_str())
    }

    /// Gives the river with this number the name `typed`, or takes its name away if that is blank. An error says why
    /// it can't be done.
    pub fn set_river_name(&mut self, river: u32, typed: &str) -> Result<(), String> {
        let name = region_name::clean_name(typed)?;
        let from = self.river_name(river).unwrap_or_default().to_owned();
        self.river_names.retain(|n| n.river != river);
        if !name.is_empty() {
            self.river_names.push(RiverName { river, name: name.clone() });
        }
        if from != name {
            self.newsreel.push(News::happening(Event::NameChanged { what: Named::River, id: river, from, to: name }));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn a_river_has_no_name_to_begin_with() {
        let day = day();
        assert!(day.river_names.is_empty());
        assert_eq!(day.river_name(4_321), None);
    }

    #[test]
    fn a_river_can_be_named_renamed_and_its_name_taken_away() {
        let mut day = day();
        day.set_river_name(4_321, "  The   Silver Run ").unwrap();
        assert_eq!(day.river_name(4_321), Some("The Silver Run"), "tidied");
        assert_eq!(day.river_name(99), None, "other rivers are not named");
        day.set_river_name(4_321, "The Brightwater").unwrap();
        assert_eq!(day.river_name(4_321), Some("The Brightwater"));
        assert_eq!(day.river_names.len(), 1, "renamed, not named again");
        day.set_river_name(4_321, "  ").unwrap();
        assert_eq!(day.river_name(4_321), None);
        assert!(day.river_names.is_empty());
    }

    #[test]
    fn a_bad_name_changes_nothing_and_two_rivers_may_share_one() {
        let mut day = day();
        day.set_river_name(1, "The Run").unwrap();
        let before = day.clone();
        assert!(day.set_river_name(1, &"a".repeat(region_name::MAX_NAME_CHARS + 1)).unwrap_err().contains("too long"));
        assert!(day.set_river_name(1, "Bad!").is_err());
        assert_eq!(day, before);
        day.set_river_name(2, "The Run").unwrap();
        assert_eq!(day.river_names.len(), 2);
    }

    #[test]
    fn names_are_saved_and_carry_forward_and_a_day_without_any_has_none() {
        let mut today = day();
        today.set_river_name(4_321, "The Silver Run").unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["river_names"][0], serde_json::json!({ "river": 4_321, "name": "The Silver Run" }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        assert_eq!(today.following(Date { year: 1, month: 1, day: 2 }).river_names, today.river_names);
        let old = serde_json::json!({ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" });
        assert!(serde_json::from_value::<DaySave>(old).unwrap().river_names.is_empty());
    }
}
