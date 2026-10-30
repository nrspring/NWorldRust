//! NWorld, the game. It loads the map and lets you fly over it, and puts countries, people and cities
//! on it. Roads and units will be laid over the map from here too; none of that belongs to
//! `nworld-map`.

mod advance_popup;
mod alerts;
mod autosave;
mod calendar;
mod camera;
mod city;
mod city_layout;
mod city_mesh;
mod city_popup;
mod city_pick;
mod city_view;
mod color_picker;
mod country;
mod country_menu;
mod day;
mod format;
mod hud;
mod ids;
mod newsreel;
mod options;
mod overlays;
mod person;
mod placement;
mod popup;
mod rules;
mod rgb;
mod saves;
#[cfg(test)]
mod test_support;
mod text;
mod title_type;

use std::path::{Path, PathBuf};

use macroquad::prelude::*;
use nworld_map::regions::RegionId;
use nworld_map::storage;
use nworld_map::world::World;
use nworld_view::style::{Style, Weight};
use nworld_view::{mouse_ray, pick_terrain, MapView};

use advance_popup::AdvancePopup;
use alerts::AlertsUi;
use autosave::Autosave;
use camera::GameCamera;
use city_popup::CityPopup;
use city_view::CityRenderer;
use day::DaySave;
use country_menu::CountryMenu;
use options::{Options, OptionsUi};
use overlays::OverlaysUi;
use placement::ClickTracker;
use popup::Popup;
use rgb::Rgb;
use saves::Saves;

