//! The newsreel: what happened on a day. Each day has its own, which starts empty; nothing in it is
//! carried to the next day.
//!
//! A news item has the id of its subject, the thing it is mainly about, and what happened to it. The
//! subject's id is stored with the item, for statistics, and is never shown: the headline looks the
//! names up in the day it is shown with, so it never goes out of date.

use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::format::{money, thousands, thousands_u64};
use crate::ids::{CityId, CountryId, Guid, NoteId, PersonId, WoodcutterId};
use crate::text;

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
    /// The subject is a country, and its woodcutters that were operating provided this much wood.
    WoodGathered { amount: u64 },
    /// The subject is a woodcutter, and it was built.
    WoodcutterBuilt,
    /// The subject is a woodcutter, which belonged to the country `country`, and it was destroyed in the region
    /// numbered `region`. It is gone, so what the news says of it is kept here.
    WoodcutterDestroyed { country: CountryId, region: u16 },
    /// The subject is a woodcutter, and it was transferred from the country `from` to the country `to`.
    WoodcutterTransferred { from: CountryId, to: CountryId },
    /// The subject is a woodcutter, and it stopped operating because it lost its route to a city.
    WoodcutterStopped,
    /// The subject is a woodcutter, and it started operating, because it has a route to a city.
    WoodcutterStarted,
    /// The subject is the note itself, which the player wrote, and this is what it says.
    Note { text: String },
}

/// The longest a note can be, in characters.
pub const MAX_NOTE_CHARS: usize = 140;

/// Whether `c` may be typed in a note: what a name allows, and the marks a sentence needs.
pub fn is_note_char(c: char) -> bool {
    text::is_text_char(c) || matches!(c, ',' | ';' | ':' | '!' | '?' | '"' | '(' | ')')
}

/// A note as typed, tidied up. An error says what is wrong with it.
pub fn clean_note(typed: &str) -> Result<String, String> {
    let text = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        Err("Enter the note.".to_owned())
    } else if text.chars().count() > MAX_NOTE_CHARS {
        Err(format!("That's too long. Use {MAX_NOTE_CHARS} characters or fewer."))
    } else if !text.chars().all(is_note_char) {
        Err("Use only letters, numbers, spaces and ordinary punctuation.".to_owned())
    } else {
        Ok(text)
    }
}

impl DaySave {
    /// Adds a note saying `typed` to today's newsreel. An error says why it can't be done.
    pub fn add_note(&mut self, typed: &str) -> Result<(), String> {
        let text = clean_note(typed)?;
        self.newsreel.push(News::about(NoteId::new(), Event::Note { text }));
        Ok(())
    }
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
            Event::WoodGathered { amount } => format!("{} gathers {} wood from its woodcutters", day.country_name(CountryId::from(self.subject)), thousands_u64(amount)),
            Event::WoodcutterBuilt => match day.woodcutter(WoodcutterId::from(self.subject)) {
                Some(woodcutter) => format!("{} builds a woodcutter in {}", day.country_name(woodcutter.country), day.region_label(woodcutter.region)),
                None => "A woodcutter is built".to_owned(),
            },
            Event::WoodcutterDestroyed { country, region } => format!("{}'s woodcutter in {} is destroyed", day.country_name(country), day.region_label(region)),
            Event::WoodcutterTransferred { from, to } => match day.woodcutter(WoodcutterId::from(self.subject)) {
                Some(woodcutter) => format!("The woodcutter in {} is transferred from {} to {}", day.region_label(woodcutter.region), day.country_name(from), day.country_name(to)),
                None => format!("A woodcutter is transferred from {} to {}", day.country_name(from), day.country_name(to)),
            },
            Event::WoodcutterStopped => match day.woodcutter(WoodcutterId::from(self.subject)) {
                Some(woodcutter) => format!("{}'s woodcutter in {} stops operating, with no route to a city", day.country_name(woodcutter.country), day.region_label(woodcutter.region)),
                None => "A woodcutter stops operating".to_owned(),
            },
            Event::WoodcutterStarted => match day.woodcutter(WoodcutterId::from(self.subject)) {
                Some(woodcutter) => format!("{}'s woodcutter in {} starts operating", day.country_name(woodcutter.country), day.region_label(woodcutter.region)),
                None => "A woodcutter starts operating".to_owned(),
            },
            Event::Note { ref text } => text.clone(),
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
    fn a_note_is_tidied_and_added_to_the_newsreel_as_it_was_written() {
        let mut day = day();
        let before = day.newsreel.len();
        day.add_note("  The  harvest, at last!  ").unwrap();
        assert_eq!(day.newsreel.len(), before + 1);
        let note = day.newsreel.last().unwrap();
        assert_eq!(note.headline(&day), "The harvest, at last!");
        assert!(matches!(note.event, Event::Note { .. }));
    }

    #[test]
    fn a_bad_note_is_refused_and_changes_nothing() {
        let mut day = day();
        let before = day.clone();
        assert!(day.add_note("   ").unwrap_err().contains("note"));
        assert!(day.add_note("no <tags>").is_err());
        assert!(day.add_note(&"a".repeat(MAX_NOTE_CHARS + 1)).unwrap_err().contains("too long"));
        assert_eq!(day, before);
        assert!(day.add_note(&"a".repeat(MAX_NOTE_CHARS)).is_ok());
    }

    #[test]
    fn a_note_is_saved_with_its_text_and_its_own_id_and_stays_with_its_day() {
        let mut today = day();
        today.add_note("Peace talks begin").unwrap();
        let json = serde_json::to_string(today.newsreel.last().unwrap()).unwrap();
        assert!(json.contains(r#""type":"note""#) && json.contains(r#""text":"Peace talks begin""#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *today.newsreel.last().unwrap());
        assert!(today.following(crate::calendar::Date { year: 1, month: 1, day: 2 }).newsreel.is_empty(), "like all news, it is for its day only");
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
