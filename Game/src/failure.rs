//! Things that fail: a path, a road or a bridge can fail when a day ends (see [`crate::rules`]). One that has failed
//! stays where it is, but is drawn as ruined, and is listed in the alerts until it is built again, which repairs it
//! for the same cost as building it.

use serde::{Deserialize, Serialize};

use crate::bridge::BridgeLink;
use crate::day::DaySave;
use crate::ids::BridgeId;
use crate::newsreel::{Event, News};
use crate::path::{Kind, PathPiece, PathSegment};

/// The chance, as a fraction, that a path fails on a day: 2%. The tests are written in terms of it, and one checks
/// that it is this.
pub const PATH_FAILURE: f32 = 0.02;
/// The chance that a road fails on a day: 0.1%.
pub const ROAD_FAILURE: f32 = 0.001;
/// The chance that a bridge fails on a day: 0.05%.
pub const BRIDGE_FAILURE: f32 = 0.0005;

/// One thing that can fail, and so can be listed as failed.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Thing {
    /// A segment of path.
    Path(PathSegment),
    /// A segment of road.
    Road(PathSegment),
    /// A segment of path that starts at the end of a bridge.
    PathLink(BridgeLink),
    /// A segment of road that starts at the end of a bridge.
    RoadLink(BridgeLink),
    Bridge(BridgeId),
}

impl Thing {
    /// The thing that a piece of path or road of `kind` is.
    pub fn of(kind: Kind, piece: PathPiece) -> Self {
        match (kind, piece) {
            (Kind::Path, PathPiece::Segment(s)) => Self::Path(s),
            (Kind::Road, PathPiece::Segment(s)) => Self::Road(s),
            (Kind::Path, PathPiece::Link(l)) => Self::PathLink(l),
            (Kind::Road, PathPiece::Link(l)) => Self::RoadLink(l),
        }
    }

    /// What to call it: "path", "road" or "bridge".
    pub fn name(self) -> &'static str {
        match self {
            Self::Path(_) | Self::PathLink(_) => "path",
            Self::Road(_) | Self::RoadLink(_) => "road",
            Self::Bridge(_) => "bridge",
        }
    }

    /// How likely it is to fail on a day, as a fraction.
    pub fn chance(self) -> f32 {
        match self {
            Self::Path(_) | Self::PathLink(_) => PATH_FAILURE,
            Self::Road(_) | Self::RoadLink(_) => ROAD_FAILURE,
            Self::Bridge(_) => BRIDGE_FAILURE,
        }
    }
}

impl DaySave {
    /// Whether `thing` has failed.
    pub fn is_failed(&self, thing: Thing) -> bool {
        self.failed.contains(&thing)
    }

    /// Everything there is that could fail, in a steady order: the paths, then the roads, then the bridges.
    pub fn things(&self) -> Vec<Thing> {
        let mut things: Vec<Thing> = self.paths.iter().map(|s| Thing::Path(*s)).collect();
        things.extend(self.bridge_links.iter().map(|l| Thing::PathLink(*l)));
        things.extend(self.roads.iter().map(|s| Thing::Road(*s)));
        things.extend(self.road_links.iter().map(|l| Thing::RoadLink(*l)));
        things.extend(self.bridges.iter().map(|b| Thing::Bridge(b.id)));
        things
    }

    /// Makes `thing` fail, if it hasn't already, which is reported in the newsreel.
    pub fn fail(&mut self, thing: Thing) {
        if !self.failed.contains(&thing) {
            self.failed.push(thing);
            let (subject, piece, regions) = self.thing_news(thing);
            self.newsreel.push(News::about(subject, Event::Failed { piece, regions }));
        }
    }

    /// Mends `thing`, if it had failed.
    pub fn repair(&mut self, thing: Thing) {
        self.failed.retain(|t| *t != thing);
    }

