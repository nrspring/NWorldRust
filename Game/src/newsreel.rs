//! The newsreel: what happened on a day. Each day has its own, which starts empty; nothing in it is
//! carried to the next day.
//!
//! A news item has the id of its subject, the thing it is mainly about, and what happened to it. The
//! subject's id is stored with the item, for statistics, and is never shown: the headline looks the
//! names up in the day it is shown with, so it never goes out of date.

use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::format::{money, thousands};
use crate::ids::{CityId, CountryId, Guid, PersonId};

/// One thing that happened, saved with the id of its subject and a `type`, so statistics can count
/// and join the news by kind and by subject.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct News {
    /// The id of whatever the news is mainly about: the country founded, the person who joined, the
    /// city. Stored only; the newsreel doesn't show it.
    pub subject: Guid,
    #[serde(flatten)]
    pub event: Event,
}

/// What happened to the subject.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The subject is a country, and it was founded.
    CountryFounded,
    /// The subject is a person, and they joined their country.
    PersonJoined,
    /// The subject is a city, and it was founded with this many people.
    CityFounded { population: u32 },
    /// The subject is a country, and it earned this many coins.
    Income { amount: i64 },
    /// The subject is a city, whose population went from `from` to `to` people, while it was or was not under
    /// siege.
    PopulationChanged { from: u32, to: u32, under_siege: bool },
}

impl News {
    /// News of `event` happening to `subject`, whose id of any kind will do.
    pub fn about(subject: impl Into<Guid>, event: Event) -> Self {
        Self { subject: subject.into(), event }
    }

    /// The line the newsreel shows for it, with the names looked up in `day`. It says nothing of ids.
    pub fn headline(&self, day: &DaySave) -> String {
        match self.event {
            Event::CountryFounded => format!("The nation of {} is founded", day.country_name(CountryId::from(self.subject))),
            Event::PersonJoined => match day.person(PersonId::from(self.subject)) {
                Some(person) => format!("{} {} joins {}", person.title, person.name, day.country_name(person.country)),
                None => "Someone joins a country".to_owned(),
            },
            Event::CityFounded { population } => match day.city(CityId::from(self.subject)) {
                Some(city) => format!("{} founds {}, a city of {population} people", day.country_name(city.country), city.name),
                None => format!("A city of {population} people is founded"),
            },
            Event::Income { amount } => format!("{} earns {} coins", day.country_name(CountryId::from(self.subject)), money(amount)),
            Event::PopulationChanged { from, to, under_siege } => {
                let name = day.city(CityId::from(self.subject)).map_or("A city", |city| city.name.as_str());
                let change = thousands(from.abs_diff(to));
                let now = thousands(to);
                match (to > from, under_siege) {
                    (true, _) => format!("{name} grows by {change} to {now}"),
                    (false, true) => format!("{name}, under siege, loses {change} people and falls to {now}"),
                    (false, false) => format!("{name} loses {change} people and falls to {now}"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day
    }

    #[test]
    fn each_kind_of_news_makes_a_headline_from_the_names() {
        let day = day();
        let headlines: Vec<String> = day.newsreel.iter().map(|n| n.headline(&day)).collect();
        assert_eq!(headlines, ["The nation of Aldoria is founded", "President Mara Voss joins Aldoria", "Aldoria founds Port Aldor, a city of 100 people"]);
    }

    #[test]
    fn income_and_population_news_read_naturally() {
        let day = day();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        let income = |amount| News::about(aldoria, Event::Income { amount }).headline(&day);
        assert_eq!(income(1_200), "Aldoria earns 1,200 coins");
        let changed = |from, to, under_siege| News::about(port, Event::PopulationChanged { from, to, under_siege }).headline(&day);
        assert_eq!(changed(1_000, 1_020, false), "Port Aldor grows by 20 to 1,020");
        assert_eq!(changed(1_000, 925, true), "Port Aldor, under siege, loses 75 people and falls to 925");
        assert_eq!(changed(10_000, 9_500, false), "Port Aldor loses 500 people and falls to 9,500");
    }

    #[test]
    fn income_and_population_news_is_saved_with_what_changed() {
        let income = News::about(CountryId::from_u128(1), Event::Income { amount: 250 });
        let json = serde_json::to_string(&income).unwrap();
        assert!(json.contains(r#""type":"income""#) && json.contains(r#""amount":250"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), income);
        let population = News::about(CityId::from_u128(2), Event::PopulationChanged { from: 100, to: 102, under_siege: false });
        let json = serde_json::to_string(&population).unwrap();
        assert!(json.contains(r#""type":"population_changed""#) && json.contains(r#""from":100"#) && json.contains(r#""to":102"#) && json.contains(r#""under_siege":false"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), population);
    }

    #[test]
    fn news_about_something_that_is_not_there_still_reads_sensibly() {
        let day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert_eq!(News::about(CountryId::new(), Event::CountryFounded).headline(&day), "The nation of an unknown country is founded");
        assert_eq!(News::about(PersonId::new(), Event::PersonJoined).headline(&day), "Someone joins a country");
        assert_eq!(News::about(CityId::new(), Event::CityFounded { population: 100 }).headline(&day), "A city of 100 people is founded");
        assert_eq!(News::about(CountryId::new(), Event::Income { amount: 5 }).headline(&day), "an unknown country earns 5 coins");
        assert_eq!(News::about(CityId::new(), Event::PopulationChanged { from: 100, to: 110, under_siege: false }).headline(&day), "A city grows by 10 to 110");
    }

    #[test]
    fn every_news_item_has_the_id_of_its_subject() {
        let day = day();
        let subjects: Vec<Guid> = day.newsreel.iter().map(|n| n.subject).collect();
        assert_eq!(subjects, [day.countries[0].id.into(), day.people[0].id.into(), day.cities[0].id.into()]);
    }

    #[test]
    fn the_subject_is_stored_but_never_shown() {
        let day = day();
        for news in &day.newsreel {
            let headline = news.headline(&day);
            assert!(!headline.contains(&news.subject.to_string()), "{headline}");
        }
    }

    #[test]
    fn news_is_saved_with_the_id_of_its_subject_and_its_kind() {
        let id = CountryId::from_u128(0x3f2b8c1e_5a47_4d0e_9b6a_7c1d2e8f4a90);
        let news = News::about(id, Event::CountryFounded);
        let json = serde_json::to_string(&news).unwrap();
        assert_eq!(json, r#"{"subject":"3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90","type":"country_founded"}"#);
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), news);

        // Whatever else an event carries is saved beside them.
        let city = News::about(CityId::from_u128(7), Event::CityFounded { population: 100 });
        let json = serde_json::to_string(&city).unwrap();
        assert!(json.contains(r#""type":"city_founded""#) && json.contains(r#""population":100"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), city);
    }
}
