//! NWorld, the game. For now it loads the map and lets you fly over it. Roads, cities and units
//! will be laid over the map from here; none of that belongs to `nworld-map`.

mod autosave;
mod calendar;
mod camera;
mod country;
mod day;
mod hud;
mod newsreel;
mod options;
mod person;
mod popup;
mod saves;
#[cfg(test)]
mod test_support;
mod text;

use std::path::{Path, PathBuf};

use macroquad::prelude::*;
use nworld_map::storage;
use nworld_map::world::World;
use nworld_view::style::{Style, Weight};
use nworld_view::MapView;

use autosave::Autosave;
use camera::GameCamera;
use day::DaySave;
use options::{Options, OptionsUi};
use popup::Popup;
use saves::Saves;

/// Where the game keeps its files, outside the repository: the maps under `Assets` and the saved
/// game under `Saves`.
const DATA_DIR: &str = r"C:\Users\nrspring\OneDrive\Secure\Projects\NWorld";

/// The map a new game is played on, made with the world editor, relative to the data folder.
const MAP_FILE: &str = "Assets/map.nworld";

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

/// The game as it is at startup.
struct Started {
    world: World,
    saves: Saves,
    /// The current day.
    day: DaySave,
}

/// Opens the saved game, starting a new one if there isn't one, and loads the map it is played on.
/// A map file can be given as the first argument to look at instead.
fn start() -> Result<Started, String> {
    let root = Path::new(DATA_DIR);
    let saves = Saves::new(root);
    let opened = saves.open(MAP_FILE)?;
    if opened.new_game {
        eprintln!("Started a new game on day {}.", opened.day.date);
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| root.join(&opened.day.map));
    let world = load_world(&path)?;
    Ok(Started { world, saves, day: opened.day })
}

/// Something the game has to ask the player for before play can begin.
enum Asking {
    Country,
    Person,
}

impl Asking {
    /// What is missing from `day`, if anything: first a country, then a person to belong to it.
    fn of(day: &DaySave) -> Option<Self> {
        if day.countries.is_empty() {
            Some(Self::Country)
        } else if day.people.is_empty() {
            Some(Self::Person)
        } else {
            None
        }
    }

    fn popup(&self, day: &DaySave) -> Popup {
        match self {
            Self::Country => Popup::country(),
            // The first country is the player's, and the person belongs to it.
            Self::Person => Popup::person(&day.countries[0].name),
        }
    }

    /// Adds what was typed in the popup to `day`.
    fn answer(&self, day: &mut DaySave, typed: &[String]) -> Result<(), String> {
        match self {
            Self::Country => day.found_country(&typed[0]),
            Self::Person => day.found_person(&typed[0], &typed[1]),
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let style = Style::load();
    let Started { world, saves, mut day } = match start() {
        Ok(started) => started,
        Err(message) => {
            eprintln!("{message}");
            show_error(&style, &message).await;
            return;
        }
    };

    let mut camera = GameCamera::new(&world);
    let mut view = MapView::new(&world, camera.camera(&world).position);
    let mut time = 0.0f32;
    // The popup for what the game is asking for, while it is.
    let mut asking: Option<(Asking, Popup)> = None;
    let mut options = Options::default();
    let mut options_ui = OptionsUi::default();
    let mut autosave = Autosave::new(&day, get_time());
    // Closing the window is a request, so the game can save before it goes.
    prevent_quit();

    loop {
        if is_quit_requested() {
            break;
        }
        if is_key_pressed(KeyCode::Escape) {
            // Esc first closes the options, and only then quits.
            if options_ui.is_open() {
                options_ui.close();
            } else {
                break;
            }
        }
        let dt = get_frame_time();
        time += dt;

        if asking.is_none() {
            asking = Asking::of(&day).map(|kind| {
                let popup = kind.popup(&day);
                (kind, popup)
            });
        }

        // While a popup is up, typing is for it and not for the camera.
        if asking.is_none() {
            camera.update(&world, dt, !options_ui.has_mouse(Vec2::from(mouse_position())));
        }
        let eye = camera.camera(&world);
        view.set_show_regions(options.show_regions);
        view.draw(&world, &eye, time);

        set_default_camera();
        let bar = hud::top_bar(&style, day.date);
        autosave.update(&saves, &day, get_time());
        if let Some(message) = autosave.error() {
            hud::save_failed(&style, message);
        }
        if asking.is_none() {
            hud::newsreel(&style, day.date, &day.newsreel);
            hud::help(&style);
            options_ui.show(&style, bar, &mut options, view.has_regions());
        }

        let mut answered = false;
        if let Some((kind, popup)) = asking.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the answer if it is saved too.
                let mut updated = day.clone();
                match kind.answer(&mut updated, &typed).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        answered = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if answered {
            asking = None;
        }
        next_frame().await;
    }

    autosave.flush(&saves, &day);
    if let Some(message) = autosave.error() {
        eprintln!("{message}");
    }
}

/// Shows why the game couldn't start, until the window is closed.
async fn show_error(style: &Style, message: &str) {
    loop {
        if is_key_pressed(KeyCode::Escape) {
            return;
        }
        clear_background(Color::new(0.1, 0.1, 0.12, 1.0));
        style.text(message, 24.0, 48.0, 22, Weight::Regular, WHITE);
        style.text("Press Esc to quit.", 24.0, 80.0, 18, Weight::Regular, GRAY);
        next_frame().await;
    }
}
