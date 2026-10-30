//! Where a city's buildings stand and what they are like, worked out from the city's population and
//! the ground it is on. Nothing here draws anything; [`crate::city_mesh`] turns it into shapes.
//!
//! A city grows outward from its middle, and never outside the region it was founded in. Building
//! sites are laid out on a tight, slightly turned grid, ranked from the middle outward with a little
//! noise so the edge is ragged, and a city of more people takes more of them from the front of that
//! ranking. So a city is always packed close, however small, and the buildings a small city has are
//! still there, where they were, when it is bigger.
//!
//! The look goes from a medieval village to a European city of the 1800s as the city grows. The
//! edges, and all of a small place, are timber-framed houses with steep thatched or tiled roofs.
//! Further in come townhouses, and in the middle of a big city, tall brick and stone blocks with
//! hipped slate roofs and chimneys. There are churches, with a tower and a spire, among the houses,
//! which keep clear of their walls.

use std::f32::consts::TAU;

use macroquad::math::{vec2, Vec2};
use nworld_map::regions::{RegionId, MOUNTAIN_HEIGHT};
use nworld_map::world::{WaterClass, World};

use crate::city::City;

/// The distance between building sites, in meters.
const SPACING: f32 = 5.5;
/// How far from the middle of a city its buildings can stand, in meters: room for the biggest.
const REACH: f32 = 80.0;
/// How many building sites the smallest and the biggest city have.
const MIN_BUILDINGS: f32 = 6.0;
const MAX_BUILDINGS: f32 = 450.0;
/// The ground a house needs, as the corners of a rectangle around its middle in its own across and
/// along directions, in meters, and a church's, which runs along its length and has its tower at the
/// far end. Sites whose ground there is wet, high, steep or in another region are left empty.
const HOUSE_GROUND: (Vec2, Vec2) = (Vec2::new(-3.4, -3.4), Vec2::new(3.4, 3.4));
const CHURCH_GROUND: (Vec2, Vec2) = (Vec2::new(-3.4, -7.0), Vec2::new(3.4, 10.0));
/// How far a house's middle is kept from a church's walls, in meters. A house is at most about this
/// wide on each side of its middle.
const HOUSE_CLEARANCE: f32 = 3.4;
/// The most the ground may rise or fall across a building's reach, in meters.
const MAX_STEP: f32 = 2.0;
/// How far walls go below the lowest ground under them, so they never float, in meters.
const FOUNDATION: f32 = 0.4;
/// Where the churches stand, as how far from the middle of the city, as a share of [`REACH`], and
/// how many buildings the city needs for it to have a church there. The first is in the middle, the
/// others further out in a bigger city. Houses that would stand where a church is built are not built.
const CHURCHES: [(f32, usize); 3] = [(0.0, 12), (0.3, 125), (0.5, 250)];
/// How many places around its ring a church is tried at.
const CHURCH_TRIES: usize = 8;

/// The shape of a roof, and how high it rises above the walls, in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Roof {
    Flat,
    /// Two slopes meeting in a ridge along the building's depth.
    Gable(f32),
    /// Four slopes: a ridge along the depth with the ends sloping too. A square building gets a
    /// point, for a spire.
    Hip(f32),
}

impl Roof {
    pub fn rise(self) -> f32 {
        match self {
            Roof::Flat => 0.0,
            Roof::Gable(rise) | Roof::Hip(rise) => rise,
        }
    }
}

/// How the walls look, which the shader uses to draw them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walls {
    /// Nothing drawn on them.
    Plain,
    /// Plaster or brick or stone, with rows of windows.
    Masonry,
    /// A timber frame, with small windows between the beams.
    Timber,
    /// Plain stone with tall narrow windows.
    Church,
}

