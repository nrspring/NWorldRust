//! Shipyards: where a country will build ships. A shipyard is a small one or a large one, belongs to whichever country owns the
//! region it stands in, which is the country that built it, and which has to own the region to build there; it pays for it in
//! coins, stone, wood and iron. It can't be given to another country: only the region changes hands, and the shipyard goes with
//! it. It has to stand on a coast: dry land with the sea within a short way of it. Only a small one can be built; a small one can then be upgraded to a large one, for a price.
//!
//! A shipyard builds warships (see [`crate::ship`]). A small shipyard can only build small ones, and has to rest for 5 days after
//! each; a large shipyard can build a small one, and rest for 3 days, or a large one, and rest for 10. While it rests it can build
//! nothing.

use macroquad::prelude::*;
use nworld_map::world::{Resource, WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::day::DaySave;
use crate::format;
use crate::ids::{CountryId, ShipyardId};
use crate::newsreel::{Cost, Event, News};
use crate::ship::ShipKind;
use crate::transfer::Asset;

/// How far from a shipyard the sea can be for it to count as on the coast, in meters.
pub const COAST_REACH: f32 = 30.0;
/// How close two shipyards can be, in meters, so that each can be seen and pointed at.
pub const SPACING: f32 = 25.0;
/// How many places round a shipyard are looked at on each ring when looking for the sea.
const COAST_DIRECTIONS: usize = 16;

/// What a small shipyard costs to build in coins.
pub const COST: i64 = 10_000;
/// What a small shipyard takes to build besides coins.
pub const MATERIALS: [(Resource, u64); 3] = [(Resource::Stone, 1_000), (Resource::Forest, 1_000), (Resource::Iron, 500)];
/// What upgrading a small shipyard to a large one costs in coins, and takes besides.
pub const UPGRADE_COST: i64 = 10_000;
pub const UPGRADE_MATERIALS: [(Resource, u64); 3] = [(Resource::Stone, 1_000), (Resource::Forest, 1_000), (Resource::Iron, 500)];

/// How big a shipyard is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShipyardKind {
    Small,
    Large,
}

impl ShipyardKind {
    /// Every kind, the small one first.
    pub const ALL: [ShipyardKind; 2] = [ShipyardKind::Small, ShipyardKind::Large];
    /// The kinds that can be built, which is all in the Buildings menu: only a small shipyard, which can then be upgraded.
    pub const BUILDABLE: [ShipyardKind; 1] = [ShipyardKind::Small];

    /// Whether a shipyard of this size can build a ship of this size: a small shipyard only small ones, a large one either.
    pub fn can_build(self, ship: ShipKind) -> bool {
        self.break_days(ship).is_some()
    }

    /// How many days a shipyard of this size has to rest after building a ship of this size, or `None` if it can't build one: 5 days
    /// for a small one building a small ship, 3 for a large one building a small ship, and 10 for a large one building a large ship.
    pub fn break_days(self, ship: ShipKind) -> Option<u32> {
        match (self, ship) {
            (Self::Small, ShipKind::Small) => Some(5),
            (Self::Large, ShipKind::Small) => Some(3),
            (Self::Large, ShipKind::Large) => Some(10),
            (Self::Small, ShipKind::Large) => None,
        }
    }

    /// What it can be upgraded to, if it can be.
    pub fn upgrade(self) -> Option<ShipyardKind> {
        match self {
            Self::Small => Some(Self::Large),
            Self::Large => None,
        }
    }

