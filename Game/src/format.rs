//! Numbers as the game writes them for the player.

/// A whole number with a comma between each three digits: 12,345.
pub fn thousands(n: u32) -> String {
    group(&n.to_string())
}

/// The same for a number that can be bigger: an amount of a resource.
pub fn thousands_u64(n: u64) -> String {
    group(&n.to_string())
}

/// An amount of money with commas, and a minus sign if it is below zero: 12,345 or -1,200.
pub fn money(n: i64) -> String {
    format!("{}{}", if n < 0 { "-" } else { "" }, group(&n.unsigned_abs().to_string()))
}

/// `digits` with a comma between each three.
fn group(digits: &str) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_has_commas_and_a_minus_sign_when_it_is_below_zero() {
        assert_eq!(money(0), "0");
        assert_eq!(money(1_000), "1,000");
        assert_eq!(money(-1_200), "-1,200");
        assert_eq!(money(1_234_567_890_123), "1,234,567,890,123");
        assert_eq!(money(i64::MIN), "-9,223,372,036,854,775,808");
    }

    #[test]
    fn the_biggest_amounts_of_a_resource_get_commas_too() {
        assert_eq!(thousands_u64(0), "0");
        assert_eq!(thousands_u64(u64::MAX), "18,446,744,073,709,551,615");
    }

    #[test]
    fn thousands_get_commas() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(100), "100");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(12_345), "12,345");
        assert_eq!(thousands(100_000), "100,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