/// How far from a new city the camera flies to look at it, in meters.
const NEW_CITY_VIEW_DISTANCE: f32 = 190.0;

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
            // The person belongs to the player's country.
            Self::Person => Popup::person(day.player_country().map_or("your country", |c| c.name.as_str()), day.title_types.iter().map(|t| t.name.clone()).collect()),
        }
    }

    /// Adds what was typed in the popup to `day`.
    fn answer(&self, day: &mut DaySave, typed: &[String]) -> Result<(), String> {
        match self {
            Self::Country => day.found_country(&typed[0]),
            Self::Person => day.found_person(&typed[0], &typed[1], &typed[2]),
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
    let mut cities = CityRenderer::new();
    let mut clicks = ClickTracker::default();
    // The popup for the name of the city being founded, and where it is going.
    let mut naming_city: Option<(Vec2, Popup)> = None;
    let mut options = Options::default();
    let mut options_ui = OptionsUi::default();
    let mut overlays_ui = OverlaysUi::default();
    let mut alerts_ui = AlertsUi::default();
    let mut newsreel_panel = hud::NewsreelPanel::default();
    // The popup that asks whether to end the day.
    let mut advance_popup = AdvancePopup::default();
    let mut country_menu = CountryMenu::default();
    // The popup for the city that was clicked.
    let mut city_popup = CityPopup::default();
    // The owned regions the map is showing, as (region, color), to know when they change.
    let mut shown_owners: Vec<(u16, Rgb)> = Vec::new();
    let mut autosave = Autosave::new(&day, get_time());
    // Closing the window is a request, so the game can save before it goes.
    prevent_quit();

    loop {
        if is_quit_requested() {
            break;
        }
        if is_key_pressed(KeyCode::Escape) {
            // Esc first backs out of the countries menu, or closes the options, or gives up on naming a
            // city, and only then quits.
            if advance_popup.is_open() {
                advance_popup.close();
            } else if city_popup.is_open() {
                city_popup.escape();
            } else if country_menu.is_open() {
                country_menu.escape();
            } else if options_ui.is_open() || overlays_ui.is_open() {
                options_ui.close();
                overlays_ui.close();
            } else if asking.as_mut().is_some_and(|(_, popup)| popup.close_dropdown()) {
                // A drop-down list was open, and is closed.
            } else if naming_city.is_some() {
                naming_city = None;
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
        let modal = asking.is_some() || naming_city.is_some() || country_menu.is_open() || city_popup.is_open() || advance_popup.is_open();
        // The mouse is over the interface, and so not for the map, when it is on the header or the
        // options (which has the footer).
        let mouse = Vec2::from(mouse_position());
        let over_interface = hud::header_rect().contains(mouse) || options_ui.has_mouse(mouse) || overlays_ui.has_mouse(mouse) || alerts_ui.has_mouse(mouse) || newsreel_panel.has_mouse(mouse);
        if modal {
            camera.skip_frame();
        } else {
            camera.update(&world, dt, !over_interface);
        }
        let eye = camera.camera(&world);
        view.set_show_regions(options.show_regions);
        view.set_show_region_colors(options.show_country_areas);
        // Owned regions are tinted in their owners' colors, which is only uploaded when it changes.
        let owned = day.region_colors();
        if owned != shown_owners {
            let colors: Vec<(RegionId, [u8; 3])> = owned.iter().filter_map(|(region, c)| Some((RegionId::from_raw(*region)?, [c.r, c.g, c.b]))).collect();
            view.set_region_colors(&world, &colors);
            shown_owners = owned;
        }
        view.draw(&world, &eye, time);

        // Once there is a country and a person, the first city has to be placed.
        let placing = !modal && day.cities.is_empty();
        let site = if placing && !over_interface { pick_terrain(&eye, &world, mouse) } else { None };
        // Why the city can't go there: the ground, or the region belonging to another country.
        let problem = site.and_then(|at| {
            city::site_problem(&world, at.x, at.y).map(str::to_owned).or_else(|| {
                let country = day.player_country()?.id;
                day.city_region_problem(country, world.region_at(at.x, at.y).map(|r| r.raw()))
            })
        });
        cities.draw(&world, &day.cities, &eye);
        if let Some(at) = site {
            cities.draw_site(&world, at, problem.is_none(), &eye);
        }
        // The city under the mouse, if there is one, which the tooltip tells about.
        let hovered = if modal || naming_city.is_some() || over_interface { None } else { cities.city_under(mouse_ray(&eye, mouse), &day.cities) };

        // A click on the map (a drag is for the camera) places the first city, and after that opens the
        // popup of the city clicked.
        if clicks.update(!modal && !over_interface) {
            if placing {
                if let (Some(at), None) = (site, &problem) {
                    // The city is founded once it has a name.
                    naming_city = Some((at, Popup::city()));
                }
            } else if let Some(id) = hovered {
                options_ui.close();
                overlays_ui.close();
                city_popup.open(id);
            }
        }

        set_default_camera();
        // The header and the footer are there behind a popup too, but the keys are only for when it isn't up.
        let header = hud::header(&style, &day, !modal);
        if !modal && is_mouse_button_pressed(MouseButton::Left) {
            if header.country.is_some_and(|b| b.contains(mouse)) {
                options_ui.close();
                overlays_ui.close();
                country_menu.open(&day);
            } else if header.day.contains(mouse) {
                // Ending the day is asked about first.
                options_ui.close();
                overlays_ui.close();
                advance_popup.open(day.date);
            }
        }
        let footer = hud::footer(&style, !modal);
        autosave.update(&saves, &day, get_time());
        if let Some(message) = autosave.error() {
            hud::save_failed(&style, message);
        }
        if !modal {
            let newsreel = newsreel_panel.show(&style, &day);
            // An alert opens the countries menu at the country it is about.
            if let Some(country) = alerts_ui.show(&style, &day, newsreel) {
                options_ui.close();
                overlays_ui.close();
                country_menu.open_at(&day, Some(country));
            }
            if placing {
                hud::placing_prompt(&style, problem.as_deref());
            }
            if let Some(city) = hovered.and_then(|id| day.city(id)) {
                hud::city_tooltip(&style, &day, city);
            }
            // Only one of the two menus is open at a time: the one that was just opened wins.
            let (options_was_open, overlays_was_open) = (options_ui.is_open(), overlays_ui.is_open());
            options_ui.show(&style, footer, &mut options, view.has_regions());
            overlays_ui.show(&style, footer, &mut options, view.has_regions());
            if overlays_ui.is_open() && !overlays_was_open {
                options_ui.close();
            } else if options_ui.is_open() && !options_was_open {
                overlays_ui.close();
            }
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

        if let Some(choice) = country_menu.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match choice.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => {
                    day = updated;
                    country_menu.done(&choice, &day);
                }
                Err(message) => country_menu.fail(message),
            }
        }

        if advance_popup.show(&style) {
            // The day that ended is saved as it was, and the game goes on to the next.
            match saves.advance(&day) {
                Ok(next) => {
                    day = next;
                    autosave.reset(&day, get_time());
                    advance_popup.close();
                }
                Err(message) => advance_popup.fail(message),
            }
        }

        if let Some(change) = city_popup.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => city_popup.fail(message),
            }
        }

        let mut founded = false;
        if let Some((at, popup)) = naming_city.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the city if it is saved too.
                let mut updated = day.clone();
                match updated.found_city(&typed[0], at.x, at.y, world.region_at(at.x, at.y).map(|r| r.raw())).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        camera.fly_to(*at, NEW_CITY_VIEW_DISTANCE);
                        founded = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if founded {
            naming_city = None;
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
