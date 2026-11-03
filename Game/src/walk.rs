//! Militia walking. A militia that is moved is moved in the day at once, and saved, but it is shown walking to where it
//! is going, along the way that it was found to take, so that the move can be seen. Nothing of this is saved: a game that
//! is opened again has every militia where it is.

use macroquad::math::Vec2;

use crate::ids::MilitiaId;
use crate::militia::Militia;

/// How fast a militia walks, in meters a second, and the shortest and the longest a walk takes, in seconds: a long way
/// is walked faster than that, so that it doesn't take for ever.
const SPEED: f32 = 70.0;
const SHORTEST: f32 = 0.6;
const LONGEST: f32 = 5.0;
/// How many points either side of one a way is smoothed over, which takes the steps of the cells out of it.
const SMOOTHING: usize = 3;
/// How much more of the way has to go along the other axis than along the one a militia faces for it to turn: so that a way
/// that goes across a diagonal doesn't have it turning this way and that.
const TURN_BIAS: f32 = 1.3;

/// One militia on its way.
#[derive(Clone, Debug)]
struct Walk {
    id: MilitiaId,
    /// The way it goes, and how long that is, in meters.
    way: Vec<Vec2>,
    length: f32,
    /// How long the walk takes, and how far into it it is, in seconds.
    duration: f32,
    elapsed: f32,
}

impl Walk {
    /// Where it is, how far along the way, from 0 to 1 and eased, so that it starts and stops gently, and the way it is
    /// going.
    fn at(&self) -> (Vec2, Vec2) {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let mut left = t * t * (3.0 - 2.0 * t) * self.length;
        for pair in self.way.windows(2) {
            let step = pair[0].distance(pair[1]);
            if left <= step && step > 0.0 {
                return (pair[0].lerp(pair[1], left / step), pair[1] - pair[0]);
            }
            left -= step;
        }
        let last = self.way.len().saturating_sub(2);
        (*self.way.last().unwrap_or(&Vec2::ZERO), self.way.get(last + 1).zip(self.way.get(last)).map_or(Vec2::ZERO, |(b, a)| *b - *a))
    }
}

/// Which of the four quarter turns faces along `heading`, if it is facing `current` now: 0 towards +z, 1 towards +x, 2
/// towards -z and 3 towards -x. It keeps facing the way it is unless the heading is clearly the other way.
fn facing_for(heading: Vec2, current: u8) -> u8 {
    if heading.length_squared() == 0.0 {
        return current;
    }
    let (along_x, along_z) = (heading.x.abs(), heading.y.abs());
    let on_x = current % 2 == 1;
    let to_x = if on_x { along_x * TURN_BIAS >= along_z } else { along_x > along_z * TURN_BIAS };
    match (to_x, if to_x { heading.x >= 0.0 } else { heading.y >= 0.0 }) {
        (true, true) => 1,
        (true, false) => 3,
        (false, true) => 0,
        (false, false) => 2,
    }
}

/// The militia that are walking now, and which way each that has walked is facing.
#[derive(Default)]
pub struct Walks {
    walks: Vec<Walk>,
    facings: Vec<(MilitiaId, u8)>,
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
    pub fn start(&mut self, id: MilitiaId, way: &[Vec2]) {
        let way = smoothed(way);
        let length: f32 = way.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        self.walks.retain(|walk| walk.id != id);
        if length > 0.0 {
            self.walks.push(Walk { id, way, length, duration: (length / SPEED).clamp(SHORTEST, LONGEST), elapsed: 0.0 });
        }
    }

    /// Lets `seconds` pass, and turns the walkers to the way they are going. The walks that are over are done with.
    pub fn update(&mut self, seconds: f32) {
        for walk in &mut self.walks {
            walk.elapsed += seconds;
            let (_, heading) = walk.at();
            let now = self.facings.iter().find(|(id, _)| *id == walk.id).map_or(0, |(_, facing)| *facing);
            let facing = facing_for(heading, now);
            match self.facings.iter_mut().find(|(id, _)| *id == walk.id) {
                Some(entry) => entry.1 = facing,
                None => self.facings.push((walk.id, facing)),
            }
        }
        self.walks.retain(|walk| walk.elapsed < walk.duration);
    }

    /// Which way the militia with this id faces, in quarter turns (0 towards +z, 1 towards +x, 2 towards -z, 3 towards -x):
    /// the way it last walked, and towards +z if it never has.
    pub fn facing(&self, id: MilitiaId) -> u8 {
        self.facings.iter().find(|(f, _)| *f == id).map_or(0, |(_, facing)| *facing)
    }

    /// Whether the militia with this id is walking.
    pub fn is_walking(&self, id: MilitiaId) -> bool {
        self.walks.iter().any(|walk| walk.id == id)
    }

    /// Where to show `militia`: part of the way along its walk if it is walking, and otherwise where it is.
    pub fn position(&self, militia: &Militia) -> Vec2 {
        self.walks.iter().find(|walk| walk.id == militia.id).map_or(Vec2::new(militia.x, militia.z), |walk| walk.at().0)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::ids::{CityId, CountryId};
    use macroquad::math::vec2;

    fn militia(at: Vec2) -> Militia {
        Militia { id: MilitiaId::new(), name: String::new(), country: CountryId::new(), city: CityId::new(), raised: Date::FIRST, members: 10, x: at.x, z: at.y, points: 0 }
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
        assert_eq!(walks.facing(unit.id), 0, "it faces south until it has walked");
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
    fn a_militia_turns_to_face_the_way_it_walks_and_keeps_facing_that_way() {
        let mut walks = Walks::default();
        let unit = militia(Vec2::ZERO);
        for (way, facing) in [(vec2(100.0, 0.0), 1), (vec2(0.0, 100.0), 0), (vec2(-100.0, 0.0), 3), (vec2(0.0, -100.0), 2)] {
            walks.start(unit.id, &[Vec2::ZERO, way]);
            walks.update(0.1);
            assert_eq!(walks.facing(unit.id), facing, "{way}");
            walks.update(10.0);
            assert!(!walks.is_walking(unit.id));
            assert_eq!(walks.facing(unit.id), facing, "it keeps facing the way it went");
        }
    }

    #[test]
    fn a_way_across_a_diagonal_does_not_turn_it_this_way_and_that() {
        assert_eq!(facing_for(vec2(1.0, 1.05), 1), 1, "close to diagonal, it stays facing x");
        assert_eq!(facing_for(vec2(1.0, 1.05), 0), 0, "and one facing z stays facing z");
        assert_eq!(facing_for(vec2(1.0, 2.0), 1), 0, "but a clear turn is made");
        assert_eq!(facing_for(vec2(2.0, 1.0), 0), 1);
        assert_eq!(facing_for(Vec2::ZERO, 3), 3, "no heading, no turn");
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
}
