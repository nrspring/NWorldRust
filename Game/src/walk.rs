//! Militia walking (and ships sailing, which is the same thing). A militia that is moved is moved in the day at once, and saved, but it is shown walking to where it
//! is going, along the way that it was found to take, so that the move can be seen. Nothing of this is saved: a game that
//! is opened again has every militia where it is.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use macroquad::math::Vec2;

use crate::ids::Guid;
use crate::militia::Militia;

/// How fast a militia walks, in meters a second, and the shortest and the longest a walk takes, in seconds: a long way
/// is walked faster than that, so that it doesn't take for ever.
const SPEED: f32 = 70.0;
const SHORTEST: f32 = 0.6;
const LONGEST: f32 = 5.0;
/// How many points either side of one a way is smoothed over, which takes the steps of the cells out of it.
const SMOOTHING: usize = 3;

/// One militia on its way.
#[derive(Clone, Debug)]
struct Walk {
    id: Guid,
    /// The way it goes, and how long that is, in meters.
    way: Vec<Vec2>,
    length: f32,
    /// How long the walk takes, and how far into it it is, in seconds.
    duration: f32,
    elapsed: f32,
}

impl Walk {
    /// Where it is, how far along the way, from 0 to 1 and eased, so that it starts and stops gently.
    fn at(&self) -> Vec2 {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let mut left = t * t * (3.0 - 2.0 * t) * self.length;
        for pair in self.way.windows(2) {
            let step = pair[0].distance(pair[1]);
            if left <= step && step > 0.0 {
                return pair[0].lerp(pair[1], left / step);
            }
            left -= step;
        }
        *self.way.last().unwrap_or(&Vec2::ZERO)
    }

    /// Which way it is going now: a little way back along the way to a little way on.
    fn heading(&self) -> Option<Vec2> {
        let mut ahead = self.clone();
        ahead.elapsed = (self.elapsed + HEADING_LOOK).min(self.duration);
        let mut behind = self.clone();
        behind.elapsed = (self.elapsed - HEADING_LOOK).max(0.0);
        let d = ahead.at() - behind.at();
        (d.length() > 1e-3).then(|| d.normalize())
    }
}

/// How far back and on along a walk, in seconds, which way it goes is worked out from.
const HEADING_LOOK: f32 = 0.08;

/// The militia that are walking now.
#[derive(Default)]
pub struct Walks {
    walks: Vec<Walk>,
    /// Which way each unit that has walked was last going, as an angle (see [`Walks::facing`]).
    facings: HashMap<Guid, f32>,
}

/// The turn about the upright, from the east, that faces the way `heading` (along x and z) goes.
pub fn yaw_of(heading: Vec2) -> f32 {
    (-heading.y).atan2(heading.x)
}

/// `way` with each point taken to the average of those round it, which keeps its ends.
fn smoothed(way: &[Vec2]) -> Vec<Vec2> {
    (0..way.len())
        .map(|k| {
            if k == 0 || k + 1 == way.len() {
                return way[k];
            }
            let reach = SMOOTHING.min(k).min(way.len() - 1 - k);
            way[k - reach..=k + reach].iter().copied().sum::<Vec2>() / (2 * reach + 1) as f32
        })
        .collect()
}

impl Walks {
    /// Starts the militia with this id walking along `way`, which starts where it is. A way with nowhere to go is no
    /// walk at all.
    pub fn start(&mut self, id: impl Into<Guid>, way: &[Vec2]) {
        let id = id.into();
        let way = smoothed(way);
        let length: f32 = way.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        self.walks.retain(|walk| walk.id != id);
        if length > 0.0 {
            self.walks.push(Walk { id, way, length, duration: (length / SPEED).clamp(SHORTEST, LONGEST), elapsed: 0.0 });
        }
    }

    /// Lets `seconds` pass. The walks that are over are done with.
    pub fn update(&mut self, seconds: f32) {
        for walk in &mut self.walks {
            walk.elapsed += seconds;
            if let Some(heading) = walk.heading() {
                self.facings.insert(walk.id, yaw_of(heading));
            }
        }
        self.walks.retain(|walk| walk.elapsed < walk.duration);
    }

    /// Whether the militia with this id is walking.
    pub fn is_walking(&self, id: impl Into<Guid>) -> bool {
        let id = id.into();
        self.walks.iter().any(|walk| walk.id == id)
    }

    /// Where to show `militia`: part of the way along its walk if it is walking, and otherwise where it is.
    pub fn position(&self, militia: &Militia) -> Vec2 {
        self.position_of(militia.id, Vec2::new(militia.x, militia.z))
    }

    /// Where to show the unit with this id, which the day has at `resting`: part of the way along its walk if it is walking, and
    /// otherwise there.
    pub fn position_of(&self, id: impl Into<Guid>, resting: Vec2) -> Vec2 {
        let id = id.into();
        self.walks.iter().find(|walk| walk.id == id).map_or(resting, Walk::at)
    }

