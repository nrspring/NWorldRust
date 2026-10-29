//! Saving and loading maps.
//!
//! A map file is a small header followed by the heightmap, the water levels and the resource
//! densities, compressed:
//!
//! | bytes | meaning |
//! | --- | --- |
//! | 8 | magic, `NWORLDMP` |
//! | 4 | format version (little endian) |
//! | 4, 4 | heightmap cells along x and z |
//! | 4 | how many times continents have been generated on the map |
//! | rest | the (cells_x + 1) × (cells_z + 1) heights, then as many water levels, as `f32`, byte-shuffled, then as many density planes for each resource, as `u8`, all deflated |
//!
//! Shuffling groups the first bytes of every height together, then the second bytes, and so
//! on, which makes the flat stretches of ocean and land compress far better. Version 1 files
//! have no water levels: the water is at sea level everywhere. Versions before 3 have no forest,
//! versions before 4 no stone, versions before 5 no iron, versions before 6 no copper and versions before 7 no sulphur or oil.

use std::fs;
use std::path::Path;

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;

use crate::world::{cells_valid, Resource, DEEP_OCEAN, SEA_LEVEL};

/// File extension of saved maps, without the dot.
pub const EXTENSION: &str = "nworld";

const MAGIC: &[u8; 8] = b"NWORLDMP";
const VERSION: u32 = 7;
/// The format version that first saved each resource, in the order of [`Resource::ALL`].
const RESOURCE_SINCE: [u32; Resource::COUNT] = [3, 4, 5, 6, 7, 7];
const HEADER_LEN: usize = 8 + 4 + 4 + 4 + 4;
/// Deflate effort; 6 is the usual balance of speed and size.
const COMPRESSION: u8 = 6;
/// Heights outside this range can't come from the editor, so they mean a damaged file.
const MAX_HEIGHT: f32 = 10_000.0;

/// Everything about a map that is written to a file.
#[derive(Debug)]
pub struct SavedMap {
    pub cells_x: usize,
    pub cells_z: usize,
    pub generations: u32,
    /// (cells_x + 1) × (cells_z + 1) samples, row-major from the north-west corner.
    pub heights: Vec<f32>,
    /// The water level at each of the same samples.
    pub water: Vec<f32>,
    /// The densities of each resource, in the order of [`Resource::ALL`], at each of the same samples, 0 to 255.
    pub resources: [Vec<u8>; Resource::COUNT],
}

/// The file contents for a map.
pub fn encode(map: &SavedMap) -> Vec<u8> {
    let n = map.heights.len();
    let mut shuffled = vec![0u8; n * (8 + Resource::COUNT)];
    for (layer, values) in [&map.heights, &map.water].into_iter().enumerate() {
        for (i, h) in values.iter().enumerate() {
            for (plane, byte) in h.to_le_bytes().into_iter().enumerate() {
                shuffled[layer * n * 4 + plane * n + i] = byte;
            }
        }
    }

    let mut out = Vec::with_capacity(HEADER_LEN + n);
    out.extend_from_slice(MAGIC);
    for value in [VERSION, map.cells_x as u32, map.cells_z as u32, map.generations] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for (k, layer) in map.resources.iter().enumerate() {
        shuffled[n * (8 + k)..n * (9 + k)].copy_from_slice(layer);
    }
    out.extend(compress_to_vec(&shuffled, COMPRESSION));
    out
}

