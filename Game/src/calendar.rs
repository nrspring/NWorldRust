//! The game's calendar. A year has 10 months and a month has 20 days, and all three count from 1.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const MONTHS_PER_YEAR: u32 = 10;
pub const DAYS_PER_MONTH: u32 = 20;

/// A day of the game, written `year-month-day`: the first is 1-1-1 and the last of a year is 1-10-20.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The day every game starts on.
    pub const FIRST: Date = Date { year: 1, month: 1, day: 1 };

    /// Whether this is a day that exists: the months and days are within the calendar.
    pub fn is_valid(self) -> bool {
        self.year >= 1 && (1..=MONTHS_PER_YEAR).contains(&self.month) && (1..=DAYS_PER_MONTH).contains(&self.day)
    }

    /// The day after this one: the next day of the month, then the first of the next month, then the first
    /// of the next year.
    pub fn next(self) -> Date {
        if self.day < DAYS_PER_MONTH {
            Date { day: self.day + 1, ..self }
        } else if self.month < MONTHS_PER_YEAR {
            Date { month: self.month + 1, day: 1, ..self }
        } else {
            Date { year: self.year.saturating_add(1), month: 1, day: 1 }
        }
    }

    /// A name for the day's save file. Names sort in date order, up to year 9999.
    pub fn file_stem(self) -> String {
        format!("year-{:04}_month-{:02}_day-{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}-{}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_day_is_1_1_1() {
        assert_eq!(Date::FIRST.to_string(), "1-1-1");
        assert!(Date::FIRST.is_valid());
    }

    #[test]
    fn a_year_has_ten_months_of_twenty_days() {
        let last = Date { year: 3, month: 10, day: 20 };
        assert!(last.is_valid());
        assert!(!Date { month: 11, ..last }.is_valid());
        assert!(!Date { day: 21, ..last }.is_valid());
    }

    #[test]
    fn the_next_day_goes_on_through_the_months_and_years() {
        assert_eq!(Date::FIRST.next(), Date { year: 1, month: 1, day: 2 });
        assert_eq!(Date { year: 1, month: 1, day: 20 }.next(), Date { year: 1, month: 2, day: 1 });
        assert_eq!(Date { year: 1, month: 10, day: 19 }.next(), Date { year: 1, month: 10, day: 20 });
        assert_eq!(Date { year: 1, month: 10, day: 20 }.next(), Date { year: 2, month: 1, day: 1 });
        // Every day from the first on is a day of the calendar, and there are 200 in a year.
        let mut date = Date::FIRST;
        for _ in 0..2_000 {
            date = date.next();
            assert!(date.is_valid(), "{date}");
        }
        assert_eq!(date, Date { year: 11, month: 1, day: 1 });
    }

    #[test]
    fn days_before_the_first_do_not_exist() {
        assert!(!Date { year: 0, month: 1, day: 1 }.is_valid());
        assert!(!Date { year: 1, month: 0, day: 1 }.is_valid());
        assert!(!Date { year: 1, month: 1, day: 0 }.is_valid());
    }

    #[test]
    fn file_names_sort_in_date_order() {
        let days = [
            Date { year: 1, month: 1, day: 2 },
            Date { year: 1, month: 2, day: 1 },
            Date { year: 1, month: 10, day: 20 },
            Date { year: 2, month: 1, day: 1 },
            Date { year: 12, month: 1, day: 1 },
        ];
        let names: Vec<String> = days.iter().map(|d| d.file_stem()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(Date::FIRST.file_stem(), "year-0001_month-01_day-01");
    }
}
