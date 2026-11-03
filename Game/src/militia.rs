//! Militia, the first kind of unit. A city can raise one group of militia a day, made of some of its people: no more than
//! a tenth of its population, and no more than 1,000, which is the most a group can have. They leave the city's
//! population, and each costs 10 coins to raise and 1 coin a day to keep. A member who isn't paid for deserts, and
//! doesn't go back to a city.
//!
//! A militia belongs to a country, which can change. It stands in a camp on the ground just outside the city it was
//! raised in, and for now it stays there.

use macroquad::math::{vec2, Vec2};
use nworld_map::world::World;
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::city::{self, City};
use crate::city_layout;
use crate::ids::{CityId, CountryId, MilitiaId};
use crate::text;

/// The most members a militia can have.
pub const MAX_MEMBERS: u32 = 1_000;
/// The most of a city's people one militia can be made of, as a percent of its population.
pub const CITY_SHARE_PERCENT: u64 = 10;
/// The longest a militia's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// What each member costs to raise, in coins.
pub const RAISING_COST: i64 = 10;
/// What each member costs to keep each day, in coins.
pub const DAILY_UPKEEP: i64 = 1;

/// How far outside the edge of a city its militia camp, in meters: the nearest, and the rings beyond that to try when
/// the ground there is wet or high, or the places are taken.
const CAMP_RINGS: [f32; 3] = [12.0, 28.0, 50.0];
/// How many places round each ring are tried, and how close two camps may stand, in meters.
const CAMP_TRIES: usize = 16;
const CAMP_SPACING: f32 = 7.0;
/// A turn of this many radians takes the next camp of a city well away from the last, however many there are.
const GOLDEN_ANGLE: f32 = 2.399_963;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Militia {
    /// Tells it apart from the other militia, and stays with it whatever else changes.
    pub id: MilitiaId,
    /// What it is called, which can be changed. It starts as the name of its city and the day it was raised.
    #[serde(default)]
    pub name: String,
    /// The country it belongs to, which pays to keep it.
    pub country: CountryId,
    /// The city it was raised in.
    pub city: CityId,
    /// The day it was raised, which is how a city knows it has raised one today.
    pub raised: Date,
    /// How many people are in it.
    pub members: u32,
    /// Where its camp stands on the map, in meters.
    pub x: f32,
    pub z: f32,
}

/// A militia name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your militia")
}

/// What a militia raised in the city called `city` on `date` is called until it is renamed: "Port Aldor Militia 1-3-14".
pub fn default_name(city: &str, date: Date) -> String {
    format!("{city} Militia {date}")
}

impl Militia {
    /// The name to show for it: its own, or just "Militia" for one saved without.
    pub fn title(&self) -> &str {
        if self.name.trim().is_empty() { "Militia" } else { &self.name }
    }
}

/// The most members a militia raised in a city of `population` people can have: a tenth of them, or 1,000 if that is
/// fewer.
pub fn most_from(population: u32) -> u32 {
    let share = u64::from(population) * CITY_SHARE_PERCENT / 100;
    share.min(u64::from(MAX_MEMBERS)) as u32
}

/// What `members` members cost to raise, in coins.
pub fn raising_cost(members: u32) -> i64 {
    i64::from(members) * RAISING_COST
}

/// What `members` members cost to keep for a day, in coins.
pub fn upkeep(members: u32) -> i64 {
    i64::from(members) * DAILY_UPKEEP
}

