//! Finding the city under the mouse. Each city is taken as an upright cylinder around its buildings,
//! and the ray from the camera through the mouse is tested against it.

use macroquad::prelude::*;

use crate::city_layout::Building;

/// The space a city takes up: an upright cylinder, centred on the city, from the lowest ground
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
        Building { x, z, width: 4.0, depth: 4.0, yaw: 0.0, base: 9.0, top: 30.0, roof: Roof::Hip(5.0), walls: Walls::Masonry, wall: [0; 3], roof_color: [0; 3], chimneys: 0 }
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
