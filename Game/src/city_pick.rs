//! Finding the city under the mouse. Each building is taken as an upright cylinder, a little wider than it is so that the streets
//! between buildings count too, and the ray from the camera through the mouse is tested against them: a city is only where its
//! buildings are, which on a coast or at the edge of its region is not all round its middle. A cylinder round them all is tried
//! first, to pass over the cities the ray is nowhere near.

use macroquad::prelude::*;

use crate::city_layout::Building;

/// How far past its walls a building counts, in meters, which closes up the streets between neighbours.
const STREET: f32 = 1.5;

/// A space taken up: an upright cylinder. For a whole city it is centred on the city, from the lowest ground
/// under its buildings to the top of its highest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub centre: Vec2,
    pub radius: f32,
    pub low: f32,
    pub high: f32,
}

/// The bounds of a city whose middle is at `centre` and whose buildings are `buildings`, or `None` if it
/// has none.
pub fn bounds(centre: Vec2, buildings: &[Building]) -> Option<Bounds> {
    if buildings.is_empty() {
        return None;
    }
    let radius = buildings.iter().map(|b| vec2(b.x, b.z).distance(centre) + b.width.max(b.depth) * 0.5).fold(0.0, f32::max);
    let low = buildings.iter().map(|b| b.base).fold(f32::MAX, f32::min);
    let high = buildings.iter().map(|b| b.top + b.roof.rise()).fold(f32::MIN, f32::max);
    Some(Bounds { centre, radius, low, high })
}

/// The space each of `buildings` takes up, with the street round it.
pub fn each(buildings: &[Building]) -> Vec<Bounds> {
    buildings.iter().map(|b| Bounds { centre: vec2(b.x, b.z), radius: b.width.max(b.depth) * 0.5 + STREET, low: b.base, high: b.top + b.roof.rise() }).collect()
}

/// How far along the ray it first enters one of the `buildings` of a city whose whole space is `all`, each made `slop` meters bigger
/// all round, or `None` if it meets none of them.
pub fn hit_city(origin: Vec3, dir: Vec3, all: &Bounds, buildings: &[Bounds], slop: f32) -> Option<f32> {
    // (A building reaches as far past `all` as its street does.)
    hit(origin, dir, all, slop + STREET)?;
    buildings.iter().filter_map(|building| hit(origin, dir, building, slop)).min_by(f32::total_cmp)
}

