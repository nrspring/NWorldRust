//! Paths and roads: laid over the map, built a segment at a time. A road is a path that costs more and is made of
//! packed stone, and it is built, drawn and saved in the same way. A segment is one of two kinds: from the
//! center of a region to the middle of one of its edges (the border it shares with a neighbor), or straight
//! across a region from the middle of one of its edges to the middle of another, which doesn't go through the
//! center. Paths and roads belong to no country, but a segment costs coins, which come from the country the player
//! is playing.

use serde::{Deserialize, Serialize};

use crate::bridge::BridgeLink;
use crate::day::DaySave;
use crate::failure::Thing;
use crate::newsreel::{Cost, Event, News};

/// What a segment of path costs, in coins.
pub const PATH_COST: i64 = 10;
/// What a segment of road costs, in coins.
pub const ROAD_COST: i64 = 100;

/// Which of the two a segment is part of: a dirt path or a stone road.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Kind {
    #[default]
    Path,
    Road,
}

impl Kind {
    /// What a segment of it costs, in coins.
    pub const fn cost(self) -> i64 {
        match self {
            Self::Path => PATH_COST,
            Self::Road => ROAD_COST,
        }
    }

    /// What to call one of it: "path" or "road".
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Road => "road",
        }
    }
}

/// One segment of path, in the region numbered `region`.
///
/// With no `other`, it runs from the center of the region to the middle of the border it shares with the region
/// numbered `neighbor`. The segment from the center of `neighbor` to the same point is a different one.
///
/// With an `other`, it runs across the region, from the middle of its border with `neighbor` to the middle of its
/// border with `other`, which is the one with the higher number.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathSegment {
    pub region: u16,
    pub neighbor: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<u16>,
}

impl PathSegment {
    /// From the center of `region` to the middle of its border with `neighbor`.
    pub const fn to_edge(region: u16, neighbor: u16) -> Self {
        Self { region, neighbor, other: None }
    }

    /// Across `region` from the middle of its border with `a` to the middle of its border with `b`, the same
    /// whichever is said first.
    pub fn across(region: u16, a: u16, b: u16) -> Self {
        Self { region, neighbor: a.min(b), other: Some(a.max(b)) }
    }
}

/// A segment of path of either kind: one joining parts of the map's regions, or one that starts at the end of a
/// bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathPiece {
    Segment(PathSegment),
    Link(BridgeLink),
}

/// Why no path or road can be built now.
#[derive(Debug, PartialEq)]
pub enum PathProblem {
    /// There is no country to pay for it.
    NoCountry(Kind),
    /// The country being played can't afford a segment.
    NotEnoughCoin { country: String, money: i64, kind: Kind },
}

impl PathProblem {
    /// The whole reason, as a sentence.
    pub fn long(&self) -> String {
        match self {
            Self::NoCountry(kind) => format!("There is no country yet to pay for a {}.", kind.noun()),
            Self::NotEnoughCoin { country, money, kind } => {
                format!("Not enough coin: a {} segment costs {} coins, and {country} has {money}.", kind.noun(), kind.cost())
            }
        }
    }
}

impl DaySave {
    /// Why a segment of `kind` can't be built, or `None` if one can: the country being played has to be able to pay
    /// for it.
    pub fn problem(&self, kind: Kind) -> Option<PathProblem> {
        match self.player_country() {
            None => Some(PathProblem::NoCountry(kind)),
            Some(country) if country.money < kind.cost() => Some(PathProblem::NotEnoughCoin { country: country.name.clone(), money: country.money, kind }),
            Some(_) => None,
        }
    }

    /// Why a path can't be built, or `None` if one can.
    pub fn path_problem(&self) -> Option<PathProblem> {
        self.problem(Kind::Path)
    }

    /// Why a road can't be built, or `None` if one can.
    pub fn road_problem(&self) -> Option<PathProblem> {
        self.problem(Kind::Road)
    }

    /// The segments of `kind` that have been built.
    pub fn segments(&self, kind: Kind) -> &[PathSegment] {
        match kind {
            Kind::Path => &self.paths,
            Kind::Road => &self.roads,
        }
    }

    /// The segments of `kind` that start at the end of a bridge.
    pub fn links(&self, kind: Kind) -> &[BridgeLink] {
        match kind {
            Kind::Path => &self.bridge_links,
            Kind::Road => &self.road_links,
        }
    }

    /// Builds a piece of `kind` of either sort (see [`build_segment`](Self::build_segment) and
    /// [`build_link_of`](Self::build_link_of)).
    pub fn build_piece(&mut self, kind: Kind, piece: PathPiece) -> Result<(), String> {
        match piece {
            PathPiece::Segment(segment) => self.build_segment(kind, segment),
            PathPiece::Link(link) => self.build_link_of(kind, link),
        }
    }

