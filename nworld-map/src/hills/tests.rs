use super::*;
use crate::gen_common::LAND_HEIGHT;
use crate::world::DEEP_OCEAN;

/// A square island of lowland in the middle of open water.
fn island(cells: usize, margin: usize) -> Vec<f32> {
    let n = cells + 1;
    let mut heights = vec![DEEP_OCEAN; n * n];
    for j in margin..=cells - margin {
        for i in margin..=cells - margin {
            heights[j * n + i] = LAND_HEIGHT;
        }
    }
    heights
}

fn painted(radius: f32) -> (Vec<f32>, Vec<f32>, usize) {
    let cells = 160;
    let n = cells + 1;
    let start = island(cells, 10);
    let mut heights = start.clone();
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let mut stroke = HillStroke::begin(&heights, n, n, 7, false, Peak::Hills);
    // Sweep back and forth long enough for the hills to finish growing.
    for k in 0..300 {
        let x = if k % 2 == 0 { -40.0 } else { 40.0 };
        stroke.paint(&mut heights, half, vec2(x, 0.0), radius, 0.1);
    }
    stroke.finish(&mut heights);
    (start, heights, n)
}

#[test]
fn hills_reach_10_to_25_m_inland() {
    let (start, heights, n) = painted(200.0);
    let top = heights.iter().copied().fold(f32::MIN, f32::max);
    assert!(top <= MAX_HILL, "hills stay at or below 25 m, got {top}");
    let centre = (n / 2) * n + n / 2;
    assert!(heights[centre] >= MIN_HILL, "the middle of the brush is a hill");
    assert!(heights.iter().zip(&start).all(|(h, s)| h >= s), "painting never digs");
}

#[test]
fn hill_heights_span_the_whole_range_and_stop_at_50() {
    let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Hills);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for j in 0..300 {
        for i in 0..300 {
            let t = stroke.target(Peak::Hills, i as f32 * 10.0, j as f32 * 10.0);
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    assert!(lo >= MIN_HILL && lo < MIN_HILL + 1.0, "lowest hill target was {lo}");
    assert!(hi <= MAX_HILL && hi > MAX_HILL - 0.5, "tallest hill target was {hi}");
}

#[test]
fn hills_grow_slowly_while_the_brush_is_held() {
    let cells = 160;
    let n = cells + 1;
    let mut heights = island(cells, 10);
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let centre = (n / 2) * n + n / 2;
    let mut stroke = HillStroke::begin(&heights, n, n, 5, false, Peak::Hills);
    stroke.paint(&mut heights, half, vec2(0.0, 0.0), 100.0, 0.1);
    let first = heights[centre] - LAND_HEIGHT;
    assert!(first > 0.0 && first <= Peak::Hills.rate() * 0.1 + 1e-4, "one frame raised it by {first}");
    for _ in 0..100 {
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 100.0, 0.1);
    }
    assert!(heights[centre] >= MIN_HILL && heights[centre] <= MAX_HILL);
}

#[test]
fn hills_vary_and_leave_water_and_coast_alone() {
    let (start, heights, _) = painted(200.0);
    let hills: Vec<f32> = heights.iter().copied().filter(|&h| h >= MIN_HILL).collect();
    let lo = hills.iter().copied().fold(f32::MAX, f32::min);
    let hi = hills.iter().copied().fold(f32::MIN, f32::max);
    assert!(hi - lo > 2.0, "hill tops should differ in height, got {lo} to {hi}");
    for (h, s) in heights.iter().zip(&start) {
        if *s < 0.0 {
            assert_eq!(h, s, "water is untouched");
        }
    }
    // The first land samples next to the water keep their lowland height.
    let n = 161;
    assert_eq!(heights[80 * n + 10], LAND_HEIGHT);
}

#[test]
fn hill_edges_slope_instead_of_dropping_in_cliffs() {
    let (_, heights, n) = painted(200.0);
    let mut steepest: f32 = 0.0;
    for j in 0..n {
        for i in 0..n {
            let h = heights[j * n + i];
            if h < 0.0 {
                continue;
            }
            for (di, dj) in [(1, 0), (0, 1)] {
                if i + di < n && j + dj < n {
                    let other = heights[(j + dj) * n + i + di];
                    if other >= 0.0 {
                        steepest = steepest.max((h - other).abs() / CELL_SIZE);
                    }
                }
            }
        }
    }
    assert!(steepest < 0.7, "steepest slope was {steepest}");
}

