//! Founding cities, changing the country one belongs to, and putting one under siege.

use super::DaySave;
use crate::city::{self, City, INITIAL_POPULATION, MIN_POPULATION, SETTLERS};
use crate::city_layout;
use crate::format;
use crate::ids::{CityId, CountryId};
use crate::newsreel::{Event, News};

/// How much clear ground, in meters, there has to be between the edges of two cities.
const CITY_GAP: f32 = 20.0;

/// How far apart (`ax`, `az`) and (`bx`, `bz`) are, in meters.
fn vec2_distance(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt()
}

impl DaySave {
    /// Why a city of `country` can't stand in `region` (`None` for a map with no regions): a region that
    /// belongs to another country is that country's.
    pub fn city_region_problem(&self, country: CountryId, region: Option<u16>) -> Option<String> {
        let owner = self.region_owner(region?).filter(|&o| o != country)?;
        Some(format!("That region belongs to {}.", self.country_name(owner)))
    }

    /// Makes the city with this id belong to `country`, and its region too. An error says why it can't be
    /// done: a region can only belong to one country, so a city can't change if another city of its old
    /// country stands in the same region.
    pub fn set_city_country(&mut self, city: CityId, country: CountryId) -> Result<(), String> {
        self.country(country).ok_or("There is no such country.")?;
        let region = self.city(city).ok_or("There is no such city.")?.region;
        if let Some(other) = self.cities.iter().find(|c| c.id != city && region.is_some() && c.region == region && c.country != country) {
            return Err(format!("{} also stands in that region, so it has to stay in {}.", other.name, self.country_name(other.country)));
        }
        let mut from = None;
        if let Some(c) = self.cities.iter_mut().find(|c| c.id == city) {
            from = Some(std::mem::replace(&mut c.country, country));
        }
        match from {
            Some(from) if from != country => self.newsreel.push(News::about(city, Event::CityChangedCountry { from, to: country })),
            _ => {}
        }
        if let Some(region) = region {
            self.put_region_owner(region, Some(country));
        }
        Ok(())
    }

    /// Puts the city with this id under siege, or lifts the siege, which is reported in the newsreel unless it made no
    /// difference. An error says why it can't be done.
    pub fn set_city_under_siege(&mut self, city: CityId, under_siege: bool) -> Result<(), String> {
        let city = self.cities.iter_mut().find(|c| c.id == city).ok_or("There is no such city.")?;
        let changed = std::mem::replace(&mut city.under_siege, under_siege) != under_siege;
        let (id, population) = (city.id, city.population);
        if changed {
            self.newsreel.push(News::about(id, Event::SiegeChanged { under_siege, population }));
        }
        Ok(())
    }

    /// Why a new city can't be settled from the city with this id now, or `None` if it can: the country being played has to
    /// own that city, which has to have 100 people to spare and still be left with the fewest a city can have, and the
    /// country has to be able to pay (see [`city::settle_cost`]), the coins first and then each resource.
    pub fn settle_problem(&self, from: CityId) -> Option<String> {
        let Some(country) = self.player_country() else { return Some("There is no country yet to settle a city.".to_owned()) };
        let Some(source) = self.city(from) else { return Some("There is no such city.".to_owned()) };
        if source.country != country.id {
            return Some(format!("{} belongs to {}, and {} can only take settlers from its own cities.", source.name, self.country_name(source.country), country.name));
        }
        let needed = SETTLERS + MIN_POPULATION;
        if source.population < needed {
            return Some(format!("{} has {} people, and needs {} to spare {} and keep {}.", source.name, format::thousands(source.population), format::thousands(needed), format::thousands(SETTLERS), format::thousands(MIN_POPULATION)));
        }
        if country.money < city::SETTLE_COINS {
            return Some(format!("Not enough coin: a new city costs {} coins, and {} has {}.", format::money(city::SETTLE_COINS), country.name, format::money(country.money)));
        }
        city::SETTLE_MATERIALS.iter().find(|(resource, needed)| country.stock.get(*resource) < *needed).map(|(resource, needed)| {
            let name = crate::country::resource_name(*resource);
            format!("Not enough {name}: a new city needs {} {name} as well as its coins, and {} has {}.", format::thousands_u64(*needed), country.name, format::thousands_u64(country.stock.get(*resource)))
        })
    }

