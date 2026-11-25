//! Which regions of the map have units in them: the regions the militia and ships of the country being played stand in, or those of
//! the other countries, the enemy (there is no peace or war between countries to tell them apart by). The map makes them glow round
//! their borders while the player holds the mouse over "Units" or "Enemy" in the footer (see [`crate::hud::footer_labels`]).

use nworld_map::regions::RegionId;
use nworld_map::world::World;

use crate::day::DaySave;

/// Whose units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// The country being played.
    Mine,
    /// Every other country.
    Enemy,
}

/// The regions that have a militia, a ship or a person of `whose` in them, each once, in order of their numbers. A unit that stands where there is
/// no region (on a map that isn't divided into them) is in none. There are none if no country is being played.
pub fn with_units(world: &World, day: &DaySave, whose: Whose) -> Vec<RegionId> {
    regions_of(world, day, whose, false)
}

/// The same, but only counting the units that still have movement points left today.
pub fn with_units_that_can_move(world: &World, day: &DaySave, whose: Whose) -> Vec<RegionId> {
    regions_of(world, day, whose, true)
}

fn regions_of(world: &World, day: &DaySave, whose: Whose, moving_only: bool) -> Vec<RegionId> {
    let Some(country) = day.player_country().map(|c| c.id) else {
        return Vec::new();
    };
    let theirs = |owner| (owner == country) == (whose == Whose::Mine);
    let militia = day.militias.iter().filter(|m| theirs(m.country) && (!moving_only || m.points > 0)).map(|m| (m.x, m.z));
    let ships = day.ships.iter().filter(|s| theirs(s.country) && (!moving_only || s.points > 0)).map(|s| (s.x, s.z));
    let people = day.people.iter().filter(|p| theirs(p.country) && (!moving_only || p.points > 0)).filter_map(|p| p.at);
    let mut regions: Vec<RegionId> = militia.chain(ships).chain(people).filter_map(|(x, z)| world.region_at(x, z)).collect();
    regions.sort_by_key(|r| r.raw());
    regions.dedup();
    regions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::test_support;
    use macroquad::math::vec2;

    #[test]
    fn it_lists_each_region_the_country_played_has_units_in_once() {
        let world = test_support::regioned_continent();
        let spots = test_support::land_spots(&world, 12);
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        assert!(with_units(&world, &day, Whose::Mine).is_empty() && with_units(&world, &day, Whose::Enemy).is_empty(), "no units");

        day.add_money(day.countries[0].id, 1_000_000).unwrap();
        // A militia in each of the first three spots (raised in a city of the country, and then put where the spot is).
        let mut expected = Vec::new();
        for (n, spot) in spots.iter().take(3).enumerate() {
            day.found_city(&format!("City {n}"), spot.x, spot.y, None).unwrap();
            day.cities[n].population = 2_000;
            day.raise_militia(day.cities[n].id, 50, *spot + vec2(10.0, 0.0)).unwrap();
            expected.push(world.region_at(spot.x + 10.0, spot.y).unwrap());
        }
        expected.sort_by_key(|r| r.raw());
        expected.dedup();
        assert_eq!(with_units(&world, &day, Whose::Mine), expected);
        assert!(with_units(&world, &day, Whose::Enemy).is_empty(), "none of them are the enemy's");

        // Freshly raised, they have moves left; once one has none, its region is no longer among those that can move.
        assert_eq!(with_units_that_can_move(&world, &day, Whose::Mine), expected);
        let spent = day.militias[0].id;
        day.end_militia_moves(spent).unwrap();
        let without_first: Vec<RegionId> = expected.iter().copied().filter(|r| Some(*r) != world.region_at(day.militias[0].x, day.militias[0].z)).collect();
        assert_eq!(with_units_that_can_move(&world, &day, Whose::Mine), without_first);
        assert_eq!(with_units(&world, &day, Whose::Mine), expected, "it still stands there");

        // A second militia in the same region adds nothing, and another country's units are not counted.
        let before = with_units(&world, &day, Whose::Mine);
        day.militias.push(day.militias[0].clone());
        assert_eq!(with_units(&world, &day, Whose::Mine), before);
        for militia in &mut day.militias {
            militia.country = day.countries[1].id;
        }
        assert!(with_units(&world, &day, Whose::Mine).is_empty());
        assert_eq!(with_units(&world, &day, Whose::Enemy), before, "now they are the other country's");
    }

    #[test]
    fn people_on_the_map_count_as_units_and_can_move_while_they_have_points() {
        let world = test_support::regioned_continent();
        let spot = test_support::land_spots(&world, 2)[0];
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        assert!(with_units(&world, &day, Whose::Mine).is_empty(), "no place on the map yet");
        day.people[0].at = Some((spot.x, spot.y));
        let here = world.region_at(spot.x, spot.y).unwrap();
        assert_eq!(with_units(&world, &day, Whose::Mine), vec![here]);
        assert_eq!(with_units_that_can_move(&world, &day, Whose::Mine), vec![here]);
        day.people[0].points = 0;
        assert_eq!(with_units(&world, &day, Whose::Mine), vec![here]);
        assert!(with_units_that_can_move(&world, &day, Whose::Mine).is_empty());
        day.people[0].country = day.countries[1].id;
        assert!(with_units(&world, &day, Whose::Mine).is_empty());
        assert_eq!(with_units(&world, &day, Whose::Enemy), vec![here]);
    }

    #[test]
    fn a_map_with_no_regions_has_none() {
        let world = test_support::small_continent();
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.add_money(day.countries[0].id, 1_000_000).unwrap();
        let spot = test_support::land_spots(&world, 1)[0];
        day.found_city("City", spot.x, spot.y, None).unwrap();
        day.cities[0].population = 2_000;
        day.raise_militia(day.cities[0].id, 50, spot + vec2(10.0, 0.0)).unwrap();
        assert!(with_units(&world, &day, Whose::Mine).is_empty());
    }
}
