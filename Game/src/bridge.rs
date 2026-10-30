//! Bridges: stone bridges that carry a path across a river. A bridge joins two places on dry ground on either
//! side of the water. It costs a lot, and belongs to no country, but the coins come from the country the
//! player is playing. Paths connect to either end of one, with a [`BridgeLink`].

use macroquad::prelude::*;
use nworld_map::world::{WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::ground;
use crate::ids::BridgeId;

/// What a bridge costs, in coins.
pub const BRIDGE_COST: i64 = 1_000;
/// The longest a bridge can be, in meters, end to end.
pub const MAX_SPAN: f32 = 80.0;
/// The least water, in meters, that the line of a bridge has to cross: otherwise it isn't crossing anything.
pub const MIN_WATER: f32 = 3.0;
/// How close, in meters, both ends of a new bridge may be to the ends of one that is there already before it
/// counts as the same.
const SAME_PLACE: f32 = 6.0;

/// A place on the map, in meters.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub x: f32,
    pub z: f32,
}

impl From<Vec2> for Spot {
    fn from(at: Vec2) -> Self {
        Self { x: at.x, z: at.y }
    }
}

impl From<Spot> for Vec2 {
    fn from(spot: Spot) -> Self {
        vec2(spot.x, spot.z)
    }
}

/// Which end of a bridge.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum End {
    First,
    Second,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Bridge {
    pub id: BridgeId,
    pub first: Spot,
    pub second: Spot,
}

impl Bridge {
    /// Where the `end` of it is.
    pub fn end(&self, end: End) -> Vec2 {
        match end {
            End::First => self.first.into(),
            End::Second => self.second.into(),
        }
    }
}

/// A segment of path from one end of a bridge to the center of the region that end is in, or to the middle of one of
/// that region's edges. It is built like any segment of path, and costs the same.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BridgeLink {
    pub bridge: BridgeId,
    pub end: End,
    /// The number of the region the end of the bridge is in.
    pub region: u16,
    /// With a number, the neighbor whose border with `region` the link goes to the middle of. Without, it goes to the
    /// center of `region`.
    pub to: Option<u16>,
}

/// Why no bridge can be built now.
#[derive(Debug, PartialEq)]
pub enum BridgeProblem {
    NoCountry,
    NotEnoughCoin { country: String, money: i64 },
}

impl BridgeProblem {
    /// A few words.
    pub fn short(&self) -> String {
        match self {
            Self::NoCountry => "No country to pay for bridges".to_owned(),
            Self::NotEnoughCoin { money, .. } => format!("Bridges need {} coins (has {money})", crate::format::money(BRIDGE_COST)),
        }
    }

    /// The whole reason, as a sentence.
    pub fn long(&self) -> String {
        match self {
            Self::NoCountry => "There is no country yet to pay for a bridge.".to_owned(),
            Self::NotEnoughCoin { country, money } => {
                format!("Not enough coin: a bridge costs {} coins, and {country} has {}.", crate::format::money(BRIDGE_COST), crate::format::money(*money))
            }
        }
    }
}

impl DaySave {
    pub fn bridge(&self, id: BridgeId) -> Option<&Bridge> {
        self.bridges.iter().find(|b| b.id == id)
    }

    /// Why a bridge can't be built, or `None` if one can: the country being played has to be able to pay for it.
    pub fn bridge_problem(&self) -> Option<BridgeProblem> {
        match self.player_country() {
            None => Some(BridgeProblem::NoCountry),
            Some(country) if country.money < BRIDGE_COST => Some(BridgeProblem::NotEnoughCoin { country: country.name.clone(), money: country.money }),
            Some(_) => None,
        }
    }

    /// Builds a bridge between `first` and `second`, which the caller has checked are fit places for one (see
    /// [`check_bridge`]), and charges its cost to the country being played. An error says why it can't be done.
    pub fn build_bridge(&mut self, first: Spot, second: Spot) -> Result<(), String> {
        if let Some(problem) = self.bridge_problem() {
            return Err(problem.long());
        }
        let country = self.player_country().ok_or("There is no country yet to pay for a bridge.")?.id;
        self.add_money(country, -BRIDGE_COST)?;
        self.bridges.push(Bridge { id: BridgeId::new(), first, second });
        Ok(())
    }

    /// Builds a segment of path from the end of a bridge, which costs what a segment of path does. The caller has
    /// checked that it is a real one, and that it doesn't cross water. An error says why it can't be done.
    pub fn build_link(&mut self, link: BridgeLink) -> Result<(), String> {
        if self.bridge(link.bridge).is_none() {
            return Err("There is no such bridge.".to_owned());
        }
        if link.to == Some(link.region) {
            return Err("A path goes from a bridge to a region's center or its edge.".to_owned());
        }
        if self.bridge_links.contains(&link) {
            return Err("There is already a path there.".to_owned());
        }
        if let Some(problem) = self.path_problem() {
            return Err(problem.long());
        }
        let country = self.player_country().ok_or("There is no country yet to pay for a path.")?.id;
        self.add_money(country, -crate::path::PATH_COST)?;
        self.bridge_links.push(link);
        Ok(())
    }
}

