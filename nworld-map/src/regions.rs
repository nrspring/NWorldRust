//! Regions: the map divided into areas that each hold one kind of ground, so the game can reason
//! about whole areas at once.
//!
//! Every sample of the map is one of four kinds: deep water, shallow water, mountain or land
//! (which includes lakes and rivers, so inland water never splits a land region). A region only
//! ever holds samples of its own kind and is one connected piece, so shallow regions are only
//! shallow water, deep regions only deep water, mountain regions only mountain, and land regions
//! everything else.
//!
//! Regions are grown outward from seeds spread over each connected stretch of one kind, each
//! seed with its own speed and a wobble in the ground it crosses, so their sizes vary a lot and
//! their borders are not straight. Slivers left too small are folded into a neighbour of the same
//! kind. Specks of a kind far smaller than any region are first absorbed by their surroundings,
//! so a few stray samples of rock don't each become a region.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::density::value_noise;
use crate::world::{CELL_SIZE, SEA_LEVEL, SHALLOW_LIMIT};

/// Ground this high above the sea, in meters, is mountain: where the terrain turns to bare rock.
pub const MOUNTAIN_HEIGHT: f32 = 30.0;

/// Most regions a map can hold: a region's number takes 14 bits of its id.
pub const MAX_REGIONS: usize = (1 << 14) - 1;

/// A kind of ground. A region is made of one kind only.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum RegionKind {
    Deep = 0,
    Shallow = 1,
    Mountain = 2,
    Land = 3,
}

impl RegionKind {
    pub const ALL: [RegionKind; 4] = [RegionKind::Deep, RegionKind::Shallow, RegionKind::Mountain, RegionKind::Land];

    fn from_bits(bits: u16) -> Self {
        Self::ALL[(bits & 3) as usize]
    }
}

/// Which region a point is in. Unique across the map; the kind of ground it holds is in its top
/// bits, so the id alone says what it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct RegionId(u16);

impl RegionId {
    /// The id of region number `number` (1 up to [`MAX_REGIONS`]) of kind `kind`.
    fn new(kind: RegionKind, number: usize) -> Self {
        debug_assert!((1..=MAX_REGIONS).contains(&number));
        Self((kind as u16) << 14 | number as u16)
    }

    /// The id stored in a map, or `None` for 0, which means no region.
    pub fn from_raw(raw: u16) -> Option<Self> {
        (raw & 0x3FFF != 0).then_some(Self(raw))
    }

    /// The number stored in a map for this id.
    pub fn raw(self) -> u16 {
        self.0
    }

    pub fn kind(self) -> RegionKind {
        RegionKind::from_bits(self.0 >> 14)
    }
}

/// The kind of ground at a sample with ground at `height` and water standing at `level`.
pub fn kind_of(height: f32, level: f32) -> RegionKind {
    if height >= level {
        if height >= MOUNTAIN_HEIGHT {
            RegionKind::Mountain
        } else {
            RegionKind::Land
        }
    } else if level > SEA_LEVEL {
        // A lake or river.
        RegionKind::Land
    } else if level - height <= SHALLOW_LIMIT {
        RegionKind::Shallow
    } else {
        RegionKind::Deep
    }
}

/// The kind of ground at every sample.
pub fn classify(heights: &[f32], water: &[f32]) -> Vec<RegionKind> {
    heights.iter().zip(water).map(|(&h, &w)| kind_of(h, w)).collect()
}

/// The samples next to sample `k` of a grid `nx` by `nz`, up to four.
#[inline]
fn around(k: usize, nx: usize, nz: usize) -> impl Iterator<Item = usize> {
    let (i, j) = (k % nx, k / nx);
    [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < nz).then(|| k + nx)]
        .into_iter()
        .flatten()
}

