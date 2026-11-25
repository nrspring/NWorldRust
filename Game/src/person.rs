//! People. Each belongs to a country, and stands on the map where the player puts them. They have the movement points the army has, which they get back each day
//! (see [`crate::movement`]), and walk where a militia can.

use macroquad::math::{vec2, Vec2};
use nworld_map::regions::MOUNTAIN_HEIGHT;
use nworld_map::world::{WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::ids::{CountryId, PersonId, TitleTypeId};
use crate::movement::DAILY_POINTS;
use crate::text;

/// The longest a person's name or title can be, in characters.
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_TITLE_CHARS: usize = 32;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    /// What they are called, like "President" or "General".
    pub title: String,
    /// The id of the kind of title it is, like "Head of State" or "Military".
    pub title_type: TitleTypeId,
    /// The id of the country they belong to.
    pub country: CountryId,
    /// Where they stand on the map, in world x and z. The player puts a new person on the map, and they have no place until then.
    /// (Nor has one saved before people were on the map.)
    #[serde(default)]
    pub at: Option<(f32, f32)>,
    /// The movement points they have left today, which they get all of back each day, like a militia does.
    #[serde(default = "full_points")]
    pub points: u32,
}

fn full_points() -> u32 {
    DAILY_POINTS
}

impl Person {
    /// Where they stand, if they have a place on the map.
    pub fn position(&self) -> Option<Vec2> {
        self.at.map(|(x, z)| vec2(x, z))
    }
}

/// Why the ground at (`x`, `z`) is no place to put a person, or `None` if it is: they go on dry land, below the high mountains.
pub fn ground_problem(world: &World, x: f32, z: f32) -> Option<&'static str> {
    let half = world.half_extents();
    if x.abs() > half.x || z.abs() > half.y {
        Some("That is off the map.")
    } else if world.water_class(x, z) != WaterClass::Dry {
        Some("People can only be placed on dry land.")
    } else if world.surface_height(x, z) >= MOUNTAIN_HEIGHT {
        Some("That is too high in the mountains to place them.")
    } else {
        None
    }
}

/// A person's name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "the person's name")
}

/// A person's title as typed, tidied up. An error says what is wrong with it.
pub fn clean_title(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_TITLE_CHARS, "the person's title")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_can_be_put_on_dry_land_on_the_map_and_not_off_it() {
        let world = crate::test_support::small_continent();
        let at = crate::test_support::inland_spot(&world, 40.0);
        assert_eq!(ground_problem(&world, at.x, at.y), None);
        assert_eq!(ground_problem(&world, world.half_extents().x + 10.0, 0.0), Some("That is off the map."));
    }

    #[test]
    fn names_and_titles_are_tidied() {
        assert_eq!(clean_name(" Mara  Voss ").unwrap(), "Mara Voss");
        assert_eq!(clean_title(" Prime  Minister ").unwrap(), "Prime Minister");
    }

    #[test]
    fn a_missing_name_or_title_says_which() {
        assert_eq!(clean_name("").unwrap_err(), "Enter the person's name.");
        assert_eq!(clean_title("").unwrap_err(), "Enter the person's title.");
    }

    #[test]
    fn they_have_length_limits() {
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(clean_title(&"a".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }
}
