//! Militia, the first kind of unit. A city can raise one group of militia a day, made of some of its people: no more than
//! a tenth of its population, and no more than 1,000, which is the most a group can have. They leave the city's
//! population, and each costs 10 coins to raise and 1 coin a day to keep. A member who isn't paid for deserts, and
//! doesn't go back to a city.
//!
//! A militia belongs to a country, which can change. It starts as a pawn standing on the ground just outside the city it
//! was raised in, and can be moved from there (see [`crate::movement`]).


use macroquad::math::{vec2, Vec2};
use nworld_map::world::World;
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::city::{self, City};
use crate::city_layout;
use crate::ids::{CityId, CountryId, FortId, MilitiaId};
use crate::movement::DAILY_POINTS;
use crate::text;

/// The most members a militia can have.
pub const MAX_MEMBERS: u32 = 1_000;
/// The most of a city's people one militia can be made of, as a percent of its population.
pub const CITY_SHARE_PERCENT: u64 = 10;
/// The longest a militia's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// What each member costs to raise, in coins.
pub const RAISING_COST: i64 = 10;
/// What each member costs to keep each day, in coins, and what each soldier (a member who has been trained at a barracks, see
/// [`crate::barracks`]) costs: three times as much, and an archer (trained at an archery range) four times.
pub const DAILY_UPKEEP: i64 = 1;
pub const SOLDIER_UPKEEP: i64 = 3;
pub const ARCHER_UPKEEP: i64 = 4;
/// What each member of cavalry (trained at a stable) costs to keep each day: they and their horses eat.
pub const CAVALRY_UPKEEP: i64 = 6;
/// How many times the movement points of a militia cavalry get each day.
pub const CAVALRY_MOVE_FACTOR: u32 = 2;

/// How far outside the edge of a city its militia camp, in meters: the nearest, and the rings beyond that to try when
/// the ground there is wet or high, or the places are taken.
const CAMP_RINGS: [f32; 3] = [20.0, 42.0, 70.0];
/// How many places round each ring are tried, and how close two camps may stand, in meters.
const CAMP_TRIES: usize = 16;
const CAMP_SPACING: f32 = 22.0;
/// How far from a militia the group split off it can stand, in meters: the nearest, and the rings beyond that to try.
const SPLIT_RINGS: [f32; 3] = [24.0, 40.0, 64.0];
/// A turn of this many radians takes the next camp of a city well away from the last, however many there are.
const GOLDEN_ANGLE: f32 = 2.399_963;

/// What a militia is: plain militia, or, once trained, soldiers (at a barracks), archers (at an archery range) or cavalry (at a stable).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Militia,
    Soldiers,
    Archers,
    Cavalry,
}

impl Class {
    /// What it is called, to begin a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Militia => "Militia",
            Self::Soldiers => "Soldiers",
            Self::Archers => "Archers",
            Self::Cavalry => "Cavalry",
        }
    }

    /// What to call the people in it, to begin a sentence.
    pub fn people_label(self) -> &'static str {
        match self {
            Self::Militia => "Members",
            Self::Soldiers => "Soldiers",
            Self::Archers => "Archers",
            Self::Cavalry => "Riders",
        }
    }

    /// What each of its members costs to keep each day, in coins.
    pub fn upkeep_per_member(self) -> i64 {
        match self {
            Self::Militia => DAILY_UPKEEP,
            Self::Soldiers => SOLDIER_UPKEEP,
            Self::Archers => ARCHER_UPKEEP,
            Self::Cavalry => CAVALRY_UPKEEP,
        }
    }

    /// The movement points it gets each day: cavalry get twice as many as the rest.
    pub fn daily_points(self) -> u32 {
        match self {
            Self::Cavalry => DAILY_POINTS * CAVALRY_MOVE_FACTOR,
            _ => DAILY_POINTS,
        }
    }
}

