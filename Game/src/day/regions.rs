//! Who owns the regions of the map.

use super::DaySave;
use crate::country::RegionOwner;
use crate::ids::CountryId;
use crate::newsreel::{Event, News};
use crate::rgb::Rgb;

impl DaySave {
    /// The country that owns the map region numbered `region`, if any does.
    pub fn region_owner(&self, region: u16) -> Option<CountryId> {
        self.region_owners.iter().find(|o| o.region == region).map(|o| o.country)
    }

    /// The color of the owner of each owned region, as the number of the region and the color, for showing
    /// who owns what.
    pub fn region_colors(&self) -> Vec<(u16, Rgb)> {
        self.region_owners.iter().filter_map(|o| Some((o.region, self.country(o.country)?.color))).collect()
    }

    /// What is in the region numbered `region` that belongs to a country, as a phrase that says where it is and whose it
    /// is: a city, "Port Aldor stands" (and which country), or a gatherer or a shipyard, "A woodcutter of Aldoria is".
    fn region_holders(&self, region: u16) -> Vec<(String, CountryId)> {
        let cities = self.cities.iter().filter(|c| c.region == Some(region)).map(|c| (format!("{} stands", c.name), c.country));
        let gatherers = self.gatherers.iter().filter(|g| g.region == region).map(|g| (format!("{} of {} is", g.kind.a_noun_capital(), self.country_name(g.country)), g.country));
        let shipyards = self.shipyards.iter().filter(|s| s.region == Some(region)).map(|s| (format!("{} of {} is", s.kind.a_noun_capital(), self.country_name(s.country)), s.country));
        let barracks = self.barracks.iter().filter(|b| b.region == Some(region)).map(|b| (format!("{} of {} is", b.kind.a_noun_capital(), self.country_name(b.country)), b.country));
        cities.chain(gatherers).chain(shipyards).chain(barracks).collect()
    }

    /// The first city standing in the region numbered `region`, as a phrase, "Port Aldor stands", and whose it is.
    fn region_city(&self, region: u16) -> Option<(String, CountryId)> {
        self.cities.iter().find(|c| c.region == Some(region)).map(|c| (format!("{} stands", c.name), c.country))
    }

    /// Why the owner of the region numbered `region` can't be changed, or `None` if it can: a region with a city in it stays with
    /// that city's country, and changes hands with the city. What else is built in a region does not hold it.
    pub fn region_lock(&self, region: u16) -> Option<String> {
        let owner = self.region_owner(region)?;
        let (what, _) = self.region_city(region)?;
        Some(format!("{what} in that region, so it has to stay with {}.", self.country_name(owner)))
    }

    /// What there is to say of the owner of the region numbered `region` where it is chosen: why it can't be changed (see
    /// [`Self::region_lock`]), or that what is built in it goes with it, or nothing.
    pub fn region_owner_note(&self, region: u16) -> Option<String> {
        self.region_lock(region).or_else(|| (!self.region_holders(region).is_empty()).then(|| "What is built in it goes with it.".to_owned()))
    }

    /// Gives the region numbered `region` to the country called `named`, or to none if that is "No country": what
    /// the region popup asks for. An error says why it can't be done (see [`set_region_owner`](Self::set_region_owner)).
    pub fn assign_region(&mut self, region: u16, named: &str) -> Result<(), String> {
        let owner = if named == crate::popup::NO_COUNTRY {
            None
        } else {
            Some(self.countries.iter().find(|c| c.name.to_lowercase() == named.to_lowercase()).ok_or_else(|| format!("There is no country called {named}."))?.id)
        };
        self.set_region_owner(region, owner)
    }

    /// Makes the country being played the owner of the region numbered `region`, which is what is asked after something
    /// is placed in a region nobody has. An error says why it can't be done (see
    /// [`set_region_owner`](Self::set_region_owner)).
    pub fn claim_region(&mut self, region: u16) -> Result<(), String> {
        let country = self.player_country().ok_or("There is no country yet to claim the region.")?.id;
        // Claiming takes nothing from anybody: what another country has in a region of its own stays there.
        if let (Some(owner), Some((what, _))) = (self.region_owner(region).filter(|&o| o != country), self.region_holders(region).into_iter().next()) {
            return Err(format!("{what} in that region, so it has to stay with {}.", self.country_name(owner)));
        }
        self.set_region_owner(region, Some(country))
    }

