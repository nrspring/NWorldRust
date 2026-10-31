//! Whether a woodcutter is connected to a city: it only operates when the region it stands in has an unbroken route of
//! paths, roads and bridges to the region of a city of its country. Some of the route can be path and some road. A
//! path, road or bridge that has failed is broken, and so isn't part of a route until it is built again.
//!
//! The way the routes go is that of the pieces themselves (see [`crate::path`] and [`crate::bridge`]): a point of a
//! route is the center of a region, the middle of the border between two regions, or one end of a bridge, and each
//! piece joins two of them. A region is reached when a route gets to its center or to the middle of any of its borders,
//! so a route that ends at the border of the region a city is in has reached it.

use std::collections::{HashMap, HashSet};

use crate::bridge::End;
use crate::day::DaySave;
use crate::failure::Thing;
use crate::ids::{BridgeId, WoodcutterId};
use crate::newsreel::{Event, News};
use crate::path::{Kind, PathPiece};
use crate::woodcutter::Woodcutter;

/// A point a route can pass through.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Point {
    /// The center of the region with this number.
    Center(u16),
    /// The middle of the border between two regions, given with the lower number first.
    Edge(u16, u16),
    /// One end of a bridge.
    BridgeEnd(BridgeId, End),
}

impl Point {
    fn edge(a: u16, b: u16) -> Self {
        Self::Edge(a.min(b), a.max(b))
    }
}

/// The points that the unbroken pieces join up, as sets of points that can be got between.
struct Network {
    index: HashMap<Point, usize>,
    parent: Vec<usize>,
    /// The points that belong to each region: its center and the middle of each of its borders.
    regions: HashMap<u16, Vec<usize>>,
}

impl Network {
    /// The network of the paths, roads and bridges of `day` that haven't failed.
    fn of(day: &DaySave) -> Self {
        let mut network = Self { index: HashMap::new(), parent: Vec::new(), regions: HashMap::new() };
        for kind in [Kind::Path, Kind::Road] {
            for segment in day.segments(kind) {
                if day.is_failed(Thing::of(kind, PathPiece::Segment(*segment))) {
                    continue;
                }
                let (from, to) = match segment.other {
                    None => (Point::Center(segment.region), Point::edge(segment.region, segment.neighbor)),
                    Some(other) => (Point::edge(segment.region, segment.neighbor), Point::edge(segment.region, other)),
                };
                network.join(from, to);
            }
            for link in day.links(kind) {
                if day.is_failed(Thing::of(kind, PathPiece::Link(*link))) {
                    continue;
                }
                let to = link.to.map_or(Point::Center(link.region), |neighbor| Point::edge(link.region, neighbor));
                network.join(Point::BridgeEnd(link.bridge, link.end), to);
            }
        }
        for bridge in day.bridges.iter().filter(|b| !day.is_failed(Thing::Bridge(b.id))) {
            network.join(Point::BridgeEnd(bridge.id, End::First), Point::BridgeEnd(bridge.id, End::Second));
        }
        network
    }

    fn node(&mut self, point: Point) -> usize {
        if let Some(node) = self.index.get(&point) {
            return *node;
        }
        let node = self.parent.len();
        self.parent.push(node);
        self.index.insert(point, node);
        match point {
            Point::Center(region) => self.regions.entry(region).or_default().push(node),
            Point::Edge(a, b) => {
                self.regions.entry(a).or_default().push(node);
                self.regions.entry(b).or_default().push(node);
            }
            Point::BridgeEnd(..) => {}
        }
        node
    }

    fn root(&mut self, mut node: usize) -> usize {
        while self.parent[node] != node {
            self.parent[node] = self.parent[self.parent[node]];
            node = self.parent[node];
        }
        node
    }

    fn join(&mut self, a: Point, b: Point) {
        let (a, b) = (self.node(a), self.node(b));
        let (a, b) = (self.root(a), self.root(b));
        self.parent[a] = b;
    }

    /// Whether regions `a` and `b` can be got between, by a route that reaches both: the same region always can.
    fn connects(&mut self, a: u16, b: u16) -> bool {
        if a == b {
            return true;
        }
        let (in_a, in_b) = (self.regions.get(&a).cloned().unwrap_or_default(), self.regions.get(&b).cloned().unwrap_or_default());
        let reached_from_b: HashSet<usize> = in_b.into_iter().map(|node| self.root(node)).collect();
        in_a.into_iter().any(|node| reached_from_b.contains(&self.root(node)))
    }
}