/// How far along the ray from `origin` in the direction `dir` (a unit vector) it first enters `bounds`
/// made `slop` meters bigger all round, or `None` if it never does, or only behind `origin`.
pub fn hit(origin: Vec3, dir: Vec3, bounds: &Bounds, slop: f32) -> Option<f32> {
    let radius = bounds.radius + slop;
    let (low, high) = (bounds.low - slop, bounds.high + slop);

    // Where the ray is within `radius` of the cylinder's axis, as a range of distances along it.
    let flat_dir = vec2(dir.x, dir.z);
    let from_axis = vec2(origin.x, origin.z) - bounds.centre;
    let a = flat_dir.length_squared();
    let (mut near, mut far) = if a < 1e-9 {
        // Straight up or down: inside the radius all along it, or never.
        if from_axis.length_squared() > radius * radius {
            return None;
        }
        (f32::MIN, f32::MAX)
    } else {
        let b = 2.0 * from_axis.dot(flat_dir);
        let c = from_axis.length_squared() - radius * radius;
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let root = discriminant.sqrt();
        ((-b - root) / (2.0 * a), (-b + root) / (2.0 * a))
    };

    // Where it is between the bottom and the top, as another range.
    if dir.y.abs() < 1e-9 {
        if origin.y < low || origin.y > high {
            return None;
        }
    } else {
        let (t0, t1) = ((low - origin.y) / dir.y, (high - origin.y) / dir.y);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
    }

    // Only in front of the camera.
    near = near.max(0.0);
    (near <= far).then_some(near)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city_layout::{Roof, Walls};

    fn tower(x: f32, z: f32) -> Building {
        Building { x, z, width: 4.0, depth: 4.0, yaw: 0.0, base: 9.0, top: 30.0, roof: Roof::Hip(5.0), walls: Walls::Masonry, wall: [0; 3], roof_color: [0; 3], chimneys: 0, dormers: 0 }
    }

    /// A city 40 m across with its ground at 9 m and its top at 35 m, centred on the origin.
    fn city() -> Bounds {
        bounds(Vec2::ZERO, &[tower(0.0, 0.0), tower(18.0, 0.0), tower(-18.0, 0.0)]).unwrap()
    }

    #[test]
    fn the_bounds_cover_all_the_buildings() {
        let b = city();
        assert_eq!((b.low, b.high), (9.0, 35.0));
        // The farthest building is 18 m out, and its walls reach 2 m past its middle.
        assert!((b.radius - 20.0).abs() < 1e-4, "{}", b.radius);
        assert_eq!(bounds(Vec2::ZERO, &[]), None);
    }

    #[test]
    fn only_the_buildings_and_the_streets_round_them_are_the_city() {
        let buildings = [tower(0.0, 0.0), tower(18.0, 0.0), tower(24.0, 0.0)];
        let (all, each) = (bounds(Vec2::ZERO, &buildings).unwrap(), each(&buildings));
        let down = |x: f32, z: f32, slop: f32| hit_city(vec3(x, 200.0, z), vec3(0.0, -1.0, 0.0), &all, &each, slop);
        assert!(down(0.0, 0.0, 0.0).is_some() && down(18.0, 0.0, 0.0).is_some());
        // The street between two neighbours is part of it.
        assert!(down(21.0, 0.0, 0.0).is_some());
        // Open ground inside the circle that goes round them all is not: between the far ones, and on the side with none.
        assert!(hit(vec3(9.0, 200.0, 0.0), vec3(0.0, -1.0, 0.0), &all, 0.0).is_some());
        assert_eq!(down(9.0, 0.0, 0.0), None);
        assert_eq!(down(-20.0, 0.0, 0.0), None);
        assert_eq!(down(0.0, 20.0, 0.0), None);
        // The street round the farthest reaches a little past that circle, and counts.
        assert!(down(27.0, 0.0, 0.0).is_some());
        assert!(down(9.0, 0.0, 6.0).is_some(), "from far off, a near miss is a hit");
    }

    #[test]
    fn a_ray_from_above_through_the_city_hits_it() {
        let t = hit(vec3(5.0, 200.0, 5.0), vec3(0.0, -1.0, 0.0), &city(), 0.0).unwrap();
        // It enters at the top of the city, 165 m down.
        assert!((t - 165.0).abs() < 1e-3, "{t}");
    }

    #[test]
    fn a_slanting_ray_hits_the_side_of_the_city() {
        // From the side and slightly above, aimed at the middle.
        let origin = vec3(-100.0, 20.0, 0.0);
        let dir = (vec3(0.0, 20.0, 0.0) - origin).normalize();
        let t = hit(origin, dir, &city(), 0.0).unwrap();
        assert!((t - 80.0).abs() < 1e-3, "{t}");
    }

    #[test]
    fn a_ray_beside_the_city_misses() {
        assert_eq!(hit(vec3(60.0, 200.0, 0.0), vec3(0.0, -1.0, 0.0), &city(), 0.0), None);
        assert_eq!(hit(vec3(-100.0, 20.0, 50.0), vec3(1.0, 0.0, 0.0), &city(), 0.0), None);
    }

    #[test]
    fn a_ray_over_the_top_or_under_the_ground_misses() {
        // Level, but above the highest roof.
        assert_eq!(hit(vec3(-100.0, 60.0, 0.0), vec3(1.0, 0.0, 0.0), &city(), 0.0), None);
        // Level, but below the lowest ground.
        assert_eq!(hit(vec3(-100.0, 2.0, 0.0), vec3(1.0, 0.0, 0.0), &city(), 0.0), None);
        // Slanting up, so that it only reaches the city's height after passing it.
        let origin = vec3(-100.0, 0.0, 0.0);
        assert_eq!(hit(origin, vec3(1.0, 1.0, 0.0).normalize(), &city(), 0.0), None);
    }

    #[test]
    fn a_ray_pointing_away_from_the_city_misses() {
        assert_eq!(hit(vec3(0.0, 200.0, 0.0), vec3(0.0, 1.0, 0.0), &city(), 0.0), None);
        assert_eq!(hit(vec3(-100.0, 20.0, 0.0), vec3(-1.0, 0.0, 0.0), &city(), 0.0), None);
    }

    #[test]
    fn a_ray_that_starts_inside_hits_at_once() {
        assert_eq!(hit(vec3(0.0, 20.0, 0.0), vec3(1.0, 0.0, 0.0), &city(), 0.0), Some(0.0));
    }

    #[test]
    fn slop_makes_a_near_miss_a_hit() {
        let ray = (vec3(26.0, 200.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(hit(ray.0, ray.1, &city(), 0.0), None);
        assert!(hit(ray.0, ray.1, &city(), 8.0).is_some());
    }
}
