//! Forts: a stronghold of stone and wood, in one of three sizes. A fort belongs to whichever country owns the region it stands
//! in, which is the country that built it, and which has to own the region to build there; it pays for it in coins, stone and
//! wood. It can't be given to another country: only the region changes hands, and the fort goes with it. It stands on dry
//! land that isn't up in the mountains. Only a small one can be built; it can then be upgraded to a medium one, and that to a
//! large one, for more of each every time.
//!
//! A fort can be given a name, and is called by its size until it has one.
//!
//! A fort holds a **garrison**: militia (of any kind) of its country that stand in the same map region as it can go into it, as
//! many members as there is room for, and a bigger fort has room for more (see [`FortSize::capacity`]). A militia in a fort is
//! where the fort is and isn't shown for itself, and it gets no movement points and can't move. It comes out to where it went in
//! from when it is marched out, and is turned out if the fort is destroyed or stops being its country's.
//!
//! A fort can be put **under siege**, which the player switches on and off. It then lives on its **supplies**: each day every
//! member of its garrison eats one, so a bigger garrison runs through them faster, and a bigger fort holds more (see
//! [`FortSize::supplies`]). Militia can still march into and out of it while the siege lasts. While there are any left the garrison loses nobody; once they are gone, each militia in it loses
//! members as a militia under siege does. Lifting the siege fills the supplies up again. A fort can't be upgraded while it is
//! under siege.

use macroquad::prelude::*;
use nworld_map::world::{Resource, World};
use serde::{Deserialize, Serialize};

use crate::city;
use crate::day::DaySave;
use crate::format;
use crate::ids::{CountryId, FortId, MilitiaId};
use crate::militia::{InFort, Militia};
use crate::newsreel::{Cost, Event, News};
use crate::text;
use crate::transfer::Asset;

/// How close two forts can be, in meters, so that each can be seen and pointed at.
pub const SPACING: f32 = 25.0;
/// The longest a fort's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// A fort name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your fort")
}

/// What a small fort costs to build in coins, and what upgrading to a medium one and then to a large one costs.
pub const COST: i64 = 5_000;
pub const MEDIUM_COST: i64 = 10_000;
pub const LARGE_COST: i64 = 20_000;

/// What a small fort takes to build besides coins.
pub const MATERIALS: [(Resource, u64); 2] = [(Resource::Stone, 500), (Resource::Forest, 500)];
/// What upgrading a small fort to a medium one takes.
pub const MEDIUM_MATERIALS: [(Resource, u64); 2] = [(Resource::Stone, 1_000), (Resource::Forest, 750)];
/// What upgrading a medium fort to a large one takes.
pub const LARGE_MATERIALS: [(Resource, u64); 2] = [(Resource::Stone, 2_000), (Resource::Forest, 1_000)];

/// How big a fort is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FortSize {
    Small,
    Medium,
    Large,
}

impl FortSize {
    /// Every size, the smallest first.
    pub const ALL: [FortSize; 3] = [FortSize::Small, FortSize::Medium, FortSize::Large];

    /// What it can be upgraded to, if it can be.
    pub fn upgrade(self) -> Option<FortSize> {
        match self {
            Self::Small => Some(Self::Medium),
            Self::Medium => Some(Self::Large),
            Self::Large => None,
        }
    }

    /// The most militia members its garrison can have, all its militia together.
    pub fn capacity(self) -> u32 {
        match self {
            Self::Small => 100,
            Self::Medium => 500,
            Self::Large => 1_000,
        }
    }

    /// The supplies it holds when full: what one member of its garrison eats in a day is one.
    pub fn supplies(self) -> u32 {
        match self {
            Self::Small => 500,
            Self::Medium => 4_000,
            Self::Large => 10_000,
        }
    }

    /// What making a fort of this size costs in coins: building it, for a small one, and else upgrading to it from the size below.
    pub fn cost(self) -> i64 {
        match self {
            Self::Small => COST,
            Self::Medium => MEDIUM_COST,
            Self::Large => LARGE_COST,
        }
    }