    /// Why a city can't stand at (`x`, `z`) because of the cities already there: it can't be on top of one.
    pub fn crowding_problem(&self, x: f32, z: f32) -> Option<String> {
        let new = City { id: CityId::new(), country: CountryId::new(), name: String::new(), x, z, region: None, population: INITIAL_POPULATION, under_siege: false };
        let new_radius = city_layout::radius(&new);
        self.cities
            .iter()
            .find(|c| vec2_distance(c.x, c.z, x, z) < city_layout::radius(c) + new_radius + CITY_GAP)
            .map(|c| format!("That is too close to {}.", c.name))
    }

    /// Settles a new city called `typed` at (`x`, `z`) in the map region numbered `region` (`None` if the map has none), for
    /// the country being played, with 100 people taken from its city `from`. It costs 10,000 coins, 1,000 wood, 1,000 stone
    /// and 100 iron. The region becomes the country's, unless another country owns it, which is an error. Whether the
    /// ground is fit for a city is for the caller to say. This is reported in the newsreel. An error says why it can't be
    /// done, and nothing changes.
    pub fn settle_city(&mut self, from: CityId, typed: &str, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.settle_problem(from) {
            return Err(problem);
        }
        let country = self.player_country().ok_or("There is no country yet to settle a city.")?.id;
        if let Some(problem) = self.city_region_problem(country, region) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.crowding_problem(x, z) {
            return Err(problem);
        }
        let name = city::clean_name(typed)?;
        if self.cities.iter().any(|c| c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a city called {name}."));
        }
        self.add_money(country, -city::SETTLE_COINS)?;
        for (resource, needed) in city::SETTLE_MATERIALS {
            self.add_resource(country, resource, -(needed as i64))?;
        }
        if let Some(source) = self.cities.iter_mut().find(|c| c.id == from) {
            source.population -= SETTLERS;
        }
        let id = CityId::new();
        self.cities.push(City { id, country, name, x, z, region, population: SETTLERS, under_siege: false });
        self.newsreel.push(News::about(id, Event::CitySettled { population: SETTLERS, region, country, from, cost: city::settle_cost() }));
        if let Some(region) = region {
            self.put_region_owner(region, Some(country));
        }
        Ok(())
    }