    /// What to call one, in lower case: "small shipyard" or "large shipyard".
    pub fn noun(self) -> &'static str {
        match self {
            Self::Small => "small shipyard",
            Self::Large => "large shipyard",
        }
    }

    /// What to call one with "a" before it: "a small shipyard".
    pub fn a_noun(self) -> String {
        format!("a {}", self.noun())
    }

    /// [`a_noun`](Self::a_noun) to start a sentence: "A small shipyard".
    pub fn a_noun_capital(self) -> String {
        format::capitalized(&self.a_noun())
    }

    /// What to call it where a name starts something: "Small shipyard" or "Large shipyard".
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small shipyard",
            Self::Large => "Large shipyard",
        }
    }

    /// What one costs to build in coins.
    pub fn cost(self) -> i64 {
        COST
    }

    /// What one takes to build besides coins: stone, wood and iron.
    pub fn materials(self) -> &'static [(Resource, u64)] {
        &MATERIALS
    }

    /// Everything one costs to build, for the newsreel: coins first, and then each resource.
    pub fn full_cost(self) -> Vec<Cost> {
        full_cost(self.cost(), self.materials())
    }

    /// Draws its icon, centered on `c` and about 16 pixels across times `s`, in `color`, with `accent` for what is marked
    /// on it: a ship on the water, with one mast for a small shipyard and two for a large one.
    pub fn draw_icon(self, c: Vec2, s: f32, color: Color, accent: Color) {
        // The hull: wider at the deck than at the keel.
        draw_triangle(vec2(c.x - 8.0 * s, c.y + 1.0 * s), vec2(c.x + 8.0 * s, c.y + 1.0 * s), vec2(c.x - 5.0 * s, c.y + 6.0 * s), color);
        draw_triangle(vec2(c.x + 8.0 * s, c.y + 1.0 * s), vec2(c.x + 5.0 * s, c.y + 6.0 * s), vec2(c.x - 5.0 * s, c.y + 6.0 * s), color);
        let masts: &[f32] = match self {
            Self::Small => &[0.0],
            Self::Large => &[-3.5, 3.5],
        };
        for x in masts {
            draw_line(c.x + x * s, c.y + 1.0 * s, c.x + x * s, c.y - 8.0 * s, 1.4 * s, color);
            draw_triangle(vec2(c.x + x * s + 0.5 * s, c.y - 7.0 * s), vec2(c.x + x * s + 0.5 * s, c.y - 1.0 * s), vec2(c.x + x * s + 4.0 * s, c.y - 1.0 * s), accent);
        }
    }
}

/// `coins` and then each of `materials`, as costs for the newsreel.
fn full_cost(coins: i64, materials: &[(Resource, u64)]) -> Vec<Cost> {
    let mut cost = vec![Cost::coins(coins)];
    cost.extend(materials.iter().map(|(resource, amount)| Cost { asset: Asset::of(*resource), amount: *amount }));
    cost
}

