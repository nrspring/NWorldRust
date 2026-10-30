//! The state of the game on one day, as saved in that day's file.
//!
//! Some of it is the state of the world, which carries forward from each day to the next: the
//! countries, the people and the cities. The rest belongs to the day alone, and the next day starts
//! without it: the newsreel.
//!
//! Everything in it refers to everything else by id, never by name: a person belongs to a country by
//! the country's [`CountryId`], and a news item names the thing it is about by its id.

use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::city::{self, City, INITIAL_POPULATION};
use crate::country::{self, Country};
use crate::ids::{CityId, CountryId, PersonId};
use crate::newsreel::{Event, News};
use crate::person::{self, Person};
use crate::saves::DAY_VERSION;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DaySave {
    pub(crate) version: u32,
    pub date: Date,
    /// The map the game is played on, relative to the data folder.
    pub map: String,
    /// Carried forward from day to day. The first is the player's.
    #[serde(default)]
    pub countries: Vec<Country>,
    /// Carried forward from day to day. Each belongs to one of the countries.
    #[serde(default)]
    pub people: Vec<Person>,
    /// Carried forward from day to day. Each belongs to one of the countries.
    #[serde(default)]
    pub cities: Vec<City>,
    /// What happened today. Not carried forward.
    #[serde(default)]
    pub newsreel: Vec<News>,
}

impl DaySave {
    pub fn new(date: Date, map: &str) -> Self {
        Self { version: DAY_VERSION, date, map: map.to_owned(), countries: Vec::new(), people: Vec::new(), cities: Vec::new(), newsreel: Vec::new() }
    }

