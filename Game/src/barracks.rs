//! Barracks: where a country trains soldiers, and its cousins the archery range, where it trains archers, and the stable, where it
//! trains cavalry (they are all the same thing but for what they cost, what they train a militia into and how they look: see
//! [`BarracksKind`]; "a barracks" below is any of them). A barracks belongs to whichever country owns the region it stands in, which is the
//! country that built it, and which has to own the region to build there; it pays for it in coins, stone, wood and iron. It can't
//! be given to another country: only the region changes hands, and the barracks goes with it. It stands on dry land that isn't up
//! in the mountains (it need not be on a coast).
//!
//! A barracks **trains a militia of its country that stands in the same map region as it**, turning the whole group into soldiers:
//! every member is trained, which costs each of them coins and iron, and each soldier then costs more to keep (see
//! [`crate::militia::SOLDIER_UPKEEP`]). To train only some of a militia, split them off first. After training, the barracks has to
//! rest for a day for every hundred members it trained (rounded up, so from 1 day to 10), and trains nothing meanwhile.

use macroquad::prelude::*;
use nworld_map::world::{Resource, World};
use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::city;
use crate::day::DaySave;
use crate::format;
use crate::ids::{BarracksId, CountryId, MilitiaId};
use crate::militia::Class;
use crate::newsreel::{Cost, Event, News};
use crate::ship::days_text;
use crate::transfer::Asset;

/// How close two barracks can be, in meters, so that each can be seen and pointed at.
pub const SPACING: f32 = 25.0;

/// What a training building is: a barracks, which trains soldiers, or an archery range, which trains archers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BarracksKind {
    /// Trains a militia into soldiers, for coins and iron.
    #[default]
    Barracks,
    /// Trains a militia into archers, for coins and wood (for the bows and arrows). Costs no iron to build.
    ArcheryRange,
    /// Trains a militia into cavalry, for more coins and iron (for the lances and armor). Cavalry move twice as far in a day.
    Stable,
}

impl BarracksKind {
    /// Every kind, in the order of the Buildings menu.
    pub const ALL: [BarracksKind; 3] = [BarracksKind::Barracks, BarracksKind::ArcheryRange, BarracksKind::Stable];

    /// What it is called, to begin a sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Barracks => "Barracks",
            Self::ArcheryRange => "Archery range",
            Self::Stable => "Stable",
        }
    }

    /// What it is called in the middle of one.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Barracks => "barracks",
            Self::ArcheryRange => "archery range",
            Self::Stable => "stable",
        }
    }

    /// What it is called with "a" before it.
    pub fn a_noun(self) -> &'static str {
        match self {
            Self::Barracks => "a barracks",
            Self::ArcheryRange => "an archery range",
            Self::Stable => "a stable",
        }
    }

    /// What it is called with "A" before it, to begin a sentence.
    pub fn a_noun_capital(self) -> &'static str {
        match self {
            Self::Barracks => "A barracks",
            Self::ArcheryRange => "An archery range",
            Self::Stable => "A stable",
        }
    }

    /// What many of them are called.
    pub fn plural(self) -> &'static str {
        match self {
            Self::Barracks => "barracks",
            Self::ArcheryRange => "archery ranges",
            Self::Stable => "stables",
        }
    }

    /// What it trains a militia into.
    pub fn trains_into(self) -> &'static str {
        match self {
            Self::Barracks => "soldiers",
            Self::ArcheryRange => "archers",
            Self::Stable => "cavalry",
        }
    }

    /// What it turns a militia into.
    pub fn class(self) -> Class {
        match self {
            Self::Barracks => Class::Soldiers,
            Self::ArcheryRange => Class::Archers,
            Self::Stable => Class::Cavalry,
        }
    }

    /// What it costs to build, in coins.
    pub fn cost(self) -> i64 {
        match self {
            Self::Barracks => COST,
            Self::ArcheryRange => ARCHERY_COST,
            Self::Stable => STABLE_COST,
        }
    }

    /// What it takes to build besides coins.
    pub fn materials(self) -> &'static [(Resource, u64)] {
        match self {
            Self::Barracks => &MATERIALS,
            Self::ArcheryRange => &ARCHERY_MATERIALS,
            Self::Stable => &STABLE_MATERIALS,
        }
    }

    /// What training takes besides coins: iron for the weapons and armor of soldiers, wood for the bows and arrows of archers.
    pub fn training_resource(self) -> Resource {
        match self {
            Self::Barracks => Resource::Iron,
            Self::ArcheryRange => Resource::Forest,
            Self::Stable => Resource::Iron,
        }
    }

    /// What training one member costs in coins.
    pub fn training_coins_each(self) -> i64 {
        match self {
            Self::Stable => STABLE_TRAINING_COINS,
            _ => TRAINING_COINS,
        }
    }

    /// What training `members` members costs in coins.
    pub fn training_coins(self, members: u32) -> i64 {
        i64::from(members) * self.training_coins_each()
    }

    /// How much of it each member takes.
    pub fn training_each(self) -> u64 {
        match self {
            Self::Barracks => TRAINING_IRON,
            Self::ArcheryRange => TRAINING_WOOD,
            Self::Stable => STABLE_TRAINING_IRON,
        }
    }

    /// How much of it training `members` members takes.
    pub fn training_materials(self, members: u32) -> u64 {
        u64::from(members) * self.training_each()
    }

    /// Everything building one costs, for the newsreel: coins first.
    pub fn full_cost(self) -> Vec<Cost> {
        let mut cost = vec![Cost::coins(self.cost())];
        cost.extend(self.materials().iter().map(|(resource, amount)| Cost { asset: Asset::of(*resource), amount: *amount }));
        cost
    }

    /// Everything training `members` members costs, for the newsreel: coins first, and then what else it takes.
    pub fn training_cost(self, members: u32) -> Vec<Cost> {
        vec![Cost::coins(self.training_coins(members)), Cost { asset: Asset::of(self.training_resource()), amount: self.training_materials(members) }]
    }
}