#[test]
fn erasing_lowers_ground_slowly_down_to_the_minimum() {
    let (_, hilly, n) = painted(200.0);
    let half = Vec2::splat((n - 1) as f32 * CELL_SIZE * 0.5);
    let centre = (n / 2) * n + n / 2;
    let mut heights = hilly.clone();
    let mut stroke = HillStroke::begin(&heights, n, n, 7, true, Peak::Hills);
    assert!(stroke.erases());

    // One frame takes only a little off, even with the brush held still.
    stroke.paint(&mut heights, half, vec2(0.0, 0.0), 400.0, 0.1);
    let lowered = hilly[centre] - heights[centre];
    assert!(lowered > 0.0 && lowered <= Peak::Hills.erase_rate() * 0.1 + 1e-4, "lowered {lowered}");

    // Holding on keeps lowering it, but never below the minimum.
    for _ in 0..400 {
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 400.0, 0.1);
    }
    stroke.finish(&mut heights);
    let top = heights.iter().copied().fold(f32::MIN, f32::max);
    assert!(top <= LAND_HEIGHT + 0.01, "everything under the brush is down to {top}");
    for (idx, (h, before)) in heights.iter().zip(&hilly).enumerate() {
        assert!(h <= before, "erasing never raises ground");
        assert!(*h >= LAND_HEIGHT.min(*before), "erasing stops at the minimum, got {h} at {idx}");
        if *before < 0.0 {
            assert_eq!(h, before, "water is untouched at {idx}");
        }
    }
}

#[test]
fn repainting_never_lowers_hills() {
    let cells = 160;
    let n = cells + 1;
    let mut heights = island(cells, 10);
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let mut stroke = HillStroke::begin(&heights, n, n, 3, false, Peak::Hills);
    stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
    let once = heights.clone();
    stroke.paint(&mut heights, half, vec2(10.0, 0.0), 150.0, 0.1);
    stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
    let twice = heights.clone();
    assert!(once.iter().zip(&twice).all(|(a, b)| b >= a));
}

#[test]
fn mountains_climb_to_50_m_and_no_further() {
    let cells = 160;
    let n = cells + 1;
    let mut heights = island(cells, 10);
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let centre = (n / 2) * n + n / 2;
    let mut stroke = HillStroke::begin(&heights, n, n, 9, false, Peak::Mountains);
    stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
    let first = heights[centre] - LAND_HEIGHT;
    assert!(first > Peak::Hills.rate() * 0.1, "mountains grow faster than hills, got {first}");
    assert!(first <= Peak::Mountains.rate() * 0.1 + 1e-4);
    for _ in 0..600 {
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 150.0, 0.1);
    }
    stroke.finish(&mut heights);
    let top = heights.iter().copied().fold(f32::MIN, f32::max);
    assert!(top <= MAX_MOUNTAIN, "mountains stop at 50 m, got {top}");
    assert!(heights[centre] >= MIN_MOUNTAIN, "the middle is a mountain, got {}", heights[centre]);
    for (h, s) in heights.iter().zip(island(cells, 10)) {
        if s < 0.0 {
            assert_eq!(*h, s, "water is untouched");
        }
    }
}

#[test]
fn a_mountain_range_is_a_long_chain_of_mountains_on_land() {
    let cells = 320;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let start = island(cells, 30);
    let range = mountain_range(&start, n, n, half, 5, 600.0, 100.0).expect("there is room for a range");
    assert!(range.iter().all(|&h| h <= MAX_MOUNTAIN + 0.01), "ranges stop at the top of the range");
    assert!(range.iter().zip(&start).all(|(&r, &s)| r >= s), "a range only raises ground");
    for (idx, &h) in start.iter().enumerate() {
        if h < 0.0 {
            assert_eq!(range[idx], h, "water is left alone");
        }
    }
    let peaks: Vec<Vec2> = (0..n * n)
        .filter(|&idx| range[idx] >= MIN_MOUNTAIN)
        .map(|idx| vec2((idx % n) as f32 * CELL_SIZE, (idx / n) as f32 * CELL_SIZE))
        .collect();
    assert!(peaks.len() > 100, "there should be mountains, got {} samples", peaks.len());
    let reach = peaks.iter().flat_map(|a| peaks.iter().map(move |b| (*a - *b).length())).fold(0.0, f32::max);
    assert!(reach > 300.0, "the mountains should run along a line, spanning {reach} m");
}

#[test]
fn mountain_ranges_vary_with_the_seed_and_repeat_with_the_same_one() {
    let cells = 320;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let start = island(cells, 30);
    let a = mountain_range(&start, n, n, half, 5, 600.0, 100.0);
    assert_eq!(a, mountain_range(&start, n, n, half, 5, 600.0, 100.0));
    assert_ne!(a, mountain_range(&start, n, n, half, 6, 600.0, 100.0));
}

