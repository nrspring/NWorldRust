//! Rules for the short pieces of text the player types, like the names of countries and people.

/// Whether `c` may be typed: letters, digits, spaces and a few marks.
pub fn is_text_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '\'' | '-' | '.')
}

/// Text as typed, tidied up: no spaces at the ends, and runs of spaces made one. `what` says what
/// was being asked for, to finish "Enter ...". An error says what is wrong with the text.
pub fn clean(typed: &str, max_chars: usize, what: &str) -> Result<String, String> {
    let text = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        Err(format!("Enter {what}."))
    } else if text.chars().count() > max_chars {
        Err(format!("That's too long. Use {max_chars} characters or fewer."))
    } else if !text.chars().all(is_text_char) {
        Err("Use only letters, numbers, spaces, hyphens, apostrophes and full stops.".to_owned())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_tidied() {
        assert_eq!(clean("  Old   Aldoria ", 32, "a name").unwrap(), "Old Aldoria");
        assert_eq!(clean("St. John's-on-Sea", 32, "a name").unwrap(), "St. John's-on-Sea");
    }

    #[test]
    fn empty_text_is_refused_and_says_what_was_asked() {
        assert_eq!(clean("   ", 32, "a name").unwrap_err(), "Enter a name.");
    }

    #[test]
    fn long_text_is_refused() {
        assert!(clean(&"a".repeat(32), 32, "a name").is_ok());
        assert!(clean(&"a".repeat(33), 32, "a name").is_err());
    }

    #[test]
    fn odd_characters_are_refused() {
        assert!(clean("Aldoria!", 32, "a name").is_err());
        assert!(clean("a/b", 32, "a name").is_err());
    }
}
