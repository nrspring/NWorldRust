//! Countries.

use serde::{Deserialize, Serialize};

use crate::ids::CountryId;
use crate::rgb::Rgb;
use crate::text;

/// The longest a country's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;
/// The money a country has when it is founded.
pub const STARTING_MONEY: i64 = 0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Country {
    /// Tells it apart from every other country, whatever it is called. Everything that belongs to
    /// the country refers to it by this.
    pub id: CountryId,
    pub name: String,
    /// What the country is shown in. Random when it is founded, and the player can change it.
    pub color: Rgb,
    /// The country's money, in whole coins. It can go below zero, which is debt. Each day's file has what the
    /// country had that day, so it can be looked back on.
    #[serde(default)]
    pub money: i64,
}

/// A region of the map that a country owns. A region nobody owns has none of these.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct RegionOwner {
    /// The number of the region, as the map knows it.
    pub region: u16,
    pub country: CountryId,
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
