//! The people of a country, and the kinds of title they hold.

use macroquad::math::Vec2;

use super::DaySave;
use crate::city::City;
use crate::ids::{PersonId, TitleTypeId};
use crate::movement::DAILY_POINTS;
use crate::newsreel::{Event, News};
use crate::person::{self, Person};
use crate::title_type::{self, TitleType};

impl DaySave {
    // For showing the kind of title a person has, which nothing shows yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn title_type(&self, id: TitleTypeId) -> Option<&TitleType> {
        self.title_types.iter().find(|t| t.id == id)
    }

    /// The title type called `name`, ignoring capitals.
    pub fn title_type_named(&self, name: &str) -> Option<&TitleType> {
        self.title_types.iter().find(|t| t.name.to_lowercase() == name.to_lowercase())
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
                self.title_types.push(TitleType { id, name: type_name.clone() });
                self.newsreel.push(News::about(id, Event::TitleTypeAdded { name: type_name }));
                id
            }
        };
        let id = PersonId::new();
        self.people.push(Person { id, name: name.clone(), title: title.clone(), title_type, country, at: None, points: DAILY_POINTS });
        self.newsreel.push(News::about(id, Event::PersonJoined { title, name, country: Some(country) }));
        Ok(())
    }

    /// The first city of the country with this id, which is where its people are put on the map.
    fn first_city_of(&self, country: crate::ids::CountryId) -> Option<&City> {
        self.cities.iter().find(|c| c.country == country)
    }

    /// Where each person who has no place on the map yet would be put, if their country has a city to put them by: beside its first city,
    /// at the place `spot` finds for the city given the places already taken near it (those of its militia, and of people put there
    /// before). Nobody is put anywhere by this: see [`Self::place_people`].
    pub fn people_to_place(&self, spot: &dyn Fn(&City, &[Vec2]) -> Option<Vec2>) -> Vec<(PersonId, Vec2)> {
        let mut found: Vec<(PersonId, Vec2)> = Vec::new();
        for person in self.people.iter().filter(|p| p.at.is_none()) {
            let Some(city) = self.first_city_of(person.country) else { continue };
            let mut taken: Vec<Vec2> = self.militias.iter().filter(|m| m.city == city.id).map(|m| Vec2::new(m.x, m.z)).collect();
            taken.extend(self.people.iter().filter(|p| p.country == person.country).filter_map(Person::position));
            taken.extend(found.iter().filter(|(id, _)| self.person(*id).is_some_and(|p| p.country == person.country)).map(|(_, at)| *at));
            if let Some(at) = spot(city, &taken) {
                found.push((person.id, at));
            }
        }
        found
    }

    /// Puts each of `places` (see [`Self::people_to_place`]) on the map, which costs them nothing. Returns how many were put there.
    pub fn place_people(&mut self, places: &[(PersonId, Vec2)]) -> usize {
        let mut placed = 0;
        for (id, at) in places {
            if let Some(person) = self.people.iter_mut().find(|p| p.id == *id && p.at.is_none() && at.x.is_finite() && at.y.is_finite()) {
                person.at = Some((at.x, at.y));
                placed += 1;
            }
        }
        placed
    }

    /// Moves the person with this id to `to`, which costs them `cost` of their movement points: that, and whether the ground between is
    /// fit to cross, is for the caller to say (see [`crate::movement::reach`]). `region` is the number of the map region they get to,
    /// if the map has them. This is reported in the newsreel. An error says why it can't be done.
    pub fn move_person(&mut self, id: PersonId, to: Vec2, cost: u32, region: Option<u16>) -> Result<(), String> {
        if !to.x.is_finite() || !to.y.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let person = self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?;
        if person.at.is_none() {
            return Err(format!("{} is not on the map yet.", person.name));
        }
        if cost > person.points {
            return Err(format!("{} has only {} movement points left, and that takes {cost}.", person.name, person.points));
        }
        person.points -= cost;
        person.at = Some((to.x, to.y));
        let name = person.name.clone();
        self.newsreel.push(News::about(id, Event::PersonMoved { name, points: cost, region }));
        Ok(())
    }

    /// Takes away what is left of the movement points of the person with this id today. They have all of them again on the next day.
    pub fn end_person_moves(&mut self, id: PersonId) -> Result<(), String> {
        self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?.points = 0;
        Ok(())
    }

    /// Gives every person all their movement points again, for a new day.
    pub fn rest_people(&mut self) {
        for person in &mut self.people {
            person.points = DAILY_POINTS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;

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
        assert_eq!(day.newsreel.last(), Some(&News::about(person.id, Event::PersonJoined { title: "President".to_owned(), name: "Mara Voss".to_owned(), country: Some(aldoria) })));
        // The first person of a kind of title makes the kind news too, just before.
        let kind = day.newsreel[day.newsreel.len() - 2].clone();
        assert_eq!(kind, News::about(person.title_type, Event::TitleTypeAdded { name: "Head of State".to_owned() }));
        assert_eq!(kind.headline(&day), "A new kind of title, Head of State, is added");
        assert_eq!(day.person(person.id), Some(person));
        // Another of the same kind adds nothing more of that kind.
        day.found_person("Tomas Rue", "Chancellor", "Head of State").unwrap();
        assert_eq!(day.newsreel.iter().filter(|n| matches!(n.event, Event::TitleTypeAdded { .. })).count(), 1);
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
    fn a_person_has_no_place_until_their_country_has_a_city_and_is_then_put_beside_its_first() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let spot = |city: &City, taken: &[Vec2]| Some(Vec2::new(city.x + 30.0 + taken.len() as f32 * 25.0, city.z));
        assert!(day.people[0].at.is_none() && day.people[0].points == DAILY_POINTS);
        assert!(day.people_to_place(&spot).is_empty(), "no city yet");

        day.found_city("Port Aldor", 100.0, 0.0, None).unwrap();
        day.found_city("Second City", 400.0, 0.0, None).unwrap();
        let places = day.people_to_place(&spot);
        assert_eq!(places, vec![(day.people[0].id, Vec2::new(130.0, 0.0))], "beside the first city, with nothing taken there");
        assert_eq!(day.place_people(&places), 1);
        assert_eq!(day.people[0].position(), Some(Vec2::new(130.0, 0.0)));
        assert!(day.people_to_place(&spot).is_empty(), "already on the map");
        assert_eq!(day.place_people(&places), 0, "and not put there twice");

        // A second person of the country stands apart from the first, and one of a country with no city waits.
        day.found_person("Tomas Rue", "Chancellor", "Head of State").unwrap();
        let places = day.people_to_place(&spot);
        assert_eq!(places, vec![(day.people[1].id, Vec2::new(155.0, 0.0))]);
        day.people[1].country = day.countries[1].id;
        assert!(day.people_to_place(&spot).is_empty(), "Brevon has no city");
        // A place that can't be found leaves them where they were.
        day.people[1].country = day.countries[0].id;
        assert!(day.people_to_place(&|_, _| None).is_empty());
    }

    #[test]
    fn a_person_moves_like_a_militia_spends_points_and_has_them_all_back_each_day() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        assert!(day.move_person(id, Vec2::new(5.0, 5.0), 10, None).unwrap_err().contains("not on the map yet"));
        day.people[0].at = Some((0.0, 0.0));
        let before = day.clone();
        assert!(day.move_person(id, Vec2::new(f32::NAN, 5.0), 10, None).is_err());
        assert!(day.move_person(id, Vec2::new(5.0, 5.0), DAILY_POINTS + 1, None).unwrap_err().contains("has only 200 movement points left"));
        assert!(day.move_person(PersonId::new(), Vec2::new(5.0, 5.0), 1, None).is_err());
        assert_eq!(day, before, "a refused move changes nothing");

        day.move_person(id, Vec2::new(60.0, 20.0), 80, Some(0x4005)).unwrap();
        assert_eq!((day.people[0].position(), day.people[0].points), (Some(Vec2::new(60.0, 20.0)), DAILY_POINTS - 80));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("Mara Voss travels to {}, spending 80 movement points", day.region_label(0x4005)));
        day.end_person_moves(id).unwrap();
        assert_eq!(day.people[0].points, 0);
        assert!(day.move_person(id, Vec2::new(61.0, 20.0), 1, None).is_err(), "no moves left today");
        day.rest_people();
        assert_eq!(day.people[0].points, DAILY_POINTS, "back the next day");
    }

    #[test]
    fn people_on_the_map_are_saved_and_carry_forward_and_an_old_save_has_them_nowhere() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.people[0].at = Some((12.0, 34.0));
        day.people[0].points = 77;
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        let tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.people[0].position(), Some(Vec2::new(12.0, 34.0)));
        let mut old = json;
        old["people"][0].as_object_mut().unwrap().remove("at");
        old["people"][0].as_object_mut().unwrap().remove("points");
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert_eq!((old.people[0].at, old.people[0].points), (None, DAILY_POINTS));
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
}