#[test]
fn no_mountain_range_fits_in_open_water_or_on_a_sliver_of_land() {
    let cells = 160;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    assert!(mountain_range(&vec![DEEP_OCEAN; n * n], n, n, half, 1, 600.0, 100.0).is_none());
    assert!(mountain_range(&island(cells, 70), n, n, half, 1, 600.0, 100.0).is_none());
}

/// Digs a lake of `radius` at the middle of a 160-cell island of `ground` meters and returns
/// the map as it began, as it ended, its water levels and its width.
fn lake_on(ground: f32, radius: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>, usize) {
    let cells = 160;
    let n = cells + 1;
    let mut start = island(cells, 10);
    start.iter_mut().filter(|h| **h > 0.0).for_each(|h| *h = ground);
    let (heights, water) = dig_lake(&start, &vec![0.0; n * n], vec2(0.0, 0.0), radius);
    (start, heights, water, n)
}

/// Holds the lakes brush of `radius` meters at `at` on a 160-cell map, long enough to dig a
/// lake, and returns the new heights and water levels.
fn dig_lake(heights: &[f32], water: &[f32], at: Vec2, radius: f32) -> (Vec<f32>, Vec<f32>) {
    let n = 161;
    let half = Vec2::splat(160.0 * CELL_SIZE * 0.5);
    let mut heights = heights.to_vec();
    let mut stroke = HillStroke::begin_on(&heights, water, n, n, 3, false, Peak::Lakes);
    for _ in 0..200 {
        stroke.paint(&mut heights, half, at, radius, 0.1);
    }
    stroke.finish(&mut heights);
    (heights, stroke.water().to_vec())
}

/// An island whose ground climbs `slope` meters per meter eastward from `ground` at the middle.
fn tilted_island(ground: f32, slope: f32) -> Vec<f32> {
    let mut heights = island(160, 10);
    for (idx, h) in heights.iter_mut().enumerate() {
        if *h > 0.0 {
            *h = ground + slope * ((idx % 161) as f32 * CELL_SIZE - 320.0);
        }
    }
    heights
}

/// The x of the centers of the samples under water in a map, in meters.
fn flooded_xs(heights: &[f32], water: &[f32]) -> Vec<f32> {
    (0..heights.len())
        .filter(|&idx| heights[idx] < water[idx] && water[idx] > 0.0)
        .map(|idx| (idx % 161) as f32 * CELL_SIZE - 320.0)
        .collect()
}

#[test]
fn a_lake_is_a_pool_at_the_level_of_the_ground_with_gentle_banks() {
    for ground in [3.0, 12.0, 30.0] {
        let (start, heights, water, n) = lake_on(ground, 40.0);
        let centre = (n / 2) * n + n / 2;
        assert_eq!(water[centre], ground, "the lake lies at the level of the flat ground around it");
        assert!(heights[centre] < ground - 1.0, "the middle is under water, got {}", heights[centre]);
        let pool = (0..heights.len()).filter(|&i| heights[i] < water[i] && water[i] > 0.0).count();
        assert!(pool > 20, "a lake of {pool} samples is too small");
        assert!(heights.iter().zip(&start).all(|(h, s)| h <= s), "lakes only lower ground");
        assert!(
            (0..heights.len()).all(|i| water[i] == 0.0 || water[i] == ground),
            "all of the water stands at the one level"
        );
        // Banks ease down: nothing near a cliff between neighbors.
        let mut steepest: f32 = 0.0;
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                // The island's own coast, where it drops into the deep sea, isn't a bank.
                let h = heights[j * n + i];
                for other in [heights[j * n + i + 1], heights[(j + 1) * n + i]] {
                    if h >= -30.0 && other >= -30.0 {
                        steepest = steepest.max((h - other).abs() / CELL_SIZE);
                    }
                }
            }
        }
        assert!(steepest < 1.0, "banks over {ground} m ground are too steep: {steepest}");
    }
}

#[test]
fn lakes_stay_clear_of_the_sea_and_only_the_lakes_brush_digs_them() {
    let cells = 160;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let start = island(cells, 10);
    // Right at the shore.
    let mut heights = start.clone();
    let mut stroke = HillStroke::begin(&heights, n, n, 3, false, Peak::Lakes);
    for _ in 0..100 {
        stroke.paint(&mut heights, half, vec2(-half.x + 10.0 * CELL_SIZE, 0.0), 40.0, 0.1);
    }
    for (idx, &h) in heights.iter().enumerate() {
        let (i, j) = (idx % n, idx / n);
        let near_coast = i < 10 + 6 || j < 10 + 6 || i > cells - 16 || j > cells - 16;
        if near_coast && start[idx] >= 0.0 {
            assert!(h >= 0.0, "the sea shouldn't flood in at sample {i},{j}: {h}");
        }
    }
    // Erasing with the lakes brush changes nothing.
    let mut erased = start.clone();
    let mut stroke = HillStroke::begin(&erased, n, n, 3, true, Peak::Lakes);
    stroke.paint(&mut erased, half, vec2(0.0, 0.0), 40.0, 1.0);
    assert_eq!(erased, start);
}

