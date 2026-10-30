//! Paths: dirt paths laid over the map, built a segment at a time. A segment runs from the center of a region
//! to the middle of one of that region's edges, the border it shares with a neighbor. Paths belong to no
//! country, but a segment costs coins, which come from the country the player is playing.

use serde::{Deserialize, Serialize};

use crate::day::DaySave;

/// What a segment of path costs, in coins.
pub const PATH_COST: i64 = 10;

/// One segment of path: from the center of the region numbered `region` to the middle of the border it shares
/// with the region numbered `neighbor`. The segment from the center of `neighbor` to the same point is a
/// different one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathSegment {
    pub region: u16,
    pub neighbor: u16,
}

/// Why no path can be built now.
#[derive(Debug, PartialEq)]
pub enum PathProblem {
    /// There is no country to pay for it.
    NoCountry,
    /// The country being played can't afford a segment.
    NotEnoughCoin { country: String, money: i64 },
}

impl PathProblem {
    /// A few words, for beside the button.
    pub fn short(&self) -> String {
        match self {
            Self::NoCountry => "No country to pay for paths".to_owned(),
            Self::NotEnoughCoin { money, .. } => format!("Not enough coin (needs {PATH_COST}, has {money})"),
        }
    }

    /// The whole reason, as a sentence.
    pub fn long(&self) -> String {
        match self {
            Self::NoCountry => "There is no country yet to pay for a path.".to_owned(),
            Self::NotEnoughCoin { country, money } => {
                format!("Not enough coin: a path segment costs {PATH_COST} coins, and {country} has {money}.")
            }
        }
    }
}

impl DaySave {
    /// Why a path can't be built, or `None` if one can: the country being played has to be able to pay for a
    /// segment.
    pub fn path_problem(&self) -> Option<PathProblem> {
        let country = self.player_country().map_or(Err(PathProblem::NoCountry), Ok);
        match country {
            Err(problem) => Some(problem),
            Ok(country) if country.money < PATH_COST => Some(PathProblem::NotEnoughCoin { country: country.name.clone(), money: country.money }),
            Ok(_) => None,
        }
    }

    /// Builds `segment`, which the caller has checked is a real one (that the regions touch, and are land), and
    /// charges its cost to the country being played. An error says why it can't be done.
    pub fn build_path(&mut self, segment: PathSegment) -> Result<(), String> {
        if segment.region == segment.neighbor {
            return Err("A path goes from a region to its edge.".to_owned());
        }
        if self.paths.contains(&segment) {
            return Err("There is already a path there.".to_owned());
        }
        if let Some(problem) = self.path_problem() {
            return Err(problem.long());
        }
        let country = self.player_country().ok_or("There is no country yet to pay for a path.")?.id;
        self.add_money(country, -PATH_COST)?;
        self.paths.push(segment);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day_with_money(money: i64) -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, money).unwrap();
        day
    }

    const A: PathSegment = PathSegment { region: 0xC001, neighbor: 0xC002 };
    const B: PathSegment = PathSegment { region: 0xC002, neighbor: 0xC001 };

    #[test]
    fn a_segment_costs_10_coins_from_the_country_being_played() {
        let mut day = day_with_money(100);
        day.found_country("Brevon").unwrap();
        day.build_path(A).unwrap();
        assert_eq!(day.paths, [A]);
        assert_eq!(day.countries[0].money, 90);
        assert_eq!(day.countries[1].money, 0, "only the country being played pays");
        // Another country that is chosen pays for the next.
        day.add_money(day.countries[1].id, 50).unwrap();
        day.select_country(day.countries[1].id).unwrap();
        day.build_path(B).unwrap();
        assert_eq!((day.countries[0].money, day.countries[1].money), (90, 40));
    }

    #[test]
    fn the_two_directions_of_an_edge_are_two_segments_and_a_repeat_is_refused() {
        let mut day = day_with_money(100);
        day.build_path(A).unwrap();
        day.build_path(B).unwrap();
        let before = day.clone();
        assert!(day.build_path(A).unwrap_err().contains("already"));
        assert_eq!(day, before, "a refused path costs nothing");
    }

    #[test]
    fn a_path_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(9);
        let before = day.clone();
        let error = day.build_path(A).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("10") && error.contains("Aldoria") && error.contains('9'), "{error}");
        assert_eq!(day, before);
        assert_eq!(day.path_problem(), Some(PathProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 9 }));
        // Exactly the cost is enough, and then there isn't enough for another.
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.path_problem(), None);
        day.build_path(A).unwrap();
        assert_eq!(day.countries[0].money, 0);
        assert!(day.path_problem().is_some());
    }

    #[test]
    fn with_no_country_there_is_nobody_to_pay() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert_eq!(day.path_problem(), Some(PathProblem::NoCountry));
        assert!(day.build_path(A).is_err());
        assert!(day.paths.is_empty());
    }

    #[test]
    fn a_segment_needs_two_different_regions() {
        let mut day = day_with_money(100);
        assert!(day.build_path(PathSegment { region: 0xC001, neighbor: 0xC001 }).is_err());
        assert_eq!(day.countries[0].money, 100);
    }

    #[test]
    fn the_reasons_read_well() {
        let problem = PathProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 3 };
        assert_eq!(problem.short(), "Not enough coin (needs 10, has 3)");
        assert_eq!(problem.long(), "Not enough coin: a path segment costs 10 coins, and Aldoria has 3.");
        assert!(!PathProblem::NoCountry.short().is_empty() && !PathProblem::NoCountry.long().is_empty());
    }

    #[test]
    fn paths_are_saved_belong_to_no_country_and_carry_forward() {
        let mut day = day_with_money(100);
        day.build_path(A).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["paths"][0], serde_json::json!({ "region": 0xC001, "neighbor": 0xC002 }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).paths, [A]);
    }
}
