//! Gatherers: a country's camp where there is a resource to gather, of a kind: a woodcutter in a forest, a
//! stonecutter in stone. One can only be put in a region that has some of the resource, only one of each kind to a
//! region, and it belongs to the country that built it, which pays for it. A region with a gatherer in it can't be one
//! that another country owns.

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;
use nworld_map::world::{Resource, WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::format;
use crate::ids::{CountryId, GathererId};
use crate::newsreel::{Event, News};

/// How dense a resource has to be at a point of the map, from 0 to 255, to count as there, and how many such points a
/// region needs to have for it to count as having the resource at all: a stray speck doesn't make a forest.
const DEPOSIT_DENSITY: u8 = 32;
const DEPOSIT_SAMPLES: usize = 4;

/// What a gatherer gathers, and so what it is called.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GathererKind {
    /// Gathers wood, from a forest.
    #[default]
    Woodcutter,
    /// Gathers stone.
    Stonecutter,
}

impl GathererKind {
    pub const ALL: [GathererKind; 2] = [GathererKind::Woodcutter, GathererKind::Stonecutter];

    /// What to call one, in lower case: "woodcutter" or "stonecutter".
    pub fn noun(self) -> &'static str {
        match self {
            Self::Woodcutter => "woodcutter",
            Self::Stonecutter => "stonecutter",
        }
    }

    /// What to call it where a name starts something: "Woodcutter" or "Stonecutter".
    pub fn label(self) -> &'static str {
        match self {
            Self::Woodcutter => "Woodcutter",
            Self::Stonecutter => "Stonecutter",
        }
    }

    /// The resource of the map it gathers.
    pub fn resource(self) -> Resource {
        match self {
            Self::Woodcutter => Resource::Forest,
            Self::Stonecutter => Resource::Stone,
        }
    }

    /// What the resource it gathers is called as it lies on the map: "forest" or "stone".
    pub fn deposit(self) -> &'static str {
        match self {
            Self::Woodcutter => "forest",
            Self::Stonecutter => "stone",
        }
    }

    /// What the country gets of it: "wood" or "stone".
    pub fn product(self) -> &'static str {
        crate::country::resource_name(self.resource())
    }

    /// What one costs to build, in coins.
    pub fn cost(self) -> i64 {
        1_000
    }

    /// How much of its resource one that is operating provides its country each day.
    pub fn daily_yield(self) -> u64 {
        100
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Gatherer {
    pub id: GathererId,
    /// What it gathers. A gatherer saved before there were kinds is a woodcutter.
    #[serde(default)]
    pub kind: GathererKind,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in.
    pub region: u16,
    /// Whether it was operating when last looked at (see [`DaySave::update_gatherer_operation`]), which is for
    /// knowing when that changes. A gatherer starts off not operating.
    #[serde(default)]
    pub operating: bool,
}

/// Which regions of a map have how much of each resource a gatherer gathers: for each kind, the numbers of the regions
/// that have some of it. This takes a pass over the whole map, so it is worked out once and kept.
pub struct Deposits {
    regions: HashMap<GathererKind, HashSet<u16>>,
}

impl Deposits {
    /// The regions of `world` that have a resource of each kind.
    pub fn of(world: &World) -> Self {
        let regions = GathererKind::ALL.into_iter().map(|kind| (kind, regions_with(world, kind.resource()))).collect();
        Self { regions }
    }

    /// Whether the region numbered `region` has the resource that `kind` gathers.
    pub fn has(&self, kind: GathererKind, region: u16) -> bool {
        self.regions.get(&kind).is_some_and(|regions| regions.contains(&region))
    }
}

/// The numbers of the regions of `world` that have `resource` in them: enough of it, at a density that counts.
fn regions_with(world: &World, resource: Resource) -> HashSet<u16> {
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for (region, density) in world.regions().iter().zip(world.resource(resource)) {
        if *region != 0 && *density >= DEPOSIT_DENSITY {
            *counts.entry(*region).or_default() += 1;
        }
    }
    counts.into_iter().filter(|(_, samples)| *samples >= DEPOSIT_SAMPLES).map(|(region, _)| region).collect()
}

/// The number of the region that a gatherer of `kind` put at `at` would be in, or why it can't go there: it has to be
/// on the map, on dry land, and in a region that has its resource, according to `deposits`.
pub fn site_region(world: &World, deposits: &Deposits, kind: GathererKind, at: Vec2) -> Result<u16, String> {
    let half = world.half_extents();
    if at.x.abs() > half.x || at.y.abs() > half.y {
        return Err("That is off the map.".to_owned());
    }
    if world.water_class(at.x, at.y) != WaterClass::Dry {
        return Err(format!("A {} has to stand on dry land.", kind.noun()));
    }
    let region = world.region_at(at.x, at.y).ok_or_else(|| format!("This map has no regions to put a {} in.", kind.noun()))?.raw();
    if !deposits.has(kind, region) {
        return Err(format!("A {} can only go in a region that has some {}.", kind.noun(), kind.deposit()));
    }
    Ok(region)
}

impl DaySave {
    /// The gatherer with this id.
    pub fn gatherer(&self, id: GathererId) -> Option<&Gatherer> {
        self.gatherers.iter().find(|g| g.id == id)
    }

    /// The gatherer of `kind` in the map region numbered `region`, if there is one.
    pub fn gatherer_in(&self, region: u16, kind: GathererKind) -> Option<&Gatherer> {
        self.gatherers.iter().find(|g| g.region == region && g.kind == kind)
    }

    /// Why a gatherer of `kind` can't be built now, or `None` if one can: the country being played has to be there and
    /// able to pay for it.
    pub fn gatherer_problem(&self, kind: GathererKind) -> Option<String> {
        match self.player_country() {
            None => Some(format!("There is no country yet to own a {}.", kind.noun())),
            Some(country) if country.money < kind.cost() => {
                Some(format!("Not enough coin: a {} costs {} coins, and {} has {}.", kind.noun(), format::money(kind.cost()), country.name, format::money(country.money)))
            }
            Some(_) => None,
        }
    }

    /// Why the country being played can't have a gatherer of `kind` in the region numbered `region`, or `None` if it
    /// can: the region can't belong to another country, and can't have one of that kind in it already.
    pub fn gatherer_region_problem(&self, region: u16, kind: GathererKind) -> Option<String> {
        let country = self.player_country()?.id;
        self.city_region_problem(country, Some(region)).or_else(|| self.gatherer_in(region, kind).map(|_| format!("There is already a {} in this region.", kind.noun())))
    }

    /// Builds a gatherer of `kind` at (`x`, `z`) in the region numbered `region` for the country being played, which
    /// pays for it, and reports it in the newsreel. Whether the place is fit for one (see [`site_region`]) is for the
    /// caller to say. An error says why it can't be done.
    pub fn build_gatherer(&mut self, kind: GathererKind, x: f32, z: f32, region: u16) -> Result<(), String> {
        if let Some(problem) = self.gatherer_problem(kind) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.gatherer_region_problem(region, kind) {
            return Err(problem);
        }
        let country = self.player_country().ok_or_else(|| format!("There is no country yet to own a {}.", kind.noun()))?.id;
        self.add_money(country, -kind.cost())?;
        let id = GathererId::new();
        self.gatherers.push(Gatherer { id, kind, country, x, z, region, operating: false });
        self.newsreel.push(News::about(id, Event::GathererBuilt { kind }));
        Ok(())
    }

    /// Transfers the gatherer with this id to `country`, which is reported in the newsreel unless it belonged to it
    /// already. The region it stands in can't belong to a different country from that, and the country can't have one of
    /// its kind there already. An error says why it can't be done.
    pub fn set_gatherer_country(&mut self, id: GathererId, country: CountryId) -> Result<(), String> {
        self.country(country).ok_or("There is no such country.")?;
        let Gatherer { kind, region, country: from, .. } = *self.gatherer(id).ok_or("There is no such gatherer.")?;
        if from == country {
            return Ok(());
        }
        if let Some(problem) = self.city_region_problem(country, Some(region)) {
            return Err(problem);
        }
        if let Some(gatherer) = self.gatherers.iter_mut().find(|g| g.id == id) {
            gatherer.country = country;
        }
        self.newsreel.push(News::about(id, Event::GathererTransferred { from, to: country, kind }));
        Ok(())
    }

    /// Destroys the gatherer with this id, which costs nothing and pays nothing back, and reports it in the
    /// newsreel. An error says why it can't be done.
    pub fn destroy_gatherer(&mut self, id: GathererId) -> Result<(), String> {
        let Gatherer { kind, country, region, .. } = *self.gatherer(id).ok_or("There is no such gatherer to destroy.")?;
        self.gatherers.retain(|g| g.id != id);
        self.newsreel.push(News::about(id, Event::GathererDestroyed { country, region, kind }));
        Ok(())
    }

    /// What to call the map region numbered `region` in a sentence: its name if it has one, and otherwise "region" and
    /// its number.
    pub fn region_label(&self, region: u16) -> String {
        self.region_name(region).map_or_else(|| format!("region {}", region & 0x3FFF), str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use nworld_map::world::SEA_LEVEL;

    fn day_with_money(money: i64) -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        let have = day.countries[0].money;
        day.add_money(id, money - have).unwrap();
        day
    }

    /// A dry world of two regions, 1 in the west half and 2 in the east, with `forest` painted at full density on
    /// the samples for which it is true.
    fn world_with_forest(forest: impl Fn(usize, usize) -> bool) -> World {
        let mut world = World::new(512.0, 512.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        world.set_regions((0..nz).flat_map(|_| (0..nx).map(|i| if i < nx / 2 { 1 } else { 2 })).collect());
        for j in 0..nz {
            for i in 0..nx {
                if forest(i, j) {
                    world.resource_mut(Resource::Forest)[j * nx + i] = 255;
                }
            }
        }
        world
    }

    #[test]
    fn a_region_has_forest_when_enough_of_it_is_dense_and_a_speck_is_not_enough() {
        // A patch in the west, and a single stray sample in the east.
        let world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j) || (i, j) == (100, 100));
        assert_eq!(regions_with(&world, Resource::Forest), HashSet::from([1]));
        assert_eq!(Deposits::of(&world).has(GathererKind::Woodcutter, 1), true);
        assert!(!Deposits::of(&world).has(GathererKind::Woodcutter, 2) && !Deposits::of(&world).has(GathererKind::Stonecutter, 1), "no stone anywhere");
        // A patch only faintly painted doesn't count either.
        let mut faint = world_with_forest(|_, _| false);
        faint.resource_mut(Resource::Forest).fill(DEPOSIT_DENSITY - 1);
        assert!(regions_with(&faint, Resource::Forest).is_empty());
        // Nor does a map with no regions.
        assert!(regions_with(&World::new(512.0, 512.0), Resource::Forest).is_empty());
    }

    #[test]
    fn a_gatherer_goes_on_dry_land_in_a_forest_region_on_the_map() {
        let world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        let forest = Deposits::of(&world);
        let west = vec2(-100.0, 0.0);
        assert_eq!(site_region(&world, &forest, GathererKind::Woodcutter, west), Ok(1));
        // The other region has no forest, so is refused, whether or not there is forest on the spot itself.
        assert!(site_region(&world, &forest, GathererKind::Woodcutter, vec2(100.0, 0.0)).unwrap_err().contains("some forest"));
        assert!(site_region(&world, &forest, GathererKind::Woodcutter, vec2(9_999.0, 0.0)).unwrap_err().contains("off the map"));
        // Water is no place for one, even in a forest region.
        let mut flooded = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        flooded.water_mut().fill(20.0);
        assert!(site_region(&flooded, &forest, GathererKind::Woodcutter, west).unwrap_err().contains("dry land"));
        // Nor is a map not divided into regions.
        let plain = World::new(512.0, 512.0);
        assert!(site_region(&plain, &forest, GathererKind::Woodcutter, Vec2::ZERO).is_err());
    }

    #[test]
    fn a_gatherer_costs_1000_coins_from_the_country_being_played_and_belongs_to_it() {
        let mut day = day_with_money(2_500);
        day.found_country("Brevon").unwrap();
        day.build_gatherer(GathererKind::Woodcutter, -100.0, 20.0, 7).unwrap();
        assert_eq!(day.gatherers.len(), 1);
        let aldoria = day.countries[0].id;
        let cutter = day.gatherers[0];
        assert_eq!((cutter.country, cutter.x, cutter.z, cutter.region), (aldoria, -100.0, 20.0, 7));
        assert_eq!(day.countries[0].money, 1_500);
        assert_eq!(day.countries[1].money, crate::country::STARTING_MONEY, "only the country being played pays");
        assert_eq!(day.gatherer(cutter.id), Some(&cutter));
        assert_eq!(day.gatherer_in(7, GathererKind::Woodcutter), Some(&cutter));
        assert_eq!(day.gatherer_in(8, GathererKind::Woodcutter), None);
    }

    #[test]
    fn building_a_gatherer_is_in_the_newsreel_with_the_gatherer_as_its_subject() {
        let mut day = day_with_money(2_000);
        let before = day.newsreel.len();
        day.build_gatherer(GathererKind::Woodcutter, -100.0, 20.0, 0xC007).unwrap();
        let id = day.gatherers[0].id;
        assert_eq!(day.newsreel.len(), before + 1);
        let news = day.newsreel.last().unwrap();
        assert_eq!(*news, News::about(id, Event::GathererBuilt { kind: GathererKind::Woodcutter }));
        assert_eq!(news.headline(&day), "Aldoria builds a woodcutter in region 7");
        day.set_region_name(0xC007, "The Greenwood").unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a woodcutter in The Greenwood");
        // It is saved with its kind, and the subject's id is never in the words.
        let json = serde_json::to_string(day.newsreel.last().unwrap()).unwrap();
        assert!(json.contains(r#""type":"gatherer_built""#), "{json}");
        assert!(!day.newsreel.last().unwrap().headline(&day).contains(&id.to_string()));
        // News of one that is gone reads on without it, and a gatherer that can't be built is not reported.
        assert_eq!(News::about(GathererId::new(), Event::GathererBuilt { kind: GathererKind::Woodcutter }).headline(&day), "A woodcutter is built");
        let count = day.newsreel.len();
        assert!(day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC007).is_err());
        assert_eq!(day.newsreel.len(), count);
    }

    #[test]
    fn a_gatherer_can_be_transferred_to_another_country_unless_the_region_is_a_third_countrys() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        day.found_country("Cardell").unwrap();
        let (aldoria, brevon, cardell) = (day.countries[0].id, day.countries[1].id, day.countries[2].id);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;
        let paid = day.countries[0].money;
        day.set_gatherer_country(id, brevon).unwrap();
        assert_eq!(day.gatherer(id).unwrap().country, brevon);
        assert_eq!(day.countries[0].money, paid, "it costs nothing");
        assert_eq!((day.gatherer(id).unwrap().x, day.gatherer(id).unwrap().region), (0.0, 0xC001), "and it stays where it is");
        // Its region being Cardell's, only Cardell can have it.
        day.set_region_owner(0xC001, Some(cardell)).unwrap();
        let before = day.clone();
        assert!(day.set_gatherer_country(id, aldoria).unwrap_err().contains("belongs to Cardell"));
        assert_eq!(day, before);
        day.set_gatherer_country(id, cardell).unwrap();
        // A country or a gatherer that isn't there is an error.
        assert!(day.set_gatherer_country(id, CountryId::new()).is_err());
        assert!(day.set_gatherer_country(GathererId::new(), cardell).is_err());
        // Which country it is changes which cities it has to reach.
        day.found_city("Port Aldor", 0.0, 0.0, Some(0xC002)).unwrap();
        day.select_country(brevon).unwrap();
        day.found_city("Brevonport", 9.0, 0.0, Some(0xC003)).unwrap();
        assert_eq!(day.cut_off_gatherers(), [id], "Cardell has no city to reach");
    }

    #[test]
    fn transferring_a_gatherer_to_another_country_and_destroying_one_are_in_the_newsreel() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;
        let before = day.newsreel.len();

        // Transferred to the country it already has, nothing happens and there is no news.
        day.set_gatherer_country(id, aldoria).unwrap();
        assert_eq!(day.newsreel.len(), before);
        day.set_gatherer_country(id, brevon).unwrap();
        let news = day.newsreel.last().unwrap();
        assert_eq!(*news, News::about(id, Event::GathererTransferred { from: aldoria, to: brevon, kind: GathererKind::Woodcutter }));
        assert_eq!(news.headline(&day), "The woodcutter in region 1 is transferred from Aldoria to Brevon");
        assert!(serde_json::to_string(news).unwrap().contains(r#""type":"gatherer_transferred""#));
        // A change that can't be made is not news.
        let count = day.newsreel.len();
        assert!(day.set_gatherer_country(id, CountryId::new()).is_err());
        assert_eq!(day.newsreel.len(), count);

        // Destroyed, it is gone, and the news still says whose it was and where.
        day.destroy_gatherer(id).unwrap();
        let news = day.newsreel.last().unwrap();
        assert_eq!(*news, News::about(id, Event::GathererDestroyed { country: brevon, region: 0xC001, kind: GathererKind::Woodcutter }));
        assert_eq!(news.headline(&day), "Brevon's woodcutter in region 1 is destroyed");
        let json = serde_json::to_string(news).unwrap();
        assert!(json.contains(r#""type":"gatherer_destroyed""#) && json.contains(r#""region":49153"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *news);
        let count = day.newsreel.len();
        assert!(day.destroy_gatherer(id).is_err());
        assert_eq!(day.newsreel.len(), count, "one that isn't there is not news");
        assert_eq!(News::about(id, Event::GathererTransferred { from: aldoria, to: brevon, kind: GathererKind::Woodcutter }).headline(&day), "A woodcutter is transferred from Aldoria to Brevon");
    }

    #[test]
    fn a_gatherer_destroyed_is_gone_for_nothing_and_its_region_has_room_for_another() {
        let mut day = day_with_money(5_000);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;
        let paid = day.countries[0].money;
        day.destroy_gatherer(id).unwrap();
        assert!(day.gatherers.is_empty() && day.gatherer(id).is_none() && day.gatherer_in(0xC001, GathererKind::Woodcutter).is_none());
        assert_eq!(day.countries[0].money, paid, "nothing is paid back");
        assert!(day.destroy_gatherer(id).unwrap_err().contains("no such gatherer"));
        assert!(day.cut_off_gatherers().is_empty());
        day.build_gatherer(GathererKind::Woodcutter, 3.0, 3.0, 0xC001).unwrap();
        assert_eq!(day.gatherers.len(), 1);
    }

    #[test]
    fn each_kind_needs_what_it_gathers_in_the_region_forest_for_a_woodcutter_and_stone_for_a_stonecutter() {
        // Forest in the west region (1) and stone in the east (2), both a patch of full density.
        let mut world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        let (nx, _) = world.samples();
        for j in 10..20 {
            for i in (nx - 20)..(nx - 10) {
                world.resource_mut(Resource::Stone)[j * nx + i] = 255;
            }
        }
        let deposits = Deposits::of(&world);
        assert!(deposits.has(GathererKind::Woodcutter, 1) && !deposits.has(GathererKind::Woodcutter, 2));
        assert!(deposits.has(GathererKind::Stonecutter, 2) && !deposits.has(GathererKind::Stonecutter, 1));
        let (west, east) = (vec2(-100.0, 0.0), vec2(100.0, 0.0));
        assert_eq!(site_region(&world, &deposits, GathererKind::Woodcutter, west), Ok(1));
        assert_eq!(site_region(&world, &deposits, GathererKind::Stonecutter, east), Ok(2));
        assert_eq!(site_region(&world, &deposits, GathererKind::Stonecutter, west).unwrap_err(), "A stonecutter can only go in a region that has some stone.");
        assert_eq!(site_region(&world, &deposits, GathererKind::Woodcutter, east).unwrap_err(), "A woodcutter can only go in a region that has some forest.");
    }

    #[test]
    fn a_region_has_room_for_one_of_each_kind_and_a_stonecutter_costs_the_same_and_is_named_for_what_it_is() {
        let mut day = day_with_money(5_000);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        // A stonecutter can share the region with the woodcutter, but a second of either kind can't.
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Stonecutter), None);
        day.build_gatherer(GathererKind::Stonecutter, 20.0, 0.0, 0xC001).unwrap();
        assert_eq!(day.countries[0].money, 3_000, "each costs 1,000");
        assert_eq!(day.gatherers.iter().map(|g| g.kind).collect::<Vec<_>>(), [GathererKind::Woodcutter, GathererKind::Stonecutter]);
        assert_eq!(day.gatherer_in(0xC001, GathererKind::Stonecutter).unwrap().kind, GathererKind::Stonecutter);
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Stonecutter).unwrap(), "There is already a stonecutter in this region.");
        let before = day.clone();
        assert!(day.build_gatherer(GathererKind::Stonecutter, 30.0, 0.0, 0xC001).unwrap_err().contains("already a stonecutter"));
        assert!(day.build_gatherer(GathererKind::Woodcutter, 30.0, 0.0, 0xC001).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before);
        // Short of coin, it says it is a stonecutter that costs it.
        let mut poor = day_with_money(999);
        assert!(poor.build_gatherer(GathererKind::Stonecutter, 0.0, 0.0, 0xC001).unwrap_err().contains("a stonecutter costs 1,000"));
        assert!(DaySave::new(Date::FIRST, "x").gatherer_problem(GathererKind::Stonecutter).unwrap().contains("own a stonecutter"));
        // Its news and the countries' lists name it too.
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a stonecutter in region 1");
        assert_eq!(day.delete_problem(day.countries[0].id).unwrap(), "Aldoria owns 1 woodcutter and 1 stonecutter, so it can't be deleted.");
        assert!(serde_json::to_string(day.newsreel.last().unwrap()).unwrap().contains(r#""type":"gatherer_built","kind":"stonecutter""#));
    }

    #[test]
    fn what_each_kind_gathers_and_is_called() {
        assert_eq!((GathererKind::Woodcutter.noun(), GathererKind::Stonecutter.noun()), ("woodcutter", "stonecutter"));
        assert_eq!((GathererKind::Woodcutter.label(), GathererKind::Stonecutter.label()), ("Woodcutter", "Stonecutter"));
        assert_eq!((GathererKind::Woodcutter.resource(), GathererKind::Stonecutter.resource()), (Resource::Forest, Resource::Stone));
        assert_eq!((GathererKind::Woodcutter.product(), GathererKind::Stonecutter.product()), ("wood", "stone"));
        assert_eq!((GathererKind::Woodcutter.deposit(), GathererKind::Stonecutter.deposit()), ("forest", "stone"));
        assert!(GathererKind::ALL.iter().all(|kind| kind.cost() == 1_000 && kind.daily_yield() == 100));
        assert_eq!(GathererKind::default(), GathererKind::Woodcutter);
    }

    #[test]
    fn a_game_saved_when_gatherers_were_all_woodcutters_still_loads() {
        let mut day = day_with_money(5_000);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        day.gatherers[0].operating = true;
        day.newsreel.push(News::about(day.countries[0].id, Event::Gathered { amount: 100, kind: GathererKind::Woodcutter }));
        let id = day.gatherers[0].id;
        let mut old = serde_json::to_value(&day).unwrap();
        // The list was called "woodcutters", and a woodcutter had no kind.
        let list = old.as_object_mut().unwrap().remove("gatherers").unwrap();
        old["woodcutters"] = list;
        old["woodcutters"][0].as_object_mut().unwrap().remove("kind");
        // And the news had names of its own, and nothing of kinds.
        for news in old["newsreel"].as_array_mut().unwrap() {
            for (new, was) in [("gatherer_built", "woodcutter_built"), ("gathered", "wood_gathered")] {
                if news["type"] == new {
                    news["type"] = was.into();
                    news.as_object_mut().unwrap().remove("kind");
                }
            }
        }
        let loaded: DaySave = serde_json::from_value(old).unwrap();
        assert_eq!(loaded.gatherers.len(), 1);
        assert_eq!((loaded.gatherers[0].id, loaded.gatherers[0].kind, loaded.gatherers[0].operating), (id, GathererKind::Woodcutter, true));
        assert_eq!(loaded, day, "nothing is lost");
    }

    #[test]
    fn a_gatherer_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(999);
        let before = day.clone();
        let error = day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 1).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("1,000") && error.contains("Aldoria") && error.contains("999"), "{error}");
        assert_eq!(day, before);
        assert!(day.gatherer_problem(GathererKind::Woodcutter).is_some());
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.gatherer_problem(GathererKind::Woodcutter), None);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 1).unwrap();
        assert_eq!(day.countries[0].money, 0);
        // With no country there is nobody to own it.
        let mut empty = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert!(empty.gatherer_problem(GathererKind::Woodcutter).unwrap().contains("no country"));
        assert!(empty.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 1).is_err());
    }

    #[test]
    fn there_can_be_only_one_in_a_region_and_nothing_is_paid_for_the_second() {
        let mut day = day_with_money(5_000);
        day.build_gatherer(GathererKind::Woodcutter, -100.0, 0.0, 3).unwrap();
        let before = day.clone();
        assert!(day.gatherer_region_problem(3, GathererKind::Woodcutter).unwrap().contains("already a woodcutter"));
        assert!(day.build_gatherer(GathererKind::Woodcutter, -50.0, 40.0, 3).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before);
        // Another region is fine.
        assert_eq!(day.gatherer_region_problem(4, GathererKind::Woodcutter), None);
        day.build_gatherer(GathererKind::Woodcutter, 50.0, 0.0, 4).unwrap();
        assert_eq!(day.gatherers.len(), 2);
    }

    #[test]
    fn a_region_that_belongs_to_another_country_is_refused() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.set_region_owner(5, Some(brevon)).unwrap();
        let before = day.clone();
        assert!(day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 5).unwrap_err().contains("belongs to Brevon"));
        assert_eq!(day, before);
        // One that is the player's own is fine.
        day.set_region_owner(6, Some(day.countries[0].id)).unwrap();
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 6).unwrap();
    }

    #[test]
    fn gatherers_are_saved_with_the_day_carry_forward_and_go_with_a_deleted_country() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        day.build_gatherer(GathererKind::Woodcutter, -100.0, 20.0, 7).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["gatherers"][0]["region"], 7);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).gatherers, day.gatherers);
        // A day saved before there were gatherers has none.
        let mut old = serde_json::to_value(&day).unwrap();
        old.as_object_mut().unwrap().remove("gatherers");
        assert!(serde_json::from_value::<DaySave>(old).unwrap().gatherers.is_empty());
        // The other country, which owns nothing, can go; the one that owns the gatherer can't.
        day.delete_country(day.countries[1].id).unwrap();
        assert_eq!(day.gatherers.len(), 1);
        let before = day.clone();
        assert_eq!(day.delete_country(day.countries[0].id).unwrap_err(), "Aldoria owns 1 woodcutter, so it can't be deleted.");
        assert_eq!(day, before);
    }
}
