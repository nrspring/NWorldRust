//! Founding, renaming, choosing, coloring and counting the countries, and the money and the resources they hold.

use nworld_map::world::Resource;

use super::{CountryTotals, DaySave};
use crate::country::{self, resource_name, Country, Stock, STARTING_MONEY};
use crate::ids::CountryId;
use crate::newsreel::{Event, News};
use crate::rgb::Rgb;

impl DaySave {
    /// Founds a country called `typed` today, and reports it in the newsreel. The first country there is
    /// becomes the player's. An error says why it can't be done.
    pub fn found_country(&mut self, typed: &str) -> Result<(), String> {
        let name = self.free_country_name(typed, None)?;
        let id = CountryId::new();
        let color = Rgb::random_distinct_from(&self.countries.iter().map(|c| c.color).collect::<Vec<_>>());
        self.countries.push(Country { id, name, color, money: STARTING_MONEY, stock: Stock::default() });
        self.player.get_or_insert(id);
        self.newsreel.push(News::about(id, Event::CountryFounded));
        Ok(())
    }

    /// Makes the country with this id the one the player plays, which is reported in the newsreel unless it was already.
    /// An error says why it can't be done.
    pub fn select_country(&mut self, id: CountryId) -> Result<(), String> {
        self.country(id).ok_or("There is no such country.")?;
        let from = self.player_country().map(|c| c.id);
        self.player = Some(id);
        if from != Some(id) {
            self.newsreel.push(News::about(id, Event::PlayerChanged { from, to: id }));
        }
        Ok(())
    }

    /// Renames the country with this id to `typed`, which is reported in the newsreel. People, cities and news follow
    /// it, since they hold its id. A name that only differs in its capitals from the country's own is fine. An error
    /// says why it can't be done.
    pub fn rename_country(&mut self, id: CountryId, typed: &str) -> Result<(), String> {
        self.country(id).ok_or("There is no such country.")?;
        let name = self.free_country_name(typed, Some(id))?;
        let mut renamed = None;
        if let Some(country) = self.countries.iter_mut().find(|c| c.id == id) {
            if country.name != name {
                renamed = Some(std::mem::replace(&mut country.name, name.clone()));
            }
        }
        if let Some(from) = renamed {
            self.newsreel.push(News::about(id, Event::CountryRenamed { from, to: name }));
        }
        Ok(())
    }

    /// What the country with this id has and owns, counted. Zero of everything for a country that isn't there.
    pub fn country_totals(&self, id: CountryId) -> CountryTotals {
        let cities = self.cities.iter().filter(|c| c.country == id);
        CountryTotals {
            cities: cities.clone().count(),
            population: cities.map(|c| c.population as u64).sum(),
            regions: self.region_owners.iter().filter(|o| o.country == id).count(),
            people: self.people.iter().filter(|p| p.country == Some(id)).count(),
        }
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

    /// Adds `amount` of `resource` to the stock of the country with this id, or takes some away if it is negative. A
    /// stock can't go below zero. An error says why it can't be done.
    // For gathering and using resources, which nothing does yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add_resource(&mut self, id: CountryId, resource: Resource, amount: i64) -> Result<(), String> {
        let country = self.countries.iter_mut().find(|c| c.id == id).ok_or("There is no such country.")?;
        let slot = country.stock.slot(resource);
        let name = resource_name(resource);
        *slot = if amount >= 0 {
            slot.checked_add(amount.unsigned_abs()).ok_or_else(|| format!("That is more {name} than there can be."))?
        } else {
            slot.checked_sub(amount.unsigned_abs()).ok_or_else(|| format!("{} doesn't have that much {name}.", country.name))?
        };
        Ok(())
    }

    /// Gives the country with this id this color, which is reported in the newsreel unless it had it. An error says why
    /// it can't be done.
    pub fn recolor_country(&mut self, id: CountryId, color: Rgb) -> Result<(), String> {
        let country = self.countries.iter_mut().find(|c| c.id == id).ok_or("There is no such country.")?;
        let from = std::mem::replace(&mut country.color, color);
        if from != color {
            self.newsreel.push(News::about(id, Event::CountryRecolored { from, to: color }));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;

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
        assert_eq!(day.people[0].country, Some(brevon));
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
        assert_eq!(day.person_country_name(&day.people[0]), "New Aldoria");
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
    fn a_countrys_totals_count_only_what_belongs_to_it() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert_eq!(day.country_totals(aldoria), CountryTotals::default());
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_city("Highmark", 50.0, 0.0, None).unwrap();
        day.cities[1].population = 2_500;
        day.set_city_country(day.cities[1].id, brevon).unwrap();
        day.cities[0].population = 400;
        assert_eq!(day.country_totals(aldoria), CountryTotals { cities: 1, population: 400, regions: 0, people: 0 });
        assert_eq!(day.country_totals(brevon), CountryTotals { cities: 1, population: 2_500, regions: 0, people: 0 });
        assert_eq!(day.country_totals(CountryId::new()), CountryTotals::default(), "no such country");
    }

    #[test]
    fn every_country_starts_with_no_resources_and_keeps_its_own_stock() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert!(day.countries.iter().all(|c| c.stock == Stock::default()));
        day.add_resource(aldoria, Resource::Forest, 120).unwrap();
        day.add_resource(aldoria, Resource::Oil, 5).unwrap();
        day.add_resource(brevon, Resource::Stone, 40).unwrap();
        day.add_resource(aldoria, Resource::Forest, -20).unwrap();
        assert_eq!(day.country(aldoria).unwrap().stock, Stock { wood: 100, oil: 5, ..Stock::default() });
        assert_eq!(day.country(brevon).unwrap().stock, Stock { stone: 40, ..Stock::default() });
    }

    #[test]
    fn a_stock_cannot_go_below_zero_or_past_the_most_there_can_be() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_resource(id, Resource::Iron, 10).unwrap();
        let before = day.clone();
        assert!(day.add_resource(id, Resource::Iron, -11).unwrap_err().contains("have that much iron"));
        assert!(day.add_resource(id, Resource::Copper, -1).is_err(), "none to take");
        assert_eq!(day, before, "a refused change changes nothing");
        day.add_resource(id, Resource::Iron, -10).unwrap();
        assert_eq!(day.countries[0].stock.iron, 0);
        day.add_resource(id, Resource::Sulphur, i64::MAX).unwrap();
        day.add_resource(id, Resource::Sulphur, i64::MAX).unwrap();
        day.add_resource(id, Resource::Sulphur, 1).unwrap();
        assert_eq!(day.countries[0].stock.sulphur, u64::MAX);
        assert!(day.add_resource(id, Resource::Sulphur, 1).is_err(), "more than a u64 holds");
        assert!(day.add_resource(CountryId::new(), Resource::Oil, 1).is_err(), "no such country");
    }

    #[test]
    fn a_stock_is_saved_with_its_country_and_a_country_saved_without_one_has_none() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_resource(id, Resource::Forest, 7).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["countries"][0]["stock"]["wood"], 7);
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).countries[0].stock.wood, 7, "carried forward");
        let mut old = json;
        old["countries"][0].as_object_mut().unwrap().remove("stock");
        assert_eq!(serde_json::from_value::<DaySave>(old).unwrap().countries[0].stock, Stock::default());
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
        assert_eq!(day.person_country_name(&day.people[0]), "New Aldoria");
        assert_eq!(day.country_name(day.cities[0].country), "New Aldoria");
        assert_eq!(day.newsreel[0].headline(&day), "The nation of New Aldoria is founded");
    }
}
