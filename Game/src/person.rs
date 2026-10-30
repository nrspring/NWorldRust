//! People. Each belongs to a country.

use serde::{Deserialize, Serialize};

use crate::ids::{CountryId, PersonId};
use crate::text;

/// The longest a person's name or title can be, in characters.
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_TITLE_CHARS: usize = 32;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    /// What they are called, like "President" or "General".
    pub title: String,
    /// The id of the country they belong to.
    pub country: CountryId,
}

/// A person's name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "the person's name")
}

/// A person's title as typed, tidied up. An error says what is wrong with it.
pub fn clean_title(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_TITLE_CHARS, "the person's title")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_titles_are_tidied() {
        assert_eq!(clean_name(" Mara  Voss ").unwrap(), "Mara Voss");
        assert_eq!(clean_title(" Prime  Minister ").unwrap(), "Prime Minister");
    }

    #[test]
    fn a_missing_name_or_title_says_which() {
        assert_eq!(clean_name("").unwrap_err(), "Enter the person's name.");
        assert_eq!(clean_title("").unwrap_err(), "Enter the person's title.");
    }

    #[test]
    fn they_have_length_limits() {
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(clean_title(&"a".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }
}
