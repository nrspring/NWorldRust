//! Woodcutters: a country's camp in a forest. One can only be put in a region that has some forest, only one to a
//! region, and it belongs to the country that built it, which pays for it. A region with a woodcutter in it can't be
//! one that another country owns.

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;
use nworld_map::world::{Resource, WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::format;
use crate::ids::{CountryId, WoodcutterId};

/// What a woodcutter costs, in coins.
pub const WOODCUTTER_COST: i64 = 1_000;
/// How dense the forest has to be at a point of the map, from 0 to 255, to count as forest there, and how many such
/// points a region needs to have for it to count as having forest at all: a stray speck doesn't make a forest.
const FOREST_DENSITY: u8 = 32;
const FOREST_SAMPLES: usize = 4;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Woodcutter {
    pub id: WoodcutterId,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in.
    pub region: u16,
}

/// The numbers of the regions of `world` that have forest in them. This takes a pass over the whole map, so it is
/// worked out once and kept.
pub fn forest_regions(world: &World) -> HashSet<u16> {
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for (region, density) in world.regions().iter().zip(world.resource(Resource::Forest)) {
        if *region != 0 && *density >= FOREST_DENSITY {
            *counts.entry(*region).or_default() += 1;
        }
    }
    counts.into_iter().filter(|(_, samples)| *samples >= FOREST_SAMPLES).map(|(region, _)| region).collect()
}

/// The number of the region that a woodcutter put at `at` would be in, or why it can't go there: it has to be on
/// the map, on dry land, and in a region that is in `forest` (see [`forest_regions`]).
pub fn site_region(world: &World, forest: &HashSet<u16>, at: Vec2) -> Result<u16, String> {
    let half = world.half_extents();
    if at.x.abs() > half.x || at.y.abs() > half.y {
        return Err("That is off the map.".to_owned());
    }
    if world.water_class(at.x, at.y) != WaterClass::Dry {
        return Err("A woodcutter has to stand on dry land.".to_owned());
    }
    let region = world.region_at(at.x, at.y).ok_or("This map has no regions to put a woodcutter in.")?.raw();
    if !forest.contains(&region) {
        return Err("A woodcutter can only go in a region that has some forest.".to_owned());
    }
    Ok(region)
}

impl DaySave {
    /// The woodcutter with this id.
    pub fn woodcutter(&self, id: WoodcutterId) -> Option<&Woodcutter> {
        self.woodcutters.iter().find(|w| w.id == id)
    }

    /// The woodcutter in the map region numbered `region`, if there is one.
    pub fn woodcutter_in(&self, region: u16) -> Option<&Woodcutter> {
        self.woodcutters.iter().find(|w| w.region == region)
    }

    /// Why a woodcutter can't be built now, or `None` if one can: the country being played has to be there and able to
    /// pay for it.
    pub fn woodcutter_problem(&self) -> Option<String> {
        match self.player_country() {
            None => Some("There is no country yet to own a woodcutter.".to_owned()),
            Some(country) if country.money < WOODCUTTER_COST => {
                Some(format!("Not enough coin: a woodcutter costs {} coins, and {} has {}.", format::money(WOODCUTTER_COST), country.name, format::money(country.money)))
            }
            Some(_) => None,
        }
    }

    /// Why the country being played can't have a woodcutter in the region numbered `region`, or `None` if it can: the
    /// region can't belong to another country, and can't have a woodcutter in it already.
    pub fn woodcutter_region_problem(&self, region: u16) -> Option<String> {
        let country = self.player_country()?.id;
        self.city_region_problem(country, Some(region)).or_else(|| self.woodcutter_in(region).map(|_| "There is already a woodcutter in this region.".to_owned()))
    }

    /// Builds a woodcutter at (`x`, `z`) in the region numbered `region` for the country being played, which pays for
    /// it. Whether the place is fit for one (see [`site_region`]) is for the caller to say. An error says why it can't
    /// be done.
    pub fn build_woodcutter(&mut self, x: f32, z: f32, region: u16) -> Result<(), String> {
        if let Some(problem) = self.woodcutter_problem() {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.woodcutter_region_problem(region) {
            return Err(problem);
        }
        let country = self.player_country().ok_or("There is no country yet to own a woodcutter.")?.id;
        self.add_money(country, -WOODCUTTER_COST)?;
        self.woodcutters.push(Woodcutter { id: WoodcutterId::new(), country, x, z, region });
        Ok(())
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
        assert_eq!(forest_regions(&world), HashSet::from([1]));
        // A patch only faintly painted doesn't count either.
        let mut faint = world_with_forest(|_, _| false);
        faint.resource_mut(Resource::Forest).fill(FOREST_DENSITY - 1);
        assert!(forest_regions(&faint).is_empty());
        // Nor does a map with no regions.
        assert!(forest_regions(&World::new(512.0, 512.0)).is_empty());
    }

    #[test]
    fn a_woodcutter_goes_on_dry_land_in_a_forest_region_on_the_map() {
        let world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        let forest = forest_regions(&world);
        let west = vec2(-100.0, 0.0);
        assert_eq!(site_region(&world, &forest, west), Ok(1));
        // The other region has no forest, so is refused, whether or not there is forest on the spot itself.
        assert!(site_region(&world, &forest, vec2(100.0, 0.0)).unwrap_err().contains("some forest"));
        assert!(site_region(&world, &forest, vec2(9_999.0, 0.0)).unwrap_err().contains("off the map"));
        // Water is no place for one, even in a forest region.
        let mut flooded = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        flooded.water_mut().fill(20.0);
        assert!(site_region(&flooded, &forest, west).unwrap_err().contains("dry land"));
        // Nor is a map not divided into regions.
        let plain = World::new(512.0, 512.0);
        assert!(site_region(&plain, &forest, Vec2::ZERO).is_err());
    }

    #[test]
    fn a_woodcutter_costs_1000_coins_from_the_country_being_played_and_belongs_to_it() {
        let mut day = day_with_money(2_500);
        day.found_country("Brevon").unwrap();
        day.build_woodcutter(-100.0, 20.0, 7).unwrap();
        assert_eq!(day.woodcutters.len(), 1);
        let aldoria = day.countries[0].id;
        let cutter = day.woodcutters[0];
        assert_eq!((cutter.country, cutter.x, cutter.z, cutter.region), (aldoria, -100.0, 20.0, 7));
        assert_eq!(day.countries[0].money, 1_500);
        assert_eq!(day.countries[1].money, crate::country::STARTING_MONEY, "only the country being played pays");
        assert_eq!(day.woodcutter(cutter.id), Some(&cutter));
        assert_eq!(day.woodcutter_in(7), Some(&cutter));
        assert_eq!(day.woodcutter_in(8), None);
    }

    #[test]
    fn a_woodcutter_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(999);
        let before = day.clone();
        let error = day.build_woodcutter(0.0, 0.0, 1).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("1,000") && error.contains("Aldoria") && error.contains("999"), "{error}");
        assert_eq!(day, before);
        assert!(day.woodcutter_problem().is_some());
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.woodcutter_problem(), None);
        day.build_woodcutter(0.0, 0.0, 1).unwrap();
        assert_eq!(day.countries[0].money, 0);
        // With no country there is nobody to own it.
        let mut empty = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert!(empty.woodcutter_problem().unwrap().contains("no country"));
        assert!(empty.build_woodcutter(0.0, 0.0, 1).is_err());
    }

    #[test]
    fn there_can_be_only_one_in_a_region_and_nothing_is_paid_for_the_second() {
        let mut day = day_with_money(5_000);
        day.build_woodcutter(-100.0, 0.0, 3).unwrap();
        let before = day.clone();
        assert!(day.woodcutter_region_problem(3).unwrap().contains("already a woodcutter"));
        assert!(day.build_woodcutter(-50.0, 40.0, 3).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before);
        // Another region is fine.
        assert_eq!(day.woodcutter_region_problem(4), None);
        day.build_woodcutter(50.0, 0.0, 4).unwrap();
        assert_eq!(day.woodcutters.len(), 2);
    }

    #[test]
    fn a_region_that_belongs_to_another_country_is_refused() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.set_region_owner(5, Some(brevon)).unwrap();
        let before = day.clone();
        assert!(day.build_woodcutter(0.0, 0.0, 5).unwrap_err().contains("belongs to Brevon"));
        assert_eq!(day, before);
        // One that is the player's own is fine.
        day.set_region_owner(6, Some(day.countries[0].id)).unwrap();
        day.build_woodcutter(0.0, 0.0, 6).unwrap();
    }

    #[test]
    fn woodcutters_are_saved_with_the_day_carry_forward_and_go_with_a_deleted_country() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        day.build_woodcutter(-100.0, 20.0, 7).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["woodcutters"][0]["region"], 7);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).woodcutters, day.woodcutters);
        // A day saved before there were woodcutters has none.
        let mut old = serde_json::to_value(&day).unwrap();
        old.as_object_mut().unwrap().remove("woodcutters");
        assert!(serde_json::from_value::<DaySave>(old).unwrap().woodcutters.is_empty());
        // The other country, which has none, can go; the one that owns it can't while it has cities, but can without.
        day.delete_country(day.countries[1].id).unwrap();
        assert_eq!(day.woodcutters.len(), 1);
        day.found_country("Cardell").unwrap();
        day.select_country(day.countries[1].id).unwrap();
        day.delete_country(day.countries[0].id).unwrap();
        assert!(day.woodcutters.is_empty(), "it was Aldoria's");
    }
}