/// Where a new militia of `city` can camp, outside the city, on dry land that isn't up in the mountains and not on top
/// of the camps in `taken` (the camps already round it). `None` if there is no such ground near it. Each camp of a
/// city goes to a different side of it.
pub fn camp_near(world: &World, city: &City, taken: &[Vec2]) -> Option<Vec2> {
    let middle = vec2(city.x, city.z);
    let first = city.id.seed() as f32 / u32::MAX as f32 * std::f32::consts::TAU;
    let start = first + GOLDEN_ANGLE * taken.len() as f32;
    for ring in CAMP_RINGS {
        let distance = city_layout::radius(city) + ring;
        for k in 0..CAMP_TRIES {
            let angle = start + std::f32::consts::TAU * k as f32 / CAMP_TRIES as f32;
            let at = middle + vec2(angle.cos(), angle.sin()) * distance;
            if city::site_problem(world, at.x, at.y).is_none() && taken.iter().all(|other| other.distance(at) >= CAMP_SPACING) {
                return Some(at);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn a_militia_is_a_tenth_of_a_city_and_no_more_than_a_thousand() {
        assert_eq!(most_from(100), 10, "the smallest city");
        assert_eq!(most_from(1_234), 123, "rounded down");
        assert_eq!(most_from(9), 0);
        assert_eq!(most_from(9_999), 999);
        assert_eq!(most_from(10_000), 1_000);
        assert_eq!(most_from(100_000), 1_000, "10,000 would be a tenth, but 1,000 is the most");
        assert_eq!(most_from(u32::MAX), 1_000);
    }

    #[test]
    fn a_militia_starts_named_for_its_city_and_the_day_and_a_name_is_tidied() {
        let date = Date { year: 1, month: 3, day: 14 };
        assert_eq!(default_name("Port Aldor", date), "Port Aldor Militia 1-3-14");
        assert!(clean_name(&default_name(&"a".repeat(crate::city::MAX_NAME_CHARS), Date { year: 99, month: 10, day: 20 })).is_ok(), "the longest default fits");
        assert_eq!(clean_name("  Port   Guard ").unwrap(), "Port Guard");
        assert_eq!(clean_name(" ").unwrap_err(), "Enter a name for your militia.");
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn raising_costs_10_a_member_and_keeping_1_a_day() {
        assert_eq!((raising_cost(0), raising_cost(1), raising_cost(120)), (0, 10, 1_200));
        assert_eq!((upkeep(0), upkeep(1), upkeep(120)), (0, 1, 120));
        assert_eq!(raising_cost(MAX_MEMBERS), 10_000);
    }

    fn city_at(at: Vec2, population: u32) -> City {
        City { id: CityId::new(), country: CountryId::new(), name: "Port Aldor".to_owned(), x: at.x, z: at.y, region: None, population, under_siege: false }
    }

    #[test]
    fn a_camp_stands_outside_the_city_on_dry_land() {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 150.0);
        for population in [100, 5_000, 100_000] {
            let city = city_at(at, population);
            let camp = camp_near(&world, &city, &[]).expect("room for a camp");
            assert!(camp.distance(at) > city_layout::radius(&city), "{population}: outside the city");
            assert!(city::site_problem(&world, camp.x, camp.y).is_none(), "{population}: on dry land");
        }
    }

    #[test]
    fn the_camps_of_one_city_go_to_different_places() {
        let world = test_support::small_continent();
        let city = city_at(test_support::inland_spot(&world, 150.0), 2_000);
        let mut camps: Vec<Vec2> = Vec::new();
        for _ in 0..6 {
            let camp = camp_near(&world, &city, &camps).unwrap();
            assert!(camps.iter().all(|other| other.distance(camp) >= CAMP_SPACING), "{camp} is on another camp");
            camps.push(camp);
        }
    }

    #[test]
    fn a_camp_is_always_the_same_for_the_same_city() {
        let world = test_support::small_continent();
        let city = city_at(test_support::inland_spot(&world, 150.0), 2_000);
        assert_eq!(camp_near(&world, &city, &[]), camp_near(&world, &city, &[]));
    }

    #[test]
    fn a_city_at_sea_has_no_ground_for_a_camp() {
        let world = test_support::small_continent();
        let (_, sea) = test_support::land_and_sea(&world);
        // Far from any land in every direction, if the sea point is. Skip the test's claim if it isn't.
        let city = city_at(sea, 100);
        if let Some(camp) = camp_near(&world, &city, &[]) {
            assert!(city::site_problem(&world, camp.x, camp.y).is_none(), "any camp found is on dry land");
        }
    }
}
