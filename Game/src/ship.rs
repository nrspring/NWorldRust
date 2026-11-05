//! Ships, the second kind of unit: warships, built at a shipyard. A ship behaves like a militia (it belongs to a country, which can
//! change, and pays to keep it; it has movement points that come back each day; it can be moved, renamed and disbanded) except that it
//! sails instead of walking, and only on the water its size can use.
//!
//! There are two sizes. A **small** warship sails rivers, lakes and shallow water, and a **large** one sails shallow and deep water
//! (so a small one can go where a large one can't, up a river, and a large one can cross the open sea). Neither can go on land, nor
//! on the other's water. A ship is one vessel, so unlike a militia it has no members, and can't be combined or split.
//!
//! A ship is built by the country that owns a shipyard, which has to rest for some days after (see [`crate::shipyard`]). It starts
//! in the water a little way off the shipyard, and is moved from there (see [`crate::movement::reach_by_water`]).

use macroquad::math::{vec2, Vec2};
use nworld_map::world::{Resource, WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::format;
use crate::ids::{CountryId, ShipId};
use crate::movement::DAILY_POINTS;
use crate::newsreel::Cost;
use crate::text;
use crate::transfer::Asset;

/// The longest a ship's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// How far from a shipyard a new ship floats, in meters: the nearest, and the rings beyond that to try when the water there is the
/// wrong kind or the places are taken.
const BERTH_RINGS: [f32; 6] = [12.0, 24.0, 36.0, 52.0, 72.0, 96.0];
/// How many places round each ring are tried, and how close two ships may float, in meters.
const BERTH_TRIES: usize = 16;
const BERTH_SPACING: f32 = 22.0;
/// A turn of this many radians takes the next berth of a shipyard well away from the last, however many there are.
const GOLDEN_ANGLE: f32 = 2.399_963;

/// How big a ship is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShipKind {
    Small,
    Large,
}

impl ShipKind {
    /// Every kind, the small one first.
    pub const ALL: [ShipKind; 2] = [ShipKind::Small, ShipKind::Large];

    /// What to call one, in lower case: "small warship" or "large warship".
    pub fn noun(self) -> &'static str {
        match self {
            Self::Small => "small warship",
            Self::Large => "large warship",
        }
    }

    /// What to call one with "a" before it: "a small warship".
    pub fn a_noun(self) -> String {
        format!("a {}", self.noun())
    }

    /// What to call it where a name starts something: "Small warship" or "Large warship".
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small warship",
            Self::Large => "Large warship",
        }
    }

    /// What one costs to build, in coins.
    pub fn cost(self) -> i64 {
        match self {
            Self::Small => 2_000,
            Self::Large => 10_000,
        }
    }

    /// What one takes to build besides coins. A small one is a wooden hull, some iron fittings and powder for its guns; a large one is
    /// much more of each, and has copper sheathing on the hull as well.
    pub fn materials(self) -> &'static [(Resource, u64)] {
        match self {
            Self::Small => &[(Resource::Forest, 400), (Resource::Iron, 100), (Resource::Sulphur, 50)],
            Self::Large => &[(Resource::Forest, 1_500), (Resource::Iron, 600), (Resource::Copper, 200), (Resource::Sulphur, 200)],
        }
    }

    /// Everything one costs to build, for the newsreel: coins first, and then each resource.
    pub fn full_cost(self) -> Vec<Cost> {
        let mut cost = vec![Cost::coins(self.cost())];
        cost.extend(self.materials().iter().map(|(resource, amount)| Cost { asset: Asset::of(*resource), amount: *amount }));
        cost
    }

    /// What one costs to keep each day, in coins: its crew's pay and the upkeep of the vessel.
    pub fn upkeep(self) -> i64 {
        match self {
            Self::Small => 50,
            Self::Large => 250,
        }
    }

    /// What a meter of water costs it in movement points. A ship under sail goes farther in a day than a militia on foot, and a large
    /// one goes farther than a small one.
    pub fn water_cost(self) -> f32 {
        match self {
            Self::Small => 0.5,
            Self::Large => 0.4,
        }
    }

    /// Whether it can sail on water of this class: a small one on rivers, lakes and shallow water, and a large one on shallow and deep
    /// water.
    pub fn can_sail(self, water: WaterClass) -> bool {
        match (self, water) {
            (_, WaterClass::Dry) => false,
            (Self::Small, water) => matches!(water, WaterClass::Inland | WaterClass::Shallow),
            (Self::Large, water) => matches!(water, WaterClass::Shallow | WaterClass::Deep),
        }
    }

    /// The water it sails, as it is said: "rivers, lakes and shallow water".
    pub fn waters_text(self) -> &'static str {
        match self {
            Self::Small => "rivers, lakes and shallow water",
            Self::Large => "shallow and deep water",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ship {
    /// Tells it apart from the other ships, and stays with it whatever else changes.
    pub id: ShipId,
    /// What it is called, which can be changed. It starts as its size and the first part of its id.
    #[serde(default)]
    pub name: String,
    pub kind: ShipKind,
    /// The country it belongs to, which pays to keep it.
    pub country: CountryId,
    /// The day it was built.
    pub built: Date,
    /// Where it floats on the map, in meters.
    pub x: f32,
    pub z: f32,
    /// The movement points it has left today, which it gets all of back each day.
    #[serde(default = "full_points")]
    pub points: u32,
}

fn full_points() -> u32 {
    DAILY_POINTS
}

impl Ship {
    /// The name to show for it: its own, or just its size for one saved without.
    pub fn title(&self) -> &str {
        if self.name.trim().is_empty() { self.kind.label() } else { &self.name }
    }
}

/// A ship name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your ship")
}