impl DaySave {
    /// The woodcutters that have no unbroken route to a city of their country, and so aren't operating, in the order
    /// they were built.
    pub fn cut_off_woodcutters(&self) -> Vec<WoodcutterId> {
        if self.woodcutters.is_empty() {
            return Vec::new();
        }
        let mut network = Network::of(self);
        self.woodcutters.iter().filter(|w| !self.reaches_a_city(&mut network, w)).map(|w| w.id).collect()
    }

    /// Whether the woodcutter with this id is operating: it has a route to a city of its country.
    pub fn woodcutter_operating(&self, id: WoodcutterId) -> bool {
        self.woodcutter(id).is_some() && !self.cut_off_woodcutters().contains(&id)
    }

    /// Brings what each woodcutter says of whether it is operating up to date with whether it has a route to a city,
    /// and reports each one that stopped or started in the newsreel. Returns whether any changed.
    pub fn update_woodcutter_operation(&mut self) -> bool {
        if self.woodcutters.is_empty() {
            return false;
        }
        let cut_off = self.cut_off_woodcutters();
        let mut news = Vec::new();
        for woodcutter in &mut self.woodcutters {
            let operating = !cut_off.contains(&woodcutter.id);
            if woodcutter.operating != operating {
                woodcutter.operating = operating;
                news.push(News::about(woodcutter.id, if operating { Event::WoodcutterStarted } else { Event::WoodcutterStopped }));
            }
        }
        let changed = !news.is_empty();
        self.newsreel.extend(news);
        changed
    }

    fn reaches_a_city(&self, network: &mut Network, woodcutter: &Woodcutter) -> bool {
        self.cities.iter().filter(|c| c.country == woodcutter.country).filter_map(|c| c.region).any(|region| network.connects(woodcutter.region, region))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{BridgeLink, Spot};
    use crate::calendar::Date;
    use crate::path::PathSegment;

    const A: u16 = 1;
    const B: u16 = 2;
    const C: u16 = 3;

    /// A day with a country, Aldoria, with a city in region `A`, and a woodcutter of it in region `B`.
    fn day() -> (DaySave, WoodcutterId) {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 10_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(A)).unwrap();
        day.build_woodcutter(100.0, 0.0, B).unwrap();
        let id = day.woodcutters[0].id;
        (day, id)
    }

    /// A route between the centers of two neighboring regions: a segment from each center to the border.
    fn join(day: &mut DaySave, kind: Kind, a: u16, b: u16) {
        day.build_segment(kind, PathSegment::to_edge(a, b)).unwrap();
        day.build_segment(kind, PathSegment::to_edge(b, a)).unwrap();
    }

    #[test]
    fn a_woodcutter_with_no_route_is_cut_off() {
        let (day, id) = day();
        assert_eq!(day.cut_off_woodcutters(), [id]);
        assert!(!day.woodcutter_operating(id));
    }