    /// Destroys `thing`, which was built: it is gone from the map, and from what has failed. A bridge takes with it the
    /// paths and roads that start at its ends. It costs nothing, and nothing is paid back. It is reported in the
    /// newsreel, as the work of the country being played. An error says why it can't be done.
    pub fn destroy(&mut self, thing: Thing) -> Result<(), String> {
        let (subject, piece, regions) = self.thing_news(thing);
        let links_before = self.bridge_links.len() + self.road_links.len();
        fn take<T: PartialEq>(from: &mut Vec<T>, item: &T) -> bool {
            let before = from.len();
            from.retain(|x| x != item);
            from.len() != before
        }
        let there = match thing {
            Thing::Path(segment) => take(&mut self.paths, &segment),
            Thing::Road(segment) => take(&mut self.roads, &segment),
            Thing::PathLink(link) => take(&mut self.bridge_links, &link),
            Thing::RoadLink(link) => take(&mut self.road_links, &link),
            Thing::Bridge(id) => {
                let before = self.bridges.len();
                self.bridges.retain(|b| b.id != id);
                self.bridges.len() != before
            }
        };
        if !there {
            return Err(format!("There is no such {} to destroy.", thing.name()));
        }
        self.repair(thing);
        if let Thing::Bridge(id) = thing {
            self.bridge_links.retain(|l| l.bridge != id);
            self.road_links.retain(|l| l.bridge != id);
            self.failed.retain(|t| !matches!(t, Thing::PathLink(l) | Thing::RoadLink(l) if l.bridge == id));
        }
        // A link that is destroyed by itself was one of the links before, and a bridge takes the rest with it.
        let with_it = match thing {
            Thing::Bridge(_) => (links_before - (self.bridge_links.len() + self.road_links.len())) as u32,
            _ => 0,
        };
        let country = self.player_country().map(|c| c.id);
        self.newsreel.push(News::about(subject, Event::Destroyed { piece, country, regions, with_it }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{End, Spot};
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, 5_000).unwrap();
        day
    }

    #[test]
    fn a_road_fails_a_tenth_of_a_percent_and_a_bridge_a_twentieth_and_a_path_by_its_own_rate() {
        let segment = PathSegment::to_edge(1, 2);
        assert_eq!(Thing::Path(segment).chance(), PATH_FAILURE);
        assert_eq!(Thing::Road(segment).chance(), 0.001);
        assert_eq!(Thing::Bridge(BridgeId::new()).chance(), 0.0005);
        // A path from a bridge is a path, and a road from a bridge a road.
        let link = BridgeLink { bridge: BridgeId::new(), end: End::First, region: 1, to: None };
        assert_eq!((Thing::PathLink(link).chance(), Thing::RoadLink(link).chance()), (PATH_FAILURE, 0.001));
        assert_eq!((Thing::PathLink(link).name(), Thing::RoadLink(link).name(), Thing::Bridge(link.bridge).name()), ("path", "road", "bridge"));
    }

    #[test]
    fn the_chances_are_2_percent_for_a_path_a_tenth_of_a_percent_for_a_road_and_a_twentieth_for_a_bridge() {
        assert_eq!((PATH_FAILURE, ROAD_FAILURE, BRIDGE_FAILURE), (0.02, 0.001, 0.0005));
    }

    #[test]
    fn a_path_is_the_most_likely_to_fail_and_a_bridge_the_least() {
        assert!(PATH_FAILURE > ROAD_FAILURE && ROAD_FAILURE > BRIDGE_FAILURE && BRIDGE_FAILURE > 0.0 && PATH_FAILURE < 1.0);
    }

    #[test]
    fn a_piece_is_the_thing_of_its_kind() {
        let segment = PathSegment::to_edge(1, 2);
        let link = BridgeLink { bridge: BridgeId::new(), end: End::Second, region: 3, to: Some(4) };
        assert_eq!(Thing::of(Kind::Path, PathPiece::Segment(segment)), Thing::Path(segment));
        assert_eq!(Thing::of(Kind::Road, PathPiece::Segment(segment)), Thing::Road(segment));
        assert_eq!(Thing::of(Kind::Path, PathPiece::Link(link)), Thing::PathLink(link));
        assert_eq!(Thing::of(Kind::Road, PathPiece::Link(link)), Thing::RoadLink(link));
    }

    #[test]
    fn a_thing_fails_once_and_is_mended() {
        let mut day = day();
        let thing = Thing::Path(PathSegment::to_edge(1, 2));
        assert!(!day.is_failed(thing));
        day.fail(thing);
        day.fail(thing);
        assert_eq!(day.failed, [thing], "it can't fail twice");
        assert!(day.is_failed(thing));
        day.repair(thing);
        assert!(day.failed.is_empty());
    }

    #[test]
    fn a_path_or_a_road_destroyed_is_gone_for_nothing_and_so_is_its_failure() {
        let mut day = day();
        let money = day.countries[0].money;
        let segment = PathSegment::to_edge(1, 2);
        day.build_segment(Kind::Path, segment).unwrap();
        day.build_segment(Kind::Road, PathSegment::to_edge(3, 4)).unwrap();
        day.fail(Thing::Path(segment));
        let paid = day.countries[0].money;
        assert_eq!(paid, money - 10 - 100);
        day.destroy(Thing::Path(segment)).unwrap();
        assert!(day.paths.is_empty() && day.failed.is_empty());
        assert_eq!(day.roads.len(), 1, "only what was destroyed");
        day.destroy(Thing::Road(PathSegment::to_edge(3, 4))).unwrap();
        assert!(day.roads.is_empty());
        assert_eq!(day.countries[0].money, paid, "no cost and no refund");
        // It can be built again, as new.
        day.build_segment(Kind::Path, segment).unwrap();
        assert!(!day.is_failed(Thing::Path(segment)));
        // Destroying what isn't there says so and changes nothing.
        let before = day.clone();
        assert!(day.destroy(Thing::Road(segment)).unwrap_err().contains("no such road"));
        assert_eq!(day, before);
    }

    #[test]
    fn a_bridge_destroyed_takes_the_paths_and_roads_from_its_ends_with_it() {
        let mut day = day();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: -30.0, z: 0.0 }, Spot { x: 30.0, z: 0.0 }).unwrap();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: -30.0, z: 50.0 }, Spot { x: 30.0, z: 50.0 }).unwrap();
        let (gone, kept) = (day.bridges[0].id, day.bridges[1].id);
        let on = |bridge, end| BridgeLink { bridge, end, region: 1, to: None };
        day.build_link_of(Kind::Path, on(gone, End::First)).unwrap();
        day.build_link_of(Kind::Road, on(gone, End::Second)).unwrap();
        day.build_link_of(Kind::Path, on(kept, End::First)).unwrap();
        day.fail(Thing::Bridge(gone));
        day.fail(Thing::PathLink(on(gone, End::First)));
        day.fail(Thing::PathLink(on(kept, End::First)));
        day.destroy(Thing::Bridge(gone)).unwrap();
        assert_eq!(day.bridges.iter().map(|b| b.id).collect::<Vec<_>>(), [kept]);
        assert_eq!(day.bridge_links, [on(kept, End::First)]);
        assert!(day.road_links.is_empty());
        assert_eq!(day.failed, [Thing::PathLink(on(kept, End::First))], "the other bridge's path is still ruined");
        // A path from a bridge can be destroyed on its own.
        day.destroy(Thing::PathLink(on(kept, End::First))).unwrap();
        assert!(day.bridge_links.is_empty() && day.failed.is_empty() && day.bridges.len() == 1);
    }

    #[test]
    fn everything_that_can_fail_is_listed_in_order() {
        let mut day = day();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: -30.0, z: 0.0 }, Spot { x: 30.0, z: 0.0 }).unwrap();
        let bridge = day.bridges[0].id;
        let link = BridgeLink { bridge, end: End::First, region: 1, to: None };
        day.build_link_of(Kind::Path, link).unwrap();
        day.build_segment(Kind::Road, PathSegment::to_edge(1, 2)).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(2, 1)).unwrap();
        assert_eq!(
            day.things(),
            [Thing::Path(PathSegment::to_edge(2, 1)), Thing::PathLink(link), Thing::Road(PathSegment::to_edge(1, 2)), Thing::Bridge(bridge)]
        );
    }

    #[test]
    fn what_has_failed_is_saved_and_carries_forward() {
        let mut day = day();
        let thing = Thing::Road(PathSegment::to_edge(1, 2));
        day.fail(thing);
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["failed"][0], serde_json::json!({ "road": { "region": 1, "neighbor": 2 } }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).failed, [thing]);
    }

    #[test]
    fn a_failed_bridge_is_saved_by_its_id() {
        let mut day = day();
        let id = BridgeId::new();
        day.fail(Thing::Bridge(id));
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["failed"][0], serde_json::json!({ "bridge": id.to_string() }));
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
    }
}
