//! The state of the game on one day, as saved in that day's file.
//!
//! Some of it is the state of the world, which carries forward from each day to the next: the
//! countries, the people and the cities. The rest belongs to the day alone, and the next day starts
//! without it: the newsreel.
//!
//! Everything in it refers to everything else by id, never by name: a person belongs to a country by
//! the country's [`CountryId`], and a news item names the thing it is about by its id.

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, BridgeLink};
use crate::calendar::Date;
use crate::city::{self, City, INITIAL_POPULATION};
use crate::country::{self, Country, RegionOwner, STARTING_MONEY};
use crate::ids::{CityId, CountryId, Guid, PersonId, TitleTypeId};
use crate::newsreel::{Event, News};
use crate::path::PathSegment;
use crate::person::{self, Person};
use crate::rgb::Rgb;
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
    /// Which country owns which regions of the map, carried forward from day to day. A region that is not
    /// here is unowned, as they all are to begin with. A region with a city in it belongs to the city's country.
    #[serde(default)]
    pub region_owners: Vec<RegionOwner>,
    /// The segments of path that have been built, which belong to no country. Carried forward from day to day.
    #[serde(default)]
    pub paths: Vec<PathSegment>,
    /// The segments of road that have been built, which belong to no country either. A road is a path of stone, and
    /// costs more. Carried forward from day to day.
    #[serde(default)]
    pub roads: Vec<PathSegment>,
    /// The bridges that have been built, which belong to no country. Carried forward from day to day.
    #[serde(default)]
    pub bridges: Vec<Bridge>,
    /// The segments of path that start at one end of a bridge. Carried forward from day to day.
    #[serde(default)]
    pub bridge_links: Vec<BridgeLink>,
    /// The segments of road that start at one end of a bridge. Carried forward from day to day.
    #[serde(default)]
    pub road_links: Vec<BridgeLink>,
    /// What happened today. Not carried forward.
    #[serde(default)]
    pub newsreel: Vec<News>,
}