    #[test]
    fn a_woodcutter_in_the_region_of_a_city_needs_no_route() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 10_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(A)).unwrap();
        day.build_woodcutter(10.0, 10.0, A).unwrap();
        assert!(day.cut_off_woodcutters().is_empty());
        assert!(day.woodcutter_operating(day.woodcutters[0].id));
    }

    #[test]
    fn a_path_between_the_two_regions_makes_it_operate() {
        let (mut day, id) = day();
        join(&mut day, Kind::Path, A, B);
        assert!(day.woodcutter_operating(id));
    }

    #[test]
    fn a_route_that_ends_at_the_border_of_the_cities_region_has_reached_it() {
        // From the woodcutter's center to the middle of its border with the city's region, and no further.
        let (mut day, id) = day();
        day.build_segment(Kind::Path, PathSegment::to_edge(B, A)).unwrap();
        assert!(day.woodcutter_operating(id));
        // The other way round, from the city's center to the border, reaches the woodcutter's region just the same.
        let (mut other, id) = self::day();
        other.build_segment(Kind::Path, PathSegment::to_edge(A, B)).unwrap();
        assert!(other.woodcutter_operating(id));
        // A route that reaches neither region doesn't.
        let (mut far, id) = self::day();
        far.build_segment(Kind::Path, PathSegment::to_edge(C, 9)).unwrap();
        assert!(!far.woodcutter_operating(id));
        // One that reaches only the woodcutter's region doesn't either.
        far.build_segment(Kind::Path, PathSegment::to_edge(B, C)).unwrap();
        assert!(!far.woodcutter_operating(id));
    }

    #[test]
    fn a_route_can_be_part_path_and_part_road_and_go_through_other_regions() {
        let (mut day, id) = day();
        // B to C by road, across C by path, and so to the border with A.
        join(&mut day, Kind::Road, B, C);
        assert!(!day.woodcutter_operating(id), "it reaches C, and no further");
        day.build_segment(Kind::Path, PathSegment::across(C, B, A)).unwrap();
        assert!(day.woodcutter_operating(id));
    }

    /// The route from `B` through `C` to the border with `A`, three segments.
    fn route(day: &mut DaySave) {
        day.build_segment(Kind::Path, PathSegment::to_edge(B, C)).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(C, B)).unwrap();
        day.build_segment(Kind::Road, PathSegment::to_edge(C, A)).unwrap();
    }

    #[test]
    fn a_broken_piece_cuts_the_route_until_it_is_built_again() {
        let (mut day, id) = day();
        route(&mut day);
        assert!(day.woodcutter_operating(id));
        let piece = Thing::Path(PathSegment::to_edge(C, B));
        day.fail(piece);
        assert!(!day.woodcutter_operating(id));
        day.build_segment(Kind::Path, PathSegment::to_edge(C, B)).unwrap();
        assert!(!day.is_failed(piece) && day.woodcutter_operating(id));
        // A road that has failed cuts it too, and destroying a piece of it does.
        day.fail(Thing::Road(PathSegment::to_edge(C, A)));
        assert!(!day.woodcutter_operating(id));
        day.repair(Thing::Road(PathSegment::to_edge(C, A)));
        day.destroy(piece).unwrap();
        assert!(!day.woodcutter_operating(id));
    }

    /// The news of the woodcutter's operation in the day's newsreel, as headlines, after the one that it was built.
    fn operation_news(day: &DaySave) -> Vec<String> {
        day.newsreel.iter().filter(|n| matches!(n.event, Event::WoodcutterStarted | Event::WoodcutterStopped)).map(|n| n.headline(day)).collect()
    }

    #[test]
    fn a_woodcutter_that_gets_a_route_starts_operating_and_it_is_news_once() {
        let (mut day, id) = day();
        assert!(!day.woodcutters[0].operating, "it starts off not operating");
        // Still no route, so nothing to report.
        assert!(!day.update_woodcutter_operation());
        assert!(operation_news(&day).is_empty());
        route(&mut day);
        assert!(day.update_woodcutter_operation());
        assert!(day.woodcutters[0].operating);
        assert_eq!(operation_news(&day), ["Aldoria's woodcutter in region 2 starts operating"]);
        assert_eq!(day.newsreel.last().unwrap().subject, id.into());
        // Looking again says nothing more.
        assert!(!day.update_woodcutter_operation());
        assert_eq!(operation_news(&day).len(), 1);
    }

    #[test]
    fn a_woodcutter_that_loses_its_route_stops_and_it_starts_again_when_it_is_mended_each_is_news() {
        let (mut day, _) = day();
        route(&mut day);
        day.update_woodcutter_operation();
        let piece = Thing::Path(PathSegment::to_edge(C, B));
        day.fail(piece);
        assert!(day.update_woodcutter_operation());
        assert!(!day.woodcutters[0].operating);
        assert_eq!(operation_news(&day).last().unwrap(), "Aldoria's woodcutter in region 2 stops operating, with no route to a city");
        assert!(!day.update_woodcutter_operation(), "and it is only said once");
        // Built again, it is mended, and the woodcutter starts again.
        day.build_segment(Kind::Path, PathSegment::to_edge(C, B)).unwrap();
        assert!(day.update_woodcutter_operation());
        assert!(day.woodcutters[0].operating);
        assert_eq!(operation_news(&day).len(), 3, "started, stopped, started");
        assert_eq!(operation_news(&day).last().unwrap(), "Aldoria's woodcutter in region 2 starts operating");
    }

    #[test]
    fn each_woodcutter_is_reported_on_its_own() {
        let (mut day, first) = day();
        day.build_woodcutter(150.0, 0.0, C).unwrap();
        let second = day.woodcutters[1].id;
        // The route from B reaches C, where the second one is, but the city is in A: neither has a route to it until the
        // route goes on to A, and then they start together.
        day.build_segment(Kind::Path, PathSegment::to_edge(B, C)).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(C, B)).unwrap();
        assert!(!day.update_woodcutter_operation());
        day.build_segment(Kind::Road, PathSegment::to_edge(C, A)).unwrap();
        assert!(day.update_woodcutter_operation());
        let started: Vec<_> = day.newsreel.iter().filter(|n| n.event == Event::WoodcutterStarted).map(|n| n.subject).collect();
        assert_eq!(started, [first.into(), second.into()]);
        // Cutting the last piece stops both.
        day.fail(Thing::Road(PathSegment::to_edge(C, A)));
        day.update_woodcutter_operation();
        assert_eq!(day.newsreel.iter().filter(|n| n.event == Event::WoodcutterStopped).count(), 2);
    }

    #[test]
    fn what_happens_as_the_day_ends_is_reported_in_the_new_days_news() {
        let (mut day, _) = day();
        route(&mut day);
        day.update_woodcutter_operation();
        // The route is lost as the day ends, and the new day begins with the news and with it recorded.
        day.destroy(Thing::Road(PathSegment::to_edge(C, A))).unwrap();
        let next = day.next_day();
        assert!(operation_news(&next).contains(&"Aldoria's woodcutter in region 2 stops operating, with no route to a city".to_owned()));
        assert!(!next.woodcutters[0].operating);
        assert!(day.woodcutters[0].operating, "the day that ended is as it was");
        // A day with nothing lost carries it on with nothing to report.
        let (mut kept, _) = self::day();
        route(&mut kept);
        kept.update_woodcutter_operation();
        assert!(kept.following(crate::calendar::Date { year: 1, month: 1, day: 2 }).woodcutters[0].operating);
    }

    #[test]
    fn an_old_save_with_a_woodcutter_that_says_nothing_of_operating_has_it_not_operating() {
        let (day, _) = day();
        let mut json = serde_json::to_value(&day).unwrap();
        json["woodcutters"][0].as_object_mut().unwrap().remove("operating");
        assert!(!serde_json::from_value::<DaySave>(json).unwrap().woodcutters[0].operating);
    }

    #[test]
    fn a_bridge_joins_the_regions_at_its_two_ends_unless_it_has_failed() {
        let (mut day, id) = day();
        day.build_bridge(Spot { x: -30.0, z: 0.0 }, Spot { x: 30.0, z: 0.0 }).unwrap();
        let bridge = day.bridges[0].id;
        let at_a = BridgeLink { bridge, end: End::First, region: A, to: None };
        let at_b = BridgeLink { bridge, end: End::Second, region: B, to: Some(C) };
        day.build_link_of(Kind::Path, at_a).unwrap();
        assert!(!day.woodcutter_operating(id), "only the city's end is reached");
        day.build_link_of(Kind::Road, at_b).unwrap();
        assert!(day.woodcutter_operating(id));
        day.fail(Thing::Bridge(bridge));
        assert!(!day.woodcutter_operating(id), "a bridge that has failed carries nothing");
        day.repair(Thing::Bridge(bridge));
        day.fail(Thing::RoadLink(at_b));
        assert!(!day.woodcutter_operating(id), "nor does a path from it that has failed");
    }

    #[test]
    fn only_a_city_of_the_same_country_counts() {
        let (mut day, id) = day();
        join(&mut day, Kind::Path, B, C);
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        day.found_city("Brevonport", 50.0, 0.0, Some(C)).unwrap();
        assert!(!day.woodcutter_operating(id), "the route reaches Brevon's city, and the woodcutter is Aldoria's");
        // A route on to the city of its own country does.
        day.select_country(day.countries[0].id).unwrap();
        join(&mut day, Kind::Path, C, A);
        assert!(day.woodcutter_operating(id));
    }

    #[test]
    fn a_country_with_no_city_has_no_route_to_one() {
        let (mut day, id) = day();
        join(&mut day, Kind::Path, A, B);
        day.cities.clear();
        assert!(!day.woodcutter_operating(id));
    }

    #[test]
    fn it_does_not_matter_which_of_the_cities_is_reached() {
        let (mut day, id) = day();
        day.found_city("Farhold", 80.0, 0.0, Some(C)).unwrap();
        join(&mut day, Kind::Road, B, C);
        assert!(day.woodcutter_operating(id));
    }

    #[test]
    fn nothing_is_cut_off_when_there_are_no_woodcutters_and_an_unknown_one_does_not_operate() {
        let day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert!(day.cut_off_woodcutters().is_empty());
        assert!(!day.woodcutter_operating(WoodcutterId::new()));
    }
}