    /// Makes `owner` the owner of the region numbered `region`, or leaves it unowned if `owner` is `None`. A region with a city
    /// in it can only be the city's country's. Any other can go to any country, and the gatherers, shipyards and barracks in it go
    /// with it, so that a region with one of those can't be left unowned. An error says why it can't be done.
    pub fn set_region_owner(&mut self, region: u16, owner: Option<CountryId>) -> Result<(), String> {
        if let Some(owner) = owner {
            self.country(owner).ok_or("There is no such country.")?;
        }
        if self.region_owner(region) == owner {
            return Ok(());
        }
        if let Some((what, holder)) = self.region_city(region).filter(|(_, holder)| Some(*holder) != owner) {
            return Err(self.region_lock(region).unwrap_or_else(|| format!("{what} in that region, so only {} can have it.", self.country_name(holder))));
        }
        if owner.is_none() {
            if let Some((what, _)) = self.region_holders(region).into_iter().next() {
                return Err(format!("{what} in that region, so it can't be left with no owner."));
            }
        }
        self.put_region_owner(region, owner);
        Ok(())
    }

    /// Makes `owner` the owner of the region numbered `region`, or leaves it unowned if `owner` is `None`, without
    /// asking whether it may be, and reports the change in the newsreel unless nothing changed.
    pub(super) fn put_region_owner(&mut self, region: u16, owner: Option<CountryId>) {
        let from = self.region_owner(region);
        if from == owner {
            return;
        }
        self.region_owners.retain(|o| o.region != region);
        if let Some(country) = owner {
            self.region_owners.push(RegionOwner { region, country });
            // What stands in a region belongs to whoever owns it, so a gatherer or a shipyard goes with the region.
            for gatherer in self.gatherers.iter_mut().filter(|g| g.region == region) {
                gatherer.country = country;
            }
            for shipyard in self.shipyards.iter_mut().filter(|s| s.region == Some(region)) {
                shipyard.country = country;
            }
            for barracks in self.barracks.iter_mut().filter(|b| b.region == Some(region)) {
                barracks.country = country;
            }
        }
        if let Some(subject) = owner.or(from) {
            self.newsreel.push(News::about(subject, Event::RegionOwnerChanged { region, from, to: owner }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;
    use crate::gatherer::GathererKind;

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
        // The region takes no second city, and is not owned twice.
        assert!(day.found_city("Highmark", 10.0, 0.0, Some(0x4005)).is_err());
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
    fn an_empty_region_can_be_given_to_a_country_by_name_and_taken_back_or_given_on() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        assert_eq!(day.region_lock(0x4005), None);
        day.assign_region(0x4005, "Aldoria").unwrap();
        assert_eq!(day.region_owner(0x4005), Some(aldoria));
        day.assign_region(0x4005, "brevon").unwrap();
        assert_eq!(day.region_owner(0x4005), Some(brevon));
        day.assign_region(0x4005, crate::popup::NO_COUNTRY).unwrap();
        assert_eq!(day.region_owner(0x4005), None);
        assert!(day.assign_region(0x4005, "Nowhere").unwrap_err().contains("no country called Nowhere"));
    }

    #[test]
    fn a_region_with_a_city_stays_with_it_and_one_with_only_buildings_changes_hands_with_them() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 50.0, 0.0, 0x4006).unwrap();

        // The city holds its region.
        assert!(day.region_lock(0x4005).unwrap().contains("has to stay with Aldoria"));
        assert_eq!(day.region_owner_note(0x4005), day.region_lock(0x4005));
        assert!(day.assign_region(0x4005, "Brevon").unwrap_err().contains("has to stay with Aldoria"));
        assert!(day.assign_region(0x4005, crate::popup::NO_COUNTRY).is_err());

        // A gatherer doesn't: its region can be given to another, which gets it too, but can't be left with nobody.
        assert_eq!(day.region_lock(0x4006), None);
        assert_eq!(day.region_owner_note(0x4006).as_deref(), Some("What is built in it goes with it."));
        assert_eq!(day.region_owner_note(0x4007), None);
        let before = day.clone();
        let err = day.assign_region(0x4006, crate::popup::NO_COUNTRY).unwrap_err();
        assert!(err.starts_with("A woodcutter of Aldoria") && err.contains("no owner"), "{err}");
        assert_eq!(day, before);
        day.assign_region(0x4006, "Brevon").unwrap();
        assert_eq!((day.region_owner(0x4006), day.gatherers[0].country), (Some(brevon), brevon));
        day.assign_region(0x4006, "Aldoria").unwrap();
        assert_eq!(day.gatherers[0].country, aldoria);

        // One saved in a region nobody owns can be given to anyone too.
        day.region_owners.retain(|o| o.region != 0x4006);
        day.assign_region(0x4006, "Brevon").unwrap();
        assert_eq!(day.gatherers[0].country, brevon);
        // Claiming a region doesn't take it, or what is in it, from another country.
        assert!(day.claim_region(0x4006).unwrap_err().contains("has to stay with Brevon"));
        assert_eq!(day.region_owner(0x4005), Some(aldoria));
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
}