    /// What it takes besides coins to make a fort of this size: to build it, for a small one, and else to upgrade to it from the size below.
    pub fn materials(self) -> &'static [(Resource, u64)] {
        match self {
            Self::Small => &MATERIALS,
            Self::Medium => &MEDIUM_MATERIALS,
            Self::Large => &LARGE_MATERIALS,
        }
    }

    /// Everything it takes, for the newsreel and for saying it: coins first, and then each resource.
    pub fn full_cost(self) -> Vec<Cost> {
        let mut cost = vec![Cost::coins(self.cost())];
        cost.extend(self.materials().iter().map(|(resource, amount)| Cost { asset: Asset::of(*resource), amount: *amount }));
        cost
    }

    /// What to call one, in lower case.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Small => "small fort",
            Self::Medium => "medium fort",
            Self::Large => "large fort",
        }
    }

    /// What to call one with "a" before it.
    pub fn a_noun(self) -> String {
        format!("a {}", self.noun())
    }

    /// [`a_noun`](Self::a_noun) to start a sentence.
    pub fn a_noun_capital(self) -> String {
        format::capitalized(&self.a_noun())
    }

    /// What to call it where a name starts something.
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small fort",
            Self::Medium => "Medium fort",
            Self::Large => "Large fort",
        }
    }

    /// Its size on its own, to start something.
    pub fn size_label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    /// Draws its icon, centered on `c` and about 16 pixels across times `s`, in `color`, with `accent` for what is marked on it:
    /// a shield with a bar for each size.
    pub fn draw_icon(self, c: Vec2, s: f32, color: Color, accent: Color) {
        draw_rectangle(c.x - 7.0 * s, c.y - 8.0 * s, 14.0 * s, 8.0 * s, color);
        draw_triangle(vec2(c.x - 7.0 * s, c.y), vec2(c.x + 7.0 * s, c.y), vec2(c.x, c.y + 8.0 * s), color);
        let bars = match self {
            Self::Small => 1,
            Self::Medium => 2,
            Self::Large => 3,
        };
        for k in 0..bars {
            draw_rectangle(c.x - 4.0 * s, c.y + (-6.0 + 3.0 * k as f32) * s, 8.0 * s, 1.6 * s, accent);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Fort {
    pub id: FortId,
    /// What it is called, if it has been given a name.
    #[serde(default)]
    pub name: String,
    pub size: FortSize,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in, if the map has regions.
    #[serde(default)]
    pub region: Option<u16>,
    /// Whether it is surrounded. The player switches it on and off.
    #[serde(default)]
    pub under_siege: bool,
    /// How many of its supplies have been eaten in the siege it is under: none when it is under none.
    #[serde(default)]
    pub supplies_used: u32,
}

impl Fort {
    /// What to call it, to start something: its name, or its size if it has none.
    pub fn title(&self) -> &str {
        if self.name.is_empty() { self.size.label() } else { &self.name }
    }

    /// The supplies it has left.
    pub fn supplies_left(&self) -> u32 {
        self.size.supplies().saturating_sub(self.supplies_used)
    }

    /// The supplies it has left as a percent of what it holds, rounded up, so that it says 0 only when there are none.
    pub fn supplies_percent(&self) -> u32 {
        (u64::from(self.supplies_left()) * 100).div_ceil(u64::from(self.size.supplies())) as u32
    }

    /// Whether it is under siege with nothing left to eat, so that its garrison is losing members.
    pub fn starved(&self) -> bool {
        self.under_siege && self.supplies_left() == 0
    }

    /// How many days the supplies left would feed `members`, the last of them perhaps not in full, or `None` if there is nobody.
    pub fn days_of_supplies(&self, members: u32) -> Option<u32> {
        (members > 0).then(|| self.supplies_left().div_ceil(members))
    }
}

/// Why a fort can't be built at `at`, or the number of the region it would be in (`None` if the map has none): it has to be on
/// the map, on dry land and not up in the mountains.
pub fn site_region(world: &World, at: Vec2) -> Result<Option<u16>, String> {
    if let Some(problem) = city::site_problem(world, at.x, at.y) {
        return Err(match problem {
            "That is off the map." => problem.to_owned(),
            "Cities can only be built on dry land." => "A fort has to stand on dry land.".to_owned(),
            _ => "That is too high in the mountains to build a fort.".to_owned(),
        });
    }
    match world.region_at(at.x, at.y) {
        Some(region) => Ok(Some(region.raw())),
        None => Err("This map has no regions to put a fort in.".to_owned()),
    }
}

impl DaySave {
    /// The fort with this id.
    pub fn fort(&self, id: FortId) -> Option<&Fort> {
        self.forts.iter().find(|f| f.id == id)
    }

    /// Why a fort can't be built now, or `None` if one can: the country being played has to be there and able to pay, in coins first,
    /// and then in stone and wood.
    pub fn fort_problem(&self) -> Option<String> {
        if self.player_country().is_none() {
            return Some("There is no country yet to own a fort.".to_owned());
        }
        self.payment_problem("a small fort", FortSize::Small.cost(), FortSize::Small.materials())
    }

    /// Why the fort with this id can't be upgraded now, or `None` if it can: it has to be one that can be, and its country has
    /// to be the one being played, which can pay for it.
    pub fn fort_upgrade_problem(&self, id: FortId) -> Option<String> {
        let Some(fort) = self.fort(id) else { return Some("There is no such fort.".to_owned()) };
        let Some(to) = fort.size.upgrade() else { return Some("A large fort can't be upgraded any more.".to_owned()) };
        if fort.under_siege {
            return Some("A fort can't be upgraded while it is under siege.".to_owned());
        }
        let Some(player) = self.player_country() else { return Some("There is no country yet to pay for an upgrade.".to_owned()) };
        if fort.country != player.id {
            return Some(format!("This fort belongs to {}, and only {} can upgrade it.", self.country_name(fort.country), self.country_name(fort.country)));
        }
        self.payment_problem(&format!("upgrading to {}", to.a_noun()), to.cost(), to.materials())
    }

    /// Why the country being played can't have a fort at (`x`, `z`) in the region numbered `region`, or `None` if it can: the
    /// region has to be the country's, and the fort can't be on top of another.
    pub fn fort_place_problem(&self, x: f32, z: f32, region: Option<u16>) -> Option<String> {
        let country = self.player_country()?.id;
        self.placement_region_problem(country, region).or_else(|| {
            self.forts.iter().find(|f| ((f.x - x).powi(2) + (f.z - z).powi(2)).sqrt() < SPACING).map(|f| format!("That is too close to another {}.", f.size.noun()))
        })
    }

    /// Builds a small fort at (`x`, `z`) in the region numbered `region` (`None` if the map has none) for the country being
    /// played, which pays for it, and reports it in the newsreel. The region has to be the country's already. Whether the place
    /// is fit ground (see [`site_region`]) is for the caller to say. An error says why it can't be done, and nothing changes.
    pub fn build_fort(&mut self, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.fort_problem() {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.fort_place_problem(x, z, region) {
            return Err(problem);
        }
        let country = self.player_country().ok_or("There is no country yet to own a fort.")?.id;
        let size = FortSize::Small;
        self.add_money(country, -size.cost())?;
        for (resource, needed) in size.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        let id = FortId::new();
        self.forts.push(Fort { id, name: String::new(), size, country, x, z, region, under_siege: false, supplies_used: 0 });
        self.newsreel.push(News::about(id, Event::FortBuilt { size, country, region, cost: size.full_cost() }));
        Ok(())
    }

    /// Renames the fort with this id to `typed`, which is reported in the newsreel unless it was called that already. An error
    /// says why it can't be done.
    pub fn rename_fort(&mut self, id: FortId, typed: &str) -> Result<(), String> {
        let name = clean_name(typed)?;
        let fort = self.forts.iter_mut().find(|f| f.id == id).ok_or("There is no such fort.")?;
        if fort.name != name {
            let from = std::mem::replace(&mut fort.name, name.clone());
            self.newsreel.push(News::about(id, Event::FortRenamed { from, size: fort.size, to: name }));
        }
        Ok(())
    }

    /// Puts the fort with this id under siege, or lifts the siege, which fills its supplies up again. This is reported in the
    /// newsreel unless it made no difference. An error says why it can't be done.
    pub fn set_fort_under_siege(&mut self, id: FortId, under_siege: bool) -> Result<(), String> {
        let fort = self.forts.iter_mut().find(|f| f.id == id).ok_or("There is no such fort.")?;
        if fort.under_siege != under_siege {
            fort.under_siege = under_siege;
            if !under_siege {
                fort.supplies_used = 0;
            }
            let event = Event::FortSiegeChanged { name: fort.name.clone(), size: fort.size, under_siege, region: fort.region };
            self.newsreel.push(News::about(id, event));
        }
        Ok(())
    }

    /// A day's eating in every fort under siege: one of its supplies for each member of its garrison. A fort that runs out is in
    /// the newsreel.
    pub fn eat_fort_supplies(&mut self) {
        let eating: Vec<(FortId, u32)> = self.forts.iter().filter(|f| f.under_siege).map(|f| (f.id, self.garrisoned_members(f.id))).collect();
        for (id, members) in eating {
            let Some(fort) = self.forts.iter_mut().find(|f| f.id == id) else { continue };
            let before = fort.supplies_left();
            fort.supplies_used = fort.supplies_used.saturating_add(members).min(fort.size.supplies());
            if before > 0 && fort.supplies_left() == 0 {
                let event = Event::FortOutOfSupplies { name: fort.name.clone(), size: fort.size, region: fort.region };
                self.newsreel.push(News::about(id, event));
            }
        }
    }

    /// Whether `militia` is losing members to a siege: in a fort, when the fort is under siege and out of supplies, whatever its
    /// own switch says; and otherwise when it is under siege itself.
    pub fn is_besieged(&self, militia: &Militia) -> bool {
        match militia.in_fort {
            Some(inside) => self.fort(inside.fort).is_some_and(Fort::starved),
            None => militia.under_siege,
        }
    }

    /// Upgrades the fort with this id to the next size, which its country, the one being played, pays for. It stays where it
    /// is. This is reported in the newsreel. An error says why it can't be done, and nothing changes.
    pub fn upgrade_fort(&mut self, id: FortId) -> Result<(), String> {
        if let Some(problem) = self.fort_upgrade_problem(id) {
            return Err(problem);
        }
        let Fort { size, country, region, ref name, .. } = *self.fort(id).ok_or("There is no such fort.")?;
        let name = name.clone();
        let to = size.upgrade().ok_or("That fort can't be upgraded any more.")?;
        self.add_money(country, -to.cost())?;
        for (resource, needed) in to.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        if let Some(fort) = self.forts.iter_mut().find(|f| f.id == id) {
            fort.size = to;
        }
        self.newsreel.push(News::about(id, Event::FortUpgraded { name, from: size, to, country, region, cost: to.full_cost() }));
        Ok(())
    }

    /// Destroys the fort with this id, which costs nothing and pays nothing back, and reports it in the newsreel. An error says
    /// why it can't be done.
    pub fn destroy_fort(&mut self, id: FortId) -> Result<(), String> {
        let Fort { size, country, region, ref name, .. } = *self.fort(id).ok_or("There is no such fort to destroy.")?;
        let name = name.clone();
        // Its garrison comes out first, while the fort is still there to be spoken of.
        let inside: Vec<MilitiaId> = self.garrison_of(id).iter().map(|m| m.id).collect();
        for militia in inside {
            self.leave_fort(militia)?;
        }
        self.forts.retain(|f| f.id != id);
        self.newsreel.push(News::about(id, Event::FortDestroyed { name, size, country, region }));
        Ok(())
    }

    /// The militia in the garrison of the fort with this id.
    pub fn garrison_of(&self, id: FortId) -> Vec<&Militia> {
        self.militias.iter().filter(|m| m.in_fort.is_some_and(|inside| inside.fort == id)).collect()
    }

    /// How many members the garrison of the fort with this id has, all its militia together.
    pub fn garrisoned_members(&self, id: FortId) -> u32 {
        self.garrison_of(id).iter().fold(0, |total, m| total.saturating_add(m.members))
    }

    /// Why the militia with this id can't go into the fort with this id, or `None` if it can. `region` is the number of the map
    /// region the militia stands in. It has to be of the fort's country, in no fort already, in the same region as the fort, and
    /// the fort has to have room for all of its members.
    pub fn enter_fort_problem(&self, fort: FortId, militia: MilitiaId, region: Option<u16>) -> Option<String> {
        let Some(found) = self.fort(fort) else { return Some("There is no such fort.".to_owned()) };
        let Some(unit) = self.militia(militia) else { return Some("There is no such militia.".to_owned()) };
        if unit.country != found.country {
            return Some(format!("{} belongs to {}, and this fort to {}.", unit.title(), self.country_name(unit.country), self.country_name(found.country)));
        }
        if unit.in_fort.is_some() {
            return Some(format!("{} is in a fort already.", unit.title()));
        }
        if found.region.is_none() || region != found.region {
            return Some(format!("{} is not in the same region as this fort.", unit.title()));
        }
        let room = found.size.capacity().saturating_sub(self.garrisoned_members(fort));
        (unit.members > room).then(|| {
            format!("No room: {} has {} members, and this {} has room for {} more.", unit.title(), format::thousands(unit.members), found.size.noun(), format::thousands(room))
        })
    }

    /// Puts the militia with this id, which stands in the map region numbered `region`, into the garrison of the fort with this
    /// id. It has no movement points left. This is reported in the newsreel. An error says why it can't be done, and nothing
    /// changes.
    pub fn enter_fort(&mut self, fort: FortId, militia: MilitiaId, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.enter_fort_problem(fort, militia, region) {
            return Err(problem);
        }
        let Fort { size, x, z, region, name: ref fort_name, .. } = *self.fort(fort).ok_or("There is no such fort.")?;
        let fort_name = fort_name.clone();
        let unit = self.militias.iter_mut().find(|m| m.id == militia).ok_or("There is no such militia.")?;
        unit.in_fort = Some(InFort { fort, from_x: unit.x, from_z: unit.z });
        (unit.x, unit.z, unit.points) = (x, z, 0);
        let (name, members) = (unit.title().to_owned(), unit.members);
        self.newsreel.push(News::about(militia, Event::MilitiaEnteredFort { name, members, fort: fort_name, size, region }));
        Ok(())
    }

    /// Brings the militia with this id out of the fort it is in, to where it went in from. It has no movement points until the
    /// next day. This is reported in the newsreel. An error says why it can't be done.
    pub fn leave_fort(&mut self, militia: MilitiaId) -> Result<(), String> {
        let unit = self.militia(militia).ok_or("There is no such militia.")?;
        let inside = unit.in_fort.ok_or_else(|| format!("{} is not in a fort.", unit.title()))?;
        let Fort { size, region, name: ref fort_name, .. } = *self.fort(inside.fort).ok_or("There is no such fort.")?;
        let fort_name = fort_name.clone();
        let unit = self.militias.iter_mut().find(|m| m.id == militia).ok_or("There is no such militia.")?;
        (unit.x, unit.z, unit.in_fort) = (inside.from_x, inside.from_z, None);
        let name = unit.title().to_owned();
        self.newsreel.push(News::about(militia, Event::MilitiaLeftFort { name, fort: fort_name, size, region }));
        Ok(())
    }

    /// Turns out of every fort the militia that are not of the country it belongs to, as after one changes hands.
    pub(crate) fn turn_out_strangers(&mut self) {
        let strangers: Vec<MilitiaId> = self
            .militias
            .iter()
            .filter(|m| m.in_fort.and_then(|inside| self.fort(inside.fort)).is_some_and(|f| f.country != m.country))
            .map(|m| m.id)
            .collect();
        for militia in strangers {
            // It is in a fort that is there, so it can't fail.
            let _ = self.leave_fort(militia);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    /// A day with Aldoria, which has 50,000 coins and 5,000 of every resource, and has claimed region 5, and Brevon.
    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 50_000 - day.countries[0].money).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 5_000).unwrap();
        }
        day.claim_region(0x4005).unwrap();
        day
    }

    #[test]
    fn a_small_fort_takes_coins_stone_and_wood() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.build_fort(10.0, 20.0, Some(0x4005)).unwrap();

        let fort = day.forts[0].clone();
        assert_eq!((fort.size, fort.country, fort.x, fort.z, fort.region), (FortSize::Small, aldoria, 10.0, 20.0, Some(0x4005)));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, 45_000);
        assert_eq!((country.stock.stone, country.stock.wood, country.stock.iron), (4_500, 4_500, 5_000));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a small fort in region 5 for 5,000 coins, 500 stone and 500 wood");
    }

    #[test]
    fn a_fort_is_upgraded_twice_to_three_sizes_and_no_further() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
        let built = day.forts[0].clone();

        day.upgrade_fort(built.id).unwrap();
        assert_eq!(day.forts[0], Fort { size: FortSize::Medium, ..built.clone() });
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria upgrades its small fort in region 5 to a medium fort for 10,000 coins, 1,000 stone and 750 wood");
        day.upgrade_fort(built.id).unwrap();
        assert_eq!(day.forts[0], Fort { size: FortSize::Large, ..built.clone() });
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, 50_000 - 5_000 - 10_000 - 20_000);
        assert_eq!((country.stock.stone, country.stock.wood), (5_000 - 500 - 1_000 - 2_000, 5_000 - 500 - 750 - 1_000));

        assert!(day.fort_upgrade_problem(built.id).unwrap().contains("can't be upgraded any more"));
        let before = day.clone();
        assert!(day.upgrade_fort(built.id).is_err() && day.upgrade_fort(FortId::new()).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn building_and_upgrading_need_the_coin_the_stone_and_the_wood_and_say_which_is_short() {
        for (resource, name) in [(Resource::Stone, "stone"), (Resource::Forest, "wood")] {
            let mut day = day();
            let aldoria = day.countries[0].id;
            day.add_resource(aldoria, resource, -4_501).unwrap(); // 499 left.
            let before = day.clone();
            let err = day.build_fort(0.0, 0.0, Some(0x4005)).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")) && err.contains("has 499"), "{err}");
            assert_eq!(day, before);
            day.add_resource(aldoria, resource, 1).unwrap();
            day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
            assert!(day.fort_upgrade_problem(day.forts[0].id).unwrap().contains(&format!("Not enough {name}")));
        }
    }

    #[test]
    fn a_fort_cannot_be_built_a_coin_short() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 4_999 - 50_000).unwrap();
        let before = day.clone();
        let err = day.build_fort(0.0, 0.0, Some(0x4005)).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("a small fort costs 5,000") && err.contains("4,999"), "{err}");
        assert_eq!(day, before);
    }

    #[test]
    fn a_fort_needs_a_region_of_its_own_and_room_and_goes_with_its_region() {
        let mut day = day();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert!(day.build_fort(0.0, 0.0, Some(0x4006)).unwrap_err().contains("claim it first"));
        day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
        assert!(day.build_fort(10.0, 0.0, Some(0x4005)).unwrap_err().contains("too close to another small fort"));
        assert!(day.delete_problem(aldoria).unwrap().contains("1 small fort"));
        assert!(day.set_region_owner(0x4005, None).unwrap_err().contains("no owner"));
        day.set_region_owner(0x4005, Some(brevon)).unwrap();
        assert_eq!(day.forts[0].country, brevon);
        assert!(day.fort_upgrade_problem(day.forts[0].id).unwrap().contains("belongs to Brevon"));
    }

    /// A [`day`] with a fort of `size` at the origin, and a militia of Aldoria's of each of `sizes` members, camped 100 m off.
    fn manned(size: FortSize, sizes: &[u32]) -> (DaySave, FortId, Vec<MilitiaId>) {
        let mut day = day();
        day.build_fort(0.0, 0.0, Some(0x4005)).unwrap();
        day.forts[0].size = size;
        day.found_city("Port Aldor", 500.0, 0.0, Some(0x4005)).unwrap();
        day.cities[0].population = 100_000;
        for &members in sizes {
            day.raise_militia(day.cities[0].id, members, vec2(100.0, 0.0)).unwrap();
            day.militias.last_mut().unwrap().raised = Date { year: 1, month: 1, day: 9 };
        }
        let fort = day.forts[0].id;
        let ids = day.militias.iter().map(|m| m.id).collect();
        (day, fort, ids)
    }

    #[test]
    fn a_bigger_fort_holds_more() {
        assert_eq!(FortSize::ALL.map(FortSize::capacity), [100, 500, 1_000]);
        let (mut day, fort, ids) = manned(FortSize::Small, &[60, 40, 1]);
        day.enter_fort(fort, ids[0], Some(0x4005)).unwrap();
        day.enter_fort(fort, ids[1], Some(0x4005)).unwrap();
        assert_eq!((day.garrisoned_members(fort), day.garrison_of(fort).len()), (100, 2));
        let before = day.clone();
        let err = day.enter_fort(fort, ids[2], Some(0x4005)).unwrap_err();
        assert!(err.starts_with("No room") && err.contains("room for 0 more"), "{err}");
        assert_eq!(day, before);
        day.upgrade_fort(fort).unwrap();
        day.enter_fort(fort, ids[2], Some(0x4005)).unwrap();
        assert_eq!(day.garrisoned_members(fort), 101);
    }

    #[test]
    fn a_fort_can_be_renamed_and_the_news_then_calls_it_by_its_name() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[50]);
        assert_eq!(day.fort(fort).unwrap().title(), "Small fort");
        day.rename_fort(fort, "  Castle   Aldor ").unwrap();
        assert_eq!(day.fort(fort).unwrap().title(), "Castle Aldor");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "A small fort is named Castle Aldor");
        let news = day.newsreel.len();
        day.rename_fort(fort, "Castle Aldor").unwrap();
        assert_eq!(day.newsreel.len(), news, "no change is no news");
        assert_eq!(day.rename_fort(fort, " ").unwrap_err(), "Enter a name for your fort.");
        assert!(day.rename_fort(fort, &"a".repeat(MAX_NAME_CHARS + 1)).is_err() && day.rename_fort(FortId::new(), "x").is_err());
        day.rename_fort(fort, "Highwatch").unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Castle Aldor is renamed Highwatch");

        day.enter_fort(fort, ids[0], Some(0x4005)).unwrap();
        assert!(day.newsreel.last().unwrap().headline(&day).ends_with("garrisons Highwatch in region 5"));
        day.leave_fort(ids[0]).unwrap();
        assert!(day.newsreel.last().unwrap().headline(&day).ends_with("comes out of Highwatch in region 5"));
        day.upgrade_fort(fort).unwrap();
        assert!(day.newsreel.last().unwrap().headline(&day).starts_with("Aldoria upgrades Highwatch in region 5 to a medium fort for"));
        assert_eq!(day.fort(fort).unwrap().title(), "Highwatch", "it keeps its name");

        // A fort saved before forts had names has none.
        let mut old = serde_json::to_value(&day).unwrap();
        old["forts"][0].as_object_mut().unwrap().remove("name");
        assert_eq!(serde_json::from_value::<DaySave>(old).unwrap().forts[0].title(), "Medium fort");
        day.destroy_fort(fort).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's Highwatch in region 5 is destroyed");
    }

    #[test]
    fn a_militia_goes_into_a_fort_stays_put_and_comes_out_to_where_it_was() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[50]);
        let id = ids[0];
        day.enter_fort(fort, id, Some(0x4005)).unwrap();
        let unit = day.militia(id).unwrap().clone();
        assert_eq!((unit.x, unit.z, unit.points), (0.0, 0.0, 0));
        assert!(day.newsreel.last().unwrap().headline(&day).ends_with("a militia of 50, garrisons a small fort in region 5"));
        assert!(day.enter_fort(fort, id, Some(0x4005)).unwrap_err().contains("in a fort already"));
        day.rest_militia();
        assert_eq!(day.militia(id).unwrap().points, 0, "and gets none on a new day");
        assert!(day.move_militia(id, vec2(5.0, 0.0), 0, Some(0x4005)).unwrap_err().contains("march out"));

        // It is saved, and carries forward.
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert!(day.following(Date { year: 1, month: 1, day: 2 }).militia(id).unwrap().in_fort.is_some());

        day.leave_fort(id).unwrap();
        let unit = day.militia(id).unwrap();
        assert_eq!((unit.x, unit.z, unit.points, unit.in_fort), (100.0, 0.0, 0, None));
        assert!(day.newsreel.last().unwrap().headline(&day).ends_with("comes out of a small fort in region 5"));
        assert!(day.leave_fort(id).is_err());
        day.rest_militia();
        assert!(day.militia(id).unwrap().points > 0);
    }

    #[test]
    fn only_a_militia_of_the_forts_country_in_its_region_can_go_in() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[50]);
        assert!(day.enter_fort(fort, ids[0], Some(0x4006)).unwrap_err().contains("not in the same region"));
        assert!(day.enter_fort(fort, ids[0], None).is_err());
        day.militias[0].country = day.countries[1].id;
        assert!(day.enter_fort(fort, ids[0], Some(0x4005)).unwrap_err().contains("belongs to Brevon"));
        assert!(day.enter_fort(FortId::new(), ids[0], Some(0x4005)).is_err() && day.enter_fort(fort, MilitiaId::new(), Some(0x4005)).is_err());
    }

    #[test]
    fn a_garrison_is_turned_out_when_the_fort_is_destroyed_or_either_changes_country() {
        let (day, fort, ids) = manned(FortSize::Small, &[50]);
        let (id, brevon) = (ids[0], day.countries[1].id);
        let mut manned = day;
        manned.enter_fort(fort, id, Some(0x4005)).unwrap();
        let out = |day: &DaySave| day.militia(id).map(|m| (m.x, m.in_fort)) == Some((100.0, None));

        let mut destroyed = manned.clone();
        destroyed.destroy_fort(fort).unwrap();
        assert!(out(&destroyed));

        let mut changed_sides = manned.clone();
        changed_sides.set_militia_country(id, brevon).unwrap();
        assert!(out(&changed_sides));

        let mut taken = manned.clone();
        let city = taken.cities[0].id;
        taken.set_city_country(city, brevon).unwrap();
        assert_eq!(taken.forts[0].country, brevon);
        assert!(out(&taken));
    }

    #[test]
    fn splitting_in_a_fort_keeps_both_in_it_and_only_those_together_can_be_combined() {
        let (mut day, fort, ids) = manned(FortSize::Medium, &[100, 40]);
        day.enter_fort(fort, ids[0], Some(0x4005)).unwrap();
        let err = day.merge_militia(ids[0], ids[1], (Some(0x4005), Some(0x4005))).unwrap_err();
        assert!(err.contains("same fort"), "{err}");
        assert!(day.merge_partners(ids[0], &|_| Some(0x4005)).is_empty());

        day.split_militia(ids[0], 30, vec2(0.0, 40.0)).unwrap();
        let new = day.militias.last().unwrap().clone();
        assert_eq!((new.x, new.z), (0.0, 0.0));
        assert_eq!(day.garrisoned_members(fort), 100);
        day.merge_militia(ids[0], new.id, (Some(0x4005), Some(0x4005))).unwrap();
        assert_eq!(day.garrisoned_members(fort), 100);
    }

    #[test]
    fn a_bigger_fort_has_more_supplies_and_a_bigger_garrison_eats_them_faster() {
        assert_eq!(FortSize::ALL.map(FortSize::supplies), [500, 4_000, 10_000]);
        let left_after_a_day = |members: u32| {
            let (mut day, fort, ids) = manned(FortSize::Small, &[members]);
            day.enter_fort(fort, ids[0], Some(0x4005)).unwrap();
            day.set_fort_under_siege(fort, true).unwrap();
            day.eat_fort_supplies();
            let fort = day.fort(fort).unwrap();
            (fort.supplies_left(), fort.supplies_percent(), fort.days_of_supplies(members))
        };
        assert_eq!(left_after_a_day(50), (450, 90, Some(9)));
        assert_eq!(left_after_a_day(100), (400, 80, Some(4)));
        // Nobody eats nothing.
        let (mut day, fort, _) = manned(FortSize::Small, &[]);
        day.set_fort_under_siege(fort, true).unwrap();
        day.eat_fort_supplies();
        assert_eq!((day.forts[0].supplies_left(), day.forts[0].days_of_supplies(0)), (500, None));
    }

    #[test]
    fn a_garrison_loses_nobody_until_the_supplies_are_gone_and_lifting_the_siege_fills_them() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[100]);
        let id = ids[0];
        day.enter_fort(fort, id, Some(0x4005)).unwrap();
        day.set_fort_under_siege(fort, true).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "A small fort in region 5 is put under siege");
        // A day as the rules run it: the besieged lose members, and then the forts eat.
        let pass = |day: &mut DaySave| {
            day.shrink_besieged_militia(|| 0.0);
            day.eat_fort_supplies();
        };
        for _ in 0..5 {
            assert!(!day.is_besieged(day.militia(id).unwrap()));
            pass(&mut day);
        }
        assert_eq!((day.militia(id).unwrap().members, day.forts[0].supplies_percent()), (100, 0));
        assert!(day.newsreel.iter().any(|n| n.headline(&day) == "A small fort in region 5 runs out of supplies"));
        assert!(day.is_besieged(day.militia(id).unwrap()));
        pass(&mut day);
        assert_eq!(day.militia(id).unwrap().members, 95);
        assert_eq!(day.newsreel.iter().filter(|n| matches!(n.event, Event::FortOutOfSupplies { .. })).count(), 1, "said once");

        day.set_fort_under_siege(fort, false).unwrap();
        assert_eq!((day.forts[0].supplies_percent(), day.forts[0].supplies_used), (100, 0));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "The siege of a small fort in region 5 is lifted");
        pass(&mut day);
        assert_eq!((day.militia(id).unwrap().members, day.forts[0].supplies_left()), (95, 500));
    }

    #[test]
    fn in_a_fort_the_forts_siege_counts_and_not_the_militias_own() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[50]);
        let id = ids[0];
        day.set_militia_under_siege(id, true).unwrap();
        day.enter_fort(fort, id, Some(0x4005)).unwrap();
        assert!(!day.is_besieged(day.militia(id).unwrap()), "the fort is not besieged");
        day.shrink_besieged_militia(|| 1.0);
        assert_eq!(day.militia(id).unwrap().members, 50);
        assert!(day.set_militia_under_siege(id, false).unwrap_err().contains("in a fort"));
        day.leave_fort(id).unwrap();
        assert!(day.is_besieged(day.militia(id).unwrap()), "out of it, its own siege counts again");
    }

    #[test]
    fn militia_can_march_into_and_out_of_a_besieged_fort_and_the_supplies_stay_as_eaten() {
        let (mut day, fort, ids) = manned(FortSize::Medium, &[50, 40]);
        let (inside, outside) = (ids[0], ids[1]);
        day.enter_fort(fort, inside, Some(0x4005)).unwrap();
        day.set_fort_under_siege(fort, true).unwrap();
        day.eat_fort_supplies();
        day.leave_fort(inside).unwrap();
        day.enter_fort(fort, outside, Some(0x4005)).unwrap();
        assert_eq!(day.forts[0].supplies_left(), 3_950, "only lifting the siege fills them");
        day.eat_fort_supplies();
        assert_eq!(day.forts[0].supplies_left(), 3_910, "and whoever is inside now eats");
    }

    #[test]
    fn a_siege_is_saved_carries_forward_and_stops_an_upgrade() {
        let (mut day, fort, ids) = manned(FortSize::Small, &[50]);
        day.enter_fort(fort, ids[0], Some(0x4005)).unwrap();
        day.set_fort_under_siege(fort, true).unwrap();
        assert!(day.fort_upgrade_problem(fort).unwrap().contains("under siege"));
        let next = day.next_day();
        assert_eq!((next.forts[0].under_siege, next.forts[0].supplies_left()), (true, 450), "a day's rules feed the garrison");
        let mut old = serde_json::to_value(&next).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(old.clone()).unwrap(), next);
        for field in ["under_siege", "supplies_used"] {
            old["forts"][0].as_object_mut().unwrap().remove(field);
        }
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert!(!old.forts[0].under_siege && old.forts[0].supplies_percent() == 100);
        assert!(day.set_fort_under_siege(FortId::new(), true).is_err());
    }

    #[test]
    fn a_fort_can_be_destroyed_and_forts_are_saved_and_carry_forward() {
        let mut day = day();
        day.build_fort(5.0, 6.0, Some(0x4005)).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["forts"][0]["size"], "small");
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).forts, day.forts);
        let mut old = json;
        old.as_object_mut().unwrap().remove("forts");
        assert!(serde_json::from_value::<DaySave>(old).unwrap().forts.is_empty());

        let id = day.forts[0].id;
        day.destroy_fort(id).unwrap();
        assert!(day.forts.is_empty());
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria's small fort in region 5 is destroyed");
        assert!(day.destroy_fort(id).is_err());
    }
}