impl DaySave {
    pub fn new(date: Date, map: &str) -> Self {
        Self { version: DAY_VERSION, date, map: map.to_owned(), title_types: Vec::new(), countries: Vec::new(), player: None, people: Vec::new(), cities: Vec::new(), region_owners: Vec::new(), paths: Vec::new(), roads: Vec::new(), bridges: Vec::new(), bridge_links: Vec::new(), road_links: Vec::new(), newsreel: Vec::new() }
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
    pub fn following(&self, date: Date) -> Self {
        Self { date, newsreel: Vec::new(), ..self.clone() }
    }

    /// The day after this one, as it starts: everything that carries forward (see [`following`](Self::following)),
    /// with the day's rules run on it (see [`crate::rules`]).
    pub fn next_day(&self) -> Self {
        let mut next = self.following(self.date.next());
        crate::rules::run(&mut next);
        next
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
        let color = Rgb::random_distinct_from(&self.countries.iter().map(|c| c.color).collect::<Vec<_>>());
        self.countries.push(Country { id, name, color, money: STARTING_MONEY });
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

    /// Adds `amount` coins to the money of the country with this id, or takes them away if it is negative (the
    /// money can go below zero). An error says why it can't be done.
    // For income and spending, which nothing does yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add_money(&mut self, id: CountryId, amount: i64) -> Result<(), String> {
        let country = self.countries.iter_mut().find(|c| c.id == id).ok_or("There is no such country.")?;
        country.money = country.money.checked_add(amount).ok_or("That is more money than there can be.")?;
        Ok(())
    }

    /// Gives the country with this id this color. An error says why it can't be done.
    pub fn recolor_country(&mut self, id: CountryId, color: Rgb) -> Result<(), String> {
        let country = self.countries.iter_mut().find(|c| c.id == id).ok_or("There is no such country.")?;
        country.color = color;
        Ok(())
    }

    /// Removes the country with this id, which can only be done if it has no cities. Its people and the regions
    /// it owns go with it (the regions become unowned), and so does the news of any of them. If it was the
    /// country the player plays, the first country left is. An error says why it can't be done.
    pub fn delete_country(&mut self, id: CountryId) -> Result<(), String> {
        let country = self.country(id).ok_or("There is no such country.")?;
        if self.cities.iter().any(|c| c.country == id) {
            return Err(format!("{} has cities, so it can't be deleted.", country.name));
        }
        let gone: Vec<Guid> = std::iter::once(id.into()).chain(self.people.iter().filter(|p| p.country == id).map(|p| Guid::from(p.id))).collect();
        self.countries.retain(|c| c.id != id);
        self.people.retain(|p| p.country != id);
        self.region_owners.retain(|o| o.country != id);
        self.newsreel.retain(|n| !gone.contains(&n.subject));
        if self.player == Some(id) {
            self.player = self.countries.first().map(|c| c.id);
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

    /// The country that owns the map region numbered `region`, if any does.
    pub fn region_owner(&self, region: u16) -> Option<CountryId> {
        self.region_owners.iter().find(|o| o.region == region).map(|o| o.country)
    }

    /// The color of the owner of each owned region, as the number of the region and the color, for showing
    /// who owns what.
    pub fn region_colors(&self) -> Vec<(u16, Rgb)> {
        self.region_owners.iter().filter_map(|o| Some((o.region, self.country(o.country)?.color))).collect()
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
        if let Some(c) = self.cities.iter_mut().find(|c| c.id == city) {
            c.country = country;
        }
        if let Some(region) = region {
            self.region_owners.retain(|o| o.region != region);
            self.region_owners.push(RegionOwner { region, country });
        }
        Ok(())
    }

    /// Puts the city with this id under siege, or lifts the siege. An error says why it can't be done.
    pub fn set_city_under_siege(&mut self, city: CityId, under_siege: bool) -> Result<(), String> {
        self.cities.iter_mut().find(|c| c.id == city).ok_or("There is no such city.")?.under_siege = under_siege;
        Ok(())
    }

    /// Why a city of `country` can't stand in `region` (`None` for a map with no regions): a region that
    /// belongs to another country is that country's.
    pub fn city_region_problem(&self, country: CountryId, region: Option<u16>) -> Option<String> {
        let owner = self.region_owner(region?).filter(|&o| o != country)?;
        Some(format!("That region belongs to {}.", self.country_name(owner)))
    }

    /// Makes `owner` the owner of the region numbered `region`, or leaves it unowned if `owner` is `None`.
    /// A region with a city in it can only belong to the city's country. An error says why it can't be
    /// done.
    #[allow(dead_code)] // For assigning regions by hand, which nothing does yet.
    pub fn set_region_owner(&mut self, region: u16, owner: Option<CountryId>) -> Result<(), String> {
        if let Some(owner) = owner {
            self.country(owner).ok_or("There is no such country.")?;
        }
        if let Some(city) = self.cities.iter().find(|c| c.region == Some(region) && Some(c.country) != owner) {
            return Err(format!("{} stands in that region, so it has to belong to {}.", city.name, self.country_name(city.country)));
        }
        self.region_owners.retain(|o| o.region != region);
        if let Some(country) = owner {
            self.region_owners.push(RegionOwner { region, country });
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
        if let Some(region) = region {
            self.region_owners.retain(|o| o.region != region);
            self.region_owners.push(RegionOwner { region, country });
        }
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
        day.found_city("Port Aldor", 120.0, -45.5, None).unwrap();

        let city = &day.cities[0];
        assert_eq!((city.country, city.name.as_str(), city.x, city.z, city.population), (day.countries[0].id, "Port Aldor", 120.0, -45.5, 100));
        assert_eq!(day.newsreel.last(), Some(&News::about(city.id, Event::CityFounded { population: 100 })));
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
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
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
    fn every_country_starts_with_a_color_and_it_can_be_changed() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        assert_ne!(day.countries[0].color, day.countries[1].color);

        let id = day.countries[0].id;
        let green = Rgb { r: 10, g: 200, b: 60 };
        day.recolor_country(id, green).unwrap();
        assert_eq!(day.country(id).unwrap().color, green);
        assert_ne!(day.countries[1].color, green);
        // The new color is saved, and read back.
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["countries"][0]["color"], "#0ac83c");
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
    }

    #[test]
    fn every_country_starts_with_the_same_money_and_keeps_its_own() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert!(day.countries.iter().all(|c| c.money == STARTING_MONEY));

        day.add_money(aldoria, 250).unwrap();
        day.add_money(brevon, -400).unwrap();
        assert_eq!(day.country(aldoria).unwrap().money, STARTING_MONEY + 250);
        assert_eq!(day.country(brevon).unwrap().money, STARTING_MONEY - 400);
        // It can go below zero, and come back up.
        day.add_money(brevon, -1_000).unwrap();
        assert_eq!(day.country(brevon).unwrap().money, STARTING_MONEY - 1_400);
        day.add_money(brevon, 2_000).unwrap();
        assert_eq!(day.country(brevon).unwrap().money, STARTING_MONEY + 600);
    }

    #[test]
    fn money_that_cannot_be_added_changes_nothing() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        let before = day.clone();
        assert!(day.add_money(CountryId::new(), 5).is_err());
        day.add_money(id, i64::MAX - STARTING_MONEY).unwrap();
        let full = day.clone();
        assert!(day.add_money(id, 1).unwrap_err().contains("more money"));
        assert_eq!(day, full);
        assert_ne!(full, before);
    }

    #[test]
    fn money_is_saved_with_the_country_and_carries_forward() {
        let mut today = day();
        today.found_country("Aldoria").unwrap();
        let id = today.countries[0].id;
        today.add_money(id, 234).unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["countries"][0]["money"], STARTING_MONEY + 234);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        // Tomorrow starts with what it ended today with, and the two days then go their own ways.
        let mut tomorrow = today.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.country(id).unwrap().money, STARTING_MONEY + 234);
        tomorrow.add_money(id, -100).unwrap();
        assert_eq!(today.country(id).unwrap().money, STARTING_MONEY + 234);
    }

    #[test]
    fn only_a_country_that_there_is_can_be_recolored() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        assert!(day.recolor_country(CountryId::new(), Rgb { r: 1, g: 2, b: 3 }).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn every_region_is_unowned_to_begin_with() {
        let mut day = day();
        assert!(day.region_owners.is_empty());
        day.found_country("Aldoria").unwrap();
        assert_eq!(day.region_owner(0x4005), None);
    }

    #[test]
    fn a_city_makes_its_region_belong_to_its_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        let aldoria = day.countries[0].id;
        assert_eq!(day.cities[0].region, Some(0x4005));
        assert_eq!(day.region_owner(0x4005), Some(aldoria));
        assert_eq!(day.region_owner(0x4006), None, "other regions stay as they were");
        // A second city of the same country in that region is fine, and doesn't own it twice.
        day.found_city("Highmark", 10.0, 0.0, Some(0x4005)).unwrap();
        assert_eq!(day.region_owners.len(), 1);
    }

    #[test]
    fn a_city_cannot_be_founded_in_a_region_that_belongs_to_another_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.set_region_owner(0x4005, Some(brevon)).unwrap();
        let before = day.clone();
        // The player is Aldoria.
        assert!(day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap_err().contains("belongs to Brevon"));
        assert_eq!(day.city_region_problem(day.countries[0].id, Some(0x4005)).unwrap(), "That region belongs to Brevon.");
        assert_eq!(day, before);
        // Brevon's own region, and any unowned one, are fine.
        assert_eq!(day.city_region_problem(brevon, Some(0x4005)), None);
        assert_eq!(day.city_region_problem(day.countries[0].id, Some(0x4006)), None);
        assert_eq!(day.city_region_problem(day.countries[0].id, None), None);
    }

    #[test]
    fn a_region_with_a_city_in_it_must_belong_to_the_citys_country() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();

        let before = day.clone();
        assert!(day.set_region_owner(0x4005, Some(brevon)).unwrap_err().contains("Port Aldor"));
        assert!(day.set_region_owner(0x4005, None).is_err());
        assert_eq!(day, before);
        // It can be set to its own country again, and other regions can go to anyone or to no one.
        day.set_region_owner(0x4005, Some(aldoria)).unwrap();
        day.set_region_owner(0x4006, Some(brevon)).unwrap();
        assert_eq!(day.region_owner(0x4006), Some(brevon));
        day.set_region_owner(0x4006, None).unwrap();
        assert_eq!(day.region_owner(0x4006), None);
        assert!(day.set_region_owner(0x4007, Some(CountryId::new())).is_err());
    }

    #[test]
    fn owned_regions_are_shown_in_their_owners_colors() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        assert!(day.region_colors().is_empty());
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        let green = Rgb { r: 10, g: 200, b: 60 };
        let id = day.countries[0].id;
        day.recolor_country(id, green).unwrap();
        assert_eq!(day.region_colors(), [(0x4005, green)]);
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

    #[test]
    fn who_owns_what_is_saved_and_carries_forward() {
        let mut today = day();
        today.found_country("Aldoria").unwrap();
        today.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        let json = serde_json::to_value(&today).unwrap();
        assert_eq!(json["region_owners"][0]["region"], 0x4005);
        assert_eq!(json["region_owners"][0]["country"], json["countries"][0]["id"]);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), today);
        assert_eq!(today.following(Date { year: 1, month: 1, day: 2 }).region_owners, today.region_owners);
    }

    #[test]
    fn a_country_with_no_cities_can_be_deleted_with_what_is_its_own() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.select_country(brevon).unwrap();
        day.found_person("Tomas Rue", "King", "Monarch").unwrap();
        day.set_region_owner(0x4005, Some(brevon)).unwrap();
        day.set_region_owner(0x4006, Some(aldoria)).unwrap();
        let tomas = day.people[1].id;

        day.delete_country(brevon).unwrap();
        assert_eq!(day.countries.len(), 1);
        assert!(day.country(brevon).is_none());
        assert_eq!(day.people.len(), 1, "its people go with it");
        assert_eq!(day.people[0].name, "Mara Voss");
        assert_eq!(day.region_owner(0x4005), None, "its regions are unowned");
        assert_eq!(day.region_owner(0x4006), Some(aldoria), "other countries' regions are not touched");
        // Its news goes: the founding and Tomas joining, and nothing of Aldoria's.
        assert!(day.newsreel.iter().all(|n| n.subject != Guid::from(brevon) && n.subject != Guid::from(tomas)));
        assert_eq!(day.newsreel.len(), 2, "Aldoria founded, Mara joined");
        // It was the country that was played, so the one left is.
        assert_eq!(day.player_country().unwrap().id, aldoria);
        assert_eq!(day.player, Some(aldoria));
    }

    #[test]
    fn a_country_with_cities_cannot_be_deleted() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        let before = day.clone();
        let err = day.delete_country(day.countries[0].id).unwrap_err();
        assert!(err.contains("Aldoria") && err.contains("cities"), "{err}");
        assert!(day.delete_country(CountryId::new()).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn the_last_country_can_be_deleted_and_a_new_one_founded() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.delete_country(day.countries[0].id).unwrap();
        assert!(day.countries.is_empty() && day.player.is_none() && day.player_country().is_none());
        day.found_country("Brevon").unwrap();
        assert_eq!(day.player_country().unwrap().name, "Brevon");
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
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();

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
        today.found_city("Port Aldor", 10.0, 20.0, None).unwrap();

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
