//! Countries.

use serde::{Deserialize, Serialize};

use crate::ids::CountryId;
use crate::text;

/// The longest a country's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Country {
    /// Tells it apart from every other country, whatever it is called. Everything that belongs to
    /// the country refers to it by this.
    pub id: CountryId,
    pub name: String,
}

/// A country name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your country")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_tidied() {
        assert_eq!(clean_name("  Old   Aldoria ").unwrap(), "Old Aldoria");
    }

    #[test]
    fn an_empty_name_asks_for_one() {
        assert_eq!(clean_name("").unwrap_err(), "Enter a name for your country.");
    }

    #[test]
    fn a_name_has_a_length_limit() {
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }
}
