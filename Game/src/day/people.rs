//! The people of a country, and the kinds of title they hold.

use macroquad::math::Vec2;

use super::DaySave;
use crate::city_layout;
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

    /// Why the person with this id can't be put on the map at `at`, in the map region numbered `region` (if the map has regions), or
    /// `None` if they can. They can't be put in another country's region, or inside a city, where they could not be seen. Whether the
    /// ground itself will do is for the caller to say (see [`crate::person::ground_problem`]).
    pub fn person_spot_problem(&self, id: PersonId, at: Vec2, region: Option<u16>) -> Option<String> {
        let Some(person) = self.person(id) else { return Some("There is no such person.".to_owned()) };
        if person.at.is_some() {
            return Some(format!("{} is on the map already.", person.name));
        }
        if !at.x.is_finite() || !at.y.is_finite() {
            return Some("That is not a place on the map.".to_owned());
        }
        self.city_region_problem(person.country, region).or_else(|| {
            let city = self.cities.iter().find(|c| Vec2::new(c.x, c.z).distance(at) < city_layout::radius(c))?;
            Some(format!("That is inside {}: place them beside it.", city.name))
        })
    }

    /// Puts the person with this id, who has no place yet, on the map at `at`, in the map region numbered `region`, which costs them
    /// nothing. An error says why it can't be done (see [`Self::person_spot_problem`]).
    pub fn place_person(&mut self, id: PersonId, at: Vec2, region: Option<u16>) -> Result<(), String> {
        if let Some(problem) = self.person_spot_problem(id, at, region) {
            return Err(problem);
        }
        self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?.at = Some((at.x, at.y));
        Ok(())
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
    fn a_person_has_no_place_until_they_are_put_on_the_map_which_is_done_once() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        assert!(day.people[0].at.is_none() && day.people[0].points == DAILY_POINTS);

        // Not inside a city, and not in a region of another country.
        day.found_city("Port Aldor", 100.0, 0.0, Some(0x4001)).unwrap();
        assert!(day.place_person(id, Vec2::new(100.0, 0.0), Some(0x4001)).unwrap_err().contains("inside Port Aldor"));
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        day.found_city("Brevik", 2000.0, 0.0, Some(0x4002)).unwrap();
        assert!(day.place_person(id, Vec2::new(2400.0, 0.0), Some(0x4002)).unwrap_err().contains("belongs to Brevon"));
        assert!(day.place_person(id, Vec2::new(f32::NAN, 0.0), None).is_err());
        assert!(day.place_person(PersonId::new(), Vec2::new(600.0, 0.0), None).is_err());
        assert!(day.people[0].at.is_none(), "a refused place changes nothing");

        // Unowned land will do, as will their own country's.
        assert_eq!(day.person_spot_problem(id, Vec2::new(600.0, 0.0), Some(0x4003)), None);
        day.place_person(id, Vec2::new(600.0, 0.0), Some(0x4001)).unwrap();
        assert_eq!((day.people[0].position(), day.people[0].points), (Some(Vec2::new(600.0, 0.0)), DAILY_POINTS));
        assert!(day.place_person(id, Vec2::new(700.0, 0.0), None).unwrap_err().contains("on the map already"));
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