/// The connected stretches of one kind: each sample's stretch number, and each stretch's size.
fn components(kinds: &[RegionKind], nx: usize, nz: usize) -> (Vec<u32>, Vec<u32>) {
    let mut labels = vec![u32::MAX; kinds.len()];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..kinds.len() {
        if labels[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        labels[start] = id;
        stack.push(start);
        let mut count = 0;
        while let Some(k) = stack.pop() {
            count += 1;
            for m in around(k, nx, nz) {
                if labels[m] == u32::MAX && kinds[m] == kinds[k] {
                    labels[m] = id;
                    stack.push(m);
                }
            }
        }
        sizes.push(count);
    }
    (labels, sizes)
}

/// Turns every stretch of fewer than `min_area` samples into whichever neighbouring kind borders
/// it most, so specks too small to matter don't become regions of their own.
pub fn despeckle(kinds: &mut [RegionKind], nx: usize, nz: usize, min_area: u32) {
    for _ in 0..2 {
        let (labels, sizes) = components(kinds, nx, nz);
        if sizes.iter().all(|&s| s >= min_area) {
            return;
        }
        let mut votes: HashMap<u32, [u32; 4]> = HashMap::new();
        for k in 0..kinds.len() {
            let c = labels[k];
            if sizes[c as usize] < min_area {
                for m in around(k, nx, nz) {
                    if kinds[m] != kinds[k] {
                        votes.entry(c).or_default()[kinds[m] as usize] += 1;
                    }
                }
            }
        }
        let new_kind: HashMap<u32, RegionKind> = votes
            .into_iter()
            .map(|(c, v)| (c, RegionKind::ALL[(0..4).max_by_key(|&i| v[i]).unwrap()]))
            .collect();
        for k in 0..kinds.len() {
            let c = labels[k];
            if sizes[c as usize] < min_area {
                if let Some(&kind) = new_kind.get(&c) {
                    kinds[k] = kind;
                }
            }
        }
    }
}

/// A cheap deterministic hash of three numbers.
fn hash(a: u32, b: u32, seed: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// How many of every hundred candidate seeds are used; the rest make their neighbours bigger.
const KEEP_PERCENT: u32 = 65;
/// Regions smaller than this share of the average are folded into a neighbour.
const MIN_REGION_SHARE: f32 = 0.18;

/// Divides the map into regions about `size` meters across on average, as a region id for every
/// sample (see [`RegionId`]). `kinds` says what ground each sample is. Returns `None` if that
/// would make more than [`MAX_REGIONS`] regions.
pub fn divide(kinds: &[RegionKind], nx: usize, nz: usize, size: f32, seed: u32) -> Option<Vec<u16>> {
    let n = kinds.len();
    let (labels, sizes) = components(kinds, nx, nz);
    let target = (size / CELL_SIZE).max(2.0).powi(2);

    // Seeds: in each square of the map, one for each stretch of ground in it, mostly, picked at
    // random. Stretches that are barely in the square don't get one; a thin one, like a shallow
    // band along a coast, still gets one in each square it crosses.
    let cell = ((0.8 * size / CELL_SIZE) as usize).max(3);
    let mut seeds: Vec<(usize, u32)> = Vec::new();
    let mut seeded = vec![false; sizes.len()];
    let mut found: HashMap<u32, (usize, u32, usize)> = HashMap::new();
    for cz in (0..nz).step_by(cell) {
        for cx in (0..nx).step_by(cell) {
            let (x1, z1) = ((cx + cell).min(nx), (cz + cell).min(nz));
            let area = (x1 - cx) * (z1 - cz);
            found.clear();
            for j in cz..z1 {
                for i in cx..x1 {
                    let k = j * nx + i;
                    let score = hash(i as u32, j as u32, seed);
                    let entry = found.entry(labels[k]).or_insert((0, u32::MAX, k));
                    entry.0 += 1;
                    if score < entry.1 {
                        entry.1 = score;
                        entry.2 = k;
                    }
                }
            }
            // In a fixed order, so the same seed always gives the same regions.
            let mut here: Vec<(u32, (usize, u32, usize))> = found.iter().map(|(&c, &v)| (c, v)).collect();
            here.sort_unstable_by_key(|&(c, _)| c);
            for (c, (count, _, k)) in here {
                if count < (area / 48).max(3) {
                    continue;
                }
                // Stretches that fill little of the square are thin, like a shallow band along a
                // coast. Every square gets a seed on those, so their regions are about as long as
                // the squares are wide and not strung out along the whole stretch.
                let thin = count * 5 < area * 2;
                if !thin && hash(cx as u32, cz as u32, seed ^ c.wrapping_mul(0x9E37_79B9)) % 100 >= KEEP_PERCENT {
                    continue;
                }
                // Each seed grows at its own speed, which is what makes regions differ in size.
                seeds.push((k, 45 + hash(k as u32, 7, seed) % 52));
                seeded[c as usize] = true;
            }
        }
    }
    // A stretch with no seed gets one at the sample nearest its middle.
    if seeded.iter().any(|&s| !s) {
        let mut sums = vec![(0.0f64, 0.0f64); sizes.len()];
        for k in 0..n {
            let c = labels[k] as usize;
            if !seeded[c] {
                sums[c].0 += (k % nx) as f64;
                sums[c].1 += (k / nx) as f64;
            }
        }
        let mut nearest = vec![(f64::MAX, 0usize); sizes.len()];
        for k in 0..n {
            let c = labels[k] as usize;
            if !seeded[c] {
                let (mx, mz) = (sums[c].0 / sizes[c] as f64, sums[c].1 / sizes[c] as f64);
                let d = ((k % nx) as f64 - mx).powi(2) + ((k / nx) as f64 - mz).powi(2);
                if d < nearest[c].0 {
                    nearest[c] = (d, k);
                }
            }
        }
        for c in 0..sizes.len() {
            if !seeded[c] {
                seeds.push((nearest[c].1, 45 + hash(c as u32, 11, seed) % 52));
            }
        }
    }

    // Growth: every seed claims the samples it reaches first, through ground of its own kind.
    // The ground is a little slower to cross in places, which wanders the borders.
    let noise: Vec<u8> = (0..n)
        .map(|k| (value_noise((k % nx) as f32 / 14.0, (k / nx) as f32 / 14.0, (seed & 0xFFFF) as i32) * 255.0) as u8)
        .collect();
    let mut dist = vec![u32::MAX; n];
    let mut owner = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    for (s, &(k, _)) in seeds.iter().enumerate() {
        dist[k] = 0;
        owner[k] = s as u32;
        heap.push(Reverse(k as u64));
    }
    while let Some(Reverse(top)) = heap.pop() {
        let (d, k) = ((top >> 32) as u32, (top & 0xFFFF_FFFF) as usize);
        if d > dist[k] {
            continue;
        }
        let o = owner[k];
        let speed = seeds[o as usize].1;
        for m in around(k, nx, nz) {
            if kinds[m] != kinds[k] {
                continue;
            }
            let nd = d + (64 + noise[m] as u32 / 4) * speed / 64;
            if nd < dist[m] {
                dist[m] = nd;
                owner[m] = o;
                heap.push(Reverse((nd as u64) << 32 | m as u64));
            }
        }
    }
    drop((dist, heap, noise));

    // Fold slivers into the neighbour they share the most border with.
    let count = seeds.len();
    let mut area = vec![0u32; count];
    // The corners of the box around each region.
    let mut low = vec![(u32::MAX, u32::MAX); count];
    let mut high = vec![(0u32, 0u32); count];
    let mut touching: HashMap<(u32, u32), u32> = HashMap::new();
    for k in 0..n {
        let o = owner[k] as usize;
        area[o] += 1;
        let (i, j) = ((k % nx) as u32, (k / nx) as u32);
        low[o] = (low[o].0.min(i), low[o].1.min(j));
        high[o] = (high[o].0.max(i), high[o].1.max(j));
        for m in [(k % nx + 1 < nx).then(|| k + 1), (k / nx + 1 < nz).then(|| k + nx)].into_iter().flatten() {
            if owner[m] != owner[k] && kinds[m] == kinds[k] {
                *touching.entry((owner[k].min(owner[m]), owner[k].max(owner[m]))).or_insert(0) += 1;
            }
        }
    }
    let mut neighbours: Vec<Vec<(u32, u32)>> = vec![Vec::new(); count];
    for (&(a, b), &edges) in &touching {
        neighbours[a as usize].push((b, edges));
        neighbours[b as usize].push((a, edges));
    }
    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }
    let mut parent: Vec<u32> = (0..count as u32).collect();
    let min_area = ((target * MIN_REGION_SHARE) as u32).max(4);
    // A region is only a sliver if it is short in every direction: a short piece of a long
    // thin stretch is as it should be.
    let short = 0.5 * (size / CELL_SIZE).max(2.0);
    let mut order: Vec<u32> = (0..count as u32).collect();
    order.sort_by_key(|&r| area[r as usize]);
    for r in order {
        let extent = (high[r as usize].0 - low[r as usize].0).max(high[r as usize].1 - low[r as usize].1) + 1;
        if parent[r as usize] != r || area[r as usize] >= min_area || extent as f32 >= short {
            continue;
        }
        let mut shared: HashMap<u32, u32> = HashMap::new();
        for (o, edges) in std::mem::take(&mut neighbours[r as usize]) {
            let root = find(&mut parent, o);
            if root != r {
                *shared.entry(root).or_insert(0) += edges;
            }
        }
        if let Some((&best, _)) = shared.iter().max_by_key(|&(&root, &edges)| (edges, root)) {
            parent[r as usize] = best;
            area[best as usize] += area[r as usize];
            low[best as usize] = (low[best as usize].0.min(low[r as usize].0), low[best as usize].1.min(low[r as usize].1));
            high[best as usize] = (high[best as usize].0.max(high[r as usize].0), high[best as usize].1.max(high[r as usize].1));
            neighbours[best as usize].extend(shared.into_iter());
        }
    }

    // Number what is left, and write every sample's region.
    let mut number = vec![0u16; count];
    let mut total = 0;
    let mut ids = vec![0u16; count];
    for r in 0..count as u32 {
        let root = find(&mut parent, r);
        if root == r {
            total += 1;
            if total > MAX_REGIONS {
                return None;
            }
            number[r as usize] = total as u16;
        }
    }
    for r in 0..count as u32 {
        let root = find(&mut parent, r);
        ids[r as usize] = RegionId::new(kinds[seeds[root as usize].0], number[root as usize] as usize).raw();
    }
    Some(owner.iter().map(|&o| ids[o as usize]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NX: usize = 300;
    const NZ: usize = 200;

    /// Deep water in the west, then a band of shallows, then land with a mountain block and a lake.
    fn map() -> Vec<RegionKind> {
        let mut kinds = vec![RegionKind::Land; NX * NZ];
        for j in 0..NZ {
            for i in 0..NX {
                kinds[j * NX + i] = match i {
                    0..=99 => RegionKind::Deep,
                    100..=129 => RegionKind::Shallow,
                    200..=259 if (50..150).contains(&j) => RegionKind::Mountain,
                    _ => RegionKind::Land,
                };
            }
        }
        kinds
    }

    #[test]
    fn ground_is_sorted_into_four_kinds() {
        assert_eq!(kind_of(10.0, 0.0), RegionKind::Land);
        assert_eq!(kind_of(45.0, 0.0), RegionKind::Mountain);
        assert_eq!(kind_of(-3.0, 0.0), RegionKind::Shallow);
        assert_eq!(kind_of(-6.0, 0.0), RegionKind::Deep);
        assert_eq!(kind_of(-200.0, 0.0), RegionKind::Deep);
        // Lakes and rivers belong to the land, however deep they are dug.
        assert_eq!(kind_of(-40.0, 20.0), RegionKind::Land);
        assert_eq!(kind_of(18.0, 20.0), RegionKind::Land);
    }

    #[test]
    fn region_ids_say_what_kind_they_are() {
        for kind in RegionKind::ALL {
            let id = RegionId::new(kind, 123);
            assert_eq!(id.kind(), kind);
            assert_eq!(RegionId::from_raw(id.raw()), Some(id));
        }
        assert_eq!(RegionId::from_raw(0), None);
    }

    #[test]
    fn specks_are_absorbed_but_real_stretches_stay() {
        let mut kinds = map();
        // A 3 x 3 patch of mountain in the land, and a 12 x 12 one.
        for (i0, j0, side) in [(150, 30, 3), (150, 120, 12)] {
            for j in j0..j0 + side {
                for i in i0..i0 + side {
                    kinds[j * NX + i] = RegionKind::Mountain;
                }
            }
        }
        despeckle(&mut kinds, NX, NZ, 40);
        assert_eq!(kinds[31 * NX + 151], RegionKind::Land);
        assert_eq!(kinds[125 * NX + 155], RegionKind::Mountain);
    }

    #[test]
    fn every_sample_is_in_a_connected_region_of_one_kind() {
        let kinds = map();
        let ids = divide(&kinds, NX, NZ, 120.0, 5).unwrap();
        assert!(ids.iter().all(|&id| RegionId::from_raw(id).is_some()), "a sample with no region");
        for (k, &id) in ids.iter().enumerate() {
            assert_eq!(RegionId::from_raw(id).unwrap().kind(), kinds[k], "a region holds two kinds");
        }
        let mut sizes: HashMap<u16, usize> = HashMap::new();
        ids.iter().for_each(|&id| *sizes.entry(id).or_insert(0) += 1);
        for (&id, &size) in &sizes {
            let start = ids.iter().position(|&x| x == id).unwrap();
            let mut seen = vec![false; ids.len()];
            let mut stack = vec![start];
            seen[start] = true;
            let mut reached = 0;
            while let Some(k) = stack.pop() {
                reached += 1;
                for m in around(k, NX, NZ) {
                    if !seen[m] && ids[m] == id {
                        seen[m] = true;
                        stack.push(m);
                    }
                }
            }
            assert_eq!(reached, size, "region {id} is in pieces");
        }
    }

    #[test]
    fn regions_are_about_the_asked_size_and_vary_a_lot() {
        let kinds = map();
        let ids = divide(&kinds, NX, NZ, 120.0, 5).unwrap();
        let mut sizes: HashMap<u16, usize> = HashMap::new();
        ids.iter().for_each(|&id| *sizes.entry(id).or_insert(0) += 1);
        // 120 m is 30 samples across, so 900 samples each on average.
        let average = ids.len() as f32 / sizes.len() as f32;
        assert!((450.0..1800.0).contains(&average), "{} regions of {average} on average", sizes.len());
        let (small, big) = (*sizes.values().min().unwrap(), *sizes.values().max().unwrap());
        assert!(big >= small * 3, "regions run from {small} to {big}");
        // Every kind is cut up, not left whole, when it is much bigger than a region.
        for kind in [RegionKind::Deep, RegionKind::Land] {
            let regions = sizes.keys().filter(|&&id| RegionId::from_raw(id).unwrap().kind() == kind).count();
            assert!(regions > 3, "only {regions} regions of {kind:?}");
        }
    }

    #[test]
    fn small_stretches_get_a_region_and_bigger_sizes_make_fewer_regions() {
        let mut kinds = map();
        // A shallow pond 6 x 6 in the middle of the mountain block.
        for j in 90..96 {
            for i in 220..226 {
                kinds[j * NX + i] = RegionKind::Shallow;
            }
        }
        let count = |size: f32| {
            let ids = divide(&kinds, NX, NZ, size, 9).unwrap();
            let mut seen: Vec<u16> = ids.clone();
            seen.sort_unstable();
            seen.dedup();
            assert!(ids.iter().all(|&id| id != 0));
            assert!(seen.contains(&ids[92 * NX + 222]));
            seen.len()
        };
        assert!(count(80.0) > count(160.0));
        assert!(count(160.0) > count(400.0));
    }

    #[test]
    fn a_long_thin_stretch_is_cut_into_pieces_no_longer_than_the_regions_are_wide() {
        // 12 samples (48 m) of shallows along the whole 900-sample length of a land edge.
        let (nx, nz) = (900, 120);
        let mut kinds = vec![RegionKind::Land; nx * nz];
        for j in 0..12 {
            for i in 0..nx {
                kinds[j * nx + i] = RegionKind::Shallow;
            }
        }
        // 500 m regions are 125 samples across.
        let ids = divide(&kinds, nx, nz, 500.0, 3).unwrap();
        let mut boxes: HashMap<u16, (usize, usize)> = HashMap::new();
        for j in 0..12 {
            for i in 0..nx {
                let e = boxes.entry(ids[j * nx + i]).or_insert((usize::MAX, 0));
                e.0 = e.0.min(i);
                e.1 = e.1.max(i);
            }
        }
        let longest = boxes.values().map(|&(lo, hi)| hi - lo + 1).max().unwrap();
        assert!(longest <= 250, "a region of shallows {longest} samples long, with regions 125 across");
        assert!(boxes.len() >= 5, "only {} pieces", boxes.len());
    }

    #[test]
    fn the_same_seed_gives_the_same_regions_and_another_gives_different_ones() {
        let kinds = map();
        assert_eq!(divide(&kinds, NX, NZ, 120.0, 5), divide(&kinds, NX, NZ, 120.0, 5));
        assert_ne!(divide(&kinds, NX, NZ, 120.0, 5), divide(&kinds, NX, NZ, 120.0, 6));
    }

    #[test]
    fn too_many_regions_are_refused() {
        let kinds = vec![RegionKind::Land; 700 * 700];
        assert!(divide(&kinds, 700, 700, 12.0, 1).is_none());
    }
}