    /// Builds a segment of path, which the caller has checked is a real one (that the regions touch, and are land),
    /// and charges its cost to the country being played. An error says why it can't be done.
    // The short way to say it for a path, which the tests use; the game builds either kind with `build_piece`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn build_path(&mut self, segment: PathSegment) -> Result<(), String> {
        self.build_segment(Kind::Path, segment)
    }

    /// Builds `segment` as a path or a road, charging the cost of it to the country being played. A road built where
    /// there is a path replaces the path, and a path can't be built where there is a road. An error says why it
    /// can't be done.
    pub fn build_segment(&mut self, kind: Kind, segment: PathSegment) -> Result<(), String> {
        if segment.region == segment.neighbor {
            return Err(format!("A {} goes from a region to its edge.", kind.noun()));
        }
        if segment.other.is_some_and(|other| other == segment.region || other == segment.neighbor) {
            return Err(format!("A {} across a region goes from one of its edges to another.", kind.noun()));
        }
        let (in_roads, in_paths) = (self.roads.contains(&segment), self.paths.contains(&segment));
        // One that has failed is built again, which mends it for what building it costs.
        let thing = Thing::of(kind, PathPiece::Segment(segment));
        let repairing = self.is_failed(thing) && if kind == Kind::Road { in_roads } else { in_paths };
        if in_roads && !repairing {
            return Err("There is already a road there.".to_owned());
        }
        if kind == Kind::Path && in_paths && !repairing {
            return Err("There is already a path there.".to_owned());
        }
        if let Some(problem) = self.problem(kind) {
            return Err(problem.long());
        }
        let country = self.player_country().ok_or_else(|| format!("There is no country yet to pay for a {}.", kind.noun()))?.id;
        self.add_money(country, -kind.cost())?;
        let cost = vec![Cost::coins(kind.cost())];
        if repairing {
            self.repair(thing);
            self.newsreel.push(News::about(country, Event::Repaired { piece: thing.piece(), country, regions: vec![segment.region], cost }));
            return Ok(());
        }
        match kind {
            Kind::Path => self.paths.push(segment),
            Kind::Road => {
                // Paving a path: the road is there instead of it, and the path, failed or not, is gone.
                self.paths.retain(|p| *p != segment);
                self.repair(Thing::Path(segment));
                self.roads.push(segment);
            }
        }
        let paved = kind == Kind::Road && in_paths;
        self.newsreel.push(News::about(country, Event::Built { piece: thing.piece(), country, regions: vec![segment.region], cost, paved }));
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

    const A: PathSegment = PathSegment::to_edge(0xC001, 0xC002);
    const B: PathSegment = PathSegment::to_edge(0xC002, 0xC001);

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
        assert_eq!(day.path_problem(), Some(PathProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 9, kind: Kind::Path }));
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
        assert_eq!(day.path_problem(), Some(PathProblem::NoCountry(Kind::Path)));
        assert!(day.build_path(A).is_err());
        assert!(day.paths.is_empty());
    }

    #[test]
    fn a_segment_needs_two_different_regions() {
        let mut day = day_with_money(100);
        assert!(day.build_path(PathSegment::to_edge(0xC001, 0xC001)).is_err());
        assert_eq!(day.countries[0].money, 100);
    }

    #[test]
    fn a_segment_can_go_across_a_region_between_two_of_its_edges_for_the_same_cost() {
        let mut day = day_with_money(100);
        let across = PathSegment::across(0xC001, 0xC003, 0xC002);
        assert_eq!(across, PathSegment::across(0xC001, 0xC002, 0xC003), "the same whichever edge is said first");
        assert_eq!((across.neighbor, across.other), (0xC002, Some(0xC003)));
        day.build_path(across).unwrap();
        assert_eq!(day.countries[0].money, 90);
        // It is a segment of its own: it doesn't stand in for the ones to the center, and can't be built twice.
        day.build_path(PathSegment::to_edge(0xC001, 0xC002)).unwrap();
        assert_eq!(day.paths.len(), 2);
        assert!(day.build_path(PathSegment::across(0xC001, 0xC002, 0xC003)).unwrap_err().contains("already"));
        assert_eq!(day.countries[0].money, 80);
    }

    #[test]
    fn a_segment_across_a_region_needs_two_different_edges_that_are_not_the_region_itself() {
        let mut day = day_with_money(100);
        assert!(day.build_path(PathSegment::across(0xC001, 0xC002, 0xC002)).is_err(), "the same edge twice");
        assert!(day.build_path(PathSegment::across(0xC001, 0xC001, 0xC002)).is_err(), "the region as its own neighbor");
        assert!(day.paths.is_empty() && day.countries[0].money == 100);
    }

    #[test]
    fn segments_saved_before_there_were_two_kinds_still_load_as_the_first_kind() {
        let old: PathSegment = serde_json::from_str(r#"{ "region": 49153, "neighbor": 49154 }"#).unwrap();
        assert_eq!(old, A);
        let across = PathSegment::across(0xC001, 0xC002, 0xC003);
        let json = serde_json::to_value(across).unwrap();
        assert_eq!(json, serde_json::json!({ "region": 0xC001, "neighbor": 0xC002, "other": 0xC003 }));
        assert_eq!(serde_json::from_value::<PathSegment>(json).unwrap(), across);
    }

    #[test]
    fn the_reasons_read_well() {
        let problem = PathProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 3, kind: Kind::Path };
        assert_eq!(problem.long(), "Not enough coin: a path segment costs 10 coins, and Aldoria has 3.");
        assert!(!PathProblem::NoCountry(Kind::Path).long().is_empty());
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

    #[test]
    fn a_road_costs_100_coins_and_a_path_10() {
        assert_eq!((Kind::Path.cost(), Kind::Road.cost()), (10, 100));
        let mut day = day_with_money(250);
        day.build_segment(Kind::Road, A).unwrap();
        assert_eq!(day.roads, [A]);
        assert!(day.paths.is_empty(), "a road isn't a path");
        assert_eq!(day.countries[0].money, 150);
        day.build_segment(Kind::Path, B).unwrap();
        assert_eq!(day.countries[0].money, 140);
        assert_eq!(day.segments(Kind::Road), [A]);
        assert_eq!(day.segments(Kind::Path), [B]);
    }

    #[test]
    fn a_road_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(99);
        let before = day.clone();
        let error = day.build_segment(Kind::Road, A).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("road") && error.contains("100") && error.contains("99"), "{error}");
        assert_eq!(day, before);
        assert_eq!(day.road_problem(), Some(PathProblem::NotEnoughCoin { country: "Aldoria".to_owned(), money: 99, kind: Kind::Road }));
        assert_eq!(day.path_problem(), None, "enough for a path, not for a road");
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.road_problem(), None);
        let none = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert_eq!(none.road_problem(), Some(PathProblem::NoCountry(Kind::Road)));
        assert!(none.road_problem().unwrap().long().contains("road"));
    }

    #[test]
    fn a_road_built_over_a_path_replaces_it_and_a_path_cannot_be_built_over_a_road() {
        let mut day = day_with_money(500);
        day.build_segment(Kind::Path, A).unwrap();
        day.build_segment(Kind::Path, B).unwrap();
        day.build_segment(Kind::Road, A).unwrap();
        assert_eq!((day.paths.as_slice(), day.roads.as_slice()), (&[B][..], &[A][..]), "the path that was paved is gone");
        assert_eq!(day.countries[0].money, 500 - 10 - 10 - 100, "the road is paid for in full");
        // A path where there is a road, and a second road, are refused for nothing.
        let before = day.clone();
        assert!(day.build_segment(Kind::Path, A).unwrap_err().contains("already a road"));
        assert!(day.build_segment(Kind::Road, A).unwrap_err().contains("already a road"));
        assert_eq!(day, before);
    }

    #[test]
    fn roads_are_saved_belong_to_no_country_and_carry_forward() {
        let mut day = day_with_money(500);
        day.build_segment(Kind::Road, A).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["roads"][0], serde_json::json!({ "region": 0xC001, "neighbor": 0xC002 }));
        assert_eq!(json["paths"].as_array().unwrap().len(), 0);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).roads, [A]);
        // A day saved before there were roads has none.
        let old = serde_json::json!({ "version": 2, "date": { "year": 1, "month": 1, "day": 1 }, "map": "Assets/map.nworld" });
        assert!(serde_json::from_value::<DaySave>(old).unwrap().roads.is_empty());
    }

    #[test]
    fn a_road_across_a_region_is_a_segment_like_a_path_across_one() {
        let mut day = day_with_money(300);
        day.build_segment(Kind::Road, PathSegment::across(0xC001, 0xC002, 0xC003)).unwrap();
        assert!(day.build_segment(Kind::Road, PathSegment::across(0xC001, 0xC003, 0xC002)).is_err());
        assert!(day.build_segment(Kind::Road, PathSegment::across(0xC001, 0xC002, 0xC002)).unwrap_err().contains("road"));
    }
}