    /// The save of the day after this one, as it starts: with everything that carries forward, and
    /// an empty newsreel.
    // Used once days advance, which nothing does yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn following(&self, date: Date) -> Self {
        Self { date, newsreel: Vec::new(), ..self.clone() }
    }

    /// The country with this id.
    pub fn country(&self, id: CountryId) -> Option<&Country> {
        self.countries.iter().find(|c| c.id == id)
    }

    pub fn person(&self, id: PersonId) -> Option<&Person> {
        self.people.iter().find(|p| p.id == id)
    }

    pub fn city(&self, id: CityId) -> Option<&City> {
        self.cities.iter().find(|c| c.id == id)
    }

    /// The name of the country with this id, for showing.
    pub fn country_name(&self, id: CountryId) -> &str {
        self.country(id).map_or("an unknown country", |c| c.name.as_str())
    }

    /// Founds a country called `typed` today, and reports it in the newsreel. An error says why it
    /// can't be done.
    pub fn found_country(&mut self, typed: &str) -> Result<(), String> {
        let name = country::clean_name(typed)?;
        if self.countries.iter().any(|c| c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a country called {name}."));
        }
        let id = CountryId::new();
        self.countries.push(Country { id, name });
        self.newsreel.push(News::about(id, Event::CountryFounded));
        Ok(())
    }

    /// Adds a person called `name` with the given `title` to the first country, and reports it in
    /// the newsreel. An error says why it can't be done.
    pub fn found_person(&mut self, name: &str, title: &str) -> Result<(), String> {
        let country = self.countries.first().ok_or("There is no country yet for them to belong to.")?.id;
        let (name, title) = (person::clean_name(name)?, person::clean_title(title)?);
        let id = PersonId::new();
        self.people.push(Person { id, name, title, country });
        self.newsreel.push(News::about(id, Event::PersonJoined));
        Ok(())
    }

    /// Founds a city called `typed` at (`x`, `z`) with the starting population, for the first country,
    /// and reports it in the newsreel. Whether the ground is fit for a city is for the caller to say.
    /// An error says why it can't be done.
    pub fn found_city(&mut self, typed: &str, x: f32, z: f32) -> Result<(), String> {
        let country = self.countries.first().ok_or("There is no country yet for the city to belong to.")?.id;
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let name = city::clean_name(typed)?;
        if self.cities.iter().any(|c| c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a city called {name}."));
        }
        let id = CityId::new();
        self.cities.push(City { id, country, name, x, z, population: INITIAL_POPULATION });
        self.newsreel.push(News::about(id, Event::CityFounded { population: INITIAL_POPULATION }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn founding_a_country_adds_it_with_an_id_and_reports_it() {
        let mut day = day();
        day.found_country("  Aldoria ").unwrap();
        assert_eq!(day.countries.len(), 1);
        assert_eq!(day.countries[0].name, "Aldoria");
        let id = day.countries[0].id;
        assert_eq!(day.newsreel, [News::about(id, Event::CountryFounded)]);
        assert_eq!(day.country(id).unwrap().name, "Aldoria");
    }

    #[test]
    fn every_country_has_an_id_of_its_own() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        assert_ne!(day.countries[0].id, day.countries[1].id);
    }

    #[test]
    fn a_bad_or_taken_name_changes_nothing() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_country("  ").is_err());
        assert!(day.found_country("aldoria").unwrap_err().contains("already"));
        assert_eq!(day, before);
    }

    #[test]
    fn a_person_joins_the_first_country_by_its_id_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person(" Mara  Voss ", "President").unwrap();

        let aldoria = day.countries[0].id;
        let person = &day.people[0];
        assert_eq!((person.name.as_str(), person.title.as_str(), person.country), ("Mara Voss", "President", aldoria));
        assert_eq!(day.newsreel.last(), Some(&News::about(person.id, Event::PersonJoined)));
        assert_eq!(day.person(person.id), Some(person));
    }

    #[test]
    fn a_person_needs_a_country_a_name_and_a_title() {
        let mut day = day();
        assert!(day.found_person("Mara Voss", "President").unwrap_err().contains("no country"));
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_person("", "President").is_err());
        assert!(day.found_person("Mara Voss", "  ").is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn a_city_starts_with_100_people_in_the_first_country_by_its_id_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city("Port Aldor", 120.0, -45.5).unwrap();

        let city = &day.cities[0];
        assert_eq!((city.country, city.name.as_str(), city.x, city.z, city.population), (day.countries[0].id, "Port Aldor", 120.0, -45.5, 100));
        assert_eq!(day.newsreel.last(), Some(&News::about(city.id, Event::CityFounded { population: 100 })));
        assert_eq!(day.city(city.id), Some(city));
    }

    #[test]
    fn a_city_needs_a_country_and_a_place() {
        let mut day = day();
        assert!(day.found_city("Port Aldor", 0.0, 0.0).unwrap_err().contains("no country"));
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_city("Port Aldor", f32::NAN, 0.0).is_err());
        assert!(day.found_city("  ", 0.0, 0.0).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn each_city_gets_its_own_id() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0).unwrap();
        day.found_city("Highmark", 100.0, 0.0).unwrap();
        assert_ne!(day.cities[0].id, day.cities[1].id);
    }

    #[test]
    fn two_cities_cannot_share_a_name() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0).unwrap();
        let before = day.clone();
        assert!(day.found_city("port aldor", 100.0, 0.0).unwrap_err().contains("already"));
        assert_eq!(day, before);
    }

    #[test]
    fn what_refers_to_a_country_follows_it_by_id_not_by_name() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0).unwrap();

        // Rename the country: the person, the city and the news still find it.
        day.countries[0].name = "New Aldoria".to_owned();
        assert_eq!(day.country_name(day.people[0].country), "New Aldoria");
        assert_eq!(day.country_name(day.cities[0].country), "New Aldoria");
        assert_eq!(day.newsreel[0].headline(&day), "The nation of New Aldoria is founded");
    }

    #[test]
    fn countries_people_and_cities_carry_forward_but_the_newsreel_does_not() {
        let mut today = day();
        today.found_country("Aldoria").unwrap();
        today.found_person("Mara Voss", "President").unwrap();
        today.found_city("Port Aldor", 10.0, 20.0).unwrap();

        let tomorrow = today.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.date, Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.countries, today.countries);
        assert_eq!(tomorrow.people, today.people);
        assert_eq!(tomorrow.cities, today.cities);
        assert_eq!(tomorrow.map, today.map);
        assert!(tomorrow.newsreel.is_empty());
        // Today's own newsreel is untouched.
        assert_eq!(today.newsreel.len(), 3);
    }

    #[test]
    fn the_saved_form_has_ids_that_refer_to_each_other() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President").unwrap();
        let json: serde_json::Value = serde_json::to_value(&day).unwrap();
        let country_id = json["countries"][0]["id"].as_str().unwrap();
        assert_eq!(country_id.len(), 36);
        assert_eq!(json["people"][0]["country"].as_str().unwrap(), country_id);
        // Each news item has the id of its subject, whatever kind of thing that is.
        assert_eq!(json["newsreel"][0]["subject"].as_str().unwrap(), country_id);
        assert_eq!(json["newsreel"][1]["subject"], json["people"][0]["id"]);
        // And it reads back the same.
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
    }

    #[test]
    fn a_day_with_nothing_in_it_still_loads() {
        let old = r#"{ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" }"#;
        let day: DaySave = serde_json::from_str(old).unwrap();
        assert!(day.countries.is_empty() && day.people.is_empty() && day.cities.is_empty() && day.newsreel.is_empty());
    }
}
