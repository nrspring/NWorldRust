//! Names for the lakes of the map. A lake here is a body of inland water that is wide in every direction (see
//! `nworld_map::water_bodies`), which is how the map tells a lake from a river. A lake has no name until the player
//! gives it one. The names are kept in each day's file and carry forward.

use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::newsreel::{Event, Named, News};
use crate::region_name;

/// The name of one lake.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LakeName {
    /// Which lake: the number of the first sample of its water, as `BodyId` has it.
    pub lake: u32,
    pub name: String,
}

impl DaySave {
    /// The name of the lake with this number, if it has one.
    pub fn lake_name(&self, lake: u32) -> Option<&str> {
        self.lake_names.iter().find(|n| n.lake == lake).map(|n| n.name.as_str())
    }

    /// Gives the lake with this number the name `typed`, or takes its name away if that is blank. An error says why
    /// it can't be done.
    pub fn set_lake_name(&mut self, lake: u32, typed: &str) -> Result<(), String> {
        let name = region_name::clean_name(typed)?;
        let from = self.lake_name(lake).unwrap_or_default().to_owned();
        self.lake_names.retain(|n| n.lake != lake);
        if !name.is_empty() {
            self.lake_names.push(LakeName { lake, name: name.clone() });
        }
        if from != name {
            self.newsreel.push(News::happening(Event::NameChanged { what: Named::Lake, id: lake, from, to: name }));
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
    fn a_lake_has_no_name_to_begin_with() {
        let day = day();
        assert!(day.lake_names.is_empty());
        assert_eq!(day.lake_name(4_321), None);
    }

    #[test]
    fn a_lake_can_be_named_renamed_and_its_name_taken_away() {
        let mut day = day();
        day.set_lake_name(4_321, "  The   Silver Run ").unwrap();
        assert_eq!(day.lake_name(4_321), Some("The Silver Run"), "tidied");
        assert_eq!(day.lake_name(99), None, "other lakes are not named");
        day.set_lake_name(4_321, "The Brightwater").unwrap();
        assert_eq!(day.lake_name(4_321), Some("The Brightwater"));
        assert_eq!(day.lake_names.len(), 1, "renamed, not named again");
        day.set_lake_name(4_321, "  ").unwrap();
        assert_eq!(day.lake_name(4_321), None);
        assert!(day.lake_names.is_empty());
    }

    #[test]
    fn a_bad_name_changes_nothing_and_two_lakes_may_share_one() {
        let mut day = day();
        day.set_lake_name(1, "The Run").unwrap();
        let before = day.clone();
        assert!(day.set_lake_name(1, &"a".repeat(region_name::MAX_NAME_CHARS + 1)).unwrap_err().contains("too long"));
        assert!(day.set_lake_name(1, "Bad!").is_err());
        assert_eq!(day, before);
        day.set_lake_name(2, "The Run").unwrap();
        assert_eq!(day.lake_names.len(), 2);
    }

    #[test]
    fn names_are_saved_and_carry_forward_and_a_day_without_any_has_none() {
        let mut today = day();
        today.set_lake_name(4_321, "The Silver Run").unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["lake_names"][0], serde_json::json!({ "lake": 4_321, "name": "The Silver Run" }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        assert_eq!(today.following(Date { year: 1, month: 1, day: 2 }).lake_names, today.lake_names);
        let old = serde_json::json!({ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" });
        assert!(serde_json::from_value::<DaySave>(old).unwrap().lake_names.is_empty());
    }
}
