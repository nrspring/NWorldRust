//! Cities. Each belongs to a country, stands somewhere on the map, and has a population.

use nworld_map::regions::MOUNTAIN_HEIGHT;
use nworld_map::world::{WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::ids::{CityId, CountryId};
use crate::text;

/// The longest a city's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;
/// The population of a new city.
pub const INITIAL_POPULATION: u32 = 100;
/// The most people a city can have.
// Used once cities grow, which nothing does yet.
#[cfg_attr(not(test), allow(dead_code))]
pub const MAX_POPULATION: u32 = 100_000;
/// The population at which a city stops growing in size. With more people than this it is the same
/// size, though the population goes on up to [`MAX_POPULATION`].
pub const FULL_SIZE_POPULATION: u32 = 50_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct City {
    /// Tells it apart from the other cities, and stays with it however it changes.
    pub id: CityId,
    /// The id of the country it belongs to.
    pub country: CountryId,
    pub name: String,
    /// Where it stands on the map, in meters.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in, which has to belong to its country. `None` on a map that
    /// has no regions.
    #[serde(default)]
    pub region: Option<u16>,
    pub population: u32,
}

impl City {
    /// How big the city is, from 0 for the smallest to 1 for the largest. Sizes go by the number of
    /// times the population has doubled, so each step up looks as big a change as the last: a
    /// village of 100 and a town of 1,000 differ as much as a town and a city of 10,000. A city of
    /// [`FULL_SIZE_POPULATION`] is as big as it gets, and one with more people is the same size.
    pub fn size(&self) -> f32 {
        size_of(self.population)
    }
}

/// A city name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your city")
}

/// The size, from 0 to 1, of a city of `population` people. See [`City::size`].
pub fn size_of(population: u32) -> f32 {
    let steps = (FULL_SIZE_POPULATION as f32 / INITIAL_POPULATION as f32).ln();
    let population = population.clamp(INITIAL_POPULATION, FULL_SIZE_POPULATION) as f32;
    ((population / INITIAL_POPULATION as f32).ln() / steps).clamp(0.0, 1.0)
}

/// Why a city can't be built at (`x`, `z`), or `None` if it can: it needs dry land that isn't up
/// in the mountains, and on the map.
pub fn site_problem(world: &World, x: f32, z: f32) -> Option<&'static str> {
    let half = world.half_extents();
    if x.abs() > half.x || z.abs() > half.y {
        Some("That is off the map.")
    } else if world.water_class(x, z) != WaterClass::Dry {
        Some("Cities can only be built on dry land.")
    } else if world.surface_height(x, z) >= MOUNTAIN_HEIGHT {
        Some("That is too high in the mountains to build a city.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn size_goes_from_nothing_to_one_in_even_steps() {
        assert_eq!(size_of(INITIAL_POPULATION), 0.0);
        assert_eq!(size_of(FULL_SIZE_POPULATION), 1.0);
        // Each tenfold is the same step, whatever the population it starts from.
        let tenfold = (10.0f32).ln() / (FULL_SIZE_POPULATION as f32 / INITIAL_POPULATION as f32).ln();
        assert!((size_of(1_000) - tenfold).abs() < 1e-5);
        assert!((size_of(10_000) - 2.0 * tenfold).abs() < 1e-5);
    }

    #[test]
    fn size_stays_within_its_range() {
        assert_eq!(size_of(0), 0.0);
        assert_eq!(size_of(MAX_POPULATION * 10), 1.0);
    }

    #[test]
    fn a_city_stops_growing_in_size_at_50_000() {
        assert!(size_of(49_000) < 1.0);
        for population in [50_000, 60_000, 75_000, MAX_POPULATION] {
            assert_eq!(size_of(population), 1.0, "{population}");
        }
    }

    #[test]
    fn a_city_name_is_tidied_and_must_be_there() {
        assert_eq!(clean_name("  Port   Aldor ").unwrap(), "Port Aldor");
        assert_eq!(clean_name("").unwrap_err(), "Enter a name for your city.");
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn a_city_is_built_on_dry_land_only() {
        let world = test_support::small_continent();
        let (land, sea) = test_support::land_and_sea(&world);
        assert_eq!(site_problem(&world, land.x, land.y), None);
        assert!(site_problem(&world, sea.x, sea.y).unwrap().contains("dry land"));
    }

    #[test]
    fn a_city_is_built_on_the_map() {
        let world = test_support::small_continent();
        let half = world.half_extents();
        assert!(site_problem(&world, half.x + 50.0, 0.0).unwrap().contains("off the map"));
    }
}
