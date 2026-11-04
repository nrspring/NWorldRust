//! Deleting a country, which can only be done when it owns nothing.

use super::DaySave;
use crate::gatherer::GathererKind;
use crate::ids::CountryId;
use crate::newsreel::{Event, News};
use crate::country::FormerCountry;

impl DaySave {
    /// Why the country with this id can't be deleted, or `None` if it can: it can't own anything, which is cities,
    /// gatherers of any kind, shipyards, and regions that it owns with no city of its in them. Its people don't count.
    pub fn delete_problem(&self, id: CountryId) -> Option<String> {
        let country = self.country(id)?;
        let cities = self.cities.iter().filter(|c| c.country == id).count();
        let regions = self.region_owners.iter().filter(|o| o.country == id && !self.cities.iter().any(|c| c.country == id && c.region == Some(o.region))).count();
        let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut owned: Vec<String> = Vec::new();
        let mut counts = vec![(cities, "city", "cities".to_owned())];
        for kind in GathererKind::ALL {
            counts.push((self.gatherers.iter().filter(|g| g.country == id && g.kind == kind).count(), kind.noun(), kind.plural().to_owned()));
        }
        counts.push((self.militias.iter().filter(|m| m.country == id).count(), "militia", "militia".to_owned()));
        for kind in crate::shipyard::ShipyardKind::ALL {
            counts.push((self.shipyards.iter().filter(|s| s.country == id && s.kind == kind).count(), kind.noun(), format!("{}s", kind.noun())));
        }
        counts.push((regions, "region", "regions".to_owned()));
        for (n, one, many) in counts {
            if n > 0 {
                owned.push(plural(n, one, &many));
            }
        }
        let list = match owned.as_slice() {
            [] => return None,
            [only] => only.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        Some(format!("{} owns {list}, so it can't be deleted.", country.name))
    }

    /// Removes the country with this id, which can only be done if it owns nothing (see
    /// [`delete_problem`](Self::delete_problem)). Its people go with it. The news of it and of them stays, since it is
    /// history, and so does its name, in `former_countries`; the deleting is reported in the newsreel. If it was the
    /// country the player plays, the first country left is. An error says why it can't be done.
    pub fn delete_country(&mut self, id: CountryId) -> Result<(), String> {
        let name = self.country(id).ok_or("There is no such country.")?.name.clone();
        if let Some(problem) = self.delete_problem(id) {
            return Err(problem);
        }
        self.countries.retain(|c| c.id != id);
        self.people.retain(|p| p.country != id);
        self.former_countries.push(FormerCountry { id, name: name.clone() });
        if self.player == Some(id) {
            self.player = self.countries.first().map(|c| c.id);
        }
        self.newsreel.push(News::about(id, Event::CountryDeleted { name }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::day;

    #[test]
    fn a_country_that_owns_nothing_can_be_deleted_with_its_people() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.select_country(brevon).unwrap();
        day.found_person("Tomas Rue", "King", "Monarch").unwrap();
        day.set_region_owner(0x4006, Some(aldoria)).unwrap();
        let tomas = day.people[1].id;

        assert_eq!(day.delete_problem(brevon), None);
        day.delete_country(brevon).unwrap();
        assert_eq!(day.countries.len(), 1);
        assert!(day.country(brevon).is_none());
        assert_eq!(day.people.len(), 1, "its people go with it");
        assert_eq!(day.people[0].name, "Mara Voss");
        assert_eq!(day.region_owner(0x4006), Some(aldoria), "other countries' regions are not touched");
        // Its news stays, since it is history, and still names it and them; the deleting is news too.
        assert_eq!(day.former_countries, [FormerCountry { id: brevon, name: "Brevon".to_owned() }]);
        assert!(day.newsreel.iter().any(|n| n.subject == crate::ids::Guid::from(tomas)));
        let headlines: Vec<String> = day.newsreel.iter().map(|n| n.headline(&day)).collect();
        assert!(headlines.contains(&"The nation of Brevon is founded".to_owned()), "{headlines:?}");
        assert!(headlines.contains(&"King Tomas Rue joins Brevon".to_owned()), "{headlines:?}");
        assert_eq!(headlines.last().unwrap(), "The nation of Brevon is deleted");
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
        assert_eq!(err, "Aldoria owns 1 city, so it can't be deleted.");
        assert!(day.delete_country(CountryId::new()).is_err());
        assert_eq!(day.delete_problem(CountryId::new()), None, "there is nothing to say of a country that isn't there");
        assert_eq!(day, before);
    }

    #[test]
    fn a_country_cannot_be_deleted_while_it_owns_anything_and_the_reason_lists_it() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        // A region it owns with no city in it.
        day.set_region_owner(0x4005, Some(brevon)).unwrap();
        assert_eq!(day.delete_problem(brevon).unwrap(), "Brevon owns 1 region, so it can't be deleted.");
        let before = day.clone();
        assert!(day.delete_country(brevon).unwrap_err().contains("owns 1 region"));
        assert_eq!(day, before, "and nothing is lost");
        // A gatherer, and cities and a region together.
        day.add_money(aldoria, 5_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4001)).unwrap();
        day.found_city("Farhold", 10.0, 0.0, Some(0x4002)).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 5.0, 5.0, 0x4003).unwrap();
        day.set_region_owner(0x4004, Some(aldoria)).unwrap();
        assert_eq!(day.delete_problem(aldoria).unwrap(), "Aldoria owns 2 cities, 1 woodcutter and 2 regions, so it can't be deleted.");
        // Letting go of the region it owns by itself is what lets it go.
        day.set_region_owner(0x4005, None).unwrap();
        assert_eq!(day.delete_problem(brevon), None);
        day.delete_country(brevon).unwrap();
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
    fn a_country_with_a_militia_cannot_be_deleted() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.add_money(aldoria, 1_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 500;
        day.raise_militia(day.cities[0].id, 20, macroquad::math::vec2(30.0, 0.0)).unwrap();
        day.set_city_country(day.cities[0].id, brevon).unwrap();
        // Brevon has the city; Aldoria has only the militia.
        let problem = day.delete_problem(aldoria).unwrap();
        assert!(problem.contains("1 militia"), "{problem}");
        assert!(day.delete_country(aldoria).is_err());
        let id = day.militias[0].id;
        day.set_militia_country(id, brevon).unwrap();
        day.delete_country(aldoria).unwrap();
    }
}
