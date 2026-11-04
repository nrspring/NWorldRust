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
use crate::failure::Thing;
use crate::city::City;
use crate::country::{Country, FormerCountry, RegionOwner};
use crate::ids::{CityId, CountryId, PersonId};
use crate::newsreel::{Event, News};
use crate::path::PathSegment;
use crate::person::Person;
use crate::region_name::RegionName;
use crate::lake_name::LakeName;
use crate::place::Place;
use crate::river_name::RiverName;
use crate::saves::DAY_VERSION;
use crate::title_type::TitleType;
use crate::gatherer::Gatherer;
use crate::militia::Militia;
use crate::shipyard::Shipyard;

mod cities;
mod countries;
mod deleting;
mod militias;
mod people;
mod regions;

/// How much a country has and owns, as [`DaySave::country_totals`] counts it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CountryTotals {
    pub cities: usize,
    /// The people living in its cities.
    pub population: u64,
    /// The regions it owns.
    pub regions: usize,
    /// The people who belong to the country, such as its rulers.
    pub people: usize,
}

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
    /// The countries that were deleted, which the news of them still names. Carried forward from day to day.
    #[serde(default)]
    pub former_countries: Vec<FormerCountry>,
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
    /// The names the regions of the map have been given. A region that isn't here has no name. Carried forward from
    /// day to day.
    #[serde(default)]
    pub region_names: Vec<RegionName>,
    /// The names the rivers of the map have been given. A river that isn't here has no name. Carried forward from day
    /// to day.
    #[serde(default)]
    pub river_names: Vec<RiverName>,
    /// The names the lakes of the map have been given, in the same way.
    #[serde(default)]
    pub lake_names: Vec<LakeName>,
    /// The places the player has put labels on, such as a strait or a plain. Carried forward from day to day.
    #[serde(default)]
    pub places: Vec<Place>,
    /// The segments of road that have been built, which belong to no country either. A road is a path of stone, and
    /// costs more. Carried forward from day to day.
    #[serde(default)]
    pub roads: Vec<PathSegment>,
    /// The paths, roads and bridges that have failed, which are ruined until they are built again. Carried forward
    /// from day to day.
    #[serde(default)]
    pub failed: Vec<Thing>,
    /// The bridges that have been built, which belong to no country. Carried forward from day to day.
    #[serde(default)]
    pub bridges: Vec<Bridge>,
    /// The segments of path that start at one end of a bridge. Carried forward from day to day.
    #[serde(default)]
    pub bridge_links: Vec<BridgeLink>,
    /// The segments of road that start at one end of a bridge. Carried forward from day to day.
    #[serde(default)]
    pub road_links: Vec<BridgeLink>,
    /// The gatherers that have been built, each owned by a country. Carried forward from day to day.
    #[serde(default, alias = "woodcutters")]
    pub gatherers: Vec<Gatherer>,
    /// The militia that have been raised, each owned by a country. Carried forward from day to day.
    #[serde(default)]
    pub militias: Vec<Militia>,
    /// The shipyards that have been built, each owned by a country. Carried forward from day to day.
    #[serde(default)]
    pub shipyards: Vec<Shipyard>,
    /// What happened today. Not carried forward.
    #[serde(default)]
    pub newsreel: Vec<News>,
}

impl DaySave {
    pub fn new(date: Date, map: &str) -> Self {
        Self { version: DAY_VERSION, date, map: map.to_owned(), title_types: Vec::new(), countries: Vec::new(), former_countries: Vec::new(), player: None, people: Vec::new(), cities: Vec::new(), region_owners: Vec::new(), paths: Vec::new(), roads: Vec::new(), region_names: Vec::new(), river_names: Vec::new(), lake_names: Vec::new(), places: Vec::new(), failed: Vec::new(), bridges: Vec::new(), bridge_links: Vec::new(), road_links: Vec::new(), gatherers: Vec::new(), militias: Vec::new(), shipyards: Vec::new(), newsreel: Vec::new() }
    }

    /// The save of the day after this one, as it starts: with everything that carries forward, and
    /// an empty newsreel.
    pub fn following(&self, date: Date) -> Self {
        Self { date, newsreel: Vec::new(), ..self.clone() }
    }

    /// The day after this one, as it starts: everything that carries forward (see [`following`](Self::following)),
    /// with the day's rules run on it (see [`crate::rules`]), and the gatherers that the rules cut off from a city
    /// reported.
    pub fn next_day(&self) -> Self {
        let mut next = self.following(self.date.next());
        next.newsreel.push(News::happening(Event::DayBegan { date: next.date }));
        crate::rules::run(&mut next);
        next.update_gatherer_operation();
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

    /// The name of the country with this id, for showing: the name it had if it was deleted.
    pub fn country_name(&self, id: CountryId) -> &str {
        self.country(id)
            .map(|c| c.name.as_str())
            .or_else(|| self.former_countries.iter().find(|c| c.id == id).map(|c| c.name.as_str()))
            .unwrap_or("an unknown country")
    }

    /// The country the player plays: the one chosen, or else the first there is.
    pub fn player_country(&self) -> Option<&Country> {
        self.player.and_then(|id| self.country(id)).or_else(|| self.countries.first())
    }
}

#[cfg(test)]
fn day() -> DaySave {
    DaySave::new(Date::FIRST, "Assets/map.nworld")
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(today.newsreel.len(), 4, "the country, the kind of title, the person and the city");
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
        assert_eq!(json["newsreel"][1]["subject"], json["title_types"][0]["id"]);
        assert_eq!(json["newsreel"][2]["subject"], json["people"][0]["id"]);
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