/// Reads a map back from file contents, checking everything that could be wrong with them.
pub fn decode(bytes: &[u8]) -> Result<SavedMap, String> {
    if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
        return Err("This is not an NWorld map file.".to_owned());
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let version = word(8);
    if version == 0 || version > VERSION {
        return Err(format!(
            "This map was saved by {} version of NWorld (file format {version}).",
            if version > VERSION { "a newer" } else { "an unknown" }
        ));
    }
    let (cells_x, cells_z, generations) = (word(12) as usize, word(16) as usize, word(20));
    if !cells_valid(cells_x) || !cells_valid(cells_z) {
        return Err(format!("The map size in this file ({cells_x} × {cells_z} cells) is not valid."));
    }

    let n = (cells_x + 1) * (cells_z + 1);
    let layers = if version == 1 { 1 } else { 2 };
    let saved = RESOURCE_SINCE.iter().filter(|&&since| version >= since).count();
    let expected = n * 4 * layers + n * saved;
    let shuffled = decompress_to_vec_with_limit(&bytes[HEADER_LEN..], expected)
        .map_err(|_| "The map data in this file is damaged.".to_owned())?;
    if shuffled.len() != expected {
        return Err("The map data in this file is incomplete.".to_owned());
    }

    let read = |layer: usize, i: usize| {
        let at = layer * n * 4 + i;
        f32::from_le_bytes([shuffled[at], shuffled[at + n], shuffled[at + 2 * n], shuffled[at + 3 * n]])
    };
    let mut heights = Vec::with_capacity(n);
    let mut water = Vec::with_capacity(n);
    for i in 0..n {
        let h = read(0, i);
        if !h.is_finite() || !(DEEP_OCEAN - 1.0..=MAX_HEIGHT).contains(&h) {
            return Err("The map data in this file is damaged.".to_owned());
        }
        heights.push(h);
        let w = if layers == 2 { read(1, i) } else { SEA_LEVEL };
        if !w.is_finite() || !(SEA_LEVEL..=MAX_HEIGHT).contains(&w) {
            return Err("The map data in this file is damaged.".to_owned());
        }
        water.push(w);
    }
    // Resources are saved in order, so the first `saved` of them are in the file.
    let resources = std::array::from_fn(|k| if k < saved { shuffled[n * (8 + k)..n * (9 + k)].to_vec() } else { vec![0; n] });
    Ok(SavedMap {
        cells_x,
        cells_z,
        generations,
        heights,
        water,
        resources,
    })
}

/// Writes a map to `path`. The data goes to a temporary file first and replaces the real one
/// only once it is fully written, so a failed save never damages an existing map.
pub fn save(path: &Path, map: &SavedMap) -> Result<(), String> {
    let temp = path.with_extension(format!("{EXTENSION}.tmp"));
    let result = fs::write(&temp, encode(map)).and_then(|()| fs::rename(&temp, path));
    if let Err(e) = result {
        let _ = fs::remove_file(&temp);
        return Err(format!("Couldn't save the map: {e}."));
    }
    Ok(())
}