#[test]
fn random_lakes_land_on_dry_ground_away_from_the_sea() {
    let cells = 320;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let start = island(cells, 30);
    let dry = vec![0.0; n * n];
    let (heights, water, added) = scatter_lakes(&start, &dry, n, n, half, 5, 4, 120.0).expect("there is room for lakes");
    assert!(added >= 1 && added <= 4);
    assert!(
        (0..n * n).any(|i| heights[i] < water[i] && water[i] > 0.0 && start[i] >= 0.0),
        "lakes were dug"
    );
    assert!(heights.iter().zip(&start).all(|(h, s)| h <= s));
    for (idx, &h) in start.iter().enumerate() {
        if h < 0.0 {
            assert_eq!(heights[idx], h, "the sea is left alone");
            assert_eq!(water[idx], 0.0, "and so is its water");
        }
    }
    // The same seed repeats, another differs, and open water has no room.
    assert_eq!(Some(&heights), scatter_lakes(&start, &dry, n, n, half, 5, 4, 120.0).as_ref().map(|r| &r.0));
    assert_ne!(Some(&heights), scatter_lakes(&start, &dry, n, n, half, 6, 4, 120.0).as_ref().map(|r| &r.0));
    assert!(scatter_lakes(&vec![DEEP_OCEAN; n * n], &dry, n, n, half, 5, 4, 120.0).is_none());
}

#[test]
fn a_lake_on_a_slope_sits_at_the_lowest_ground_under_the_brush() {
    let start = tilted_island(12.0, 0.03);
    let (heights, water) = dig_lake(&start, &vec![0.0; 161 * 161], vec2(0.0, 0.0), 30.0);
    let centre = 80 * 161 + 80;
    // The shoreline can wander as far as 1.35 radii, plus a little, to the west: where the
    // ground is 12 - 0.03 * 48.5 = 10.5 m.
    let level = water[centre];
    assert!((10.2..10.9).contains(&level), "the lake's level is {level}");
    let xs = flooded_xs(&heights, &water);
    assert!(!xs.is_empty());
    for idx in 0..heights.len() {
        if water[idx] > 0.0 {
            assert_eq!(water[idx], level, "one lake, one level");
        }
    }
    // Far from the lake the water is still the sea's.
    assert_eq!(water[10 * 161 + 20], 0.0);
    assert_eq!(heights.iter().filter(|&&h| h < 0.0).count(), start.iter().filter(|&&h| h < 0.0).count());
}

#[test]
fn a_stroke_that_touches_a_lake_matches_its_level() {
    let start = tilted_island(12.0, 0.03);
    let dry = vec![0.0; 161 * 161];
    let (once, water) = dig_lake(&start, &dry, vec2(-100.0, 0.0), 40.0);
    let level = water[80 * 161 + 80 - 25];
    assert!((6.5..8.0).contains(&level), "the first lake's level is {level}");
    let east_edge = flooded_xs(&once, &water).into_iter().fold(f32::MIN, f32::max);

    // A second stroke overlapping the first would, on its own, find higher ground to sit on.
    let (twice, water) = dig_lake(&once, &water, vec2(-40.0, 0.0), 40.0);
    let levels: Vec<f32> = (0..twice.len()).filter(|&i| water[i] > 0.0).map(|i| water[i]).collect();
    assert!(levels.iter().all(|&l| l == level), "the lakes joined up at one level");
    let new_edge = flooded_xs(&twice, &water).into_iter().fold(f32::MIN, f32::max);
    assert!(new_edge > east_edge + 20.0, "the lake grew from {east_edge} to {new_edge}");
}

#[test]
fn a_lake_does_not_spread_onto_ground_lower_than_its_water() {
    let start = tilted_island(12.0, 0.03);
    let dry = vec![0.0; 161 * 161];
    let (once, water) = dig_lake(&start, &dry, vec2(100.0, 0.0), 40.0);
    let level = water[80 * 161 + 80 + 25];
    assert!(level > 12.5, "the first lake's level is {level}");
    // Downhill from the lake the ground lies lower than its water, so the brush does nothing.
    let (twice, water_after) = dig_lake(&once, &water, vec2(20.0, 0.0), 40.0);
    assert_eq!(twice, once);
    assert_eq!(water_after, water);
}