    /// Founds a city called `typed` at (`x`, `z`) in the map region numbered `region` (`None` if the map has
    /// none) with the starting population, for the player's country, and reports it in the newsreel. The
    /// region becomes the country's, unless another country owns it, which is an error. Whether the ground
    /// is fit for a city is for the caller to say. An error says why it can't be done.
    pub fn found_city(&mut self, typed: &str, x: f32, z: f32, region: Option<u16>) -> Result<(), String> {
        let country = self.player_country().ok_or("There is no country yet for the city to belong to.")?.id;
        if let Some(problem) = self.city_region_problem(country, region) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let name = city::clean_name(typed)?;
        if self.cities.iter().any(|c| c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a city called {name}."));
        }
        let id = CityId::new();
        self.cities.push(City { id, country, name, x, z, region, population: INITIAL_POPULATION, under_siege: false });
        self.newsreel.push(News::about(id, Event::CityFounded { population: INITIAL_POPULATION, region, country: Some(country) }));
        if let Some(region) = region {
            self.put_region_owner(region, Some(country));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;
    use crate::newsreel::{Event, News};

    /// A day with Aldoria, which has 20,000 coins and plenty of every resource, and its city Port Aldor of 1,000 people.
    fn day_that_can_settle() -> DaySave {
        use nworld_map::world::Resource;
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 20_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 2_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        day.cities[0].population = 1_000;
        day
    }

    #[test]
    fn a_new_city_takes_100_people_from_another_and_10000_coins_1000_wood_1000_stone_and_100_iron() {
        let mut day = day_that_can_settle();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        assert_eq!(day.settle_problem(port), None);
        day.settle_city(port, "  Highmark ", 500.0, 0.0, Some(0x4006)).unwrap();

        let new = day.cities[1].clone();
        assert_eq!((new.name.as_str(), new.country, new.population, new.x, new.z, new.region), ("Highmark", aldoria, 100, 500.0, 0.0, Some(0x4006)));
        assert_eq!(day.cities[0].population, 900, "the settlers came from Port Aldor");
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, 10_000);
        assert_eq!((country.stock.wood, country.stock.stone, country.stock.iron), (1_000, 1_000, 1_900));
        assert_eq!((country.stock.copper, country.stock.sulphur, country.stock.oil), (2_000, 2_000, 2_000), "nothing else is taken");
        assert_eq!(day.region_owner(0x4006), Some(aldoria), "the region is now Aldoria's");
        // (The region changing hands is the last news, after the city.)
        assert_eq!(day.newsreel.iter().rev().nth(1), Some(&News::about(new.id, Event::CitySettled { population: 100, region: Some(0x4006), country: aldoria, from: port, cost: city::settle_cost() })));
    }

    #[test]
    fn the_cost_in_the_newsreel_lists_everything_it_took() {
        let mut day = day_that_can_settle();
        let port = day.cities[0].id;
        day.settle_city(port, "Highmark", 500.0, 0.0, None).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria settles Highmark, a city of 100 people from Port Aldor for 10,000 coins, 1,000 wood, 1,000 stone and 100 iron");
    }

    #[test]
    fn a_new_city_needs_the_coin_and_each_resource_and_says_which_is_short() {
        use nworld_map::world::Resource;
        let port = |day: &DaySave| day.cities[0].id;
        let aldoria = |day: &DaySave| day.countries[0].id;

        let mut day = day_that_can_settle();
        let id = aldoria(&day);
        day.add_money(id, -10_001).unwrap(); // 9,999 left.
        let err = day.settle_problem(port(&day)).unwrap();
        assert!(err.contains("Not enough coin") && err.contains("10,000") && err.contains("9,999"), "{err}");

        for (resource, name, short_by) in [(Resource::Forest, "wood", 1_000), (Resource::Stone, "stone", 1_000), (Resource::Iron, "iron", 100)] {
            let mut day = day_that_can_settle();
            let id = aldoria(&day);
            day.add_resource(id, resource, -(2_000 - short_by as i64 + 1)).unwrap(); // One short.
            let before = day.clone();
            let err = day.settle_city(port(&day), "Highmark", 500.0, 0.0, None).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")) && err.contains(&format!("has {}", crate::format::thousands_u64(short_by - 1))), "{err}");
            assert_eq!(day, before, "a refused city changes nothing");
            day.add_resource(id, resource, 1).unwrap();
            day.settle_city(port(&day), "Highmark", 500.0, 0.0, None).unwrap();
        }
    }

    #[test]
    fn the_city_the_people_come_from_must_be_the_countrys_and_keep_at_least_100() {
        let mut day = day_that_can_settle();
        let (aldoria, brevon, port) = (day.countries[0].id, day.countries[1].id, day.cities[0].id);
        day.cities[0].population = 199;
        let err = day.settle_problem(port).unwrap();
        assert!(err.contains("Port Aldor has 199 people") && err.contains("needs 200"), "{err}");
        day.cities[0].population = 200;
        assert_eq!(day.settle_problem(port), None, "exactly enough");
        day.settle_city(port, "Highmark", 500.0, 0.0, None).unwrap();
        assert_eq!(day.cities[0].population, 100, "it is left with the fewest a city has");

        // A city of another country is not Aldoria's to take people from, and nor is one that isn't there.
        day.cities[0].population = 1_000;
        day.set_city_country(port, brevon).unwrap();
        assert!(day.settle_problem(port).unwrap().contains("belongs to Brevon"));
        assert!(day.settle_problem(CityId::new()).unwrap().contains("no such city"));
        assert_eq!(day.player_country().unwrap().id, aldoria);
    }

    #[test]
    fn a_new_city_cannot_go_in_a_region_of_another_country_on_another_city_or_under_a_taken_name() {
        let mut day = day_that_can_settle();
        let (brevon, port) = (day.countries[1].id, day.cities[0].id);
        day.put_region_owner(0x4009, Some(brevon));
        let before = day.clone();
        assert!(day.settle_city(port, "Highmark", 500.0, 0.0, Some(0x4009)).unwrap_err().contains("belongs to Brevon"));
        assert!(day.settle_city(port, "Highmark", f32::NAN, 0.0, None).is_err());
        assert!(day.settle_city(port, "Highmark", 5.0, 0.0, None).unwrap_err().contains("too close to Port Aldor"));
        assert!(day.settle_city(port, "port aldor", 500.0, 0.0, None).unwrap_err().contains("already a city"));
        assert!(day.settle_city(port, "  ", 500.0, 0.0, None).is_err());
        assert_eq!(day, before, "nothing was taken for any of them");
        assert_eq!(day.crowding_problem(500.0, 0.0), None);
    }

    #[test]
    fn a_new_city_is_saved_and_can_settle_another() {
        let mut day = day_that_can_settle();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.settle_city(port, "Highmark", 500.0, 0.0, None).unwrap();
        day.add_money(aldoria, 10_000).unwrap();
        let highmark = day.cities[1].id;
        day.cities[1].population = 300;
        day.settle_city(highmark, "Lowmoor", 1_000.0, 0.0, None).unwrap();
        assert_eq!((day.cities[1].population, day.cities[2].population), (200, 100));
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
    }

    #[test]
    fn a_city_starts_with_100_people_in_the_first_country_by_its_id_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city("Port Aldor", 120.0, -45.5, None).unwrap();

        let city = &day.cities[0];
        assert_eq!((city.country, city.name.as_str(), city.x, city.z, city.population), (day.countries[0].id, "Port Aldor", 120.0, -45.5, 100));
        assert_eq!(day.newsreel.last(), Some(&News::about(city.id, Event::CityFounded { population: 100, region: None, country: Some(city.country) })));
        assert_eq!(day.city(city.id), Some(city));
    }

