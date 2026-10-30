//! NWorld, the game. For now it loads the map and lets you fly over it, asking the map what is
//! under the mouse. Roads, cities and units will be laid over the map from here; none of that
//! belongs to `nworld-map`.

mod calendar;
mod camera;
mod saves;

use std::path::{Path, PathBuf};

use macroquad::prelude::*;
use nworld_map::storage;
use nworld_map::world::{Resource, WaterClass, World};
use nworld_view::{pick_terrain, MapView};

use calendar::Date;
use camera::GameCamera;
use saves::Saves;

/// Where the game keeps its files, outside the repository: the maps under `Assets` and the saved
/// game under `Saves`.
const DATA_DIR: &str = r"C:\Users\nrspring\OneDrive\Secure\Projects\NWorld";

/// The map a new game is played on, made with the world editor, relative to the data folder.
const MAP_FILE: &str = "Assets/map.nworld";

/// How far around the mouse to look for resources, in meters.
const NEARBY_RADIUS: f32 = 100.0;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: "NWorld".to_owned(),
            window_width: 1600,
            window_height: 900,
            sample_count: 4,
            ..Default::default()
        },
        // Batches share u16 indices, so a draw call must stay under 65536 vertices.
        draw_call_vertex_capacity: 65_000,
        draw_call_index_capacity: 240_000,
        ..Default::default()
    }
}

fn load_world(path: &Path) -> Result<World, String> {
    let saved = storage::load(path).map_err(|e| format!("Couldn't load {}: {e}", path.display()))?;
    World::from_saved(saved)
}

/// Opens the saved game, starting a new one if there isn't one, and loads the map it is played on.
/// A map file can be given as the first argument to look at instead.
fn start() -> Result<(World, PathBuf, Date), String> {
    let root = Path::new(DATA_DIR);
    let opened = Saves::new(root).open(MAP_FILE)?;
    if opened.new_game {
        eprintln!("Started a new game on day {}.", opened.day.date);
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| root.join(&opened.day.map));
    let world = load_world(&path)?;
    Ok((world, path, opened.day.date))
}

#[macroquad::main(window_conf)]
async fn main() {
    let (world, path, date) = match start() {
        Ok(started) => started,
        Err(message) => {
            eprintln!("{message}");
            show_error(&message).await;
            return;
        }
    };

    let mut camera = GameCamera::new(&world);
    let mut view = MapView::new(&world, camera.camera(&world).position);
    let mut time = 0.0f32;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        let dt = get_frame_time();
        time += dt;

        camera.update(&world, dt);
        let eye = camera.camera(&world);
        view.draw(&world, &eye, time);

        set_default_camera();
        let under_mouse = pick_terrain(&eye, &world, Vec2::from(mouse_position()));
        draw_hud(&world, &path, date, under_mouse);
        next_frame().await;
    }
}

/// Shows why the map couldn't be loaded until the window is closed.
async fn show_error(message: &str) {
    loop {
        if is_key_pressed(KeyCode::Escape) {
            return;
        }
        clear_background(Color::new(0.1, 0.1, 0.12, 1.0));
        text(message, 24.0, 48.0, 24.0, WHITE);
        text("Press Esc to quit.", 24.0, 80.0, 20.0, GRAY);
        next_frame().await;
    }
}

/// Text with a dark edge so it reads over any ground.
fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(s, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.8));
    draw_text(s, x, y, size, color);
}

fn draw_hud(world: &World, path: &Path, date: Date, under_mouse: Option<Vec2>) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let size = world.size();
    text(&format!("Day {date}   {name}  {:.0} x {:.0} m   {} fps", size.x, size.y, get_fps()), 16.0, 28.0, 22.0, WHITE);
    text("WASD pan   Q/E turn   wheel zoom   right-drag look   left-drag grab   Shift fast   Esc quit", 16.0, screen_height() - 16.0, 18.0, LIGHTGRAY);

    // Past the edge of the map there is only the backdrop, which the map knows nothing about.
    let half = world.half_extents();
    let Some(at) = under_mouse.filter(|at| at.x.abs() <= half.x && at.y.abs() <= half.y) else { return };
    let mut lines = vec![format!("{:.0}, {:.0}   {:.0} m high", at.x, at.y, world.surface_height(at.x, at.y))];
    lines.push(
        match world.water_class(at.x, at.y) {
            WaterClass::Dry => "Dry land",
            WaterClass::Inland => "Inland water",
            WaterClass::Shallow => "Shallow water",
            WaterClass::Deep => "Deep water",
        }
        .to_owned(),
    );
    if let Some(region) = world.region_at(at.x, at.y) {
        lines.push(format!("Region {:?} {}", region.kind(), region.raw() & 0x3fff));
    }
    let circle = std::f32::consts::PI * NEARBY_RADIUS * NEARBY_RADIUS;
    for kind in Resource::ALL {
        let cover = world.resource_near(kind, at.x, at.y, NEARBY_RADIUS) / circle;
        if cover >= 0.05 {
            lines.push(format!("{kind:?} {:.0}% within {NEARBY_RADIUS:.0} m", cover * 100.0));
        }
    }

    let mouse = Vec2::from(mouse_position());
    let (width, height) = (240.0, 12.0 + 20.0 * lines.len() as f32);
    let x = (mouse.x + 18.0).min(screen_width() - width - 8.0);
    let y = (mouse.y + 18.0).min(screen_height() - height - 8.0);
    draw_rectangle(x, y, width, height, Color::new(0.05, 0.06, 0.08, 0.75));
    for (k, line) in lines.iter().enumerate() {
        text(line, x + 8.0, y + 22.0 + 20.0 * k as f32, 18.0, WHITE);
    }
}