#[test]
fn mountains_rise_steeply_where_hills_do_not() {
    let cells = 160;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let steepest = |peak: Peak| {
        let mut heights = island(cells, 10);
        let mut stroke = HillStroke::begin(&heights, n, n, 9, false, peak);
        for _ in 0..600 {
            stroke.paint(&mut heights, half, vec2(0.0, 0.0), 60.0, 0.1);
        }
        stroke.finish(&mut heights);
        let mut steepest: f32 = 0.0;
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                let h = heights[j * n + i];
                if h < 0.0 {
                    continue;
                }
                for other in [heights[j * n + i + 1], heights[(j + 1) * n + i]] {
                    if other >= 0.0 {
                        steepest = steepest.max((h - other).abs() / CELL_SIZE);
                    }
                }
            }
        }
        steepest
    };
    let (hills, mountains) = (steepest(Peak::Hills), steepest(Peak::Mountains));
    assert!(mountains > 2.0, "mountain sides should be steep, got {mountains}");
    assert!(mountains > hills * 2.0, "mountains {mountains} vs hills {hills}");
}

#[test]
fn mountains_are_ringed_by_foothills_that_ease_into_the_lowlands() {
    let cells = 200;
    let n = cells + 1;
    let half = Vec2::splat(cells as f32 * CELL_SIZE * 0.5);
    let mut heights = island(cells, 10);
    let mut stroke = HillStroke::begin(&heights, n, n, 9, false, Peak::Mountains);
    for _ in 0..600 {
        stroke.paint(&mut heights, half, vec2(0.0, 0.0), 60.0, 0.1);
    }
    stroke.finish(&mut heights);
    let (mut ring, mut far) = (Vec::new(), Vec::new());
    for j in 0..n {
        for i in 0..n {
            let d = (vec2(i as f32 * CELL_SIZE, j as f32 * CELL_SIZE) - half).length();
            if d > 66.0 && d < 100.0 {
                ring.push(heights[j * n + i]);
            } else if d > 190.0 && heights[j * n + i] >= 0.0 {
                far.push(heights[j * n + i]);
            }
        }
    }
    let mean = ring.iter().sum::<f32>() / ring.len() as f32;
    assert!(ring.iter().all(|&h| h <= MAX_HILL + 0.01), "foothills are hills, not mountains");
    assert!(mean >= MIN_HILL, "the ring around the mountain stands at hill height, got {mean}");
    assert!(far.iter().all(|&h| h <= 10.0), "beyond the foothills the ground is untouched");
}

#[test]
fn mountain_tops_vary_from_low_peaks_to_tall_ones() {
    let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Mountains);
    let mut tops = Vec::new();
    for j in 0..200 {
        for i in 0..200 {
            tops.push(stroke.target(Peak::Mountains, i as f32 * 15.0, j as f32 * 15.0));
        }
    }
    let share = |test: &dyn Fn(f32) -> bool| tops.iter().filter(|&&t| test(t)).count() as f32 / tops.len() as f32;
    let mean = tops.iter().sum::<f32>() / tops.len() as f32;
    let (low, mid, tall) = (share(&|t| t < 26.0), share(&|t| (26.0..42.0).contains(&t)), share(&|t| t >= 42.0));
    eprintln!("mean {mean:.1} m; below 26 m {low:.2}, 26 to 42 m {mid:.2}, 42 m and up {tall:.2}");
    // Some of everything: lots of low ground and saddles, a good share of middling peaks,
    // and a few tall ones standing above the rest.
    assert!(low > 0.12, "only {low} of the ground is below 26 m");
    assert!(mid > 0.3, "only {mid} is 26 to 42 m");
    assert!(tall > 0.03 && tall < 0.35, "{tall} reaches 42 m");
}

#[test]
fn mountain_heights_run_from_valleys_to_the_top_of_the_range() {
    let stroke = HillStroke::begin(&[0.0; 4], 2, 2, 11, false, Peak::Mountains);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for j in 0..300 {
        for i in 0..300 {
            let t = stroke.target(Peak::Mountains, i as f32 * 20.0, j as f32 * 20.0);
            lo = lo.min(t);
            hi = hi.max(t);
        }
    }
    assert!(lo >= MIN_HILL && lo < MIN_MOUNTAIN, "lowest was {lo}");
    assert!(hi <= MAX_MOUNTAIN && hi > MAX_MOUNTAIN - 1.0, "tallest was {hi}");
}