pub fn load(path: &Path) -> Result<SavedMap, String> {
    let bytes = fs::read(path).map_err(|e| format!("The file can't be read ({e})."))?;
    decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map of 64 × 80 cells (256 × 320 m) with ocean, land and sloping shallows.
    fn sample_map() -> SavedMap {
        let (cells_x, cells_z) = (64, 80);
        let mut heights = vec![DEEP_OCEAN; (cells_x + 1) * (cells_z + 1)];
        for (i, h) in heights.iter_mut().enumerate() {
            match i % 11 {
                0..=2 => *h = 1.0,
                3..=5 => *h = -2.5 - (i % 97) as f32 * 0.7,
                6 => *h = 0.25,
                _ => {}
            }
        }
        // A lake: ground around 12 m with water standing at 10 m.
        let mut water = vec![SEA_LEVEL; heights.len()];
        for (i, w) in water.iter_mut().enumerate() {
            if i % 13 < 4 {
                *w = 10.0;
            }
        }
        let forest = (0..heights.len()).map(|i| (i % 5 * 60) as u8).collect();
        let stone = (0..heights.len()).map(|i| (i % 7 * 40) as u8).collect();
        let iron = (0..heights.len()).map(|i| (i % 11 * 25) as u8).collect();
        let copper = (0..heights.len()).map(|i| (i % 13 * 19) as u8).collect();
        let sulphur = (0..heights.len()).map(|i| (i % 17 * 15) as u8).collect();
        let oil = (0..heights.len()).map(|i| (i % 19 * 13) as u8).collect();
        SavedMap {
            cells_x,
            cells_z,
            generations: 7,
            heights,
            water,
            resources: [forest, stone, iron, copper, sulphur, oil],
        }
    }

    #[test]
    fn round_trip_is_exact() {
        let map = sample_map();
        let back = decode(&encode(&map)).unwrap();
        assert_eq!((back.cells_x, back.cells_z, back.generations), (64, 80, 7));
        assert_eq!(back.heights, map.heights);
        assert_eq!(back.water, map.water);
        assert_eq!(back.resources, map.resources);
    }

    #[test]
    fn maps_saved_before_forests_load_without_any() {
        let map = sample_map();
        let n = map.heights.len();
        let mut shuffled = vec![0u8; n * 8];
        for (layer, values) in [&map.heights, &map.water].into_iter().enumerate() {
            for (i, v) in values.iter().enumerate() {
                for (plane, byte) in v.to_le_bytes().into_iter().enumerate() {
                    shuffled[layer * n * 4 + plane * n + i] = byte;
                }
            }
        }
        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        for value in [2u32, map.cells_x as u32, map.cells_z as u32, map.generations] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        old.extend(compress_to_vec(&shuffled, COMPRESSION));
        let back = decode(&old).unwrap();
        assert_eq!(back.water, map.water);
        assert!(back.resources.iter().all(|layer| layer.len() == n && layer.iter().all(|&d| d == 0)));
    }

    #[test]
    fn maps_saved_before_iron_and_copper_load_with_their_forest_and_stone_and_none() {
        let map = sample_map();
        let n = map.heights.len();
        let mut shuffled = vec![0u8; n * 10];
        for (layer, values) in [&map.heights, &map.water].into_iter().enumerate() {
            for (i, v) in values.iter().enumerate() {
                for (plane, byte) in v.to_le_bytes().into_iter().enumerate() {
                    shuffled[layer * n * 4 + plane * n + i] = byte;
                }
            }
        }
        shuffled[n * 8..n * 9].copy_from_slice(&map.resources[0]);
        shuffled[n * 9..].copy_from_slice(&map.resources[1]);
        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        for value in [4u32, map.cells_x as u32, map.cells_z as u32, map.generations] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        old.extend(compress_to_vec(&shuffled, COMPRESSION));
        let back = decode(&old).unwrap();
        assert_eq!(back.resources[0], map.resources[0]);
        assert_eq!(back.resources[1], map.resources[1]);
        for layer in &back.resources[2..] {
            assert!(layer.len() == n && layer.iter().all(|&d| d == 0));
        }
    }

    #[test]
    fn maps_saved_before_copper_load_with_their_iron_and_no_copper() {
        let map = sample_map();
        let n = map.heights.len();
        let mut shuffled = vec![0u8; n * 11];
        for (layer, values) in [&map.heights, &map.water].into_iter().enumerate() {
            for (i, v) in values.iter().enumerate() {
                for (plane, byte) in v.to_le_bytes().into_iter().enumerate() {
                    shuffled[layer * n * 4 + plane * n + i] = byte;
                }
            }
        }
        for k in 0..3 {
            shuffled[n * (8 + k)..n * (9 + k)].copy_from_slice(&map.resources[k]);
        }
        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        for value in [5u32, map.cells_x as u32, map.cells_z as u32, map.generations] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        old.extend(compress_to_vec(&shuffled, COMPRESSION));
        let back = decode(&old).unwrap();
        assert_eq!(back.resources[..3], map.resources[..3]);
        for layer in &back.resources[3..] {
            assert!(layer.len() == n && layer.iter().all(|&d| d == 0));
        }
    }

    #[test]
    fn maps_saved_before_sulphur_and_oil_load_with_their_other_resources_and_none() {
        let map = sample_map();
        let n = map.heights.len();
        let mut shuffled = vec![0u8; n * 12];
        for (layer, values) in [&map.heights, &map.water].into_iter().enumerate() {
            for (i, v) in values.iter().enumerate() {
                for (plane, byte) in v.to_le_bytes().into_iter().enumerate() {
                    shuffled[layer * n * 4 + plane * n + i] = byte;
                }
            }
        }
        for k in 0..4 {
            shuffled[n * (8 + k)..n * (9 + k)].copy_from_slice(&map.resources[k]);
        }
        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        for value in [6u32, map.cells_x as u32, map.cells_z as u32, map.generations] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        old.extend(compress_to_vec(&shuffled, COMPRESSION));
        let back = decode(&old).unwrap();
        assert_eq!(back.resources[..4], map.resources[..4]);
        for layer in &back.resources[4..] {
            assert!(layer.len() == n && layer.iter().all(|&d| d == 0));
        }
    }

    #[test]
    fn maps_saved_before_lakes_load_with_the_water_at_sea_level() {
        let map = sample_map();
        let n = map.heights.len();
        let mut shuffled = vec![0u8; n * 4];
        for (i, h) in map.heights.iter().enumerate() {
            for (plane, byte) in h.to_le_bytes().into_iter().enumerate() {
                shuffled[plane * n + i] = byte;
            }
        }
        let mut old = Vec::new();
        old.extend_from_slice(MAGIC);
        for value in [1u32, map.cells_x as u32, map.cells_z as u32, map.generations] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        old.extend(compress_to_vec(&shuffled, COMPRESSION));
        let back = decode(&old).unwrap();
        assert_eq!(back.heights, map.heights);
        assert!(back.water.iter().all(|&w| w == SEA_LEVEL));
    }

    #[test]
    fn mostly_flat_maps_compress_well() {
        let (cells_x, cells_z) = (256, 256);
        let map = SavedMap {
            cells_x,
            cells_z,
            generations: 0,
            heights: vec![DEEP_OCEAN; (cells_x + 1) * (cells_z + 1)],
            water: vec![SEA_LEVEL; (cells_x + 1) * (cells_z + 1)],
            resources: std::array::from_fn(|_| vec![0; (cells_x + 1) * (cells_z + 1)]),
        };
        let bytes = encode(&map);
        assert!(bytes.len() * 50 < map.heights.len() * 4, "{} bytes", bytes.len());
        assert_eq!(decode(&bytes).unwrap().heights, map.heights);
    }

    #[test]
    fn rejects_files_that_are_not_maps() {
        assert!(decode(b"").is_err());
        assert!(decode(b"hello, this is just some text").is_err());
        let mut bytes = encode(&sample_map());
        bytes[0] = b'X';
        assert!(decode(&bytes).unwrap_err().contains("not an NWorld map"));
    }

    #[test]
    fn rejects_newer_versions() {
        let mut bytes = encode(&sample_map());
        bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(decode(&bytes).unwrap_err().contains("newer"));
    }

    #[test]
    fn rejects_invalid_sizes() {
        for (x, z) in [(0u32, 80u32), (65, 80), (64, 100_000), (16, 16), (64, 4096)] {
            let mut bytes = encode(&sample_map());
            bytes[12..16].copy_from_slice(&x.to_le_bytes());
            bytes[16..20].copy_from_slice(&z.to_le_bytes());
            assert!(decode(&bytes).is_err(), "size {x} x {z} should be rejected");
        }
    }

    #[test]
    fn rejects_truncated_and_corrupt_data() {
        let bytes = encode(&sample_map());
        for cut in [HEADER_LEN, HEADER_LEN + 5, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode(&bytes[..cut]).is_err(), "cut at {cut} should be rejected");
        }
        let mut corrupt = bytes.clone();
        for b in &mut corrupt[HEADER_LEN + 8..] {
            *b = b.wrapping_add(0x5B);
        }
        assert!(decode(&corrupt).is_err());
    }

    #[test]
    fn rejects_heights_that_cannot_come_from_the_editor() {
        let mut map = sample_map();
        map.heights[10] = f32::NAN;
        assert!(decode(&encode(&map)).is_err());
        map.heights[10] = 1.0e9;
        assert!(decode(&encode(&map)).is_err());
        map.heights[10] = 1.0;
        map.water[10] = f32::NAN;
        assert!(decode(&encode(&map)).is_err());
        map.water[10] = -5.0;
        assert!(decode(&encode(&map)).is_err());
    }

    #[test]
    fn saving_replaces_the_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("nworld-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("map.nworld");
        fs::write(&path, b"old contents").unwrap();
        save(&path, &sample_map()).unwrap();
        assert_eq!(load(&path).unwrap().water, sample_map().water);
        assert!(!dir.join("map.nworld.tmp").exists());

        // A path in a folder that doesn't exist fails cleanly with a readable message.
        let missing = dir.join("no-such-folder").join("map.nworld");
        assert!(save(&missing, &sample_map()).unwrap_err().starts_with("Couldn't save"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