/// What a ship of this size with this id is called until it is renamed: its size and the first part of its id, like "Small warship
/// 3f9a2b1c".
pub fn default_name(kind: ShipKind, id: ShipId) -> String {
    format!("{} {}", kind.label(), id.short())
}

/// Where a new ship of `kind` can float, off a shipyard at `yard`: in water that size can sail, and not on top of the ships at
/// `taken`. `None` if there is no such water near it. Each ship of a shipyard goes to a different side of it.
pub fn berth_near(world: &World, kind: ShipKind, yard: Vec2, taken: &[Vec2]) -> Option<Vec2> {
    let half = world.half_extents();
    let start = GOLDEN_ANGLE * taken.len() as f32;
    for ring in BERTH_RINGS {
        for k in 0..BERTH_TRIES {
            let angle = start + std::f32::consts::TAU * k as f32 / BERTH_TRIES as f32;
            let at = yard + vec2(angle.cos(), angle.sin()) * ring;
            let on_map = at.x.abs() <= half.x && at.y.abs() <= half.y;
            if on_map && kind.can_sail(world.water_class(at.x, at.y)) && taken.iter().all(|other| other.distance(at) >= BERTH_SPACING) {
                return Some(at);
            }
        }
    }
    None
}

/// `n` days, as it is said: "1 day" or "5 days".
pub fn days_text(n: u32) -> String {
    format!("{} {}", format::thousands(n), if n == 1 { "day" } else { "days" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn a_small_warship_sails_rivers_lakes_and_shallows_and_a_large_one_shallow_and_deep_water() {
        use WaterClass::*;
        assert_eq!([Dry, Inland, Shallow, Deep].map(|w| ShipKind::Small.can_sail(w)), [false, true, true, false]);
        assert_eq!([Dry, Inland, Shallow, Deep].map(|w| ShipKind::Large.can_sail(w)), [false, false, true, true]);
    }

    #[test]
    fn a_large_warship_costs_more_to_build_and_keep_but_goes_farther() {
        assert_eq!((ShipKind::Small.cost(), ShipKind::Large.cost()), (2_000, 10_000));
        assert_eq!((ShipKind::Small.upkeep(), ShipKind::Large.upkeep()), (50, 250));
        assert_eq!(ShipKind::Small.materials(), &[(Resource::Forest, 400), (Resource::Iron, 100), (Resource::Sulphur, 50)]);
        assert_eq!(ShipKind::Large.materials(), &[(Resource::Forest, 1_500), (Resource::Iron, 600), (Resource::Copper, 200), (Resource::Sulphur, 200)]);
        for (small, large) in ShipKind::Small.materials().iter().zip(ShipKind::Large.materials()) {
            assert!(large.1 > small.1, "{:?}: a large one takes more", small.0);
        }
        assert!(ShipKind::Large.water_cost() < ShipKind::Small.water_cost(), "a large one gets farther on a point");
        // Both go farther than a militia on foot, which spends a point a meter.
        assert!(ShipKind::Small.water_cost() < crate::movement::LAND_COST);
        assert_eq!(ShipKind::Large.full_cost().len(), 1 + 4);
        assert_eq!(ShipKind::Small.full_cost()[0], Cost::coins(2_000));
    }

    #[test]
    fn a_ship_starts_named_for_its_size_and_its_id_and_a_name_is_tidied() {
        let id = ShipId::from_u128(0x3f9a2b1c_5a47_4d0e_9b6a_7c1d2e8f4a90);
        assert_eq!(default_name(ShipKind::Small, id), "Small warship 3f9a2b1c");
        assert_eq!(default_name(ShipKind::Large, id), "Large warship 3f9a2b1c");
        assert!(clean_name(&default_name(ShipKind::Large, id)).is_ok());
        assert_eq!(clean_name("  Sea   Wolf ").unwrap(), "Sea Wolf");
        assert_eq!(clean_name(" ").unwrap_err(), "Enter a name for your ship.");
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
        let nameless = Ship { id, name: String::new(), kind: ShipKind::Large, country: CountryId::new(), built: Date::FIRST, x: 0.0, z: 0.0, points: 0 };
        assert_eq!(nameless.title(), "Large warship");
    }

    #[test]
    fn days_are_counted_in_the_singular_and_the_plural() {
        assert_eq!((days_text(1).as_str(), days_text(5).as_str(), days_text(10).as_str()), ("1 day", "5 days", "10 days"));
    }

    /// A shore of the test continent: the first dry ground on a walk from the sea to the land.
    fn shore(world: &World) -> Vec2 {
        let (land, sea) = test_support::land_and_sea(world);
        let steps = 2_000;
        (0..=steps).map(|k| sea.lerp(land, k as f32 / steps as f32)).find(|p| world.water_class(p.x, p.y) == WaterClass::Dry).expect("land at the end of the walk")
    }

    #[test]
    fn a_new_ship_floats_in_water_its_size_can_sail_off_the_shipyard() {
        let world = test_support::small_continent();
        let yard = shore(&world);
        for kind in ShipKind::ALL {
            let berth = berth_near(&world, kind, yard, &[]);
            if let Some(at) = berth {
                assert!(kind.can_sail(world.water_class(at.x, at.y)), "{kind:?} at {at}: {:?}", world.water_class(at.x, at.y));
                assert!(at.distance(yard) <= BERTH_RINGS[BERTH_RINGS.len() - 1] + 0.01);
            }
        }
        assert!(berth_near(&world, ShipKind::Large, yard, &[]).is_some() || berth_near(&world, ShipKind::Small, yard, &[]).is_some(), "a shore has water by it");
    }

    #[test]
    fn the_ships_of_one_shipyard_go_to_different_places_and_inland_there_is_no_berth() {
        let world = test_support::small_continent();
        let yard = shore(&world);
        let kind = ShipKind::ALL.into_iter().find(|kind| berth_near(&world, *kind, yard, &[]).is_some()).expect("a berth for some size");
        let mut berths: Vec<Vec2> = Vec::new();
        for _ in 0..4 {
            if let Some(at) = berth_near(&world, kind, yard, &berths) {
                assert!(berths.iter().all(|other| other.distance(at) >= BERTH_SPACING), "{at} is on another ship");
                berths.push(at);
            }
        }
        assert!(!berths.is_empty());
        // Far from any water, a ship has nowhere to float.
        let inland = test_support::inland_spot(&world, 150.0);
        if ShipKind::ALL.iter().all(|kind| (0..BERTH_TRIES).all(|k| {
            let angle = std::f32::consts::TAU * k as f32 / BERTH_TRIES as f32;
            BERTH_RINGS.iter().all(|ring| !kind.can_sail(world.water_class(inland.x + angle.cos() * ring, inland.y + angle.sin() * ring)))
        })) {
            assert_eq!(berth_near(&world, ShipKind::Small, inland, &[]), None);
        }
    }

    #[test]
    fn a_berth_is_always_the_same_for_the_same_shipyard() {
        let world = test_support::small_continent();
        let yard = shore(&world);
        assert_eq!(berth_near(&world, ShipKind::Small, yard, &[]), berth_near(&world, ShipKind::Small, yard, &[]));
    }
}
