//! The map crate has to stay free of anything to do with drawing, so that another application can
//! use it without taking a graphics library, a window or a GPU along. These tests fail the moment
//! one creeps in.

use std::fs;
use std::path::Path;

/// Libraries that draw, open windows or show dialogs.
const DISPLAY_LIBRARIES: [&str; 10] = ["macroquad", "miniquad", "rfd", "wgpu", "winit", "glutin", "sdl2", "bevy", "egui", "raylib"];

/// Names from those libraries that would mean the code is drawing.
const DISPLAY_TERMS: [&str; 8] = ["macroquad", "miniquad", "get_internal_gl", "Texture2D", "Camera3D", "RenderingBackend", "BufferId", "screen_width"];

#[test]
fn the_manifest_depends_on_no_graphics_library() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for library in DISPLAY_LIBRARIES {
        assert!(!manifest.contains(library), "Cargo.toml mentions {library}");
    }
}

fn rust_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_source_file_uses_anything_that_draws() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 10, "found only {} source files", files.len());
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for term in DISPLAY_TERMS {
            assert!(!text.contains(term), "{} mentions {term}", file.display());
        }
    }
}
