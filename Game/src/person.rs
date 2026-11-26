//! People. Each belongs to a country, or to none, as a nomad, and stands on the map where the player puts them. They have the movement points the army has, which they get back each day
//! (see [`crate::movement`]), and walk where a militia can.

use macroquad::math::{vec2, Vec2};
use nworld_map::regions::MOUNTAIN_HEIGHT;
use nworld_map::world::{WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::calendar::DAYS_PER_YEAR;
use crate::ids::{CountryId, PersonId, TitleTypeId};
use crate::movement::DAILY_POINTS;
use crate::text;

/// What a person who belongs to no country is called, where a country would be named.
pub const NOMAD: &str = "Nomad";

/// The youngest a person can die of age, in years. Nobody dies before it.
pub const FIRST_RISK_AGE: u32 = 25;
/// The chance of dying in a year of age: this much for each year from the one before [`FIRST_RISK_AGE`]...
const CHANCE_PER_YEAR: f32 = 0.0022;
/// ...and, past this age, this much more times the square of the years past it.
const OLD_AGE: u32 = 60;
const OLD_AGE_CHANCE: f32 = 0.002;
/// How old a person saved before people had ages is taken to be, in years.
const UNKNOWN_AGE: u32 = FIRST_RISK_AGE;

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
    /// The id of the country they belong to, or `None` for a nomad, who belongs to none. It can be changed, and the people of a
    /// country that is deleted become nomads.
    #[serde(default)]
    pub country: Option<CountryId>,
    /// Where they stand on the map, in world x and z. The player puts a new person on the map, and they have no place until then.
    /// (Nor has one saved before people were on the map.)
    #[serde(default)]
    pub at: Option<(f32, f32)>,
    /// The movement points they have left today, which they get all of back each day, like a militia does.
    #[serde(default = "full_points")]
    pub points: u32,
    /// How old they are, in days of the game. A new person is born that day, so it starts at nothing, and each day adds one.
    #[serde(default = "unknown_age")]
    pub age_days: u32,
}

fn full_points() -> u32 {
    DAILY_POINTS
}

fn unknown_age() -> u32 {
    UNKNOWN_AGE * DAYS_PER_YEAR
}

/// The chance that somebody `age` years old dies within that year of their life, from 0 to 1. It is nothing before
/// [`FIRST_RISK_AGE`], grows by the same amount each year from then, and past 60 much faster. They live to 50 on average.
pub fn yearly_death_chance(age: u32) -> f32 {
    if age < FIRST_RISK_AGE {
        return 0.0;
    }
    let past_old = age.saturating_sub(OLD_AGE) as f32;
    (CHANCE_PER_YEAR * (age + 1 - FIRST_RISK_AGE) as f32 + OLD_AGE_CHANCE * past_old * past_old).min(1.0)
}

/// The chance that somebody `age_days` old dies on the day they reach that age: what makes the year's chance of
/// [`yearly_death_chance`] over the days of a year.
pub fn daily_death_chance(age_days: u32) -> f32 {
    1.0 - (1.0 - yearly_death_chance(age_days / DAYS_PER_YEAR)).powf(1.0 / DAYS_PER_YEAR as f32)
}

impl Person {
    /// How old they are, as it is said: "1 year", "30 years".
    pub fn age_words(&self) -> String {
        let age = self.age();
        format!("{age} {}", if age == 1 { "year" } else { "years" })
    }

    /// How old they are in whole years.
    pub fn age(&self) -> u32 {
        self.age_days / DAYS_PER_YEAR
    }

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
    fn nobody_dies_before_25_and_the_chance_grows_each_year_and_much_faster_past_60() {
        assert!((0..25).all(|age| yearly_death_chance(age) == 0.0));
        assert!(yearly_death_chance(25) > 0.0);
        assert!((25..70).all(|age| yearly_death_chance(age + 1) > yearly_death_chance(age)));
        let step = |age| yearly_death_chance(age + 1) - yearly_death_chance(age);
        assert!((step(30) - step(50)).abs() < 1e-6, "the same each year to 60");
        assert!(step(65) > step(50) * 5.0, "and then it ramps up");
        assert_eq!(yearly_death_chance(150), 1.0);
        // A day's chance adds up, over a year of days, to the year's.
        let survive_a_year = (1.0 - daily_death_chance(40 * DAYS_PER_YEAR)).powi(DAYS_PER_YEAR as i32);
        assert!((1.0 - survive_a_year - yearly_death_chance(40)).abs() < 1e-4);
    }

    #[test]
    fn on_average_people_live_to_about_50() {
        let (mut alive, mut years) = (1.0_f64, 0.0_f64);
        for day in 0..200 * DAYS_PER_YEAR {
            years += alive / DAYS_PER_YEAR as f64;
            alive *= 1.0 - daily_death_chance(day) as f64;
        }
        assert!((49.0..51.0).contains(&years), "{years}");
        assert!(alive < 1e-6);
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