/// That a militia is in a fort's garrison (see [`crate::fort`]).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct InFort {
    pub fort: FortId,
    /// Where it stood before it went in, which is where it comes out to.
    pub from_x: f32,
    pub from_z: f32,
}

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
    /// Where it stands on the map, in meters: in a camp outside its city when it is raised, until it moves.
    pub x: f32,
    pub z: f32,
    /// The movement points it has left today, which it gets all of back each day (see [`crate::movement`]). A militia
    /// saved before it could move has all of them.
    #[serde(default = "full_points")]
    pub points: u32,
    /// Whether it is surrounded. The player switches it on and off, as for a city. A militia under siege loses members each day
    /// (see [`after_siege`]).
    #[serde(default)]
    pub under_siege: bool,
    /// Whether it has been trained at a barracks or an archery range, and so is soldiers or archers and not militia. Every member of
    /// it is trained, and each costs more to keep. A trained group can only be combined with another of the same kind. (One saved
    /// before there were barracks is not.)
    #[serde(default)]
    pub trained: bool,
    /// Whether it was trained at an archery range, and so is archers and not soldiers. Only a trained group can be this. (One saved
    /// before there were archers is not.)
    #[serde(default)]
    pub archers: bool,
    /// Whether it was trained at a stable, and so is cavalry. Only a trained group can be this. (One saved before there was cavalry is
    /// not.)
    #[serde(default)]
    pub cavalry: bool,
    /// Whether the player picked it to be paid for, in the list that comes up when its country can't afford all its upkeep (see
    /// [`crate::upkeep`]). The country pays for the units picked before the others, so when there is not enough money it is the others
    /// that go unpaid.
    #[serde(default)]
    pub pay_first: bool,
    /// Whether it is a garrison: it stays where it is, and so gets no movement points back each day. The player switches it on and off.
    #[serde(default)]
    pub garrison: bool,
    /// The fort it is in the garrison of, if it is in one. It is then where the fort is, isn't shown on the map for itself, and can't
    /// move until it has marched out.
    #[serde(default)]
    pub in_fort: Option<InFort>,
}

fn full_points() -> u32 {
    DAILY_POINTS
}

/// A militia name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your militia")
}

/// What a militia with this id that was raised in the city called `city` (or split off one that was) is called until it
/// is renamed: the city and the first part of its id, like "Port Aldor 3f9a2b1c".
pub fn default_name(city: &str, id: MilitiaId) -> String {
    format!("{city} {}", id.short())
}

/// Where a group split off the militia standing at `from` can stand: a little way off, on dry land in the same map
/// region (`region` says which region a place is in), and not on top of the militia at `taken`. `None` if there is
/// no such ground near it.
pub fn split_spot(world: &World, from: Vec2, taken: &[Vec2], region: &dyn Fn(Vec2) -> Option<u16>) -> Option<Vec2> {
    let here = region(from);
    for distance in SPLIT_RINGS {
        for k in 0..CAMP_TRIES {
            let angle = std::f32::consts::TAU * k as f32 / CAMP_TRIES as f32;
            let at = from + vec2(angle.cos(), angle.sin()) * distance;
            if city::site_problem(world, at.x, at.y).is_none() && region(at) == here && taken.iter().all(|other| other.distance(at) >= CAMP_SPACING) {
                return Some(at);
            }
        }
    }
    None
}

impl Militia {
    /// The name to show for it: its own, or just "Militia" for one saved without.
    pub fn title(&self) -> &str {
        if self.name.trim().is_empty() { self.kind_label() } else { &self.name }
    }

    /// What it is: plain militia, or soldiers, archers or cavalry once trained.
    pub fn class(&self) -> Class {
        match (self.trained, self.cavalry, self.archers) {
            (false, _, _) => Class::Militia,
            (true, true, _) => Class::Cavalry,
            (true, false, true) => Class::Archers,
            (true, false, false) => Class::Soldiers,
        }
    }

