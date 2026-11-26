//! The people of a country, and the kinds of title they hold.

use macroquad::math::Vec2;

use super::DaySave;
use crate::city_layout;
use crate::ids::{CountryId, PersonId, TitleTypeId};
use crate::movement::DAILY_POINTS;
use crate::newsreel::{Event, News};
use crate::person::{self, Person};
use crate::title_type::{self, TitleType};

impl DaySave {
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
        let title_type = self.title_type_id(type_name);
        let id = PersonId::new();
        self.people.push(Person { id, name: name.clone(), title: title.clone(), title_type, country: Some(country), at: None, points: DAILY_POINTS, age_days: 0, parent: None, unnamed_child_of: None });
        self.newsreel.push(News::about(id, Event::PersonJoined { title, name, country: Some(country) }));
        Ok(())
    }

    /// The id of the kind of title called `name`, which is added, and is news, if there is no such kind yet.
    fn title_type_id(&mut self, name: String) -> TitleTypeId {
        if let Some(existing) = self.title_type_named(&name) {
            return existing.id;
        }
        let id = TitleTypeId::new();
        self.title_types.push(TitleType { id, name: name.clone() });
        self.newsreel.push(News::about(id, Event::TitleTypeAdded { name }));
        id
    }

    /// Gives the person with this id the name `name`, the title `title` and the kind of title `title_type`, which is added if the
    /// game doesn't have it yet. A change of name or title is reported in the newsreel. An error says why it can't be done.
    pub fn edit_person(&mut self, id: PersonId, name: &str, title: &str, title_type: &str) -> Result<(), String> {
        let old = self.person(id).ok_or("There is no such person.")?;
        let (old_name, old_title) = (old.name.clone(), old.title.clone());
        let (name, title) = (person::clean_name(name)?, person::clean_title(title)?);
        let title_type = self.title_type_id(title_type::clean_name(title_type)?);
        let person = self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?;
        (person.name, person.title, person.title_type) = (name.clone(), title.clone(), title_type);
        if (&name, &title) != (&old_name, &old_title) {
            self.newsreel.push(News::about(id, Event::PersonRenamed { from: old_name, name, title }));
        }
        Ok(())
    }

    /// Lets the people of an age for it have children: each with the chance their age gives (see [`person::daily_birth_chance`]),
    /// which `random`, a number from 0 up to 1 for each, decides. A child is born where their parent stands (nowhere, if the parent
    /// is not on the map), belongs where their parent does, and is in the newsreel.
    pub fn have_children(&mut self, mut random: impl FnMut() -> f32) {
        let parents: Vec<Person> = self.people.iter().filter(|p| random() < person::daily_birth_chance(p.age_days)).cloned().collect();
        for parent in parents {
            let title_type = self.title_type_id(person::CHILD_TITLE.to_owned());
            let id = PersonId::new();
            let name = person::child_name(&parent.name);
            self.people.push(Person { id, name: name.clone(), title: person::CHILD_TITLE.to_owned(), title_type, country: parent.country, at: parent.at, points: DAILY_POINTS, age_days: 0, parent: Some(parent.id), unnamed_child_of: Some(parent.name.clone()) });
            self.newsreel.push(News::about(id, Event::PersonBorn { name, parent: parent.name, country: parent.country }));
        }
    }

    /// The first newborn who has not been named yet, if there is one.
    pub fn unnamed_child(&self) -> Option<&Person> {
        self.people.iter().find(|p| p.unnamed_child_of.is_some())
    }

    /// Gives the newborn with this id their name, which is reported in the newsreel. An error says why it can't be done.
    pub fn name_child(&mut self, id: PersonId, name: &str) -> Result<(), String> {
        let name = person::clean_name(name)?;
        let person = self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?;
        let parent = person.unnamed_child_of.take().ok_or_else(|| format!("{} has a name already.", person.name))?;
        person.name = name.clone();
        self.newsreel.push(News::about(id, Event::ChildNamed { name, parent }));
        Ok(())
    }

    /// The name of the country `person` belongs to, for showing: "Nomad" if they belong to none.
    pub fn person_country_name(&self, person: &Person) -> &str {
        person.country.map_or(person::NOMAD, |id| self.country_name(id))
    }

    /// Makes the person with this id belong to the country `to`, or to none, as a nomad, which is reported in the newsreel unless
    /// it is no change. An error says why it can't be done.
    pub fn set_person_country(&mut self, id: PersonId, to: Option<CountryId>) -> Result<(), String> {
        if let Some(to) = to {
            self.country(to).ok_or("There is no such country.")?;
        }
        let person = self.people.iter_mut().find(|p| p.id == id).ok_or("There is no such person.")?;
        let from = std::mem::replace(&mut person.country, to);
        if from != to {
            let name = person.name.clone();
            self.newsreel.push(News::about(id, Event::PersonChangedCountry { name, from, to }));
        }
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
        // A nomad belongs to no country, so no country's region is closed to them.
        person.country.and_then(|country| self.city_region_problem(country, region)).or_else(|| {
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

    /// Makes every person a day older, and lets those who die of age die: each of them with the chance their age gives (see
    /// [`person::daily_death_chance`]), which `random`, a number from 0 up to 1 for each, decides. The dead are gone, and each
    /// is in the newsreel.
    pub fn age_people(&mut self, mut random: impl FnMut() -> f32) {
        let mut died = Vec::new();
        for person in &mut self.people {
            person.age_days = person.age_days.saturating_add(1);
            if random() < person::daily_death_chance(person.age_days) {
                died.push(person.id);
            }
        }
        for id in died {
            let Some(at) = self.people.iter().position(|p| p.id == id) else { continue };
            let person = self.people.remove(at);
            let (age, name, title, country) = (person.age(), person.name, person.title, person.country);
            self.newsreel.push(News::about(id, Event::PersonDied { name, title, age, country }));
        }
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
    use crate::calendar::{Date, DAYS_PER_YEAR};
    use crate::day::day;

    #[test]
    fn a_person_joins_the_first_country_by_its_id_and_is_reported() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person(" Mara  Voss ", "President", "Head of State").unwrap();

        let aldoria = day.countries[0].id;
        let person = &day.people[0];
        assert_eq!((person.name.as_str(), person.title.as_str(), person.country), ("Mara Voss", "President", Some(aldoria)));
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
    fn a_person_can_change_country_or_become_a_nomad_who_can_go_anywhere_and_it_is_news() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        assert!(day.set_person_country(id, Some(CountryId::new())).is_err(), "no such country");
        assert!(day.set_person_country(PersonId::new(), None).is_err(), "no such person");
        let news = day.newsreel.len();
        day.set_person_country(id, Some(aldoria)).unwrap();
        assert_eq!(day.newsreel.len(), news, "no change is no news");

        day.set_person_country(id, Some(brevon)).unwrap();
        assert_eq!(day.people[0].country, Some(brevon));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss leaves Aldoria for Brevon");
        day.set_person_country(id, None).unwrap();
        assert_eq!((day.people[0].country, day.person_country_name(&day.people[0])), (None, "Nomad"));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss leaves Brevon and becomes a nomad");
        // A nomad can be put in a country's region.
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4001)).unwrap();
        assert_eq!(day.person_spot_problem(id, Vec2::new(600.0, 0.0), Some(0x4001)), None);
        day.set_person_country(id, Some(brevon)).unwrap();
        assert!(day.person_spot_problem(id, Vec2::new(600.0, 0.0), Some(0x4001)).unwrap().contains("belongs to Aldoria"));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss the nomad joins Brevon");
        // The news of their joining still names the country they joined then.
        assert!(day.newsreel.iter().any(|n| n.headline(&day) == "President Mara Voss joins Aldoria"));
        // And a nomad is saved and read back as one.
        day.set_person_country(id, None).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap().people[0].country, None);
    }

    #[test]
    fn a_person_is_born_new_grows_older_each_day_and_may_die_of_age_which_is_news() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_person("Tomas Rue", "Chancellor", "Head of State").unwrap();
        assert_eq!((day.people[0].age_days, day.people[0].age()), (0, 0));
        // However the dice fall, the young don't die.
        day.age_people(|| 0.0);
        assert_eq!((day.people.len(), day.people[0].age_days), (2, 1));

        let old = 60 * DAYS_PER_YEAR - 1;
        day.people[0].age_days = old;
        day.people[1].age_days = old;
        day.age_people(|| 0.999);
        assert_eq!((day.people.len(), day.people[0].age()), (2, 60), "lucky today");
        // The first rolls low and dies, and the second doesn't.
        let mut rolls = [0.0, 0.999].into_iter();
        day.set_person_country(day.people[1].id, None).unwrap();
        let mara = day.people[0].id;
        day.age_people(|| rolls.next().unwrap());
        assert_eq!(day.people.len(), 1);
        assert_eq!(day.people[0].name, "Tomas Rue");
        assert!(day.person(mara).is_none());
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "President Mara Voss of Aldoria dies, aged 60");
        day.age_people(|| 0.0);
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Chancellor Tomas Rue, a nomad, dies, aged 60");
        assert!(day.people.is_empty());
        // The news of them stays and still says who they were.
        assert!(day.newsreel.iter().any(|n| n.headline(&day) == "President Mara Voss joins Aldoria"));
    }

    #[test]
    fn somebody_saved_before_ages_is_25() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let mut json = serde_json::to_value(&day).unwrap();
        json["people"][0].as_object_mut().unwrap().remove("age_days");
        let old: DaySave = serde_json::from_value(json).unwrap();
        assert_eq!(old.people[0].age(), 25);
    }

    #[test]
    fn a_child_is_born_where_their_parent_stands_and_belongs_where_they_do() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.have_children(|| 0.0);
        assert_eq!(day.people.len(), 1, "too young, however the dice fall");
        day.people[0].age_days = 30 * DAYS_PER_YEAR;
        day.people[0].at = Some((5.0, 6.0));
        day.have_children(|| 0.999);
        assert_eq!(day.people.len(), 1, "not today");
        day.have_children(|| 0.0);
        assert_eq!(day.people.len(), 2, "one child: the newborn has none");
        let child = day.people[1].clone();
        assert_eq!((child.name.as_str(), child.title.as_str(), child.age_days), ("Child of Mara Voss", "Child", 0));
        assert_eq!((child.country, child.at, child.points), (day.people[0].country, Some((5.0, 6.0)), DAILY_POINTS));
        assert_eq!((child.parent, day.people[0].parent), (Some(day.people[0].id), None), "the child knows whose they are");
        assert_eq!(day.title_type(child.title_type).unwrap().name, "Child");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss of Aldoria has a child");

        // A nomad's child is a nomad, and one of somebody not on the map is not on it.
        day.set_person_country(day.people[0].id, None).unwrap();
        day.people[0].at = None;
        day.have_children(|| 0.0);
        assert_eq!((day.people[2].country, day.people[2].at), (None, None));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss, a nomad, has a child");
        assert_eq!(day.title_types.iter().filter(|t| t.name == "Child").count(), 1);
    }

    #[test]
    fn each_newborn_waits_to_be_named_one_after_another_and_naming_them_is_news() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.people[0].age_days = 30 * DAYS_PER_YEAR;
        assert_eq!(day.unnamed_child(), None);
        day.have_children(|| 0.0);
        day.have_children(|| 0.0);
        let (first, second) = (day.people[1].id, day.people[2].id);
        assert_eq!(day.unnamed_child().map(|c| (c.id, c.unnamed_child_of.as_deref())), Some((first, Some("Mara Voss"))));
        // Waiting to be named is saved, and carries on into later days.
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap().unnamed_child().map(|c| c.id), Some(first));
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).people[1].unnamed_child_of.as_deref(), Some("Mara Voss"));

        let before = day.clone();
        assert!(day.name_child(first, "  ").is_err());
        assert!(day.name_child(day.people[0].id, "Ada").unwrap_err().contains("has a name already"));
        assert!(day.name_child(PersonId::new(), "Ada").is_err());
        assert_eq!(day, before);
        day.name_child(first, " Ada  Voss ").unwrap();
        assert_eq!((day.people[1].name.as_str(), day.people[1].title.as_str()), ("Ada Voss", "Child"));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss's child is named Ada Voss");
        assert_eq!(day.unnamed_child().map(|c| c.id), Some(second), "then the next");
        day.name_child(second, "Ben Voss").unwrap();
        assert_eq!(day.unnamed_child(), None);
        // Naming them, saving them and a new day leave whose child they are, even once the parent is gone.
        let mother = day.people[0].id;
        assert_eq!(day.people[1].parent, Some(mother));
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap().people[1].parent, Some(mother));
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).people[2].parent, Some(mother));
        // A name can't be given twice, though it can be changed.
        assert!(day.name_child(first, "Cara").is_err());
        day.edit_person(first, "Cara Voss", "Child", "Child").unwrap();
    }

    #[test]
    fn a_person_can_be_given_another_name_and_title_which_is_news() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        let id = day.people[0].id;
        let before = day.clone();
        assert!(day.edit_person(id, "", "President", "Head of State").is_err());
        assert!(day.edit_person(id, "Mara", "President", " ").is_err());
        assert!(day.edit_person(PersonId::new(), "Mara", "President", "Monarch").is_err());
        assert_eq!(day, before, "a refused change changes nothing");
        day.edit_person(id, "Mara Voss", "President", "Head of State").unwrap();
        assert_eq!(day, before, "no change is no news");

        day.edit_person(id, " Ilse  Brandt ", "Queen", "Monarch").unwrap();
        let person = day.people[0].clone();
        assert_eq!((person.name.as_str(), person.title.as_str()), ("Ilse Brandt", "Queen"));
        assert_eq!(day.title_type(person.title_type).unwrap().name, "Monarch");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Mara Voss is now known as Queen Ilse Brandt");
        day.edit_person(id, "Ilse Brandt", "Empress", "Monarch").unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Ilse Brandt takes the title Empress");
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
