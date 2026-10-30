//! Names for the regions of the map. A region is a patch of the map, land or sea, with a number, and it has no
//! name until the player gives it one. The names are kept in each day's file and carry forward.

use nworld_map::regions::{RegionId, RegionKind};
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::text;

/// The longest a region's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;

/// The name of one region.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegionName {
    /// The number of the region, as the map knows it.
    pub region: u16,
    pub name: String,
}

/// A region name as typed, tidied up. Blank is allowed: it means no name. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    if typed.trim().is_empty() {
        return Ok(String::new());
    }
    text::clean(typed, MAX_NAME_CHARS, "a name for the region")
}

/// What kind of ground a region is, in a few words: "land", "mountain", "shallow water" or "deep water".
pub fn kind_text(region: RegionId) -> &'static str {
    match region.kind() {
        RegionKind::Land => "land",
        RegionKind::Mountain => "mountain",
        RegionKind::Shallow => "shallow water",
        RegionKind::Deep => "deep water",
    }
}

impl DaySave {
    /// The name of the region numbered `region`, if it has one.
    pub fn region_name(&self, region: u16) -> Option<&str> {
        self.region_names.iter().find(|n| n.region == region).map(|n| n.name.as_str())
    }

    /// Gives the region numbered `region` the name `typed`, or takes its name away if that is blank. An error says
    /// why it can't be done.
    pub fn set_region_name(&mut self, region: u16, typed: &str) -> Result<(), String> {
        if RegionId::from_raw(region).is_none() {
            return Err("There is no such region.".to_owned());
        }
        let name = clean_name(typed)?;
        self.region_names.retain(|n| n.region != region);
        if !name.is_empty() {
            self.region_names.push(RegionName { region, name });
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
    fn a_region_has_no_name_to_begin_with() {
        let day = day();
        assert!(day.region_names.is_empty());
        assert_eq!(day.region_name(0xC001), None);
    }

    #[test]
    fn a_region_can_be_named_renamed_and_its_name_taken_away() {
        let mut day = day();
        day.set_region_name(0xC001, "  The   Marches ").unwrap();
        assert_eq!(day.region_name(0xC001), Some("The Marches"), "tidied");
        assert_eq!(day.region_name(0xC002), None, "other regions are not named");
        day.set_region_name(0xC001, "Old Marches").unwrap();
        assert_eq!(day.region_name(0xC001), Some("Old Marches"));
        assert_eq!(day.region_names.len(), 1, "renamed, not named again");
        day.set_region_name(0xC001, "   ").unwrap();
        assert_eq!(day.region_name(0xC001), None, "a blank name takes it away");
        assert!(day.region_names.is_empty());
    }

    #[test]
    fn a_bad_name_or_region_changes_nothing() {
        let mut day = day();
        day.set_region_name(0xC001, "The Marches").unwrap();
        let before = day.clone();
        assert!(day.set_region_name(0xC001, &"a".repeat(MAX_NAME_CHARS + 1)).unwrap_err().contains("too long"));
        assert!(day.set_region_name(0xC001, "Bad!").is_err());
        assert!(day.set_region_name(0, "Nowhere").unwrap_err().contains("no such region"));
        assert!(day.set_region_name(0x4000, "Nowhere").is_err(), "a kind of ground with no number is no region");
        assert_eq!(day, before);
    }

    #[test]
    fn two_regions_can_have_the_same_name() {
        let mut day = day();
        day.set_region_name(0xC001, "The Reach").unwrap();
        day.set_region_name(0xC002, "The Reach").unwrap();
        assert_eq!(day.region_names.len(), 2);
    }

    #[test]
    fn names_are_saved_and_carry_forward_and_a_day_without_any_has_none() {
        let mut today = day();
        today.set_region_name(0xC001, "The Marches").unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["region_names"][0], serde_json::json!({ "region": 0xC001, "name": "The Marches" }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        assert_eq!(today.following(Date { year: 1, month: 1, day: 2 }).region_names, today.region_names);
        let old = serde_json::json!({ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" });
        assert!(serde_json::from_value::<DaySave>(old).unwrap().region_names.is_empty());
    }

    #[test]
    fn a_blank_name_is_allowed_and_the_kind_of_ground_has_words() {
        assert_eq!(clean_name("").unwrap(), "");
        assert_eq!(clean_name("  ").unwrap(), "");
        let kinds: Vec<&str> = [0xC001, 0x8001, 0x4001, 0x0001].iter().map(|raw| kind_text(RegionId::from_raw(*raw).unwrap())).collect();
        assert_eq!(kinds, ["land", "mountain", "shallow water", "deep water"]);
    }
}
