//! The state of the game on one day, as saved in that day's file.
//!
//! Some of it is the state of the world, which carries forward from each day to the next: the
//! countries, the people and the cities. The rest belongs to the day alone, and the next day starts without it:
//! the newsreel.

use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::city::{City, INITIAL_POPULATION};
use crate::country::{self, Country};
use crate::newsreel::News;
use crate::person::{self, Person};
use crate::saves::VERSION;

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
        Self { version: VERSION, date, map: map.to_owned(), countries: Vec::new(), people: Vec::new(), cities: Vec::new(), newsreel: Vec::new() }
    }

    /// The save of the day after this one, as it starts: with everything that carries forward, and
    /// an empty newsreel.
    // Used once days advance, which nothing does yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn following(&self, date: Date) -> Self {
        Self { date, newsreel: Vec::new(), ..self.clone() }
    }

    /// Founds a country called `typed` today, and reports it in the newsreel. An error says why it
    /// can't be done.
    pub fn found_country(&mut self, typed: &str) -> Result<(), String> {
        let name = country::clean_name(typed)?;
        if self.countries.iter().any(|c| c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a country called {name}."));
        }
        self.countries.push(Country { name: name.clone() });
        self.newsreel.push(News::CountryFounded { country: name });
        Ok(())
    }

    /// Adds a person called `name` with the given `title` to the first country, and reports it in
    /// the newsreel. An error says why it can't be done.
    pub fn found_person(&mut self, name: &str, title: &str) -> Result<(), String> {
        let country = self.countries.first().ok_or("There is no country yet for them to belong to.")?.name.clone();
        let (name, title) = (person::clean_name(name)?, person::clean_title(title)?);
        self.people.push(Person { name: name.clone(), title: title.clone(), country: country.clone() });
        self.newsreel.push(News::PersonJoined { person: name, title, country });
        Ok(())
    }

    /// Founds a city at (`x`, `z`) with the starting population, for the first country, and reports it
    /// in the newsreel. Whether the ground is fit for a city is for the caller to say. An error says
    /// why it can't be done.
    pub fn found_city(&mut self, x: f32, z: f32) -> Result<(), String> {
        let country = self.countries.first().ok_or("There is no country yet for the city to belong to.")?.name.clone();
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let id = self.cities.iter().map(|c| c.id + 1).max().unwrap_or(1);
        self.cities.push(City { id, country: country.clone(), x, z, population: INITIAL_POPULATION });
        self.newsreel.push(News::CityFounded { country, population: INITIAL_POPULATION });
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
    fn founding_a_country_adds_it_and_reports_it() {
        let mut day = day();
        day.found_country("  Aldoria ").unwrap();
        assert_eq!(day.countries, [Country { name: "Aldoria".to_owned() }]);
        assert_eq!(day.newsreel, [News::CountryFounded { country: "Aldoria".to_owned() }]);
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
    fn a_person_joins_the_first_country_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person(" Mara  Voss ", "President").unwrap();

        assert_eq!(day.people, [Person { name: "Mara Voss".to_owned(), title: "President".to_owned(), country: "Aldoria".to_owned() }]);
        assert_eq!(day.newsreel.last(), Some(&News::PersonJoined { person: "Mara Voss".to_owned(), title: "President".to_owned(), country: "Aldoria".to_owned() }));
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
    fn a_city_starts_with_100_people_in_the_first_country_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city(120.0, -45.5).unwrap();

        assert_eq!(day.cities, [City { id: 1, country: "Aldoria".to_owned(), x: 120.0, z: -45.5, population: 100 }]);
        assert_eq!(day.newsreel.last(), Some(&News::CityFounded { country: "Aldoria".to_owned(), population: 100 }));
    }

    #[test]
    fn a_city_needs_a_country_and_a_place() {
        let mut day = day();
        assert!(day.found_city(0.0, 0.0).unwrap_err().contains("no country"));
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_city(f32::NAN, 0.0).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn each_city_gets_its_own_id() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city(0.0, 0.0).unwrap();
        day.found_city(100.0, 0.0).unwrap();
        assert_eq!(day.cities.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn countries_people_and_cities_carry_forward_but_the_newsreel_does_not() {
        let mut today = day();
        today.found_country("Aldoria").unwrap();
        today.found_person("Mara Voss", "President").unwrap();
        today.found_city(10.0, 20.0).unwrap();

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
    fn a_day_saved_before_countries_existed_still_loads() {
        let old = r#"{ "version": 1, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" }"#;
        let day: DaySave = serde_json::from_str(old).unwrap();
        assert!(day.countries.is_empty() && day.people.is_empty() && day.cities.is_empty() && day.newsreel.is_empty());
    }
}