/// Whether a bridge can go between `a` and `b` (the places the player clicked), given the `existing` bridges. Each
/// end is moved a little inland if it is on the bank, so that a path can reach it. Returns where the two ends
/// will be, or why it can't be built: both ends have to be on dry ground, they can't be too far apart, the line
/// between them has to cross a river or a lake and not the sea, and it can't be a bridge that is there already.
pub fn check_bridge(world: &World, existing: &[Bridge], a: Vec2, b: Vec2) -> Result<(Vec2, Vec2), String> {
    let (Some(a), Some(b)) = (ground::nearest_clear(world, a), ground::nearest_clear(world, b)) else {
        return Err("A bridge has to start and end on dry ground by the water.".to_owned());
    };
    let length = a.distance(b);
    if length > MAX_SPAN {
        return Err(format!("That is too far: a bridge spans at most {MAX_SPAN} meters."));
    }
    let steps = length.ceil().max(1.0) as usize;
    let step = length / steps as f32;
    let mut water = 0.0;
    for k in 0..=steps {
        match world.water_class(a.lerp(b, k as f32 / steps as f32).x, a.lerp(b, k as f32 / steps as f32).y) {
            WaterClass::Dry => {}
            WaterClass::Inland => water += step,
            WaterClass::Shallow | WaterClass::Deep => return Err("A bridge can cross a river or a lake, but not the sea.".to_owned()),
        }
    }
    if water < MIN_WATER {
        return Err("There is no river between those two places.".to_owned());
    }
    let near = |p: Vec2, q: Vec2| p.distance(q) <= SAME_PLACE;
    if existing.iter().any(|e| (near(e.first.into(), a) && near(e.second.into(), b)) || (near(e.first.into(), b) && near(e.second.into(), a))) {
        return Err("There is already a bridge there.".to_owned());
    }
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::CountryId;
    use nworld_map::world::CELL_SIZE;

    fn day_with_money(money: i64) -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, money).unwrap();
        day
    }

    const A: Spot = Spot { x: -30.0, z: 0.0 };
    const B: Spot = Spot { x: 30.0, z: 0.0 };

    #[test]
    fn a_bridge_costs_1000_coins_from_the_country_being_played() {
        let mut day = day_with_money(2_500);
        day.found_country("Brevon").unwrap();
        day.build_bridge(A, B).unwrap();
        assert_eq!(day.bridges.len(), 1);
        assert_eq!((day.bridges[0].first, day.bridges[0].second), (A, B));
        assert_eq!(day.countries[0].money, 1_500);
        assert_eq!(day.countries[1].money, 0, "only the country being played pays");
        assert!(day.bridge(day.bridges[0].id).is_some());
    }

    #[test]
    fn a_bridge_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(999);
        let before = day.clone();
        let error = day.build_bridge(A, B).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("1,000") && error.contains("Aldoria") && error.contains("999"), "{error}");
        assert_eq!(day, before);
        assert_eq!(day.bridge_problem(), Some(BridgeProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 999 }));
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.bridge_problem(), None);
        day.build_bridge(A, B).unwrap();
        assert_eq!(day.countries[0].money, 0);
        // With no country there is nobody to pay either.
        let mut empty = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert_eq!(empty.bridge_problem(), Some(BridgeProblem::NoCountry));
        assert!(empty.build_bridge(A, B).is_err());
    }

    #[test]
    fn a_bridge_is_saved_with_the_day_and_carries_forward_and_belongs_to_no_country() {
        let mut day = day_with_money(1_000);
        day.build_bridge(A, B).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["bridges"][0]["first"], serde_json::json!({ "x": -30.0, "z": 0.0 }));
        assert!(json["bridges"][0].get("country").is_none());
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).bridges, day.bridges);
    }

    #[test]
    fn a_path_can_be_linked_to_either_end_of_a_bridge_for_the_cost_of_a_path() {
        let mut day = day_with_money(1_100);
        day.build_bridge(A, B).unwrap();
        let id = day.bridges[0].id;
        let first = BridgeLink { bridge: id, end: End::First, region: 0xC001, to: None };
        let second = BridgeLink { bridge: id, end: End::Second, region: 0xC002, to: Some(0xC003) };
        day.build_link(first).unwrap();
        day.build_link(second).unwrap();
        assert_eq!(day.bridge_links, [first, second]);
        assert_eq!(day.countries[0].money, 100 - 2 * crate::path::PATH_COST);
        // The same one twice, one for a bridge that isn't there and a link to nowhere are refused for nothing.
        let before = day.clone();
        assert!(day.build_link(first).unwrap_err().contains("already"));
        assert!(day.build_link(BridgeLink { bridge: BridgeId::new(), ..first }).is_err());
        assert!(day.build_link(BridgeLink { to: Some(first.region), end: End::Second, ..first }).is_err());
        assert_eq!(day, before);
        // The links are saved and carry forward as well.
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["bridge_links"].as_array().unwrap().len(), 2);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).bridge_links, day.bridge_links);
    }

    #[test]
    fn a_link_needs_the_coin_for_a_path() {
        let mut day = day_with_money(1_005);
        day.build_bridge(A, B).unwrap();
        let link = BridgeLink { bridge: day.bridges[0].id, end: End::First, region: 0xC001, to: None };
        let error = day.build_link(link).unwrap_err();
        assert!(error.contains("Not enough coin"), "{error}");
        assert!(day.bridge_links.is_empty() && day.countries[0].money == 5);
    }

    #[test]
    fn the_reasons_read_well() {
        let problem = BridgeProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 250 };
        assert_eq!(problem.short(), "Bridges need 1,000 coins (has 250)");
        assert!(problem.long().contains("1,000") && problem.long().contains("250"));
        assert!(!BridgeProblem::NoCountry.short().is_empty());
        let _ = CountryId::new();
    }

    /// A map 512 m across of dry ground, with a river 20 m wide across it (a lake level of 10 over ground at 5).
    fn river_world(river: impl Fn(Vec2) -> bool) -> World {
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        for j in 0..nz {
            for i in 0..nx {
                if river(vec2(i as f32 * CELL_SIZE - half.x, j as f32 * CELL_SIZE - half.y)) {
                    world.water_mut()[j * nx + i] = 10.0;
                }
            }
        }
        world
    }

    #[test]
    fn a_bridge_goes_across_a_river_between_two_places_on_dry_ground() {
        let world = river_world(|p| p.x.abs() < 10.0);
        let (a, b) = check_bridge(&world, &[], vec2(-30.0, 20.0), vec2(30.0, 20.0)).unwrap();
        assert_eq!((a, b), (vec2(-30.0, 20.0), vec2(30.0, 20.0)), "places clear of the bank stay where they were");
        // Clicked on the bank, the ends are moved a little inland so that a path can reach them.
        let (a, _) = check_bridge(&world, &[], vec2(-9.0, 20.0), vec2(30.0, 20.0)).unwrap();
        assert!(ground::clear(&world, a) && a.x < -9.0, "{a}");
    }

    #[test]
    fn a_bridge_needs_a_river_between_its_ends_and_dry_ground_at_them() {
        let world = river_world(|p| p.x.abs() < 10.0);
        let no_river = check_bridge(&world, &[], vec2(-60.0, 0.0), vec2(-20.0, 0.0)).unwrap_err();
        assert!(no_river.contains("no river"), "{no_river}");
        let in_water = check_bridge(&world, &[], vec2(0.0, 0.0), vec2(30.0, 0.0)).unwrap_err();
        assert!(in_water.contains("dry ground"), "{in_water}");
    }

    #[test]
    fn a_bridge_cannot_be_longer_than_80_meters() {
        let wide = river_world(|p| p.x.abs() < 45.0);
        let error = check_bridge(&wide, &[], vec2(-60.0, 0.0), vec2(60.0, 0.0)).unwrap_err();
        assert!(error.contains("too far") && error.contains("80"), "{error}");
        // But 80 or less is fine.
        let narrow = river_world(|p| p.x.abs() < 30.0);
        assert!(check_bridge(&narrow, &[], vec2(-39.0, 0.0), vec2(39.0, 0.0)).is_ok());
    }

    #[test]
    fn a_bridge_cannot_cross_the_sea() {
        // The same ground, but the water is the sea: ground below sea level.
        let mut world = World::new(512.0, 512.0);
        world.heights_mut().fill(5.0);
        let half = world.half_extents();
        let (nx, nz) = world.samples();
        for j in 0..nz {
            for i in 0..nx {
                if (i as f32 * CELL_SIZE - half.x).abs() < 10.0 {
                    world.heights_mut()[j * nx + i] = -10.0;
                }
            }
        }
        let error = check_bridge(&world, &[], vec2(-30.0, 0.0), vec2(30.0, 0.0)).unwrap_err();
        assert!(error.contains("not the sea"), "{error}");
    }

    #[test]
    fn the_same_bridge_cannot_be_built_twice_from_either_end() {
        let world = river_world(|p| p.x.abs() < 10.0);
        let (a, b) = (vec2(-30.0, 20.0), vec2(30.0, 20.0));
        let existing = [Bridge { id: BridgeId::new(), first: a.into(), second: b.into() }];
        assert!(check_bridge(&world, &existing, a, b).unwrap_err().contains("already"));
        assert!(check_bridge(&world, &existing, b + vec2(0.0, 2.0), a).unwrap_err().contains("already"), "the other way round, a little off");
        assert!(check_bridge(&world, &existing, a + vec2(0.0, 40.0), b + vec2(0.0, 40.0)).is_ok(), "somewhere else along the river");
    }
}
