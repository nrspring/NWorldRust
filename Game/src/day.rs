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
use crate::ids::{CityId, CountryId, PersonId, TitleTypeId};
use crate::newsreel::{Event, News};
use crate::person::{self, Person};
use crate::saves::DAY_VERSION;
use crate::title_type::{self, TitleType};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DaySave {
    pub(crate) version: u32,
    pub date: Date,
    /// The map the game is played on, relative to the data folder.
    pub map: String,
    /// The kinds of title there are: none to begin with, and one more each time a person is added with a
    /// kind that isn't here yet. Carried forward from day to day.
    #[serde(default)]
    pub title_types: Vec<TitleType>,
    /// Carried forward from day to day.
    #[serde(default)]
    pub countries: Vec<Country>,
    /// The country the player plays, which new people and cities belong to. The first country founded
    /// is it until another is chosen. Carried forward from day to day.
    #[serde(default)]
    pub player: Option<CountryId>,
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
        Self { version: DAY_VERSION, date, map: map.to_owned(), title_types: Vec::new(), countries: Vec::new(), player: None, people: Vec::new(), cities: Vec::new(), newsreel: Vec::new() }
    }

    // For showing the kind of title a person has, which nothing shows yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn title_type(&self, id: TitleTypeId) -> Option<&TitleType> {
        self.title_types.iter().find(|t| t.id == id)
    }

    /// The title type called `name`, ignoring capitals.
    pub fn title_type_named(&self, name: &str) -> Option<&TitleType> {
        self.title_types.iter().find(|t| t.name.to_lowercase() == name.to_lowercase())
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

    /// The country the player plays: the one chosen, or else the first there is.
    pub fn player_country(&self) -> Option<&Country> {
        self.player.and_then(|id| self.country(id)).or_else(|| self.countries.first())
    }

    /// Founds a country called `typed` today, and reports it in the newsreel. The first country there is
    /// becomes the player's. An error says why it can't be done.
    pub fn found_country(&mut self, typed: &str) -> Result<(), String> {
        let name = self.free_country_name(typed, None)?;
        let id = CountryId::new();
        self.countries.push(Country { id, name });
        self.player.get_or_insert(id);
        self.newsreel.push(News::about(id, Event::CountryFounded));
        Ok(())
    }

    /// Makes the country with this id the one the player plays. An error says why it can't be done.
    pub fn select_country(&mut self, id: CountryId) -> Result<(), String> {
        self.country(id).ok_or("There is no such country.")?;
        self.player = Some(id);
        Ok(())
    }

    /// Renames the country with this id to `typed`. People, cities and news follow it, since they hold
    /// its id. A name that only differs in its capitals from the country's own is fine. An error says
    /// why it can't be done.
    pub fn rename_country(&mut self, id: CountryId, typed: &str) -> Result<(), String> {
        self.country(id).ok_or("There is no such country.")?;
        let name = self.free_country_name(typed, Some(id))?;
        if let Some(country) = self.countries.iter_mut().find(|c| c.id == id) {
            country.name = name;
        }
        Ok(())
    }

    /// `typed` as a country name, if it is a good one and no country other than `except` has it.
    fn free_country_name(&self, typed: &str, except: Option<CountryId>) -> Result<String, String> {
        let name = country::clean_name(typed)?;
        if self.countries.iter().any(|c| Some(c.id) != except && c.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There is already a country called {name}."));
        }
        Ok(name)
    }

    /// Adds a person called `name` with the given `title`, of the kind of title called `title_type`,
    /// to the player's country, and reports it in the newsreel. A `title_type` the game doesn't have yet
    /// is added to the kinds there are. An error says why it can't be done.
    pub fn found_person(&mut self, name: &str, title: &str, title_type: &str) -> Result<(), String> {
        let country = self.player_country().ok_or("There is no country yet for them to belong to.")?.id;
        let (name, title) = (person::clean_name(name)?, person::clean_title(title)?);
        let type_name = title_type::clean_name(title_type)?;
        let title_type = match self.title_type_named(&type_name) {
            Some(existing) => existing.id,
            None => {
                let id = TitleTypeId::new();
                self.title_types.push(TitleType { id, name: type_name });
                id
            }
        };
        let id = PersonId::new();
        self.people.push(Person { id, name, title, title_type, country });
        self.newsreel.push(News::about(id, Event::PersonJoined));
        Ok(())
    }

    /// Founds a city called `typed` at (`x`, `z`) with the starting population, for the player's country,
    /// and reports it in the newsreel. Whether the ground is fit for a city is for the caller to say.
    /// An error says why it can't be done.
    pub fn found_city(&mut self, typed: &str, x: f32, z: f32) -> Result<(), String> {
        let country = self.player_country().ok_or("There is no country yet for the city to belong to.")?.id;
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
        day.found_person(" Mara  Voss ", "President", "Head of State").unwrap();

        let aldoria = day.countries[0].id;
        let person = &day.people[0];
        assert_eq!((person.name.as_str(), person.title.as_str(), person.country), ("Mara Voss", "President", aldoria));
        assert_eq!(day.title_type(person.title_type).unwrap().name, "Head of State");
        assert_eq!(day.newsreel.last(), Some(&News::about(person.id, Event::PersonJoined)));
        assert_eq!(day.person(person.id), Some(person));
    }

    #[test]
    fn a_person_needs_a_country_a_name_and_a_title() {
        let mut day = day();
        assert!(day.found_person("Mara Voss", "President", "Head of State").unwrap_err().contains("no country"));
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.found_person("", "President", "Head of State").is_err());
        assert!(day.found_person("Mara Voss", "  ", "Head of State").is_err());
        assert!(day.found_person("Mara Voss", "President", "  ").is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn a_new_day_has_no_title_types() {
        assert!(day().title_types.is_empty());
    }

    #[test]
    fn the_first_person_makes_the_first_title_type() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "Queen", "Monarch").unwrap();
        assert_eq!(day.title_types.len(), 1);
        assert_eq!(day.title_type(day.people[0].title_type).unwrap().name, "Monarch");
    }

    #[test]
    fn a_title_type_that_exists_is_used_whatever_its_capitals() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "Queen", "Monarch").unwrap();
        day.found_person("Tomas Rue", "King", "  monarch ").unwrap();
        assert_eq!(day.title_types.len(), 1);
        assert_eq!(day.people[0].title_type, day.people[1].title_type);
    }

    #[test]
    fn a_new_title_type_is_added_once_and_kept_for_next_time() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let before = day.title_types.len();
        day.found_person("Mara Voss", "Sea Mother", "  Sea   Priest ").unwrap();
        assert_eq!(day.title_types.len(), before + 1);
        let added = day.title_types.last().unwrap().clone();
        assert_eq!(added.name, "Sea Priest");
        assert_eq!(day.people[0].title_type, added.id);

        // A second person of the same new type uses it, and doesn't add another.
        day.found_person("Tomas Rue", "Sea Father", "sea priest").unwrap();
        assert_eq!(day.title_types.len(), before + 1);
        assert_eq!(day.people[1].title_type, added.id);
        // And it carries forward to the next day.
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).title_types, day.title_types);
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
    fn the_first_country_is_the_players_until_another_is_chosen() {
        let mut day = day();
        assert!(day.player_country().is_none());
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert_eq!(day.player, Some(aldoria), "founding another doesn't change whose it is");
        assert_eq!(day.player_country().unwrap().id, aldoria);

        day.select_country(brevon).unwrap();
        assert_eq!(day.player_country().unwrap().name, "Brevon");
    }

    #[test]
    fn a_day_saved_without_a_chosen_country_plays_the_first() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.player = None;
        assert_eq!(day.player_country().unwrap().name, "Aldoria");
        // And one that names a country that is gone does too.
        day.player = Some(CountryId::new());
        assert_eq!(day.player_country().unwrap().name, "Aldoria");
    }

    #[test]
    fn people_and_cities_join_the_country_that_is_chosen() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0).unwrap();
        assert_eq!(day.people[0].country, brevon);
        assert_eq!(day.cities[0].country, brevon);
    }

    #[test]
    fn only_a_country_that_there_is_can_be_chosen() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.select_country(CountryId::new()).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn a_country_can_be_renamed_and_everything_of_it_follows() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.countries[0].id;
        day.rename_country(id, "  New   Aldoria ").unwrap();
        assert_eq!(day.countries[0].name, "New Aldoria");
        assert_eq!(day.countries[0].id, id);
        assert_eq!(day.country_name(day.people[0].country), "New Aldoria");
        // Just the capitals, or the very same name, is allowed.
        day.rename_country(id, "NEW ALDORIA").unwrap();
        assert_eq!(day.countries[0].name, "NEW ALDORIA");
    }

    #[test]
    fn a_country_cannot_be_renamed_to_a_bad_or_taken_name() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let before = day.clone();
        let brevon = day.countries[1].id;
        assert!(day.rename_country(brevon, " ").is_err());
        assert!(day.rename_country(brevon, "aldoria").unwrap_err().contains("already"));
        assert!(day.rename_country(CountryId::new(), "Cardell").is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn what_refers_to_a_country_follows_it_by_id_not_by_name() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0).unwrap();

        // Rename the country: the person, the city and the news still find it.
        let id = day.countries[0].id;
        day.rename_country(id, "New Aldoria").unwrap();
        assert_eq!(day.country_name(day.people[0].country), "New Aldoria");
        assert_eq!(day.country_name(day.cities[0].country), "New Aldoria");
        assert_eq!(day.newsreel[0].headline(&day), "The nation of New Aldoria is founded");
    }

    #[test]
    fn countries_people_and_cities_carry_forward_but_the_newsreel_does_not() {
        let mut today = day();
        today.found_country("Aldoria").unwrap();
        today.found_person("Mara Voss", "President", "Head of State").unwrap();
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
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let json: serde_json::Value = serde_json::to_value(&day).unwrap();
        let country_id = json["countries"][0]["id"].as_str().unwrap();
        assert_eq!(country_id.len(), 36);
        assert_eq!(json["people"][0]["country"].as_str().unwrap(), country_id);
        // A person's kind of title is the id of one of the day's title types, which are saved too.
        let type_id = json["people"][0]["title_type"].as_str().unwrap();
        let named: Vec<&str> = json["title_types"].as_array().unwrap().iter().filter(|t| t["id"] == type_id).map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(named, ["Head of State"]);
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
        assert!(day.title_types.is_empty());
    }
}