    /// Makes it of `class`: trained, as soldiers, archers or cavalry, or not trained at all.
    pub fn set_class(&mut self, class: Class) {
        self.trained = class != Class::Militia;
        self.archers = class == Class::Archers;
        self.cavalry = class == Class::Cavalry;
    }

    /// What it is, to begin a sentence: "Militia", "Soldiers", "Archers" or "Cavalry".
    pub fn kind_label(&self) -> &'static str {
        self.class().label()
    }

    /// What to call the people in it, to begin a sentence: "Members", "Soldiers", "Archers" or "Riders".
    pub fn people_label(&self) -> &'static str {
        self.class().people_label()
    }

    /// What each of its members costs to keep each day, in coins.
    pub fn upkeep_per_member(&self) -> i64 {
        self.class().upkeep_per_member()
    }

    /// The movement points it gets each day.
    pub fn daily_points(&self) -> u32 {
        self.class().daily_points()
    }

    /// What it costs to keep for a day, in coins.
    pub fn upkeep(&self) -> i64 {
        i64::from(self.members) * self.upkeep_per_member()
    }
}

/// The most members a militia raised in a city of `population` people can have: a tenth of them, or 1,000 if that is
/// fewer.
pub fn most_from(population: u32) -> u32 {
    let share = u64::from(population) * CITY_SHARE_PERCENT / 100;
    share.min(u64::from(MAX_MEMBERS)) as u32
}

/// How many members a militia of `members` has the next day if it is under siege: it loses the same share a besieged city does (5
/// to 10 percent, `unit` being how far through that range, from 0 up to 1), rounded up, so that even a small militia loses at least
/// one member a day. It is never left with fewer than 1, as a city is never left with fewer than 100.
pub fn after_siege(members: u32, unit: f32) -> u32 {
    let (least, most) = crate::rules::SIEGE_LOSS_PERCENT;
    let percent = least + (most - least) * f64::from(unit.clamp(0.0, 1.0));
    let lost = (f64::from(members) * percent / 100.0).ceil() as u32;
    members.saturating_sub(lost).max(1).min(members)
}

/// What `members` members cost to raise, in coins.
pub fn raising_cost(members: u32) -> i64 {
    i64::from(members) * RAISING_COST
}

/// What `members` members cost to keep for a day, in coins, if they are not trained.
#[cfg(test)]
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
    fn a_militia_starts_named_for_its_city_and_its_id_and_a_name_is_tidied() {
        let id = MilitiaId::from_u128(0x3f9a2b1c_5a47_4d0e_9b6a_7c1d2e8f4a90);
        assert_eq!(id.short(), "3f9a2b1c");
        assert_eq!(default_name("Port Aldor", id), "Port Aldor 3f9a2b1c");
        assert!(clean_name(&default_name(&"a".repeat(crate::city::MAX_NAME_CHARS), id)).is_ok(), "the longest default fits");
        assert_eq!(clean_name("  Port   Guard ").unwrap(), "Port Guard");
        assert_eq!(clean_name(" ").unwrap_err(), "Enter a name for your militia.");
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn a_besieged_militia_loses_five_to_ten_percent_rounded_up_and_is_never_left_with_none() {
        assert_eq!(after_siege(1_000, 0.0), 950, "the least");
        assert_eq!(after_siege(1_000, 1.0), 900, "the most");
        assert_eq!(after_siege(1_000, 0.5), 925);
        assert_eq!(after_siege(101, 0.0), 95, "5.05 is rounded up to 6");
        assert_eq!(after_siege(10, 0.0), 9, "a small militia loses one");
        assert_eq!(after_siege(2, 1.0), 1);
        assert_eq!(after_siege(1, 1.0), 1, "the last member stays");
        assert_eq!(after_siege(0, 0.5), 0, "an empty one is as it was");
        assert!(after_siege(u32::MAX, 1.0) < u32::MAX);
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
