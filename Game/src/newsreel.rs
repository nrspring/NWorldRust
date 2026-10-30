//! The newsreel: what happened on a day. Each day has its own, which starts empty; nothing in it is
//! carried to the next day.

use serde::{Deserialize, Serialize};

/// One thing that happened. Saved with a `type` field, so statistics can count them by kind.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum News {
    CountryFounded { country: String },
    PersonJoined { person: String, title: String, country: String },
}

impl News {
    /// The line the newsreel shows for it.
    pub fn headline(&self) -> String {
        match self {
            News::CountryFounded { country } => format!("The nation of {country} is founded"),
            News::PersonJoined { person, title, country } => format!("{title} {person} joins {country}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_founded_country_makes_a_headline() {
        let news = News::CountryFounded { country: "Aldoria".to_owned() };
        assert_eq!(news.headline(), "The nation of Aldoria is founded");
    }

    #[test]
    fn a_person_joining_makes_a_headline() {
        let news = News::PersonJoined { person: "Mara Voss".to_owned(), title: "President".to_owned(), country: "Aldoria".to_owned() };
        assert_eq!(news.headline(), "President Mara Voss joins Aldoria");
    }

    #[test]
    fn news_is_saved_with_its_kind() {
        let news = News::CountryFounded { country: "Aldoria".to_owned() };
        let json = serde_json::to_string(&news).unwrap();
        assert_eq!(json, r#"{"type":"country_founded","country":"Aldoria"}"#);
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), news);
    }
}