/// What a barracks costs to build in coins, and what it takes to build besides coins. It is a land building and smaller than a
/// shipyard, so it is half the price in coins and stone and wood, and takes less than half the iron.
pub const COST: i64 = 5_000;
pub const MATERIALS: [(Resource, u64); 3] = [(Resource::Stone, 500), (Resource::Forest, 500), (Resource::Iron, 200)];

/// What an archery range costs to build in coins, and what it takes besides coins: it needs a lot of timber and some stone, and no
/// iron, so a country with no iron can still have archers.
pub const ARCHERY_COST: i64 = 5_000;
pub const ARCHERY_MATERIALS: [(Resource, u64); 2] = [(Resource::Forest, 800), (Resource::Stone, 200)];

/// What training one member costs, in coins and in iron: twice what raising a militia member costs in coins, and an iron weapon
/// and armor besides. An archer costs the same coins, and two wood for a bow and arrows instead of the iron.
pub const TRAINING_COINS: i64 = 20;
pub const TRAINING_IRON: u64 = 1;
pub const TRAINING_WOOD: u64 = 2;

/// What a stable costs to build in coins, and what it takes besides coins: the most of the training buildings, for the paddocks and
/// the stalls and the smithy.
pub const STABLE_COST: i64 = 6_000;
pub const STABLE_MATERIALS: [(Resource, u64); 3] = [(Resource::Forest, 600), (Resource::Stone, 300), (Resource::Iron, 100)];
/// What training one member as cavalry costs: twice the coins of a soldier, and twice the iron, for the horse's shoes and the lance and
/// armor.
pub const STABLE_TRAINING_COINS: i64 = 40;
pub const STABLE_TRAINING_IRON: u64 = 2;
/// How many members a barracks can train for each day it then rests.
pub const MEMBERS_PER_REST_DAY: u32 = 100;

/// How many days a barracks has to rest after training `members` members: one for each hundred, rounded up, and at least one.
pub fn rest_days(members: u32) -> u32 {
    members.div_ceil(MEMBERS_PER_REST_DAY).max(1)
}

