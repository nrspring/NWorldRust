//! A circle over each city, in its country's color, with the number of its people and its name in it, which the map shows while the player holds
//! the mouse over "Population" in the footer. The circles are sized against each other: the most populous city has the biggest, and
//! the others have less area the fewer people they have, though none is too small for its number to be read.

use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{Style, Weight};

use crate::city::City;
use crate::day::DaySave;
use crate::format::thousands;
use crate::region_labels;

/// The radius of the circle of a city with next to nobody, which is room for the longest number, and of the most populous, in pixels.
const LEAST_RADIUS: f32 = 26.0;
const GREATEST_RADIUS: f32 = 72.0;
const TEXT_SIZE: u16 = 13;
const NAME_SIZE: u16 = 11;
/// How far above the middle of its circle the number is, and below it the name, in pixels.
const LINE_APART: f32 = 7.0;
/// A country that can't be found is shown in gray.
const NO_COUNTRY: Color = Color::new(0.6, 0.6, 0.6, 1.0);

/// The radius of the circle for a city of `population` people, where the most populous city has `largest`. The area of the circle,
/// over that of the least, goes as the number of people.
pub fn radius(population: u32, largest: u32) -> f32 {
    let share = (population as f32 / largest.max(1) as f32).clamp(0.0, 1.0);
    LEAST_RADIUS + (GREATEST_RADIUS - LEAST_RADIUS) * share.sqrt()
}

/// The cities of `day` with the radius of the circle of each, the biggest first, so that a smaller one is drawn over a bigger.
fn circles(day: &DaySave) -> Vec<(&City, f32)> {
    let largest = day.cities.iter().map(|c| c.population).max().unwrap_or(0);
    let mut circles: Vec<(&City, f32)> = day.cities.iter().map(|c| (c, radius(c.population, largest))).collect();
    circles.sort_by(|a, b| b.1.total_cmp(&a.1));
    circles
}

/// Draws the circles over the cities of `day` as `camera` sees them, on the screen, so call it with the camera for the interface set.
pub fn draw(style: &Style, day: &DaySave, world: &World, camera: &Camera3D) {
    for (city, radius) in circles(day) {
        let point = vec3(city.x, world.surface_height(city.x, city.z), city.z);
        let Some(at) = region_labels::to_screen(camera, point) else { continue };
        if at.x < -radius || at.x > screen_width() + radius || at.y < -radius || at.y > screen_height() + radius {
            continue;
        }
        let color = day.country(city.country).map_or(NO_COUNTRY, |c| c.color.color());
        draw_circle(at.x, at.y, radius, Color::new(color.r, color.g, color.b, 0.72));
        draw_circle_lines(at.x, at.y, radius, 2.0, WHITE);
        // The number, with the name of the city under it.
        line(style, &thousands(city.population), at - vec2(0.0, LINE_APART), TEXT_SIZE, Weight::Semibold);
        line(style, &city.name, at + vec2(0.0, LINE_APART), NAME_SIZE, Weight::Regular);
    }
}

/// A line of text centered on `at`, in white with a dark edge all round, so that it can be read on a light color, and past the
/// circle, which a long name goes.
fn line(style: &Style, text: &str, at: Vec2, size: u16, weight: Weight) {
    let x = at.x - style.measure(text, size, weight).width * 0.5;
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (1.0, 1.0)] {
        style.text_mid(text, x + dx, at.y + dy, size, weight, Color::new(0.0, 0.0, 0.0, 0.6));
    }
    style.text_mid(text, x, at.y, size, weight, WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    #[test]
    fn the_most_populous_city_has_the_biggest_circle_and_area_goes_with_people() {
        assert_eq!(radius(50_000, 50_000), GREATEST_RADIUS);
        assert_eq!(radius(0, 50_000), LEAST_RADIUS);
        // A quarter of the people: half of the way from the least radius to the greatest.
        assert_eq!(radius(2_500, 10_000), (LEAST_RADIUS + GREATEST_RADIUS) * 0.5);
        assert!(radius(100, 10_000) < radius(1_000, 10_000) && radius(1_000, 10_000) < radius(9_000, 10_000));
        // Sizes are against each other, so the same city is bigger among smaller ones.
        assert!(radius(1_000, 2_000) > radius(1_000, 100_000));
        assert_eq!(radius(0, 0), LEAST_RADIUS, "no people anywhere");
    }

    #[test]
    fn there_is_a_circle_for_every_city_of_every_country_the_biggest_first() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert!(circles(&day).is_empty());
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.select_country(day.countries[1].id).unwrap();
        day.found_city("Brevik", 900.0, 0.0, None).unwrap();
        day.cities[1].population = 4_000;
        let all = circles(&day);
        assert_eq!(all.iter().map(|(c, _)| c.name.as_str()).collect::<Vec<_>>(), vec!["Brevik", "Port Aldor"]);
        assert_eq!(all[0].1, GREATEST_RADIUS);
        assert!(all[1].1 < all[0].1);
    }
}
