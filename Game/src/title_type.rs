//! Title types: what kind of title a person's title is, such as a monarch's, a noble's or a general's.
//! There are none to begin with. A kind is made the first time it is typed in when a person is added,
//! and after that it can be picked from the list. The kinds are kept in each day's file, so a
//! person's `title_type` can be looked up there.

use serde::{Deserialize, Serialize};

use crate::ids::TitleTypeId;
use crate::text;

/// The longest a title type's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TitleType {
    pub id: TitleTypeId,
    pub name: String,
}

/// A title type name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a title type")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_type_name_is_tidied_and_must_be_there() {
        assert_eq!(clean_name("  War   Chief ").unwrap(), "War Chief");
        assert_eq!(clean_name(" ").unwrap_err(), "Enter a title type.");
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }
}