/// Everything upgrading a shipyard costs, for the newsreel: coins first, and then each resource.
pub fn upgrade_full_cost() -> Vec<Cost> {
    full_cost(UPGRADE_COST, &UPGRADE_MATERIALS)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Shipyard {
    pub id: ShipyardId,
    pub kind: ShipyardKind,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in, if the map has regions.
    #[serde(default)]
    pub region: Option<u16>,
    /// The first day it can build a ship again, if it is resting after building one. A shipyard that has never built a ship, or whose
    /// rest is over, can build one now. (A shipyard saved before there were ships has none.)
    #[serde(default)]
    pub ready_on: Option<Date>,
}

impl Shipyard {
    /// How many days it still has to rest on `today`: 0 if it can build a ship now.
    pub fn rest_left(&self, today: Date) -> u32 {
        self.ready_on.map_or(0, |ready| today.days_until(ready))
    }
}

/// Whether there is sea within [`COAST_REACH`] of (`x`, `z`): shallow or deep water, which rivers and lakes are not.
pub fn on_a_coast(world: &World, x: f32, z: f32) -> bool {
    [COAST_REACH * 0.25, COAST_REACH * 0.5, COAST_REACH * 0.75, COAST_REACH].into_iter().any(|distance| {
        (0..COAST_DIRECTIONS).any(|k| {
            let angle = std::f32::consts::TAU * k as f32 / COAST_DIRECTIONS as f32;
            matches!(world.water_class(x + angle.cos() * distance, z + angle.sin() * distance), WaterClass::Shallow | WaterClass::Deep)
        })
    })
}

/// Why a shipyard can't be built at `at`, or the number of the region it would be in (`None` if the map has none): it
/// has to be on the map, on dry land, and on a coast.
pub fn site_region(world: &World, kind: ShipyardKind, at: Vec2) -> Result<Option<u16>, String> {
    let half = world.half_extents();
    if at.x.abs() > half.x || at.y.abs() > half.y {
        return Err("That is off the map.".to_owned());
    }
    if world.water_class(at.x, at.y) != WaterClass::Dry {
        return Err(format!("{} has to stand on dry land.", kind.a_noun_capital()));
    }
    if !on_a_coast(world, at.x, at.y) {
        return Err(format!("{} has to stand on the coast, with the sea close by.", kind.a_noun_capital()));
    }
    match world.region_at(at.x, at.y) {
        Some(region) => Ok(Some(region.raw())),
        None => Err(format!("This map has no regions to put {} in.", kind.a_noun())),
    }
}

impl DaySave {
    /// The shipyard with this id.
    pub fn shipyard(&self, id: ShipyardId) -> Option<&Shipyard> {
        self.shipyards.iter().find(|s| s.id == id)
    }

    /// Why the country being played can't pay `coins` and `materials` for `what` ("a small shipyard"), or `None` if it can:
    /// the coins are asked for first, and then each resource in turn.
    pub(crate) fn payment_problem(&self, what: &str, coins: i64, materials: &[(Resource, u64)]) -> Option<String> {
        let country = self.player_country()?;
        if country.money < coins {
            return Some(format!("Not enough coin: {what} costs {} coins, and {} has {}.", format::money(coins), country.name, format::money(country.money)));
        }
        materials.iter().find(|(resource, needed)| country.stock.get(*resource) < *needed).map(|(resource, needed)| {
            let name = crate::country::resource_name(*resource);
            format!("Not enough {name}: {what} needs {} {name} as well as its coins, and {} has {}.", format::thousands_u64(*needed), country.name, format::thousands_u64(country.stock.get(*resource)))
        })
    }

    /// Why a shipyard of `kind` can't be built now, or `None` if one can: only a small one can be built, and the country
    /// being played has to be there and able to pay for it, in coins first, and then in stone, wood and iron.
    pub fn shipyard_problem(&self, kind: ShipyardKind) -> Option<String> {
        if !ShipyardKind::BUILDABLE.contains(&kind) {
            return Some(format!("{} can't be built: build a small shipyard and upgrade it.", kind.a_noun_capital()));
        }
        if self.player_country().is_none() {
            return Some(format!("There is no country yet to own {}.", kind.a_noun()));
        }
        self.payment_problem(&kind.a_noun(), kind.cost(), kind.materials())
    }

    /// Why the shipyard with this id can't be upgraded now, or `None` if it can: it has to be one that can be, and its country
    /// has to be the one being played, which pays for it, in coins first, and then in stone, wood and iron.
    pub fn upgrade_problem(&self, id: ShipyardId) -> Option<String> {
        let Some(yard) = self.shipyard(id) else { return Some("There is no such shipyard.".to_owned()) };
        let Some(to) = yard.kind.upgrade() else { return Some(format!("{} can't be upgraded any more.", yard.kind.a_noun_capital())) };
        let Some(player) = self.player_country() else { return Some("There is no country yet to pay for an upgrade.".to_owned()) };
        if yard.country != player.id {
            return Some(format!("This shipyard belongs to {}, and only {} can upgrade it.", self.country_name(yard.country), self.country_name(yard.country)));
        }
        self.payment_problem(&format!("upgrading to {}", to.a_noun()), UPGRADE_COST, &UPGRADE_MATERIALS)
    }

    /// Why the country being played can't have a shipyard at (`x`, `z`) in the region numbered `region`, or `None` if it
    /// can: the region has to be the country's, and the shipyard can't be on top of another.
    pub fn shipyard_place_problem(&self, x: f32, z: f32, region: Option<u16>) -> Option<String> {
        let country = self.player_country()?.id;
        self.placement_region_problem(country, region).or_else(|| {
            self.shipyards.iter().find(|s| ((s.x - x).powi(2) + (s.z - z).powi(2)).sqrt() < SPACING).map(|s| format!("That is too close to another {}.", s.kind.noun()))
        })
    }

    /// Builds a shipyard of `kind` at (`x`, `z`) in the region numbered `region` (`None` if the map has none) for the
    /// country being played, which pays for it, and reports it in the newsreel. The region has to be the country's already.
    /// Whether the place is a coast (see [`site_region`]) is for the caller to say. An error says why it can't be done, and
    /// nothing changes.
    pub fn build_shipyard(&mut self, kind: ShipyardKind, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.shipyard_problem(kind) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.shipyard_place_problem(x, z, region) {
            return Err(problem);
        }
        let country = self.player_country().ok_or_else(|| format!("There is no country yet to own {}.", kind.a_noun()))?.id;
        self.add_money(country, -kind.cost())?;
        for (resource, needed) in kind.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        let id = ShipyardId::new();
        self.shipyards.push(Shipyard { id, kind, country, x, z, region, ready_on: None });
        self.newsreel.push(News::about(id, Event::ShipyardBuilt { kind, country, region, cost: kind.full_cost() }));
        Ok(())
    }

    /// Upgrades the shipyard with this id to the next size, which its country, the one being played, pays for. It stays where
    /// it is. This is reported in the newsreel. An error says why it can't be done, and nothing changes.
    pub fn upgrade_shipyard(&mut self, id: ShipyardId) -> Result<(), String> {
        if let Some(problem) = self.upgrade_problem(id) {
            return Err(problem);
        }
        let Shipyard { kind, country, region, .. } = *self.shipyard(id).ok_or("There is no such shipyard.")?;
        let to = kind.upgrade().ok_or("That shipyard can't be upgraded any more.")?;
        self.add_money(country, -UPGRADE_COST)?;
        for (resource, needed) in UPGRADE_MATERIALS {
            self.add_resource(country, resource, -(needed as i64))?;
        }
        if let Some(yard) = self.shipyards.iter_mut().find(|s| s.id == id) {
            yard.kind = to;
        }
        self.newsreel.push(News::about(id, Event::ShipyardUpgraded { from: kind, to, country, region, cost: upgrade_full_cost() }));
        Ok(())
    }

    /// For tests: claims the region numbered `region` for the country being played if there is one and nobody owns it, and then
    /// builds a shipyard there.
    #[cfg(test)]
    pub fn build_shipyard_claiming(&mut self, kind: ShipyardKind, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        if let Some(region) = region.filter(|r| self.region_owner(*r).is_none()) {
            self.claim_region(region)?;
        }
        self.build_shipyard(kind, x, z, region)
    }

    /// Destroys the shipyard with this id, which costs nothing and pays nothing back, and reports it in the newsreel. An
    /// error says why it can't be done.
    pub fn destroy_shipyard(&mut self, id: ShipyardId) -> Result<(), String> {
        let Shipyard { kind, country, region, .. } = *self.shipyard(id).ok_or("There is no such shipyard to destroy.")?;
        self.shipyards.retain(|s| s.id != id);
        self.newsreel.push(News::about(id, Event::ShipyardDestroyed { kind, country, region }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::test_support;

    /// A day with Aldoria, which has 20,000 coins and 2,000 of every resource.
    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 20_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 2_000).unwrap();
        }
        day
    }

    #[test]
    fn a_shipyard_costs_10000_coins_1000_stone_1000_wood_and_500_iron_to_build_and_the_same_to_upgrade() {
        for kind in ShipyardKind::ALL {
            assert_eq!(kind.cost(), 10_000);
            assert_eq!(kind.materials(), &[(Resource::Stone, 1_000), (Resource::Forest, 1_000), (Resource::Iron, 500)]);
        }
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.build_shipyard_claiming(ShipyardKind::Small, 10.0, 20.0, Some(0x4005)).unwrap();

        let yard = day.shipyards[0];
        assert_eq!((yard.kind, yard.country, yard.x, yard.z, yard.region), (ShipyardKind::Small, aldoria, 10.0, 20.0, Some(0x4005)));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, 10_000);
        assert_eq!((country.stock.stone, country.stock.wood, country.stock.iron), (1_000, 1_000, 1_500));
        assert_eq!((country.stock.copper, country.stock.sulphur, country.stock.oil), (2_000, 2_000, 2_000), "nothing else is taken");
        assert_eq!(day.shipyard(yard.id), Some(&yard));
        assert_eq!(day.newsreel.last(), Some(&News::about(yard.id, Event::ShipyardBuilt { kind: ShipyardKind::Small, country: aldoria, region: Some(0x4005), cost: ShipyardKind::Small.full_cost() })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a small shipyard in region 5 for 10,000 coins, 1,000 stone, 1,000 wood and 500 iron");
    }

    #[test]
    fn only_a_small_shipyard_can_be_built() {
        let mut day = day();
        day.claim_region(0x4005).unwrap();
        let before = day.clone();
        assert_eq!(ShipyardKind::BUILDABLE, [ShipyardKind::Small]);
        let err = day.build_shipyard(ShipyardKind::Large, 0.0, 0.0, Some(0x4005)).unwrap_err();
        assert!(err.contains("A large shipyard can't be built") && err.contains("upgrade"), "{err}");
        assert_eq!(day, before);
        assert_eq!(day.shipyard_problem(ShipyardKind::Small), None);
    }

    #[test]
    fn a_small_shipyard_is_upgraded_to_a_large_one_for_a_price_and_stays_where_it_is() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.build_shipyard_claiming(ShipyardKind::Small, 10.0, 20.0, Some(0x4005)).unwrap();
        let (id, built) = (day.shipyards[0].id, day.shipyards[0]);
        assert_eq!(day.upgrade_problem(id), None);
        day.upgrade_shipyard(id).unwrap();

        let yard = *day.shipyard(id).unwrap();
        assert_eq!(yard, Shipyard { kind: ShipyardKind::Large, ..built }, "only its size changed");
        assert_eq!(day.shipyards.len(), 1);
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, 0, "10,000 to build and 10,000 to upgrade");
        assert_eq!((country.stock.stone, country.stock.wood, country.stock.iron), (0, 0, 1_000));
        assert_eq!(day.newsreel.last(), Some(&News::about(id, Event::ShipyardUpgraded { from: ShipyardKind::Small, to: ShipyardKind::Large, country: aldoria, region: Some(0x4005), cost: upgrade_full_cost() })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria upgrades its small shipyard in region 5 to a large shipyard for 10,000 coins, 1,000 stone, 1,000 wood and 500 iron");
        // A large one is as big as it gets, and a shipyard that isn't there can't be upgraded.
        assert!(day.upgrade_problem(id).unwrap().contains("A large shipyard can't be upgraded any more"));
        assert!(day.upgrade_shipyard(id).is_err() && day.upgrade_shipyard(ShipyardId::new()).is_err());
    }

    #[test]
    fn an_upgrade_needs_the_coin_and_each_material_and_the_players_own_country() {
        let upgradable = |extra: i64| {
            let mut day = day();
            let id = day.countries[0].id;
            day.add_money(id, extra).unwrap();
            day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
            // What is left after building: 10,000 coins, 1,000 stone, 1,000 wood and 1,500 iron.
            day
        };
        let mut poor = upgradable(0);
        let (id, yard) = (poor.countries[0].id, poor.shipyards[0].id);
        poor.add_money(id, -1).unwrap();
        let err = poor.upgrade_shipyard(yard).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("upgrading to a large shipyard costs 10,000") && err.contains("9,999"), "{err}");

        for (resource, name, needed) in [(Resource::Stone, "stone", 1_000), (Resource::Forest, "wood", 1_000), (Resource::Iron, "iron", 500)] {
            let mut day = upgradable(0);
            let id = day.countries[0].id;
            let has = day.country(id).unwrap().stock.get(resource);
            day.add_resource(id, resource, -(has as i64 - needed as i64 + 1)).unwrap(); // One short.
            let before = day.clone();
            let err = day.upgrade_shipyard(day.shipyards[0].id).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")) && err.contains(&format!("has {}", format::thousands_u64(needed - 1))), "{err}");
            assert_eq!(day, before, "a refused upgrade changes nothing");
        }

        // Only the country that is being played can pay for it.
        let mut day = upgradable(0);
        let (brevon, yard) = (day.countries[1].id, day.shipyards[0].id);
        day.shipyards[0].country = brevon;
        assert!(day.upgrade_problem(yard).unwrap().contains("belongs to Brevon"));
    }

    #[test]
    fn a_shipyard_needs_the_coin_and_each_material_and_says_which_is_short() {
        let mut broke = day();
        let id = broke.countries[0].id;
        broke.add_money(id, -10_001).unwrap();
        let err = broke.build_shipyard(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("a small shipyard costs 10,000") && err.contains("9,999"), "{err}");

        for (resource, name, needed) in [(Resource::Stone, "stone", 1_000), (Resource::Forest, "wood", 1_000), (Resource::Iron, "iron", 500)] {
            let mut day = day();
            let id = day.countries[0].id;
            day.add_resource(id, resource, -(2_000 - needed as i64 + 1)).unwrap(); // One short.
            day.claim_region(0x4005).unwrap();
            let before = day.clone();
            let err = day.build_shipyard(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")) && err.contains(&format!("has {}", format::thousands_u64(needed - 1))), "{err}");
            assert_eq!(day, before, "a refused shipyard changes nothing");
            day.add_resource(id, resource, 1).unwrap();
            day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        }
        assert!(DaySave::new(Date::FIRST, "x").shipyard_problem(ShipyardKind::Small).unwrap().contains("no country"));
    }

    #[test]
    fn a_shipyard_cannot_go_in_a_region_of_another_country_or_on_another_shipyard_or_off_the_map() {
        let mut day = day();
        let brevon = day.countries[1].id;
        day.set_region_owner(0x4009, Some(brevon)).unwrap();
        day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        let before = day.clone();
        assert!(day.build_shipyard_claiming(ShipyardKind::Small, 100.0, 0.0, Some(0x4009)).unwrap_err().contains("belongs to Brevon"));
        assert!(day.build_shipyard_claiming(ShipyardKind::Small, 10.0, 0.0, Some(0x4005)).unwrap_err().contains("too close to another small shipyard"));
        assert!(day.build_shipyard_claiming(ShipyardKind::Small, f32::NAN, 0.0, Some(0x4005)).is_err());
        assert_eq!(day, before, "nothing was taken for any of them");
        day.build_shipyard_claiming(ShipyardKind::Small, SPACING, 0.0, Some(0x4005)).unwrap();
    }

    #[test]
    fn a_shipyard_belongs_to_whoever_owns_its_region_and_goes_with_it() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4007)).unwrap();
        day.build_shipyard_claiming(ShipyardKind::Small, 100.0, 0.0, Some(0x4007)).unwrap();
        let id = day.shipyards[0].id;
        assert_eq!(day.shipyard(id).unwrap().country, aldoria);

        // A shipyard in a region keeps it from being given away, or made nobody's, even with no city in it.
        day.claim_region(0x4008).unwrap();
        day.build_shipyard(ShipyardKind::Small, 300.0, 0.0, Some(0x4008)).unwrap();
        let before = day.clone();
        let err = day.set_region_owner(0x4008, Some(brevon)).unwrap_err();
        assert!(err.contains("small shipyard of Aldoria is") && err.contains("has to stay with Aldoria"), "{err}");
        assert!(day.set_region_owner(0x4008, None).is_err());
        assert_eq!(day, before);

        // But when the region changes hands with its city, the shipyard goes with it.
        let city = day.cities[0].id;
        day.set_city_country(city, brevon).unwrap();
        assert_eq!(day.region_owner(0x4007), Some(brevon));
        assert_eq!(day.shipyard(id).unwrap().country, brevon);
    }

    #[test]
    fn a_country_can_only_build_a_shipyard_in_a_region_it_owns() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        // Nobody's region: it has to be claimed first, and nothing is taken for asking.
        let before = day.clone();
        let err = day.build_shipyard(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap_err();
        assert!(err.contains("Aldoria doesn't own this region") && err.contains("claim it first"), "{err}");
        assert!(day.build_shipyard(ShipyardKind::Small, 0.0, 0.0, None).unwrap_err().contains("no regions"));
        assert_eq!(day, before);
        day.claim_region(0x4005).unwrap();
        day.build_shipyard(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        assert_eq!(day.shipyards[0].country, aldoria);
        // Someone else's region is not for building in either.
        day.set_region_owner(0x4006, Some(brevon)).unwrap();
        assert!(day.build_shipyard(ShipyardKind::Small, 100.0, 0.0, Some(0x4006)).unwrap_err().contains("belongs to Brevon"));
    }
    #[test]
    fn a_shipyard_can_be_destroyed_for_nothing() {
        let mut day = day();
        day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        let (id, money) = (day.shipyards[0].id, day.countries[0].money);
        day.destroy_shipyard(id).unwrap();
        assert!(day.shipyards.is_empty() && day.countries[0].money == money, "nothing is paid back");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's small shipyard in region 5 is destroyed");
        assert!(day.destroy_shipyard(id).is_err());
    }

    #[test]
    fn shipyards_are_saved_and_carry_forward_and_a_day_saved_before_them_has_none() {
        let mut day = day();
        day.build_shipyard_claiming(ShipyardKind::Small, 5.0, 6.0, Some(0x4005)).unwrap();
        day.upgrade_shipyard(day.shipyards[0].id).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["shipyards"][0]["kind"], "large");
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).shipyards, day.shipyards);
        let mut old = json;
        old.as_object_mut().unwrap().remove("shipyards");
        assert!(serde_json::from_value::<DaySave>(old).unwrap().shipyards.is_empty());
    }

    #[test]
    fn a_country_with_a_shipyard_cannot_be_deleted() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        let problem = day.delete_problem(aldoria).unwrap();
        assert!(problem.contains("1 small shipyard"), "{problem}");
    }

    #[test]
    fn only_dry_land_near_the_sea_is_a_coast() {
        let world = test_support::regioned_continent();
        let (land, sea) = test_support::land_and_sea(&world);
        // Walking from the sea towards the land, the first dry ground is a coast, and a spot far inland, if there is one, is not.
        let steps = 2_000;
        let shore = (0..=steps).map(|k| sea.lerp(land, k as f32 / steps as f32)).find(|p| world.water_class(p.x, p.y) == WaterClass::Dry).expect("land at the end of the walk");
        assert!(on_a_coast(&world, shore.x, shore.y));
        assert!(site_region(&world, ShipyardKind::Small, shore).is_ok(), "a shipyard can stand on the shore");
        assert!(site_region(&world, ShipyardKind::Small, sea).is_err());
        assert!(site_region(&world, ShipyardKind::Small, vec2(f32::MAX / 4.0, 0.0)).unwrap_err().contains("off the map"));
        let inland = test_support::inland_spot(&world, 150.0);
        if !on_a_coast(&world, inland.x, inland.y) {
            assert!(site_region(&world, ShipyardKind::Large, inland).unwrap_err().contains("coast"));
        }
    }
}