    /// Which way the unit with this id faces, as a turn about the upright from the east (as [`crate::model::Model::new`] takes
    /// it): the way it is going while it walks, and then the way it last went. One that hasn't moved since the game started
    /// faces a way of its own, which is always the same for it.
    pub fn facing(&self, id: impl Into<Guid>) -> f32 {
        let id = id.into();
        if let Some(heading) = self.walks.iter().find(|walk| walk.id == id).and_then(Walk::heading) {
            return yaw_of(heading);
        }
        self.facings.get(&id).copied().unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            id.hash(&mut hasher);
            (hasher.finish() % 3_600) as f32 / 3_600.0 * std::f32::consts::TAU
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::{CityId, CountryId, MilitiaId, ShipId};
    use macroquad::math::vec2;

    fn militia(at: Vec2) -> Militia {
        Militia { id: MilitiaId::new(), name: String::new(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members: 10, x: at.x, z: at.y, points: 0, under_siege: false, trained: false, archers: false, cavalry: false, pay_first: false, garrison: false, in_fort: None }
    }

    fn straight() -> Vec<Vec2> {
        (0..=10).map(|k| vec2(k as f32 * 10.0, 0.0)).collect()
    }

    #[test]
    fn a_militia_that_is_not_walking_is_shown_where_it_is() {
        let walks = Walks::default();
        let unit = militia(vec2(5.0, 6.0));
        assert_eq!(walks.position(&unit), vec2(5.0, 6.0));
        assert!(!walks.is_walking(unit.id));
    }

    #[test]
    fn a_walker_goes_from_the_start_to_the_end_of_its_way_and_is_then_done() {
        let mut walks = Walks::default();
        let unit = militia(vec2(100.0, 0.0));
        walks.start(unit.id, &straight());
        assert!(walks.is_walking(unit.id));
        assert_eq!(walks.position(&unit), vec2(0.0, 0.0), "it starts where it set out from");
        let mut last = -1.0;
        for _ in 0..30 {
            walks.update(0.04);
            let x = walks.position(&unit).x;
            assert!(x >= last, "it never goes back: {x} after {last}");
            last = x;
        }
        // 100 m at 70 m/s is about 1.4 s; a little over that, it has got there.
        walks.update(1.0);
        assert!(!walks.is_walking(unit.id));
        assert_eq!(walks.position(&unit), vec2(100.0, 0.0), "and is shown where it is");
    }

    #[test]
    fn a_walk_takes_about_as_long_as_the_way_but_never_too_short_or_too_long() {
        let seconds = |meters: f32| {
            let mut walks = Walks::default();
            let unit = militia(Vec2::ZERO);
            walks.start(unit.id, &[Vec2::ZERO, vec2(meters, 0.0)]);
            walks.walks[0].duration
        };
        assert!((seconds(140.0) - 2.0).abs() < 1e-4);
        assert_eq!(seconds(5.0), SHORTEST);
        assert_eq!(seconds(3_000.0), LONGEST);
    }

    #[test]
    fn it_starts_and_stops_gently() {
        let mut walks = Walks::default();
        let unit = militia(vec2(100.0, 0.0));
        walks.start(unit.id, &straight());
        let duration = walks.walks[0].duration;
        walks.update(duration * 0.1);
        let early = walks.position(&unit).x;
        assert!(early < 100.0 * 0.1, "a tenth of the time is less than a tenth of the way: {early}");
        walks.update(duration * 0.4);
        let middle = walks.position(&unit).x;
        assert!((middle - 50.0).abs() < 1.0, "halfway at half time: {middle}");
    }

    #[test]
    fn a_new_walk_replaces_an_old_one_and_a_walk_with_nowhere_to_go_is_none() {
        let mut walks = Walks::default();
        let unit = militia(vec2(100.0, 0.0));
        walks.start(unit.id, &straight());
        walks.start(unit.id, &[vec2(0.0, 0.0), vec2(0.0, 50.0)]);
        assert_eq!(walks.walks.len(), 1);
        walks.start(unit.id, &[vec2(3.0, 3.0)]);
        assert!(!walks.is_walking(unit.id));
    }

    #[test]
    fn smoothing_takes_the_steps_out_but_keeps_both_ends() {
        let stairs = vec![vec2(0.0, 0.0), vec2(4.0, 0.0), vec2(4.0, 4.0), vec2(8.0, 4.0), vec2(8.0, 8.0), vec2(12.0, 8.0)];
        let smooth = smoothed(&stairs);
        assert_eq!((smooth[0], smooth[5]), (stairs[0], stairs[5]));
        assert_eq!(smooth.len(), stairs.len());
        let length = |way: &[Vec2]| way.windows(2).map(|p| p[0].distance(p[1])).sum::<f32>();
        assert!(length(&smooth) < length(&stairs), "it is shorter without the corners");
    }

    #[test]
    fn a_ship_walks_like_a_militia_under_its_own_id() {
        let ship = ShipId::new();
        let mut walks = Walks::default();
        assert!(!walks.is_walking(ship));
        walks.start(ship, &[vec2(0.0, 0.0), vec2(100.0, 0.0)]);
        assert!(walks.is_walking(ship));
        walks.update(0.1);
        let at = walks.position_of(ship, vec2(100.0, 0.0));
        assert!(at.x < 100.0, "it is on its way, not yet where the day says: {at}");
        assert_eq!(walks.position_of(ShipId::new(), vec2(7.0, 8.0)), vec2(7.0, 8.0), "one that isn't walking is where it is");
        walks.update(10.0);
        assert!(!walks.is_walking(ship));
    }

    #[test]
    fn a_walker_faces_the_way_it_goes_and_keeps_facing_it_when_it_stops() {
        let mut walks = Walks::default();
        let id = MilitiaId::new();
        let still = walks.facing(id);
        assert_eq!(still, walks.facing(id), "one that hasn't moved always faces the same way");
        walks.start(id, &[Vec2::ZERO, Vec2::new(0.0, -200.0)]);
        walks.update(0.5);
        let north = yaw_of(Vec2::new(0.0, -1.0));
        assert!((walks.facing(id) - north).abs() < 1e-3, "going north: {}", walks.facing(id));
        walks.update(100.0);
        assert!(!walks.is_walking(id));
        assert!((walks.facing(id) - north).abs() < 1e-3, "and still facing north when it gets there");
        assert!((yaw_of(Vec2::new(1.0, 0.0))).abs() < 1e-6, "east is no turn");
    }
}