impl Walls {
    /// The number the shader is given.
    pub fn flag(self) -> f32 {
        match self {
            Walls::Plain => 0.0,
            Walls::Masonry => 1.0,
            Walls::Timber => 2.0,
            Walls::Church => 3.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    /// The middle of the building, in meters on the map.
    pub x: f32,
    pub z: f32,
    pub width: f32,
    pub depth: f32,
    /// The way it is turned, in radians.
    pub yaw: f32,
    /// The height of the bottom of its walls, which stands in the ground, and of the top of them.
    pub base: f32,
    pub top: f32,
    pub roof: Roof,
    pub walls: Walls,
    pub wall: [u8; 3],
    pub roof_color: [u8; 3],
    /// How many chimneys stand on the roof.
    pub chimneys: u8,
}

impl Building {
    /// How high the building reaches in all, above where its walls start.
    #[cfg(test)]
    pub fn height(&self) -> f32 {
        self.top - self.base + self.roof.rise()
    }
}

/// A number from 0 to 1 that depends only on its arguments.
fn hash(seed: u32, a: i32, b: i32, k: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E37_79B1) ^ (a as u32).wrapping_mul(0x85EB_CA6B) ^ (b as u32).wrapping_mul(0xC2B2_AE35) ^ k.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// One of `palette`, chosen by `r` from 0 to 1.
fn pick<T: Copy>(palette: &[T], r: f32) -> T {
    palette[((r * palette.len() as f32) as usize).min(palette.len() - 1)]
}

/// How many building sites a city has when it is `size` big, from 0 to 1.
fn building_count(size: f32) -> usize {
    (MIN_BUILDINGS + (MAX_BUILDINGS - MIN_BUILDINGS) * size.powf(1.5)).round() as usize
}

const PLASTER: [[u8; 3]; 4] = [[228, 216, 188], [218, 204, 172], [236, 228, 208], [222, 196, 160]];
const STONE: [[u8; 3]; 3] = [[206, 198, 180], [190, 184, 170], [214, 204, 182]];
const BRICK: [[u8; 3]; 3] = [[168, 96, 76], [154, 88, 70], [178, 110, 84]];
const THATCH: [[u8; 3]; 2] = [[170, 138, 76], [156, 126, 70]];
const TILE: [[u8; 3]; 3] = [[178, 88, 60], [160, 76, 54], [190, 104, 70]];
const SLATE: [[u8; 3]; 2] = [[92, 98, 110], [82, 88, 100]];

/// The buildings of `city` on the ground of `world`, middle first. A church is two buildings, its
/// nave and its tower.
pub fn buildings(city: &City, world: &World) -> Vec<Building> {
    let wanted = building_count(city.size());
    let seed = city.id.seed().wrapping_add(1);
    let grid_turn = hash(seed, 0, 0, 0) * TAU;
    let centre = vec2(city.x, city.z);
    // The city stays in the region it was founded in. A map with no regions has no such limit.
    let region = world.region_at(city.x, city.z);

    // Every site within reach, ranked from the middle outward with a little noise.
    let cells = (REACH / SPACING).ceil() as i32 + 1;
    let mut sites: Vec<(f32, Vec2)> = Vec::new();
    for gz in -cells..=cells {
        for gx in -cells..=cells {
            let jitter = vec2(hash(seed, gx, gz, 1) - 0.5, hash(seed, gx, gz, 2) - 0.5) * 0.2;
            let offset = (vec2(gx as f32, gz as f32) + jitter) * SPACING;
            let distance = offset.length();
            if distance <= REACH {
                sites.push((distance + hash(seed, gx, gz, 3) * SPACING * 0.35, offset));
            }
        }
    }
    sites.sort_by(|a, b| a.0.total_cmp(&b.0));

    // The churches, which stand among the houses. A church needs a clear stretch of ground, so each is
    // tried at a few places around its ring, and if none will do, there is no church there.
    let mut churches: Vec<Building> = Vec::new();
    for (n, &(fraction, needed)) in CHURCHES.iter().enumerate() {
        if wanted < needed {
            continue;
        }
        let first_angle = hash(seed, n as i32, 0, 20);
        let tries = if fraction == 0.0 { 1 } else { CHURCH_TRIES };
        let found = (0..tries).find_map(|attempt| {
            let angle = (first_angle + attempt as f32 / CHURCH_TRIES as f32) * TAU;
            let at = centre + rotate(vec2(fraction * REACH, 0.0), angle);
            clear_ground(world, at, grid_turn, CHURCH_GROUND, region).map(|ground| (at, ground))
        });
        if let Some((at, (low, high))) = found {
            churches.extend(church(at, grid_turn, low, high, city.size()));
        }
    }

    let mut placed = churches.clone();
    let mut used = 0;
    for (k, (_, offset)) in sites.iter().enumerate() {
        if used == wanted {
            break;
        }
        let at = centre + rotate(*offset, grid_turn);
        if churches.iter().any(|church| touches(church, at)) {
            continue;
        }
        // Houses are turned a little this way and that.
        let yaw = grid_turn + (hash(seed, k as i32, 0, 4) - 0.5) * 0.25;
        let Some((low, high)) = clear_ground(world, at, yaw, HOUSE_GROUND, region) else { continue };

        // How far out from the middle of the city this is, from 0 to 1, and so how grand.
        let rank = used as f32 / wanted as f32;
        let grandeur = (city.size() * (1.0 - rank).powf(0.7) + 0.06 * hash(seed, k as i32, 1, 12)).clamp(0.0, 1.0);
        placed.push(house(seed, k, at, yaw, low, high, grandeur));
        used += 1;
    }
    placed
}

/// Whether a house with its middle at `at` would be too close to the walls of `church`.
fn touches(church: &Building, at: Vec2) -> bool {
    // The house's place in the church's own across and along directions.
    let local = rotate(at - vec2(church.x, church.z), -church.yaw);
    local.x.abs() < church.width * 0.5 + HOUSE_CLEARANCE && local.y.abs() < church.depth * 0.5 + HOUSE_CLEARANCE
}

/// A house, townhouse or block at `at`, whichever fits how grand the place is, from 0 to 1.
fn house(seed: u32, k: usize, at: Vec2, yaw: f32, low: f32, high: f32, grandeur: f32) -> Building {
    let r = |n: u32| hash(seed, k as i32, 1, n);
    let (width, depth, walls_high, roof, walls, wall, roof_color, chimneys) = if grandeur < 0.14 {
        // A timber-framed house with a steep roof, of thatch or tile.
        (
            2.7 + 1.2 * r(5),
            3.2 + 1.5 * r(6),
            (1.0 + (r(7) > 0.5) as u8 as f32) * 1.8,
            Roof::Gable(1.75 + 0.8 * r(8)),
            Walls::Timber,
            pick(&PLASTER, r(9)),
            if r(10) < 0.5 { pick(&THATCH, r(13)) } else { pick(&TILE, r(13)) },
            (r(14) > 0.5) as u8,
        )
    } else if grandeur < 0.38 {
        // A townhouse of two or three storeys, timber-framed or plastered stone, with a tiled roof.
        let timber = r(15) < 0.55;
        (
            3.5 + 1.35 * r(5),
            4.0 + 1.6 * r(6),
            (2.0 + (r(7) > 0.4) as u8 as f32) * 1.9,
            Roof::Gable(1.75 + 0.8 * r(8)),
            if timber { Walls::Timber } else { Walls::Masonry },
            if timber { pick(&PLASTER, r(9)) } else { pick(&STONE, r(9)) },
            if r(10) < 0.75 { pick(&TILE, r(13)) } else { pick(&SLATE, r(13)) },
            1 + (r(14) > 0.5) as u8,
        )
    } else {
        // A block of four to six storeys in brick or stone, with a hipped slate roof and chimneys.
        (
            4.6 + 1.6 * r(5),
            5.1 + 1.6 * r(6),
            (4.0 + (2.0 * grandeur + r(7)).floor().min(2.0)) * 1.95,
            Roof::Hip(1.75),
            Walls::Masonry,
            if r(9) < 0.5 { pick(&BRICK, r(13)) } else { pick(&STONE, r(13)) },
            pick(&SLATE, r(10)),
            2,
        )
    };
    Building { x: at.x, z: at.y, width, depth, yaw, base: low - FOUNDATION, top: high + walls_high, roof, walls, wall, roof_color, chimneys }
}

/// A church at `at`: a nave with a gabled roof, and a tower at one end of it with a spire. Both are
/// bigger in a bigger city.
fn church(at: Vec2, yaw: f32, low: f32, high: f32, size: f32) -> [Building; 2] {
    let (width, depth) = (4.4 + 1.3 * size, 10.0 + 3.4 * size);
    let tower = 2.8 + 0.55 * size;
    // The tower stands at one end of the nave, along the way the nave runs.
    let along = vec2(-yaw.sin(), yaw.cos());
    let tower_at = at + along * (depth * 0.5 + tower * 0.5 - 0.55);
    let nave = Building {
        x: at.x,
        z: at.y,
        width,
        depth,
        yaw,
        base: low - FOUNDATION,
        top: high + 3.4 + 1.0 * size,
        roof: Roof::Gable(3.0 + 1.35 * size),
        walls: Walls::Church,
        wall: STONE[0],
        roof_color: SLATE[1],
        chimneys: 0,
    };
    let spire = Building {
        x: tower_at.x,
        z: tower_at.y,
        width: tower,
        depth: tower,
        yaw,
        base: low - FOUNDATION,
        top: high + 6.7 + 7.4 * size,
        roof: Roof::Hip(4.4 + 8.0 * size),
        walls: Walls::Church,
        wall: STONE[1],
        roof_color: SLATE[0],
        chimneys: 0,
    };
    [nave, spire]
}

/// The lowest and the highest ground in the rectangle with the corners `ground`, which are measured
/// across and along from `at` in the direction `yaw`, if a building can stand there: it is all dry,
/// below the mountains, not too steep, and in `region`. `None` if it can't. The rectangle is looked
/// at every few meters, so no stream or boundary slips between the points.
fn clear_ground(world: &World, at: Vec2, yaw: f32, ground: (Vec2, Vec2), region: Option<RegionId>) -> Option<(f32, f32)> {
    let (min, max) = ground;
    // Every two meters or so, which is finer than the 4 m cells the map keeps its regions and water in.
    let steps = (((max - min) / 2.0).ceil().max(Vec2::ONE)).as_ivec2();
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for iz in 0..=steps.y {
        for ix in 0..=steps.x {
            let local = min + (max - min) * vec2(ix as f32 / steps.x as f32, iz as f32 / steps.y as f32);
            let p = at + rotate(local, yaw);
            let height = world.surface_height(p.x, p.y);
            if world.water_class(p.x, p.y) != WaterClass::Dry || height >= MOUNTAIN_HEIGHT || world.region_at(p.x, p.y) != region {
                return None;
            }
            low = low.min(height);
            high = high.max(height);
        }
    }
    (high - low <= MAX_STEP).then_some((low, high))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::{FULL_SIZE_POPULATION, INITIAL_POPULATION, MAX_POPULATION};
    use crate::ids::{CityId, CountryId};
    use crate::test_support;

    fn city_at(at: Vec2, population: u32) -> City {
        City { id: CityId::from_u128(7), country: CountryId::from_u128(1), name: "Test".to_owned(), x: at.x, z: at.y, region: None, population }
    }

    /// The middle of the map's one continent, where there is room for the biggest city. The map has
    /// no regions, so nothing but the ground limits a city.
    fn setting() -> (World, Vec2) {
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, REACH + 30.0);
        (world, at)
    }

    fn church_count(buildings: &[Building]) -> usize {
        buildings.iter().filter(|b| b.walls == Walls::Church && b.depth > 8.0).count()
    }

    #[test]
    fn the_same_city_always_has_the_same_buildings() {
        let (world, at) = setting();
        let city = city_at(at, 5_000);
        assert_eq!(buildings(&city, &world), buildings(&city, &world));
    }

    #[test]
    fn a_new_city_is_a_handful_of_buildings_and_the_biggest_has_hundreds() {
        let (world, at) = setting();
        let small = buildings(&city_at(at, INITIAL_POPULATION), &world);
        let big = buildings(&city_at(at, MAX_POPULATION), &world);
        assert!(small.len() >= 5 && small.len() <= 8, "{} buildings", small.len());
        assert!(big.len() >= 420, "{} buildings", big.len());
    }

    #[test]
    fn a_city_with_more_than_50_000_people_is_no_bigger() {
        let (world, at) = setting();
        let full = buildings(&city_at(at, FULL_SIZE_POPULATION), &world);
        assert_eq!(buildings(&city_at(at, 75_000), &world), full);
        assert_eq!(buildings(&city_at(at, MAX_POPULATION), &world), full);
        // Just below it, the city is still growing.
        assert!(buildings(&city_at(at, 30_000), &world).len() < full.len());
    }

    #[test]
    fn more_people_means_more_buildings() {
        let (world, at) = setting();
        let counts: Vec<usize> = [100, 300, 1_000, 3_000, 10_000, 30_000, 50_000].iter().map(|&p| buildings(&city_at(at, p), &world).len()).collect();
        assert!(counts.windows(2).all(|w| w[0] < w[1]), "{counts:?}");
    }

    #[test]
    fn a_city_that_grows_keeps_its_houses_where_they_were() {
        let (world, at) = setting();
        let spot = |b: &Building| (b.x.to_bits(), b.z.to_bits());
        let before = buildings(&city_at(at, 1_000), &world);
        let big = buildings(&city_at(at, 10_000), &world);
        let after: Vec<_> = big.iter().map(spot).collect();
        let churches: Vec<&Building> = big.iter().filter(|b| b.walls == Walls::Church).collect();
        for b in before.iter().filter(|b| b.walls != Walls::Church) {
            // A house is only lost where the city has built a church.
            let lost_to_a_church = churches.iter().any(|church| touches(church, vec2(b.x, b.z)));
            assert!(after.contains(&spot(b)) || lost_to_a_church, "{b:?} is gone");
        }
    }

    #[test]
    fn a_bigger_city_covers_more_ground() {
        let (world, at) = setting();
        let reach = |population| buildings(&city_at(at, population), &world).iter().map(|b| vec2(b.x, b.z).distance(at)).fold(0.0, f32::max);
        let (small, medium, big) = (reach(100), reach(3_000), reach(100_000));
        assert!(small < 20.0, "{small}");
        assert!(small < medium && medium < big);
        assert!(big <= REACH + SPACING + 20.0, "{big}");
        // The biggest city is about 130 m across.
        assert!(big > 55.0 && big < 75.0, "{big}");
    }

    #[test]
    fn a_city_is_packed_tight_however_small() {
        let (world, at) = setting();
        for population in [100, 300, 1_000, 3_000, 10_000, 100_000] {
            let city = buildings(&city_at(at, population), &world);
            let houses: Vec<Vec2> = city.iter().filter(|b| b.walls != Walls::Church).map(|b| vec2(b.x, b.z)).collect();
            // Every house has a neighbour close by, so none is out on its own.
            for (i, h) in houses.iter().enumerate() {
                let nearest = houses.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, o)| h.distance(*o)).fold(f32::MAX, f32::min);
                assert!(nearest <= SPACING * 1.6, "a house of the city of {population} is {nearest} m from the next");
            }
            // And the sites it uses fill most of the disc they are in, with no big empty places.
            let widest = houses.iter().map(|h| h.distance(at)).fold(0.0, f32::max);
            let filled = houses.len() as f32 * SPACING * SPACING / (std::f32::consts::PI * (widest + SPACING * 0.5).powi(2));
            assert!(filled > 0.45, "the city of {population} only fills {filled} of its disc");
        }
    }

    #[test]
    fn the_biggest_buildings_are_church_spires_and_none_is_a_skyscraper() {
        let (world, at) = setting();
        let tallest = |population| buildings(&city_at(at, population), &world).iter().map(Building::height).fold(0.0, f32::max);
        let (small, medium, big) = (tallest(100), tallest(3_000), tallest(100_000));
        assert!(small < 11.0, "a village's tallest is {small}");
        assert!(small < medium && medium < big, "{small} {medium} {big}");
        assert!(big > 24.0 && big < 40.0, "{big}");
        for b in buildings(&city_at(at, 100_000), &world) {
            if b.walls != Walls::Church {
                assert!(b.height() < 18.0, "{b:?} is {} m", b.height());
            }
        }
    }

    #[test]
    fn a_village_is_timber_houses_and_a_big_city_has_blocks_inside_a_ring_of_them() {
        let (world, at) = setting();
        let village = buildings(&city_at(at, 100), &world);
        assert!(village.iter().all(|b| b.walls == Walls::Timber && b.roof.rise() > 1.5), "{village:?}");

        let city = buildings(&city_at(at, 100_000), &world);
        let blocks: Vec<_> = city.iter().filter(|b| matches!(b.roof, Roof::Hip(_)) && b.walls == Walls::Masonry).collect();
        assert!(blocks.len() > 150, "{} blocks", blocks.len());
        let distance = |b: &Building| vec2(b.x, b.z).distance(at);
        let farthest_block = blocks.iter().map(|b| distance(b)).fold(0.0, f32::max);
        let farthest_house = city.iter().filter(|b| b.walls == Walls::Timber).map(distance).fold(0.0, f32::max);
        assert!(farthest_block < farthest_house, "{farthest_block} {farthest_house}");
    }

    #[test]
    fn churches_come_with_size_and_houses_keep_clear_of_them() {
        let (world, at) = setting();
        assert_eq!(church_count(&buildings(&city_at(at, 100), &world)), 0);
        assert_eq!(church_count(&buildings(&city_at(at, 500), &world)), 1);
        assert_eq!(church_count(&buildings(&city_at(at, 100_000), &world)), 3);

        let city = buildings(&city_at(at, 100_000), &world);
        let churches: Vec<&Building> = city.iter().filter(|b| b.walls == Walls::Church).collect();
        for house in city.iter().filter(|b| b.walls != Walls::Church) {
            // No corner of a house is inside a church.
            let half = vec2(house.width, house.depth) * 0.5;
            for corner in [half, -half, vec2(half.x, -half.y), vec2(-half.x, half.y)] {
                let p = vec2(house.x, house.z) + rotate(corner, house.yaw);
                for church in &churches {
                    let local = rotate(p - vec2(church.x, church.z), -church.yaw);
                    assert!(local.x.abs() >= church.width * 0.5 || local.y.abs() >= church.depth * 0.5, "{house:?} is inside {church:?}");
                }
            }
        }
    }

    #[test]
    fn buildings_stand_on_dry_ground_with_walls_that_reach_it() {
        let (world, at) = setting();
        for building in buildings(&city_at(at, 30_000), &world) {
            assert_eq!(world.water_class(building.x, building.z), WaterClass::Dry);
            let ground = world.surface_height(building.x, building.z);
            assert!(building.base < ground && building.top > ground, "{building:?}");
            assert!(building.width <= 7.0 && building.depth <= 13.5);
        }
    }

    #[test]
    fn a_city_on_a_coast_leaves_the_sea_alone() {
        let world = test_support::small_continent();
        let (land, sea) = test_support::land_and_sea(&world);
        let at = (land + sea) * 0.5;
        for building in buildings(&city_at(at, 100_000), &world) {
            assert_eq!(world.water_class(building.x, building.z), WaterClass::Dry);
        }
    }

    #[test]
    fn a_city_stays_inside_its_region() {
        let world = test_support::regioned_continent();
        let free = test_support::small_continent();
        let mut held_back = 0;
        for at in test_support::land_spots(&world, 12) {
            let region = world.region_at(at.x, at.y);
            assert!(region.is_some());
            let city = city_at(at, MAX_POPULATION);
            let built = buildings(&city, &world);
            for b in &built {
                // The middle of the building and the corners of its walls are all in the city's region.
                let half = vec2(b.width, b.depth) * 0.5;
                for corner in [Vec2::ZERO, half, -half, vec2(half.x, -half.y), vec2(-half.x, half.y)] {
                    let p = vec2(b.x, b.z) + rotate(corner, b.yaw);
                    assert_eq!(world.region_at(p.x, p.y), region, "{b:?} spills out of its region");
                }
            }
            // The same city with no regions to hold it in has more room.
            if buildings(&city, &free).len() > built.len() {
                held_back += 1;
            }
        }
        assert!(held_back > 0, "no city was held back by its region");
    }

    #[test]
    fn a_full_region_stops_a_city_growing() {
        let world = test_support::regioned_continent();
        let at = test_support::land_spots(&world, 12)[0];
        let counts: Vec<usize> = [10_000, 30_000, 100_000].iter().map(|&p| buildings(&city_at(at, p), &world).len()).collect();
        // A bigger city fills the region more, though a church built in it takes the room of a few
        // houses, so it can be a little smaller in count.
        assert!(counts.windows(2).all(|w| w[1] as f32 >= w[0] as f32 * 0.95), "{counts:?}");
        // However big it is made, it fits in a region, which is a few hundred meters across.
        assert!(counts[2] < 440, "{counts:?}");
    }
}