/// Draws the icon of a training building of `kind`, centered on `c` and about 16 pixels across times `s`, in `color`, with `accent`
/// for what is marked on it: a barracks is a little fort with three battlements and a door, and an archery range is a target with
/// an arrow in it.
pub fn draw_icon(kind: BarracksKind, c: Vec2, s: f32, color: Color, accent: Color) {
    match kind {
        BarracksKind::Barracks => {
            draw_rectangle(c.x - 7.0 * s, c.y - 2.0 * s, 14.0 * s, 9.0 * s, color);
            for x in [-7.0, -1.5, 4.0] {
                draw_rectangle(c.x + x * s, c.y - 6.0 * s, 3.0 * s, 4.5 * s, color);
            }
            draw_rectangle(c.x - 2.0 * s, c.y + 1.5 * s, 4.0 * s, 5.5 * s, accent);
        }
        BarracksKind::Stable => {
            // A horseshoe: a ring open at the bottom, with its nail holes.
            draw_circle(c.x, c.y - 0.5 * s, 7.5 * s, color);
            draw_circle(c.x, c.y - 0.5 * s, 4.2 * s, accent);
            draw_rectangle(c.x - 3.0 * s, c.y + 1.0 * s, 6.0 * s, 8.0 * s, accent);
            for (x, y) in [(-6.0, -3.0), (6.0, -3.0), (-5.0, 2.0), (5.0, 2.0)] {
                draw_circle(c.x + x * s, c.y + y * s, 0.9 * s, accent);
            }
        }
        BarracksKind::ArcheryRange => {
            draw_circle(c.x, c.y, 7.5 * s, color);
            draw_circle(c.x, c.y, 5.0 * s, accent);
            draw_circle(c.x, c.y, 3.2 * s, color);
            draw_circle(c.x, c.y, 1.4 * s, accent);
            // The arrow, in the target from the top right, with its feathers.
            draw_line(c.x + 0.5 * s, c.y - 0.5 * s, c.x + 8.0 * s, c.y - 7.5 * s, 1.4 * s, color);
            draw_line(c.x + 6.0 * s, c.y - 7.5 * s, c.x + 8.5 * s, c.y - 8.0 * s, 1.4 * s, color);
            draw_line(c.x + 7.5 * s, c.y - 5.5 * s, c.x + 8.5 * s, c.y - 8.0 * s, 1.4 * s, color);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Barracks {
    pub id: BarracksId,
    /// Whether it is a barracks or an archery range. One saved before there were archery ranges is a barracks.
    #[serde(default)]
    pub kind: BarracksKind,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in, if the map has regions.
    #[serde(default)]
    pub region: Option<u16>,
    /// The first day it can train again, if it is resting after training. One that has never trained, or whose rest is over, can
    /// train now.
    #[serde(default)]
    pub ready_on: Option<Date>,
}

impl Barracks {
    /// How many days it still has to rest on `today`: 0 if it can train now.
    pub fn rest_left(&self, today: Date) -> u32 {
        self.ready_on.map_or(0, |ready| today.days_until(ready))
    }
}

/// Why a barracks can't be built at `at`, or the number of the region it would be in (`None` if the map has none): it has to be
/// on the map, on dry land and not up in the mountains.
pub fn site_region(world: &World, at: Vec2) -> Result<Option<u16>, String> {
    if let Some(problem) = city::site_problem(world, at.x, at.y) {
        return Err(match problem {
            "That is off the map." => problem.to_owned(),
            "Cities can only be built on dry land." => "A barracks has to stand on dry land.".to_owned(),
            _ => "That is too high in the mountains to build a barracks.".to_owned(),
        });
    }
    match world.region_at(at.x, at.y) {
        Some(region) => Ok(Some(region.raw())),
        None => Err("This map has no regions to put a barracks in.".to_owned()),
    }
}

impl DaySave {
    /// The barracks with this id.
    pub fn find_barracks(&self, id: BarracksId) -> Option<&Barracks> {
        self.barracks.iter().find(|b| b.id == id)
    }

    /// Why a training building of `kind` can't be built now, or `None` if one can: the country being played has to be there and able
    /// to pay for it, in coins first, and then in what else it takes.
    pub fn barracks_problem(&self, kind: BarracksKind) -> Option<String> {
        if self.player_country().is_none() {
            return Some(format!("There is no country yet to own {}.", kind.a_noun()));
        }
        self.payment_problem(kind.a_noun(), kind.cost(), kind.materials())
    }

    /// Why the country being played can't have a barracks at (`x`, `z`) in the region numbered `region`, or `None` if it can: the
    /// region has to be the country's, and the barracks can't be on top of another (of either kind).
    pub fn barracks_place_problem(&self, x: f32, z: f32, region: Option<u16>) -> Option<String> {
        let country = self.player_country()?.id;
        self.placement_region_problem(country, region).or_else(|| {
            self.barracks.iter().find(|b| ((b.x - x).powi(2) + (b.z - z).powi(2)).sqrt() < SPACING).map(|b| format!("That is too close to another {}.", b.kind.noun()))
        })
    }

    /// Builds a training building of `kind` at (`x`, `z`) in the region numbered `region` (`None` if the map has none) for the country
    /// being played, which pays for it, and reports it in the newsreel. The region has to be the country's already. Whether the place
    /// is fit ground (see [`site_region`]) is for the caller to say. An error says why it can't be done, and nothing changes.
    pub fn build_barracks(&mut self, kind: BarracksKind, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.barracks_problem(kind) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.barracks_place_problem(x, z, region) {
            return Err(problem);
        }
        let country = self.player_country().ok_or_else(|| format!("There is no country yet to own {}.", kind.a_noun()))?.id;
        self.add_money(country, -kind.cost())?;
        for (resource, needed) in kind.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        let id = BarracksId::new();
        self.barracks.push(Barracks { id, kind, country, x, z, region, ready_on: None });
        self.newsreel.push(News::about(id, Event::BarracksBuilt { kind, country, region, cost: kind.full_cost() }));
        Ok(())
    }

    /// Destroys the barracks with this id, which costs nothing and pays nothing back, and reports it in the newsreel. An error says
    /// why it can't be done.
    pub fn destroy_barracks(&mut self, id: BarracksId) -> Result<(), String> {
        let Barracks { kind, country, region, .. } = *self.find_barracks(id).ok_or("There is no such barracks to destroy.")?;
        self.barracks.retain(|b| b.id != id);
        self.newsreel.push(News::about(id, Event::BarracksDestroyed { kind, country, region }));
        Ok(())
    }

    /// Why the barracks with this id can't train the militia with this id now, or `None` if it can. `region` is the number of the
    /// map region the militia stands in. The barracks has to belong to the country being played, which pays; the militia has to be
    /// that country's, not trained already (as soldiers or as archers), and in the same region as the barracks; the barracks can't be
    /// resting; and the country has to be able to pay, in coins first, and then in iron (or wood, at an archery range).
    pub fn train_problem(&self, barracks: BarracksId, militia: MilitiaId, region: Option<u16>) -> Option<String> {
        let Some(found) = self.find_barracks(barracks) else { return Some("There is no such barracks.".to_owned()) };
        let Some(unit) = self.militia(militia) else { return Some("There is no such militia.".to_owned()) };
        let Some(player) = self.player_country() else { return Some("There is no country yet to pay for training.".to_owned()) };
        let noun = found.kind.noun();
        if found.country != player.id {
            return Some(format!("This {noun} belongs to {}, and only {} can train {} there.", self.country_name(found.country), self.country_name(found.country), found.kind.trains_into()));
        }
        if unit.country != found.country {
            return Some(format!("{} belongs to {}, and this {noun} to {}.", unit.title(), self.country_name(unit.country), self.country_name(found.country)));
        }
        if unit.in_fort.is_some() {
            return Some(format!("{} is in a fort, and has to march out before it can be trained.", unit.title()));
        }
        if unit.trained {
            return Some(format!("{} is {} already.", unit.title(), unit.kind_label().to_lowercase()));
        }
        if found.region.is_none() || region != found.region {
            return Some(format!("{} is not in the same region as this {noun}.", unit.title()));
        }
        let left = found.rest_left(self.date);
        if left > 0 {
            return Some(format!("The {noun} is resting after training, and can train again in {}.", days_text(left)));
        }
        self.payment_problem(
            &format!("training {} members", format::thousands(unit.members)),
            found.kind.training_coins(unit.members),
            &[(found.kind.training_resource(), found.kind.training_materials(unit.members))],
        )
    }

    /// Trains all the members of the militia with this id, which stands in the map region numbered `region`, at the barracks with
    /// this id, for the country being played, which pays 20 coins and 1 iron (or 2 wood, at an archery range) for each of them.
    /// The militia is soldiers (or archers) from then on, and the barracks rests for a day for every hundred members. This is
    /// reported in the newsreel. An error says why it can't be done, and nothing changes.
    pub fn train_militia(&mut self, barracks: BarracksId, militia: MilitiaId, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.train_problem(barracks, militia, region) {
            return Err(problem);
        }
        let (kind, country) = self.find_barracks(barracks).map(|b| (b.kind, b.country)).ok_or("There is no such barracks.")?;
        let (name, members) = self.militia(militia).map(|m| (m.title().to_owned(), m.members)).ok_or("There is no such militia.")?;
        self.add_money(country, -kind.training_coins(members))?;
        self.add_resource(country, kind.training_resource(), -(kind.training_materials(members) as i64))?;
        if let Some(unit) = self.militias.iter_mut().find(|m| m.id == militia) {
            unit.set_class(kind.class());
        }
        let ready = self.date.plus_days(rest_days(members));
        if let Some(found) = self.barracks.iter_mut().find(|b| b.id == barracks) {
            found.ready_on = Some(ready);
        }
        self.newsreel.push(News::about(militia, Event::MilitiaTrained { name, kind, country, members, region, cost: kind.training_cost(members) }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;
    use nworld_map::world::WaterClass;

    /// A day with Aldoria, which has 100,000 coins and 5,000 of every resource, a city of 5,000 people in region 0x4005, and Brevon.
    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 100_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 5_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        day.cities[0].population = 5_000;
        day
    }

    const CAMP: Vec2 = Vec2::new(40.0, -10.0);
    const HERE: Option<u16> = Some(0x4005);

    /// A day with a barracks and a militia of `members` standing in the barracks' region.
    fn day_with_a_militia(members: u32) -> (DaySave, BarracksId, MilitiaId) {
        let mut day = day();
        day.build_barracks(BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap();
        day.raise_militia(day.cities[0].id, members, CAMP).unwrap();
        (day.clone(), day.barracks[0].id, day.militias[0].id)
    }

    #[test]
    fn a_barracks_costs_5000_coins_500_stone_500_wood_and_200_iron() {
        assert_eq!((COST, MATERIALS), (5_000, [(Resource::Stone, 500), (Resource::Forest, 500), (Resource::Iron, 200)]));
        let mut day = day();
        let aldoria = day.countries[0].id;
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        day.build_barracks(BarracksKind::Barracks, 100.0, 20.0, HERE).unwrap();

        let barracks = day.barracks[0];
        assert_eq!((barracks.country, barracks.x, barracks.z, barracks.region, barracks.ready_on), (aldoria, 100.0, 20.0, HERE, None));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 5_000);
        assert_eq!((country.stock.stone, country.stock.wood, country.stock.iron), (stock.stone - 500, stock.wood - 500, stock.iron - 200));
        assert_eq!((country.stock.copper, country.stock.sulphur, country.stock.oil), (stock.copper, stock.sulphur, stock.oil), "nothing else is taken");
        assert_eq!(day.find_barracks(barracks.id), Some(&barracks));
        assert_eq!(day.newsreel.last(), Some(&News::about(barracks.id, Event::BarracksBuilt { kind: BarracksKind::Barracks, country: aldoria, region: HERE, cost: BarracksKind::Barracks.full_cost() })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a barracks in region 5 for 5,000 coins, 500 stone, 500 wood and 200 iron");
        assert!(5_000 < crate::shipyard::COST && 200 < crate::shipyard::MATERIALS[2].1, "cheaper than a shipyard");
    }

    #[test]
    fn a_barracks_needs_the_coin_and_each_material_and_a_region_of_its_own_and_room() {
        let mut broke = day();
        let id = broke.countries[0].id;
        let money = broke.country(id).unwrap().money;
        broke.add_money(id, -(money - 4_999)).unwrap();
        let err = broke.build_barracks(BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("a barracks costs 5,000") && err.contains("4,999"), "{err}");

        for (resource, name, needed) in [(Resource::Stone, "stone", 500), (Resource::Forest, "wood", 500), (Resource::Iron, "iron", 200)] {
            let mut day = day();
            let id = day.countries[0].id;
            let has = day.country(id).unwrap().stock.get(resource);
            day.add_resource(id, resource, -(has as i64 - needed as i64 + 1)).unwrap(); // One short.
            let before = day.clone();
            let err = day.build_barracks(BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")), "{err}");
            assert_eq!(day, before, "a refused barracks changes nothing");
        }

        let mut day = day();
        let brevon = day.countries[1].id;
        day.set_region_owner(0x4009, Some(brevon)).unwrap();
        let before = day.clone();
        assert!(day.build_barracks(BarracksKind::Barracks, 300.0, 0.0, Some(0x4009)).unwrap_err().contains("belongs to Brevon"));
        assert!(day.build_barracks(BarracksKind::Barracks, 300.0, 0.0, Some(0x400A)).unwrap_err().contains("doesn't own this region"), "it has to be claimed first");
        assert!(day.build_barracks(BarracksKind::Barracks, 300.0, 0.0, None).unwrap_err().contains("no regions"));
        assert!(day.build_barracks(BarracksKind::Barracks, f32::NAN, 0.0, HERE).is_err());
        assert_eq!(day, before, "nothing was taken for any of them");
        day.build_barracks(BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap();
        assert!(day.build_barracks(BarracksKind::Barracks, 110.0, 0.0, HERE).unwrap_err().contains("too close to another barracks"));
        day.build_barracks(BarracksKind::Barracks, 100.0 + SPACING, 0.0, HERE).unwrap();
        assert!(DaySave::new(Date::FIRST, "x").barracks_problem(BarracksKind::Barracks).unwrap().contains("no country"));
    }

    #[test]
    fn a_barracks_belongs_to_whoever_owns_its_region_and_goes_with_it_and_can_be_destroyed_for_nothing() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.claim_region(0x4008).unwrap();
        day.build_barracks(BarracksKind::Barracks, 300.0, 0.0, Some(0x4008)).unwrap();
        let before = day.clone();
        // It can't be left with nobody, though it can be given to another, which gets what is in it.
        assert!(day.set_region_owner(0x4008, None).unwrap_err().contains("no owner"));
        assert_eq!(day, before);
        day.set_region_owner(0x4008, Some(brevon)).unwrap();
        assert!(day.barracks.iter().any(|b| b.region == Some(0x4008) && b.country == brevon));
        day.set_region_owner(0x4008, Some(aldoria)).unwrap();
        // When the region changes hands with its city, the barracks goes with it.
        day.build_barracks(BarracksKind::Barracks, 100.0, 0.0, HERE).unwrap();
        day.set_city_country(day.cities[0].id, brevon).unwrap();
        assert!(day.barracks.iter().any(|b| b.region == HERE && b.country == brevon));
        assert_eq!(day.barracks.iter().find(|b| b.region == Some(0x4008)).unwrap().country, aldoria);

        let money = day.country(aldoria).unwrap().money;
        let id = day.barracks[0].id;
        day.destroy_barracks(id).unwrap();
        assert!(day.find_barracks(id).is_none() && day.country(aldoria).unwrap().money == money, "nothing is paid back");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's barracks in region 8 is destroyed");
        assert!(day.destroy_barracks(id).is_err());
        // A country that owns one can't be deleted.
        let problem = day.delete_problem(brevon).unwrap();
        assert!(problem.contains("1 barracks"), "{problem}");
    }

    #[test]
    fn training_costs_20_coins_and_1_iron_a_member_and_the_barracks_rests_a_day_for_every_hundred() {
        assert_eq!((BarracksKind::Barracks.training_coins(150), BarracksKind::Barracks.training_materials(150)), (3_000, 150));
        assert_eq!((TRAINING_COINS, TRAINING_IRON, TRAINING_COINS / crate::militia::RAISING_COST), (20, 1, 2));
        assert_eq!([1, 100, 101, 250, 999, 1_000].map(rest_days), [1, 1, 2, 3, 10, 10]);
        assert_eq!(rest_days(0), 1, "always at least a day");

        let (mut day, barracks, unit) = day_with_a_militia(150);
        let aldoria = day.countries[0].id;
        let (coins, iron) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock.iron);
        assert_eq!(day.train_problem(barracks, unit, HERE), None);
        day.train_militia(barracks, unit, HERE).unwrap();

        assert!(day.militia(unit).unwrap().trained);
        assert_eq!(day.militia(unit).unwrap().members, 150, "every member is trained, none are lost");
        assert_eq!(day.country(aldoria).unwrap().money, coins - 3_000);
        assert_eq!(day.country(aldoria).unwrap().stock.iron, iron - 150);
        let yard = *day.find_barracks(barracks).unwrap();
        assert_eq!((yard.rest_left(day.date), yard.ready_on), (2, Some(Date::FIRST.plus_days(2))), "150 members: 2 days");
        let name = day.militia(unit).unwrap().name.clone();
        assert_eq!(day.newsreel.last(), Some(&News::about(unit, Event::MilitiaTrained { name: name.clone(), kind: BarracksKind::Barracks, country: aldoria, members: 150, region: HERE, cost: BarracksKind::Barracks.training_cost(150) })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("Aldoria trains {name}, 150 members, as soldiers in region 5 for 3,000 coins and 150 iron"));
    }

    #[test]
    fn a_resting_barracks_trains_nothing_until_its_break_is_over() {
        let (mut day, barracks, first) = day_with_a_militia(250);
        day.train_militia(barracks, first, HERE).unwrap();
        // Another militia, raised the next day, can't be trained until the 3 days of rest are over.
        let mut today = day.next_day();
        today.cities[0].population = 5_000;
        today.raise_militia(today.cities[0].id, 100, vec2(60.0, 40.0)).unwrap();
        let second = today.militias[1].id;
        for left in [2, 1] {
            let before = today.clone();
            let err = today.train_militia(barracks, second, HERE).unwrap_err();
            assert!(err.contains("resting") && err.contains(&format!("in {}", days_text(left))), "{err}");
            assert_eq!(today, before);
            today = today.next_day();
        }
        assert_eq!(today.find_barracks(barracks).unwrap().rest_left(today.date), 0, "3 days after it can train again");
        today.train_militia(barracks, second, HERE).unwrap();
        assert!(today.militias.iter().all(|m| m.trained));
    }

    #[test]
    fn only_a_militia_of_the_same_country_in_the_same_region_that_is_not_soldiers_already_can_be_trained() {
        let (day, barracks, unit) = day_with_a_militia(100);
        // In another region.
        assert!(day.train_problem(barracks, unit, Some(0x4006)).unwrap().contains("not in the same region"));
        assert!(day.train_problem(barracks, unit, None).unwrap().contains("not in the same region"));
        // Of another country.
        let mut other = day.clone();
        let brevon = other.countries[1].id;
        other.set_militia_country(unit, brevon).unwrap();
        assert!(other.train_problem(barracks, unit, HERE).unwrap().contains("belongs to Brevon"));
        // Not the country being played: only it can pay.
        let mut yard = day.clone();
        yard.barracks[0].country = brevon;
        assert!(yard.train_problem(barracks, unit, HERE).unwrap().contains("This barracks belongs to Brevon"));
        // Soldiers already.
        let mut done = day.clone();
        done.train_militia(barracks, unit, HERE).unwrap();
        done.barracks[0].ready_on = None;
        assert!(done.train_problem(barracks, unit, HERE).unwrap().contains("is soldiers already"));
        // Things that aren't there.
        assert!(day.train_problem(BarracksId::new(), unit, HERE).unwrap().contains("no such barracks"));
        assert!(day.train_problem(barracks, MilitiaId::new(), HERE).unwrap().contains("no such militia"));
        let before = day.clone();
        let mut refused = day.clone();
        assert!(refused.train_militia(barracks, unit, Some(0x4006)).is_err());
        assert_eq!(refused, before, "a refused training changes nothing");
    }

    #[test]
    fn training_needs_the_coin_and_the_iron_and_says_which_is_short() {
        let (day, barracks, unit) = day_with_a_militia(200);
        let aldoria = day.countries[0].id;
        let mut broke = day.clone();
        let money = broke.country(aldoria).unwrap().money;
        broke.add_money(aldoria, -(money - 3_999)).unwrap(); // 200 members cost 4,000.
        let err = broke.train_militia(barracks, unit, HERE).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("training 200 members costs 4,000") && err.contains("3,999"), "{err}");

        let mut no_iron = day.clone();
        let iron = no_iron.country(aldoria).unwrap().stock.iron;
        no_iron.add_resource(aldoria, Resource::Iron, -(iron as i64 - 199)).unwrap();
        let before = no_iron.clone();
        let err = no_iron.train_militia(barracks, unit, HERE).unwrap_err();
        assert!(err.contains("Not enough iron") && err.contains("200 iron") && err.contains("has 199"), "{err}");
        assert_eq!(no_iron, before);
        no_iron.add_resource(aldoria, Resource::Iron, 1).unwrap();
        no_iron.train_militia(barracks, unit, HERE).unwrap();
    }

    #[test]
    fn soldiers_cost_three_coins_a_member_a_day_to_keep_and_can_only_be_combined_with_soldiers() {
        let (mut day, barracks, unit) = day_with_a_militia(100);
        assert_eq!(day.militia(unit).unwrap().upkeep(), 100);
        day.train_militia(barracks, unit, HERE).unwrap();
        assert_eq!(day.militia(unit).unwrap().upkeep(), 300, "3 coins a member");
        assert_eq!((day.militia(unit).unwrap().kind_label(), day.militia(unit).unwrap().upkeep_per_member()), ("Soldiers", 3));

        // They are paid for at that rate, and a country that can't pay for all of them loses the ones it can't.
        let aldoria = day.countries[0].id;
        let money = day.country(aldoria).unwrap().money;
        let mut paid = day.clone();
        paid.pay_militia();
        assert_eq!(paid.country(aldoria).unwrap().money, money - 300);
        let mut short = day.clone();
        short.add_money(aldoria, -(money - 160)).unwrap();
        short.pay_militia();
        assert_eq!(short.militia(unit).unwrap().members, 53, "160 coins pay for 53 soldiers at 3 each");

        // Militia and soldiers can't be combined, but soldiers and soldiers can.
        day.cities[0].population = 5_000;
        day.set_city_country(day.cities[0].id, aldoria).unwrap();
        let mut tomorrow = day.following(day.date.next());
        tomorrow.raise_militia(tomorrow.cities[0].id, 100, vec2(60.0, 40.0)).unwrap();
        let militia = tomorrow.militias[1].id;
        let err = tomorrow.merge_militia(unit, militia, (HERE, HERE)).unwrap_err();
        assert!(err.contains("one is soldiers") && err.contains("the other is militia"), "{err}");
        assert!(tomorrow.merge_partners(unit, &|_| HERE).is_empty() && tomorrow.merge_partners(militia, &|_| HERE).is_empty());
        tomorrow.barracks[0].ready_on = None;
        tomorrow.train_militia(barracks, militia, HERE).unwrap();
        tomorrow.merge_militia(unit, militia, (HERE, HERE)).unwrap();
        assert!(tomorrow.militia(unit).unwrap().trained && tomorrow.militia(unit).unwrap().members == 200);
        // And a detachment split off soldiers is soldiers.
        tomorrow.split_militia(unit, 50, vec2(0.0, 80.0)).unwrap();
        assert!(tomorrow.militias.iter().all(|m| m.trained));
    }

    #[test]
    fn barracks_and_trained_militia_are_saved_and_carry_forward_and_a_day_saved_before_them_has_none() {
        let (mut day, barracks, unit) = day_with_a_militia(100);
        day.train_militia(barracks, unit, HERE).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["militias"][0]["trained"], true);
        assert_eq!(json["barracks"][0]["region"], 0x4005);
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        let tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.barracks, day.barracks);
        assert!(tomorrow.militia(unit).unwrap().trained, "soldiers stay soldiers");
        let mut old = json;
        old.as_object_mut().unwrap().remove("barracks");
        old["militias"][0].as_object_mut().unwrap().remove("trained");
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert!(old.barracks.is_empty() && !old.militias[0].trained);
    }

    /// A day with an archery range and a militia of `members` standing in its region.
    fn day_with_a_range_and_a_militia(members: u32) -> (DaySave, BarracksId, MilitiaId) {
        let mut day = day();
        day.build_barracks(BarracksKind::ArcheryRange, 100.0, 0.0, HERE).unwrap();
        day.raise_militia(day.cities[0].id, members, CAMP).unwrap();
        (day.clone(), day.barracks[0].id, day.militias[0].id)
    }

    #[test]
    fn an_archery_range_costs_5000_coins_800_wood_and_200_stone_and_no_iron() {
        assert_eq!((ARCHERY_COST, ARCHERY_MATERIALS), (5_000, [(Resource::Forest, 800), (Resource::Stone, 200)]));
        let mut day = day();
        let aldoria = day.countries[0].id;
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        day.build_barracks(BarracksKind::ArcheryRange, 100.0, 20.0, HERE).unwrap();

        let range = day.barracks[0];
        assert_eq!((range.kind, range.country, range.region, range.ready_on), (BarracksKind::ArcheryRange, aldoria, HERE, None));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 5_000);
        assert_eq!((country.stock.wood, country.stock.stone, country.stock.iron), (stock.wood - 800, stock.stone - 200, stock.iron), "no iron is taken");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds an archery range in region 5 for 5,000 coins, 800 wood and 200 stone");

        // A country with too little wood can't build one, and says so.
        let mut poor = self::day();
        let aldoria = poor.countries[0].id;
        let wood = poor.country(aldoria).unwrap().stock.wood;
        poor.add_resource(aldoria, Resource::Forest, -(wood as i64 - 799)).unwrap();
        let before = poor.clone();
        assert!(poor.build_barracks(BarracksKind::ArcheryRange, 100.0, 0.0, HERE).unwrap_err().contains("Not enough wood"));
        assert_eq!(poor, before);

        // It is as close to a barracks as to another archery range.
        assert!(day.build_barracks(BarracksKind::Barracks, 110.0, 20.0, HERE).unwrap_err().contains("too close to another archery range"));
        day.build_barracks(BarracksKind::Barracks, 100.0 + SPACING, 20.0, HERE).unwrap();
        let err = day.build_barracks(BarracksKind::ArcheryRange, 110.0 + SPACING, 20.0, HERE).unwrap_err();
        assert!(err.contains("too close to another barracks"), "{err}");

        // Destroyed, it is reported as what it was.
        day.destroy_barracks(range.id).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's archery range in region 5 is destroyed");
    }

    #[test]
    fn an_archery_range_trains_a_militia_into_archers_for_20_coins_and_2_wood_a_member() {
        assert_eq!((BarracksKind::ArcheryRange.training_resource(), BarracksKind::ArcheryRange.training_materials(150)), (Resource::Forest, 300));
        let (mut day, range, unit) = day_with_a_range_and_a_militia(150);
        let aldoria = day.countries[0].id;
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        assert_eq!(day.train_problem(range, unit, HERE), None);
        day.train_militia(range, unit, HERE).unwrap();

        let archers = day.militia(unit).unwrap();
        assert!(archers.trained && archers.archers, "trained, as archers and not soldiers");
        assert_eq!((archers.kind_label(), archers.people_label(), archers.members, archers.upkeep_per_member(), archers.upkeep()), ("Archers", "Archers", 150, 4, 600));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 3_000);
        assert_eq!((country.stock.wood, country.stock.iron), (stock.wood - 300, stock.iron), "wood, and no iron");
        assert_eq!(day.find_barracks(range).unwrap().rest_left(day.date), 2, "it rests like a barracks, a day for every hundred");
        let name = day.militia(unit).unwrap().name.clone();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("Aldoria trains {name}, 150 members, as archers in region 5 for 3,000 coins and 300 wood"));

        // Without iron it can still train archers, and without wood it can't.
        let (mut no_iron, range, unit) = day_with_a_range_and_a_militia(100);
        let aldoria = no_iron.countries[0].id;
        let iron = no_iron.country(aldoria).unwrap().stock.iron;
        no_iron.add_resource(aldoria, Resource::Iron, -(iron as i64)).unwrap();
        no_iron.train_militia(range, unit, HERE).unwrap();
        let (mut no_wood, range, unit) = day_with_a_range_and_a_militia(100);
        let aldoria = no_wood.countries[0].id;
        let wood = no_wood.country(aldoria).unwrap().stock.wood;
        no_wood.add_resource(aldoria, Resource::Forest, -(wood as i64 - 199)).unwrap(); // 100 members need 200.
        let err = no_wood.train_militia(range, unit, HERE).unwrap_err();
        assert!(err.contains("Not enough wood") && err.contains("200 wood") && err.contains("has 199"), "{err}");
        assert!(!no_wood.militia(unit).unwrap().trained);
    }

    #[test]
    fn only_a_plain_militia_can_be_trained_so_soldiers_and_archers_are_not_trained_again() {
        let (mut day, range, unit) = day_with_a_range_and_a_militia(100);
        day.train_militia(range, unit, HERE).unwrap();
        day.barracks[0].ready_on = None;
        assert!(day.train_problem(range, unit, HERE).unwrap().contains("is archers already"));
        // Archers can't become soldiers.
        day.build_barracks(BarracksKind::Barracks, 100.0 + 2.0 * SPACING, 0.0, HERE).unwrap();
        let barracks = day.barracks[1].id;
        assert!(day.train_problem(barracks, unit, HERE).unwrap().contains("is archers already"));
        // And soldiers can't become archers.
        let (mut soldiers, barracks, unit) = day_with_a_militia(100);
        soldiers.train_militia(barracks, unit, HERE).unwrap();
        soldiers.build_barracks(BarracksKind::ArcheryRange, 100.0 + 2.0 * SPACING, 0.0, HERE).unwrap();
        let range = soldiers.barracks[1].id;
        assert!(soldiers.train_problem(range, unit, HERE).unwrap().contains("is soldiers already"));
        // The error says what kind of building it is.
        soldiers.barracks[1].country = soldiers.countries[1].id;
        assert!(soldiers.train_problem(range, unit, HERE).unwrap().contains("This archery range belongs to Brevon, and only Brevon can train archers there"));
    }

    #[test]
    fn archers_combine_only_with_archers_and_a_split_off_detachment_is_archers() {
        let (mut day, range, unit) = day_with_a_range_and_a_militia(100);
        day.train_militia(range, unit, HERE).unwrap();
        let mut tomorrow = day.following(day.date.next());
        tomorrow.cities[0].population = 5_000;
        tomorrow.raise_militia(tomorrow.cities[0].id, 100, vec2(60.0, 40.0)).unwrap();
        let militia = tomorrow.militias[1].id;
        let err = tomorrow.merge_militia(unit, militia, (HERE, HERE)).unwrap_err();
        assert!(err.contains("one is archers") && err.contains("the other is militia"), "{err}");
        // Soldiers and archers are not the same either.
        tomorrow.militias[1].trained = true;
        let err = tomorrow.merge_militia(unit, militia, (HERE, HERE)).unwrap_err();
        assert!(err.contains("one is archers") && err.contains("the other is soldiers"), "{err}");
        assert!(tomorrow.merge_partners(unit, &|_| HERE).is_empty());
        tomorrow.militias[1].archers = true;
        assert_eq!(tomorrow.merge_partners(unit, &|_| HERE).len(), 1);
        tomorrow.merge_militia(unit, militia, (HERE, HERE)).unwrap();
        assert!(tomorrow.militia(unit).unwrap().archers && tomorrow.militia(unit).unwrap().members == 200);
        tomorrow.split_militia(unit, 50, vec2(0.0, 80.0)).unwrap();
        assert!(tomorrow.militias.iter().all(|m| m.trained && m.archers));
    }

    #[test]
    fn archery_ranges_and_archers_are_saved_and_a_day_saved_before_them_has_neither() {
        let (mut day, range, unit) = day_with_a_range_and_a_militia(100);
        day.train_militia(range, unit, HERE).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!((json["barracks"][0]["kind"].as_str(), json["militias"][0]["archers"].as_bool()), (Some("ArcheryRange"), Some(true)));
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        let tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert!(tomorrow.barracks[0].kind == BarracksKind::ArcheryRange && tomorrow.militia(unit).unwrap().archers);
        // A save from before archery ranges: a barracks, and trained militia that are soldiers.
        let mut old = json;
        old["barracks"][0].as_object_mut().unwrap().remove("kind");
        old["militias"][0].as_object_mut().unwrap().remove("archers");
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert_eq!(old.barracks[0].kind, BarracksKind::Barracks);
        assert!(old.militias[0].trained && !old.militias[0].archers, "trained, so soldiers");
    }

    /// A day with a stable and a militia of `members` standing in its region.
    fn day_with_a_stable_and_a_militia(members: u32) -> (DaySave, BarracksId, MilitiaId) {
        let mut day = day();
        day.build_barracks(BarracksKind::Stable, 100.0, 0.0, HERE).unwrap();
        day.raise_militia(day.cities[0].id, members, CAMP).unwrap();
        (day.clone(), day.barracks[0].id, day.militias[0].id)
    }

    #[test]
    fn a_stable_costs_6000_coins_600_wood_300_stone_and_100_iron() {
        assert_eq!((STABLE_COST, STABLE_MATERIALS), (6_000, [(Resource::Forest, 600), (Resource::Stone, 300), (Resource::Iron, 100)]));
        let mut day = day();
        let aldoria = day.countries[0].id;
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        day.build_barracks(BarracksKind::Stable, 100.0, 20.0, HERE).unwrap();
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 6_000);
        assert_eq!((country.stock.wood, country.stock.stone, country.stock.iron), (stock.wood - 600, stock.stone - 300, stock.iron - 100));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a stable in region 5 for 6,000 coins, 600 wood, 300 stone and 100 iron");
        assert!(STABLE_COST > COST && STABLE_COST > ARCHERY_COST, "the dearest of the three");
        day.destroy_barracks(day.barracks[0].id).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's stable in region 5 is destroyed");
        assert!(day.barracks.is_empty());
    }

    #[test]
    fn a_stable_trains_a_militia_into_cavalry_for_40_coins_and_2_iron_a_member() {
        assert_eq!((BarracksKind::Stable.training_coins(150), BarracksKind::Stable.training_materials(150), BarracksKind::Stable.training_resource()), (6_000, 300, Resource::Iron));
        let (mut day, stable, unit) = day_with_a_stable_and_a_militia(150);
        let aldoria = day.countries[0].id;
        let (coins, iron) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock.iron);
        assert_eq!(day.train_problem(stable, unit, HERE), None);
        day.train_militia(stable, unit, HERE).unwrap();

        let cavalry = day.militia(unit).unwrap();
        assert_eq!(cavalry.class(), Class::Cavalry);
        assert!(cavalry.trained && cavalry.cavalry && !cavalry.archers);
        assert_eq!((cavalry.kind_label(), cavalry.people_label(), cavalry.upkeep_per_member(), cavalry.upkeep()), ("Cavalry", "Riders", 6, 900));
        assert_eq!(day.country(aldoria).unwrap().money, coins - 6_000);
        assert_eq!(day.country(aldoria).unwrap().stock.iron, iron - 300);
        assert_eq!(day.find_barracks(stable).unwrap().rest_left(day.date), 2);
        let name = day.militia(unit).unwrap().name.clone();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("Aldoria trains {name}, 150 members, as cavalry in region 5 for 6,000 coins and 300 iron"));

        // Cavalry is never trained again as anything else, and nor is anything else as cavalry.
        day.barracks[0].ready_on = None;
        assert!(day.train_problem(stable, unit, HERE).unwrap().contains("is cavalry already"));
    }

    #[test]
    fn cavalry_get_twice_the_movement_points_each_day_and_only_combine_with_cavalry() {
        let (mut day, stable, unit) = day_with_a_stable_and_a_militia(100);
        assert_eq!(day.militia(unit).unwrap().daily_points(), crate::movement::DAILY_POINTS, "plain militia until trained");
        day.train_militia(stable, unit, HERE).unwrap();
        assert_eq!(day.militia(unit).unwrap().daily_points(), 2 * crate::movement::DAILY_POINTS);
        // A new day gives them all 400, a garrison none, and ending their moves takes them away until the next day.
        let mut tomorrow = day.following(day.date.next());
        tomorrow.rest_militia();
        assert_eq!(tomorrow.militia(unit).unwrap().points, 400);
        tomorrow.end_militia_moves(unit).unwrap();
        assert_eq!(tomorrow.militia(unit).unwrap().points, 0);
        tomorrow.rest_militia();
        assert_eq!(tomorrow.militia(unit).unwrap().points, 400, "back the next day");
        tomorrow.set_militia_garrison(unit, true).unwrap();
        tomorrow.rest_militia();
        assert_eq!(tomorrow.militia(unit).unwrap().points, 0, "a garrison gets none");

        // Another group of cavalry can be combined with it, and soldiers and militia can't.
        tomorrow.set_militia_garrison(unit, false).unwrap();
        tomorrow.cities[0].population = 5_000;
        tomorrow.raise_militia(tomorrow.cities[0].id, 100, vec2(60.0, 40.0)).unwrap();
        let other = tomorrow.militias[1].id;
        let err = tomorrow.merge_militia(unit, other, (HERE, HERE)).unwrap_err();
        assert!(err.contains("one is cavalry") && err.contains("the other is militia"), "{err}");
        tomorrow.militias[1].set_class(Class::Soldiers);
        assert!(tomorrow.merge_partners(unit, &|_| HERE).is_empty());
        tomorrow.militias[1].set_class(Class::Cavalry);
        assert_eq!(tomorrow.merge_partners(unit, &|_| HERE).len(), 1);
        tomorrow.merge_militia(unit, other, (HERE, HERE)).unwrap();
        assert!(tomorrow.militia(unit).unwrap().cavalry && tomorrow.militia(unit).unwrap().members == 200);
        tomorrow.split_militia(unit, 50, vec2(0.0, 80.0)).unwrap();
        assert!(tomorrow.militias.iter().all(|m| m.class() == Class::Cavalry));
    }

    #[test]
    fn cavalry_and_stables_are_saved_and_a_day_saved_before_them_has_neither() {
        let (mut day, stable, unit) = day_with_a_stable_and_a_militia(100);
        day.train_militia(stable, unit, HERE).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!((json["barracks"][0]["kind"].as_str(), json["militias"][0]["cavalry"].as_bool()), (Some("Stable"), Some(true)));
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        let mut old = json;
        old["militias"][0].as_object_mut().unwrap().remove("cavalry");
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert_eq!(old.militias[0].class(), Class::Soldiers, "trained, with neither flag: soldiers");
    }

    #[test]
    fn a_barracks_stands_on_dry_land_that_is_not_in_the_mountains() {
        let world = test_support::regioned_continent();
        let (land, sea) = test_support::land_and_sea(&world);
        assert!(site_region(&world, sea).unwrap_err().contains("dry land"));
        assert!(site_region(&world, vec2(f32::MAX / 4.0, 0.0)).unwrap_err().contains("off the map"));
        let inland = test_support::inland_spot(&world, 150.0);
        assert_eq!(world.water_class(inland.x, inland.y), WaterClass::Dry);
        assert!(site_region(&world, inland).is_ok(), "inland ground is fine: a barracks need not be on a coast");
        let _ = land;
    }
}
