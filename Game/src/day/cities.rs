//! Founding cities, changing the country one belongs to, and putting one under siege.

use super::DaySave;
use crate::city::{self, City, INITIAL_POPULATION};
use crate::ids::{CityId, CountryId};
use crate::newsreel::{Event, News};

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