    #[test]
    fn a_city_needs_a_country_and_a_place() {
        let mut day = day();
        assert!(day.found_city("Port Aldor", 0.0, 0.0, None).unwrap_err().contains("no country"));
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_city("Port Aldor", f32::NAN, 0.0, None).is_err());
        assert!(day.found_city("  ", 0.0, 0.0, None).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn each_city_gets_its_own_id() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_city("Highmark", 100.0, 0.0, None).unwrap();
        assert_ne!(day.cities[0].id, day.cities[1].id);
    }

    #[test]
    fn two_cities_cannot_share_a_name() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        let before = day.clone();
        assert!(day.found_city("port aldor", 100.0, 0.0, None).unwrap_err().contains("already"));
        assert_eq!(day, before);
    }

    #[test]
    fn a_city_can_change_country_and_takes_its_region_with_it() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        day.found_city("Highmark", 500.0, 0.0, Some(0x4006)).unwrap();
        let id = day.cities[0].id;

        day.set_city_country(id, brevon).unwrap();
        assert_eq!(day.city(id).unwrap().country, brevon);
        assert_eq!(day.region_owner(0x4005), Some(brevon));
        assert_eq!(day.region_owners.len(), 2, "no region owned twice");
        assert_eq!(day.region_owner(0x4006), Some(aldoria), "other cities and regions stay as they were");
        // The same again changes nothing.
        let before = day.clone();
        day.set_city_country(id, brevon).unwrap();
        assert_eq!(day, before);
    }

    #[test]
    fn a_city_cannot_change_country_from_another_city_in_its_region() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        day.found_city("Highmark", 10.0, 0.0, Some(0x4005)).unwrap();
        let before = day.clone();
        let err = day.set_city_country(day.cities[0].id, brevon).unwrap_err();
        assert!(err.contains("Highmark") && err.contains("Aldoria"), "{err}");
        assert_eq!(day, before);
        assert!(day.set_city_country(CityId::new(), brevon).is_err());
        assert!(day.set_city_country(day.cities[0].id, CountryId::new()).is_err());
    }

    #[test]
    fn a_city_on_a_map_without_regions_can_change_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_city("Highmark", 10.0, 0.0, None).unwrap();
        let brevon = day.countries[1].id;
        day.set_city_country(day.cities[0].id, brevon).unwrap();
        assert_eq!(day.cities[0].country, brevon);
        assert!(day.region_owners.is_empty());
    }

    #[test]
    fn a_city_starts_free_and_can_be_put_under_siege_and_freed() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_city("Highmark", 100.0, 0.0, None).unwrap();
        let (port, highmark) = (day.cities[0].id, day.cities[1].id);
        assert!(day.cities.iter().all(|c| !c.under_siege));

        day.set_city_under_siege(port, true).unwrap();
        assert!(day.city(port).unwrap().under_siege);
        assert!(!day.city(highmark).unwrap().under_siege, "each city has its own");
        // It is saved, carries forward, and can be lifted.
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["cities"][0]["under_siege"], true);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert!(day.following(Date { year: 1, month: 1, day: 2 }).city(port).unwrap().under_siege);
        day.set_city_under_siege(port, false).unwrap();
        assert!(!day.city(port).unwrap().under_siege);
        assert!(day.set_city_under_siege(CityId::new(), true).is_err());
    }

    #[test]
    fn a_city_saved_without_a_siege_is_not_under_one() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        let mut json = serde_json::to_value(&day).unwrap();
        json["cities"][0].as_object_mut().unwrap().remove("under_siege");
        let loaded: DaySave = serde_json::from_value(json).unwrap();
        assert!(!loaded.cities[0].under_siege);
    }
}
