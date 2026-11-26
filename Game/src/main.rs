//! NWorld, the game. It loads the map and lets you fly over it, and puts countries, people and cities
//! on it. Roads and units will be laid over the map from here too; none of that belongs to
//! `nworld-map`.

mod advance_popup;
mod alerts;
mod idle_units;
mod upkeep;
mod upkeep_popup;
mod autosave;
mod bridge;
mod bridge_tool;
mod claim_prompt;
mod destroy_tool;
mod connection;
mod costs;
mod gatherer;
mod gatherer_popup;
mod gatherer_tool;
mod ship;
mod ship_popup;
mod ship_view;
mod barracks;
mod barracks_popup;
mod barracks_tool;
mod barracks_view;
mod shipyard;
mod shipyard_popup;
mod shipyard_tool;
mod shipyard_view;
mod gatherer_view;
mod bridge_view;
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
mod country_dropdown;
mod country_menu;
mod day;
mod failure;
mod format;
mod ground;
mod hud;
mod ids;
mod militia;
mod militia_popup;
mod movement;
mod movement_mesh;
mod movement_view;
mod militia_view;
mod newsreel;
mod resource_hover;
mod transfer;
mod treasury;
mod options;
mod path;
mod path_tool;
mod path_view;
mod miniatures;
mod model;
mod pieces;
mod overlays;
mod greyscale;
mod people_dropdown;
mod population_view;
mod person;
mod person_popup;
mod person_view;
mod placement;
mod popup;
mod region_labels;
mod region_name;
mod lake_name;
mod place;
mod river_name;
mod rules;
mod rgb;
mod saves;
#[cfg(test)]
mod test_support;
mod text;
mod unit_bubbles;
mod unit_regions;
mod walk;
mod title_type;
mod toolbar;

use std::path::{Path, PathBuf};

use macroquad::prelude::*;
use nworld_map::region_map::RegionMap;
use nworld_map::regions::RegionId;
use nworld_map::storage;
use nworld_map::water_bodies::WaterBodies;
use nworld_map::world::{World, CELL_SIZE};
use nworld_view::style::{Style, Weight};
use nworld_view::{mouse_ray, pick_terrain, MapView};

use advance_popup::AdvancePopup;
use alerts::{AlertTarget, AlertsUi};
use idle_units::IdleUnitsUi;
use autosave::Autosave;
use camera::GameCamera;
use city_popup::CityPopup;
use city_view::CityRenderer;
use day::DaySave;
use country_dropdown::{CountryDropdown, Pick};
use country_menu::CountryMenu;
use treasury::Treasury;
use options::{Options, OptionsUi};
use bridge_tool::BridgeTool;
use claim_prompt::ClaimPrompt;
use destroy_tool::DestroyTool;
use gatherer::GathererKind;
use gatherer_popup::GathererPopup;
use militia_popup::MilitiaPopup;
use movement_view::{MovementView, PanelAction};
use people_dropdown::{PeopleDropdown, PeoplePick};
use person_popup::PersonPopup;
use walk::Walks;
use gatherer_tool::GathererTool;
use ship_popup::ShipPopup;
use upkeep_popup::UpkeepPopup;
use barracks_popup::BarracksPopup;
use barracks::BarracksKind;
use barracks_tool::BarracksTool;
use shipyard::ShipyardKind;
use shipyard_popup::ShipyardPopup;
use shipyard_tool::ShipyardTool;
use bridge_view::BridgeRenderer;
use path::Kind;
use path_tool::{PathTool, Scene};
use path_view::PathRenderer;
use overlays::OverlaysUi;
use placement::ClickTracker;
use popup::Popup;
use rgb::Rgb;
use saves::Saves;
use toolbar::{Menu, Toolbar};

/// How far from a new city the camera flies to look at it, in meters.
const NEW_CITY_VIEW_DISTANCE: f32 = 190.0;
/// How far from a region the camera is taken to look at it, in meters.
const REGION_VIEW_DISTANCE: f32 = 350.0;

/// Where the game keeps its files, outside the repository: the maps under `Assets` and the saved
/// game under `Saves`.
const DATA_DIR: &str = r"C:\Users\nrspring\OneDrive\Secure\Projects\NWorld";

/// The map a new game is played on, made with the world editor, relative to the data folder.
const MAP_FILE: &str = "Assets/map.nworld";

const WINDOW_TITLE: &str = "NWorld";

/// Maximizes the window, which is opened at a size, since the window settings can't ask for it to be maximized. It is
/// found by its title.
#[cfg(windows)]
fn maximize_window() {
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut std::ffi::c_void;
        fn ShowWindow(window: *mut std::ffi::c_void, command: i32) -> i32;
    }
    const SW_MAXIMIZE: i32 = 3;
    let title: Vec<u16> = WINDOW_TITLE.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both are plain Win32 calls, and the title is null-terminated and outlives the call.
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if !window.is_null() {
            ShowWindow(window, SW_MAXIMIZE);
        }
    }
}

#[cfg(not(windows))]
fn maximize_window() {}

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: WINDOW_TITLE.to_owned(),
            window_width: 1600,
            window_height: 900,
            // An ordinary bordered window, so it can be minimized, maximized and resized.
            window_resizable: true,
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
    /// What is missing from `day`, if anything: first a country, then a first person to belong to it.
    fn of(day: &DaySave) -> Option<Self> {
        if day.countries.is_empty() {
            Some(Self::Country)
        } else if day.people.is_empty() && day.title_types.is_empty() {
            // (There are kinds of title once there has been a person, who may since have died.)
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
    maximize_window();
    let style = Style::load();
    let Started { world, saves, mut day } = match start() {
        Ok(started) => started,
        Err(message) => {
            eprintln!("{message}");
            show_error(&style, &message).await;
            return;
        }
    };

    // Where the regions are and how they meet, for the paths.
    let region_map = RegionMap::build(&world);
    // The regions with what each kind of gatherer gathers in them, which is where it can go.
    let deposits = gatherer::Deposits::of(&world);
    let water_bodies = WaterBodies::build(&world);
    let mut camera = GameCamera::new(&world);
    let mut view = MapView::new(&world, camera.camera(&world).position);
    let mut time = 0.0f32;
    // The popup for what the game is asking for, while it is.
    let mut asking: Option<(Asking, Popup)> = None;
    let mut cities = CityRenderer::new();
    let mut clicks = ClickTracker::default();
    // The popup for the name of the city being founded, and where it is going.
    let mut naming_city: Option<(Vec2, Popup)> = None;
    // The city that the people for a new city are being taken from, while the player chooses where it goes.
    let mut settling_from: Option<ids::CityId> = None;
    // The person being put on the map by a click, and why the last click couldn't be kept.
    let mut placing_person: Option<ids::PersonId> = None;
    let mut placing_error: Option<String> = None;
    // The popup for the name of the region that was clicked while the region names are showing, and its number.
    let mut naming_region: Option<(u16, Popup)> = None;
    // The same for a river or a lake: its number, whether it is a lake, and the popup.
    let mut naming_water: Option<(u32, bool, Popup)> = None;
    // The same for a place: its number if it is already there, where it goes if it is new, and the popup.
    let mut naming_place: Option<(Option<u32>, Vec2, Popup)> = None;
    // The popup for a note to add to the newsreel.
    let mut noting: Option<Popup> = None;
    // The popup for another person to introduce to the country played, while it is up.
    let mut introducing: Option<Popup> = None;
    // The treasury of the country played, which is also where it sends money and resources to another.
    let mut treasury = Treasury::default();
    // The question of whether to claim a region that something was just placed in.
    let mut claim_prompt = ClaimPrompt::default();
    let mut options = Options::default();
    let mut options_ui = OptionsUi::default();
    let mut overlays_ui = OverlaysUi::default();
    // The Travel and Buildings menus of tools.
    let mut toolbar = Toolbar::default();
    let mut alerts_ui = AlertsUi::default();
    let mut idle_units_ui = IdleUnitsUi::default();
    // What failed when the last day ended, which the alerts report. Not saved: a game opened again has none.
    let mut day_failures: Vec<failure::Thing> = Vec::new();
    let mut newsreel_panel = hud::NewsreelPanel::default();
    let mut path_tool = PathTool::new(Kind::Path);
    let mut paths = PathRenderer::new(Kind::Path);
    let mut road_tool = PathTool::new(Kind::Road);
    let mut roads = PathRenderer::new(Kind::Road);
    let mut bridge_tool = BridgeTool::default();
    let mut destroy_tool = DestroyTool::default();
    // The tool for each kind of gatherer, in the order of the kinds.
    let mut gatherer_tools = GathererKind::ALL.map(GathererTool::new);
    let mut shipyard_tools = ShipyardKind::BUILDABLE.map(ShipyardTool::new);
    let mut barracks_tools = BarracksKind::ALL.map(BarracksTool::new);
    let mut barracks_popup = BarracksPopup::default();
    let mut bridges = BridgeRenderer::default();
    // The popup that asks whether to end the day.
    let mut advance_popup = AdvancePopup::default();
    let mut country_menu = CountryMenu::default();
    let mut country_dropdown = CountryDropdown::default();
    let mut people_dropdown = PeopleDropdown::default();
    // The popup for the city that was clicked.
    let mut city_popup = CityPopup::default();
    // The popup for the gatherer that was clicked.
    let mut gatherer_popup = GathererPopup::default();
    let mut shipyard_popup = ShipyardPopup::default();
    // The popup for the militia that was clicked.
    let mut militia_popup = MilitiaPopup::default();
    // The popup for a new name for a militia: its id, and the popup.
    let mut naming_militia: Option<(ids::MilitiaId, Popup)> = None;
    // The militia that was clicked, which shows where it can go, and why the last move couldn't be made.
    let mut selected_militia: Option<ids::MilitiaId> = None;
    // The popup for the ship that was clicked, the popup for a new name for a ship, and the ship that was clicked, which shows where it
    // can go. (Only one militia or ship is selected at a time.)
    let mut ship_popup = ShipPopup::default();
    let mut upkeep_popup = UpkeepPopup::default();
    let mut naming_ship: Option<(ids::ShipId, Popup)> = None;
    let mut selected_ship: Option<ids::ShipId> = None;
    let mut selected_person: Option<ids::PersonId> = None;
    let mut person_popup = PersonPopup::default();
    let mut movement_view = MovementView::default();
    let mut move_error: Option<String> = None;
    // The militia that are walking to where they were moved, which is only for showing.
    let mut walks = Walks::default();
    // The owned regions the map is showing, as (region, color), to know when they change.
    let mut shown_owners: Vec<(u16, Rgb)> = Vec::new();
    // The regions glowing now, which is the ones with units in them while the mouse is on Units in the footer (as of the last frame),
    // and which is only uploaded when it changes.
    let mut glowing: Vec<(u16, [u8; 3])> = Vec::new();
    let mut label_hovered: Option<hud::FooterLabel> = None;
    let mut autosave = Autosave::new(&day, get_time());
    // What the routes of the gatherers were last worked out from.
    let mut route_watch = connection::RouteWatch::default();
    // Closing the window is a request, so the game can save before it goes.
    prevent_quit();

    loop {
        if is_quit_requested() {
            break;
        }
        if is_key_pressed(KeyCode::Escape) {
            // Esc first backs out of the countries menu, or closes the options, or gives up on naming a
            // city, and only then quits.
            if claim_prompt.is_open() {
                claim_prompt.escape();
            } else if treasury.is_open() {
                treasury.escape();
            } else if advance_popup.is_open() {
                advance_popup.close();
            } else if city_popup.is_open() {
                city_popup.escape();
            } else if gatherer_popup.is_open() {
                gatherer_popup.escape();
            } else if shipyard_popup.is_open() {
                shipyard_popup.escape();
            } else if barracks_popup.is_open() {
                barracks_popup.escape();
            } else if person_popup.is_open() {
                person_popup.escape();
            } else if militia_popup.is_open() {
                militia_popup.escape();
            } else if ship_popup.is_open() {
                ship_popup.escape();
            } else if upkeep_popup.is_open() {
                upkeep_popup.close();
            } else if country_menu.is_open() {
                country_menu.escape();
            } else if country_dropdown.is_open() {
                country_dropdown.close();
            } else if people_dropdown.is_open() {
                people_dropdown.close();
            } else if options_ui.is_open() || overlays_ui.is_open() || toolbar.is_open() {
                options_ui.close();
                overlays_ui.close();
                toolbar.close();
            } else if asking.as_mut().is_some_and(|(_, popup)| popup.close_dropdown()) || introducing.as_mut().is_some_and(Popup::close_dropdown) {
                // A drop-down list was open, and is closed.
            } else if naming_city.is_some() {
                naming_city = None;
            } else if naming_region.is_some() {
                naming_region = None;
            } else if naming_water.is_some() {
                naming_water = None;
            } else if naming_place.is_some() {
                naming_place = None;
            } else if naming_militia.is_some() {
                naming_militia = None;
            } else if naming_ship.is_some() {
                naming_ship = None;
            } else if placing_person.is_some() {
                placing_person = None;
            } else if settling_from.is_some() {
                settling_from = None;
            } else if selected_militia.is_some() {
                selected_militia = None;
            } else if selected_ship.is_some() {
                selected_ship = None;
            } else if selected_person.is_some() {
                selected_person = None;
            } else if noting.is_some() {
                noting = None;
            } else if introducing.is_some() {
                introducing = None;
            } else if path_tool.is_active() {
                path_tool.escape();
            } else if road_tool.is_active() {
                road_tool.escape();
            } else if bridge_tool.is_active() {
                bridge_tool.escape();
            } else if destroy_tool.is_active() {
                destroy_tool.escape();
            } else if let Some(tool) = gatherer_tools.iter_mut().find(|tool| tool.is_active()) {
                tool.escape();
            } else if let Some(tool) = shipyard_tools.iter_mut().find(|tool| tool.is_active()) {
                tool.escape();
            } else if let Some(tool) = barracks_tools.iter_mut().find(|tool| tool.is_active()) {
                tool.escape();
            } else {
                break;
            }
        }
        let dt = get_frame_time();
        time += dt;
        // A gatherer that has been cut off from a city, or has got a route to one, since last looked at is news. This is
        // not saved at once: the autosave finds the change. The routes are only worked out when something they depend on
        // has changed.
        route_watch.update(&mut day);
        // When another country is played, by whatever way it was switched, nobody is picked any more.
        if people_dropdown.country_changed(&day) {
            selected_person = None;
            placing_person = None;
        }

        if asking.is_none() {
            asking = Asking::of(&day).map(|kind| {
                let popup = kind.popup(&day);
                (kind, popup)
            });
        }

        // While a popup is up, typing is for it and not for the camera.
        let modal = asking.is_some() || naming_city.is_some() || naming_region.is_some() || naming_water.is_some() || naming_place.is_some() || naming_militia.is_some() || naming_ship.is_some() || noting.is_some() || introducing.is_some() || claim_prompt.is_open() || treasury.is_open() || country_menu.is_open() || city_popup.is_open() || gatherer_popup.is_open() || shipyard_popup.is_open() || barracks_popup.is_open() || person_popup.is_open() || militia_popup.is_open() || ship_popup.is_open() || upkeep_popup.is_open() || advance_popup.is_open();
        if modal {
            toolbar.close();
        }
        // The mouse is over the interface, and so not for the map, when it is on the header or the
        // options (which has the footer).
        let mouse = Vec2::from(mouse_position());
        let over_interface = hud::header_rect().contains(mouse) || options_ui.has_mouse(mouse) || overlays_ui.has_mouse(mouse) || toolbar.has_mouse(mouse) || alerts_ui.has_mouse(mouse) || idle_units_ui.has_mouse(mouse) || country_dropdown.has_mouse(mouse) || people_dropdown.has_mouse(mouse) || newsreel_panel.has_mouse(mouse) || path_tool.has_mouse(mouse) || road_tool.has_mouse(mouse) || bridge_tool.has_mouse(mouse) || destroy_tool.has_mouse(mouse) || gatherer_tools.iter().any(|tool| tool.has_mouse(mouse)) || shipyard_tools.iter().any(|tool| tool.has_mouse(mouse)) || barracks_tools.iter().any(|tool| tool.has_mouse(mouse)) || ((selected_militia.is_some() || selected_ship.is_some() || selected_person.is_some()) && movement_view::panel_rect().contains(mouse));
        if modal {
            camera.skip_frame();
        } else {
            camera.update(&world, dt, !over_interface);
        }
        let eye = camera.camera(&world);
        // Where the mouse is on the ground, found the first time something asks this frame and kept for the rest of it:
        // following the ray down to the ground takes many steps, and several things want to know.
        let ground_under_mouse = std::cell::OnceCell::new();
        let ground_at = || *ground_under_mouse.get_or_init(|| pick_terrain(&eye, &world, mouse));
        view.set_show_regions(options.show_regions);
        view.set_show_region_colors(options.show_country_areas);
        // Owned regions are tinted in their owners' colors, which is only uploaded when it changes.
        let owned = day.region_colors();
        if owned != shown_owners {
            let colors: Vec<(RegionId, [u8; 3])> = owned.iter().filter_map(|(region, c)| Some((RegionId::from_raw(*region)?, [c.r, c.g, c.b]))).collect();
            view.set_region_colors(&world, &colors);
            shown_owners = owned;
        }
        let glow: Vec<(RegionId, [u8; 3])> = match label_hovered {
            Some(hud::FooterLabel::Units) => {
                // The regions where a unit still has moves left glow another color from those where all have used theirs.
                let moving = unit_regions::with_units_that_can_move(&world, &day, unit_regions::Whose::Mine);
                let color = |region: &RegionId| if moving.contains(region) { hud::MOVES_GLOW } else { hud::FooterLabel::Units.glow() };
                unit_regions::with_units(&world, &day, unit_regions::Whose::Mine).iter().map(|r| (*r, color(r))).collect()
            }
            Some(label @ hud::FooterLabel::People) => unit_regions::with_people(&world, &day).into_iter().map(|r| (r, label.glow())).collect(),
            Some(label @ hud::FooterLabel::Enemy) => unit_regions::with_units(&world, &day, unit_regions::Whose::Enemy).into_iter().map(|r| (r, label.glow())).collect(),
            Some(label @ hud::FooterLabel::Alerts) => alerts::regions(&day, &day_failures, &region_map, &world).into_iter().map(|r| (r, label.glow())).collect(),
            // Population lights up no region: it has circles over the cities, on a map that is grey all over.
            Some(hud::FooterLabel::Population) | None => Vec::new(),
        };
        let shown: Vec<(u16, [u8; 3])> = glow.iter().map(|(r, c)| (r.raw(), *c)).collect();
        if shown != glowing {
            view.set_glowing_regions(&world, &glow);
            glowing = shown;
        }
        // While a label is hovered, all but the regions it lights up is shown in grey.
        view.set_greyscale(label_hovered.is_some());
        let lit: Vec<RegionId> = glow.iter().map(|(region, _)| *region).collect();
        greyscale::set(label_hovered.map(|_| lit.as_slice()));
        view.draw(&world, &eye, time);

        // Once there is a country and a person, the first city has to be placed.
        // Why no path can be built, which switches the tool off.
        let path_problem = day.path_problem();
        let road_problem = day.road_problem();
        let no_regions = region_map.is_empty();
        if path_problem.is_some() || no_regions {
            path_tool.deactivate();
        }
        if road_problem.is_some() || no_regions {
            road_tool.deactivate();
        }
        let bridge_problem = day.bridge_problem();
        if bridge_problem.is_some() {
            bridge_tool.deactivate();
        }
        for tool in &mut gatherer_tools {
            if day.gatherer_problem(tool.kind()).is_some() {
                tool.deactivate();
            }
        }
        for tool in &mut shipyard_tools {
            if day.shipyard_problem(tool.kind()).is_some() {
                tool.deactivate();
            }
        }
        for tool in &mut barracks_tools {
            if day.barracks_problem(tool.kind()).is_some() {
                tool.deactivate();
            }
        }
        // Whether one of the tools is on, and so the map is not for anything else.
        let tool_on = path_tool.is_active() || road_tool.is_active() || bridge_tool.is_active() || destroy_tool.is_active() || gatherer_tools.iter().any(|tool| tool.is_active()) || shipyard_tools.iter().any(|tool| tool.is_active()) || barracks_tools.iter().any(|tool| tool.is_active());
        // A new city that can no longer be paid for, or settled from the city it was to be, is given up.
        if settling_from.is_some_and(|from| day.settle_problem(from).is_some()) {
            settling_from = None;
        }
        // A person being put on the map comes before a city that is to be placed.
        if placing_person.is_some_and(|id| day.person(id).is_none_or(|p| p.at.is_some())) {
            placing_person = None;
        }
        if placing_person.is_none() {
            placing_error = None;
        }
        let person_to_place = placing_person.filter(|_| !modal && !tool_on);
        let placing = !modal && (day.cities.is_empty() || settling_from.is_some()) && !tool_on && placing_person.is_none();
        let site = if placing && !over_interface { ground_at() } else { None };
        // Why the city can't go there: the ground, or the region belonging to another country.
        let problem = site.and_then(|at| {
            city::site_problem(&world, at.x, at.y).map(str::to_owned).or_else(|| {
                let country = day.player_country()?.id;
                day.city_region_problem(country, world.region_at(at.x, at.y).map(|r| r.raw()))
            })
            .or_else(|| day.crowding_problem(at.x, at.y))
        });
        // The cities can be taken away to see what is in them.
        if options.show_cities {
            cities.draw(&world, &day.cities, &eye);
        }
        paths.draw(&world, &region_map, &day);
        roads.draw(&world, &region_map, &day);
        bridges.draw(&world, &day);
        gatherer_view::draw(&world, &day, eye.position, time);
        shipyard_view::draw(&world, &day, eye.position, time);
        barracks_view::draw(&world, &day, eye.position, time);
        walks.update(dt);
        militia_view::draw(&world, &day, &walks, time, eye.position);
        // Where the militia that was clicked can go, tinted over everything else on the map, once it has got where it is going.
        if selected_militia.is_some_and(|id| day.militia(id).is_none()) {
            selected_militia = None;
        }
        ship_view::draw(&world, &day, &walks, time, eye.position);
        if selected_ship.is_some_and(|id| day.ship(id).is_none()) {
            selected_ship = None;
        }
        person_view::draw(&world, &day, &walks, time, eye.position);
        if selected_person.is_some_and(|id| day.person(id).is_none()) {
            selected_person = None;
        }
        // The overlay is for the militia, the ship or the person that is selected, whichever it is, once it has got where it is going.
        if selected_ship.is_some() {
            movement_view.update_ship(&world, &day, selected_ship.filter(|id| !walks.is_walking(*id)));
        } else if selected_person.is_some() {
            movement_view.update_person(&world, &day, &paths, &roads, selected_person.filter(|id| !walks.is_walking(*id)));
        } else {
            let at_rest = selected_militia.filter(|id| !walks.is_walking(*id));
            movement_view.update(&world, &day, &paths, &roads, at_rest);
        }
        movement_view.draw();
        if let Some(at) = site {
            cities.draw_site(&world, at, problem.is_none(), &eye);
        }
        // Where the person being put on the map would go, and why they can't, if they can't. They are shown there in their country's
        // color, or in red where they can't go.
        let person_site = if person_to_place.is_some() && !over_interface { ground_at() } else { None };
        let person_problem = person_site.zip(person_to_place).and_then(|(at, id)| {
            person::ground_problem(&world, at.x, at.y).map(str::to_owned).or_else(|| day.person_spot_problem(id, at, world.region_at(at.x, at.y).map(|r| r.raw())))
        });
        if let Some((at, person)) = person_site.zip(person_to_place.and_then(|id| day.person(id))) {
            // (A nomad is gray.)
            let own = person.country.and_then(|id| day.country(id)).map_or(GRAY, |c| c.color.color());
            let color = if person_problem.is_none() { own } else { RED };
            person_view::draw_ghost(&world, at, color, time, eye.position);
        }
        // The city under the mouse, if there is one, which the tooltip tells about. One that isn't shown is not in the way of what is in it.
        let hovered = if !options.show_cities || modal || naming_city.is_some() || over_interface || tool_on { None } else { cities.city_under(mouse_ray(&eye, mouse), &day.cities) };

        // The person under the mouse, if there is one and no city, militia or ship is, which the tooltip tells about. They come before
        // the buildings, which they are shown in front of.
        let unit_in_the_way = || militia_view::under_ray(mouse_ray(&eye, mouse), &world, &day.militias, &walks).is_some() || ship_view::under_ray(mouse_ray(&eye, mouse), &world, &day.ships, &walks).is_some();
        let person_hover = if modal || over_interface || tool_on || hovered.is_some() { None } else { person_view::under_ray(mouse_ray(&eye, mouse), &world, &day, &walks).filter(|_| !unit_in_the_way()) };

        // The gatherer under the mouse, if there is one and no city or person is in front of it, which the tooltip tells about.
        let gatherer_hover = if modal || over_interface || tool_on || hovered.is_some() || person_hover.is_some() { None } else { gatherer_view::under_ray(mouse_ray(&eye, mouse), &world, &day.gatherers) };

        // The shipyard under the mouse, if there is one and no city or gatherer is in front of it, which the tooltip tells about.
        let shipyard_hover = if modal || over_interface || tool_on || hovered.is_some() || person_hover.is_some() || gatherer_hover.is_some() { None } else { shipyard_view::under_ray(mouse_ray(&eye, mouse), &world, &day.shipyards) };

        // The barracks under the mouse, if there is one and nothing is in front of it, which the tooltip tells about.
        let barracks_hover = if modal || over_interface || tool_on || hovered.is_some() || person_hover.is_some() || gatherer_hover.is_some() || shipyard_hover.is_some() { None } else { barracks_view::under_ray(mouse_ray(&eye, mouse), &world, &day.barracks) };

        // The militia under the mouse, if there is one and no city or gatherer is in front of it, which the tooltip tells about.
        let militia_hover = if modal || over_interface || tool_on || hovered.is_some() || gatherer_hover.is_some() || shipyard_hover.is_some() || barracks_hover.is_some() { None } else { militia_view::under_ray(mouse_ray(&eye, mouse), &world, &day.militias, &walks) };

        // The ship under the mouse, if there is one and nothing else is in front of it, which the tooltip tells about.
        let ship_hover = if modal || over_interface || tool_on || hovered.is_some() || gatherer_hover.is_some() || shipyard_hover.is_some() || barracks_hover.is_some() || militia_hover.is_some() { None } else { ship_view::under_ray(mouse_ray(&eye, mouse), &world, &day.ships, &walks) };

        // The label of a place under the mouse while the named places are showing.
        let place_hover = if options.show_named_places && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && shipyard_hover.is_none() && barracks_hover.is_none() && militia_hover.is_none() && ship_hover.is_none() && person_hover.is_none() && !placing && person_to_place.is_none() && !tool_on {
            region_labels::place_under(&style, &day, &world, &eye, mouse)
        } else {
            None
        };
        // The river or lake under the mouse while the named places are showing, with a little give so that a thin river can be
        // hit when the camera is far away.
        let water_hover = if options.show_named_places && place_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && shipyard_hover.is_none() && barracks_hover.is_none() && militia_hover.is_none() && ship_hover.is_none() && person_hover.is_none() && !placing && person_to_place.is_none() && !tool_on {
            let reach = (camera.distance() * 0.01).clamp(CELL_SIZE * 1.5, 40.0);
            ground_at().and_then(|at| water_bodies.near(at.x, at.y, reach))
        } else {
            None
        };

        // A click on the map (a drag is for the camera) places the first city, and after that opens the
        // popup of the city clicked.
        let clicked = clicks.update(!modal && !over_interface);
        // A click on the map puts away a menu of tools that was left open.
        if clicked {
            toolbar.close();
        }
        // With the path tool or the road tool on, clicks are for choosing the ends of a segment of it.
        if !modal && (path_tool.is_active() || road_tool.is_active()) {
            let tool = if path_tool.is_active() { &mut path_tool } else { &mut road_tool };
            let kind = tool.kind();
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            if let Some(piece) = tool.update(&scene, &day, clicked) {
                // Only keep it if it is saved too.
                let mut updated = day.clone();
                match updated.build_piece(kind, piece).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        tool.built();
                        claim_prompt.placed_in(&day, &[match piece {
                            path::PathPiece::Segment(segment) => segment.region,
                            path::PathPiece::Link(link) => link.region,
                        }]);
                    }
                    Err(message) => tool.fail(message),
                }
            }
        } else if !modal && bridge_tool.is_active() {
            // With the bridge tool on, clicks are for choosing the two ends of a bridge.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            // A bridge that has failed doesn't count as there, since building it again mends it.
            let standing: Vec<bridge::Bridge> = day.bridges.iter().filter(|b| !day.is_failed(failure::Thing::Bridge(b.id))).copied().collect();
            if let Some((first, second)) = bridge_tool.update(&scene, &standing, clicked) {
                // Only keep the bridge if it is saved too.
                let mut updated = day.clone();
                let ends = [first, second].map(|end| bridge::Spot::from(end));
                let in_regions = ends.map(|end| world.region_at(end.x, end.z).map(|region| region.raw()));
                match updated.build_bridge_in(first.into(), second.into(), in_regions).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        bridge_tool.built();
                        let regions: Vec<u16> = in_regions.into_iter().flatten().collect();
                        claim_prompt.placed_in(&day, &regions);
                    }
                    Err(message) => bridge_tool.fail(message),
                }
            }
        } else if !modal && gatherer_tools.iter().any(|tool| tool.is_active()) {
            // With a gatherer tool on, a click is for building one of its kind where the mouse is.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            if let Some(tool) = gatherer_tools.iter_mut().find(|tool| tool.is_active()) {
                let kind = tool.kind();
                if let Some((at, region)) = tool.update(&scene, &day, &deposits, clicked) {
                    // Only keep the gatherer if it is saved too.
                    let mut updated = day.clone();
                    match updated.build_gatherer(kind, at.x, at.y, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => {
                            day = updated;
                            tool.built();
                        }
                        Err(message) => tool.fail(message),
                    }
                }
            }
        } else if !modal && shipyard_tools.iter().any(|tool| tool.is_active()) {
            // With a shipyard tool on, a click is for building one of its size where the mouse is.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            if let Some(tool) = shipyard_tools.iter_mut().find(|tool| tool.is_active()) {
                let kind = tool.kind();
                if let Some((at, region)) = tool.update(&scene, &day, clicked) {
                    // Only keep the shipyard if it is saved too.
                    let mut updated = day.clone();
                    match updated.build_shipyard(kind, at.x, at.y, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => {
                            day = updated;
                            tool.built();
                        }
                        Err(message) => tool.fail(message),
                    }
                }
            }
        } else if !modal && barracks_tools.iter().any(|tool| tool.is_active()) {
            // With a barracks tool on, a click is for building one of its kind where the mouse is.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            if let Some(tool) = barracks_tools.iter_mut().find(|tool| tool.is_active()) {
                let kind = tool.kind();
                if let Some((at, region)) = tool.update(&scene, &day, clicked) {
                    // Only keep the barracks if it is saved too.
                    let mut updated = day.clone();
                    match updated.build_barracks(kind, at.x, at.y, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => {
                            day = updated;
                            tool.built();
                        }
                        Err(message) => tool.fail(message),
                    }
                }
            }
        } else if !modal && destroy_tool.is_active() {
            // With the destroy tool on, a click is for destroying the path, road, bridge or gatherer the mouse is on.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface, time };
            let reach = (camera.distance() * 0.006).clamp(0.0, 20.0);
            if let Some(target) = destroy_tool.update(&scene, &day, &paths, &roads, reach, clicked) {
                // Only keep it destroyed if that is saved too.
                let mut updated = day.clone();
                match target.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        destroy_tool.destroyed();
                    }
                    Err(message) => destroy_tool.fail(message),
                }
            }
        } else if clicked {
            if let Some(id) = person_to_place {
                if let (Some(at), None) = (person_site, &person_problem) {
                    // Only keep the place if it is saved too.
                    let mut updated = day.clone();
                    let region = world.region_at(at.x, at.y).map(|r| r.raw());
                    match updated.place_person(id, at, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => {
                            day = updated;
                            placing_person = None;
                        }
                        Err(message) => placing_error = Some(message),
                    }
                }
            } else if placing {
                if let (Some(at), None) = (site, &problem) {
                    // The city is founded once it has a name.
                    naming_city = Some((at, Popup::city()));
                }
            } else if let Some(id) = hovered {
                options_ui.close();
                overlays_ui.close();
                city_popup.open(id);
            } else if let Some(id) = shipyard_hover {
                options_ui.close();
                overlays_ui.close();
                shipyard_popup.open(id);
            } else if let Some(id) = barracks_hover {
                options_ui.close();
                overlays_ui.close();
                barracks_popup.open(id);
            } else if let Some(id) = gatherer_hover {
                options_ui.close();
                overlays_ui.close();
                gatherer_popup.open(id);
            } else if let Some(id) = militia_hover {
                // A click on a militia shows where it can go, and a second click on it opens its popup.
                options_ui.close();
                overlays_ui.close();
                move_error = None;
                selected_ship = None;
                selected_person = None;
                if selected_militia == Some(id) {
                    militia_popup.open(id);
                } else {
                    selected_militia = Some(id);
                }
            } else if let Some(id) = ship_hover {
                // A click on a ship shows where it can sail, and a second click on it opens its popup.
                options_ui.close();
                overlays_ui.close();
                move_error = None;
                selected_militia = None;
                selected_person = None;
                if selected_ship == Some(id) {
                    ship_popup.open(id);
                } else {
                    selected_ship = Some(id);
                }
            } else if let Some(id) = person_hover {
                // A click on a person shows where they can go, and a second click on them opens their popup.
                options_ui.close();
                overlays_ui.close();
                move_error = None;
                selected_militia = None;
                selected_ship = None;
                if selected_person == Some(id) {
                    person_popup.open(id);
                } else {
                    selected_person = Some(id);
                }
            } else if let Some(id) = selected_person {
                // With a person selected, a click on a tinted spot walks them there, and any other click puts them away. While they
                // are walking there is nothing tinted, and a click is left alone.
                options_ui.close();
                overlays_ui.close();
                let target = ground_at().and_then(|at| {
                    let reach = movement_view.reach()?;
                    Some((reach.snap(at), reach.cost_to(at)?.ceil() as u32, reach.route_to(at)?))
                });
                match target {
                    _ if walks.is_walking(id) => {}
                    Some((to, cost, way)) if cost > 0 => {
                        // Only keep the move if it is saved too.
                        let mut updated = day.clone();
                        let region = world.region_at(to.x, to.y).map(|r| r.raw());
                        match updated.move_person(id, to, cost, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                            Ok(()) => {
                                day = updated;
                                move_error = None;
                                walks.start(id, &way);
                            }
                            Err(message) => move_error = Some(message),
                        }
                    }
                    _ => {
                        selected_person = None;
                        move_error = None;
                    }
                }
            } else if let Some(id) = selected_ship {
                // With a ship selected, a click on a tinted spot of water sails it there, and any other click puts it away. While it
                // is sailing there is nothing tinted, and a click is left alone.
                options_ui.close();
                overlays_ui.close();
                let target = ground_at().and_then(|at| {
                    let reach = movement_view.reach()?;
                    Some((reach.snap(at), reach.cost_to(at)?.ceil() as u32, reach.route_to(at)?))
                });
                match target {
                    _ if walks.is_walking(id) => {}
                    Some((to, cost, way)) if cost > 0 => {
                        // Only keep the move if it is saved too.
                        let mut updated = day.clone();
                        let region = world.region_at(to.x, to.y).map(|r| r.raw());
                        match updated.move_ship(id, to, cost, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                            Ok(()) => {
                                day = updated;
                                move_error = None;
                                walks.start(id, &way);
                            }
                            Err(message) => move_error = Some(message),
                        }
                    }
                    _ => {
                        selected_ship = None;
                        move_error = None;
                    }
                }
            } else if let Some(id) = selected_militia {
                // With a militia selected, a click on a tinted spot moves it there, and any other click puts it away. While
                // it is walking there is nothing tinted, and a click is left alone.
                options_ui.close();
                overlays_ui.close();
                let target = ground_at().and_then(|at| {
                    let reach = movement_view.reach()?;
                    Some((reach.snap(at), reach.cost_to(at)?.ceil() as u32, reach.route_to(at)?))
                });
                match target {
                    _ if walks.is_walking(id) => {}
                    Some((to, cost, way)) if cost > 0 => {
                        // Only keep the move if it is saved too.
                        let mut updated = day.clone();
                        let region = world.region_at(to.x, to.y).map(|r| r.raw());
                        match updated.move_militia(id, to, cost, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                            Ok(()) => {
                                day = updated;
                                move_error = None;
                                walks.start(id, &way);
                            }
                            Err(message) => move_error = Some(message),
                        }
                    }
                    _ => {
                        selected_militia = None;
                        move_error = None;
                    }
                }
            } else if let Some(id) = place_hover {
                // With the named places showing, a click on the label of a place is for changing it.
                options_ui.close();
                overlays_ui.close();
                let at = day.place(id).map_or(Vec2::ZERO, |p| vec2(p.x, p.z));
                naming_place = Some((Some(id), at, Popup::place(day.place(id).map(|p| p.name.as_str()))));
            } else if let Some(id) = water_hover {
                // With the names of rivers or lakes showing, a click on one is for naming it.
                options_ui.close();
                overlays_ui.close();
                let lake = water_bodies.body(id).is_some_and(|b| b.lake);
                let current = if lake { day.lake_name(id.0) } else { day.river_name(id.0) };
                naming_water = Some((id.0, lake, Popup::river(current.unwrap_or(""), lake)));
            } else if options.show_region_names {
                // With the region names showing, a click on a region is for naming it.
                if let Some(region) = ground_at().and_then(|at| world.region_at(at.x, at.y)) {
                    options_ui.close();
                    overlays_ui.close();
                    let owner = day.region_owner(region.raw()).map_or("", |id| day.country_name(id));
                    let countries = day.countries.iter().map(|c| c.name.clone()).collect();
                    naming_region = Some((region.raw(), Popup::region(day.region_name(region.raw()).unwrap_or(""), region_name::kind_text(region), owner, countries, day.region_lock(region.raw()).as_deref())));
                }
            } else if options.show_named_places {
                // Anywhere else on the map, with the named places showing, a click is for adding a place there.
                if let Some(at) = ground_at() {
                    options_ui.close();
                    overlays_ui.close();
                    naming_place = Some((None, at, Popup::place(None)));
                }
            }
        }

        // The region under the mouse, for a note about it while the region names are showing, unless it is a river or
        // lake that is under the mouse and the named places are showing.
        let region_hover = if options.show_region_names && water_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && shipyard_hover.is_none() && barracks_hover.is_none() && militia_hover.is_none() && ship_hover.is_none() && person_hover.is_none() && !placing && person_to_place.is_none() && !tool_on {
            ground_at().and_then(|at| world.region_at(at.x, at.y))
        } else {
            None
        };
        // The resource under the mouse, for a note about what it is, when nothing else has the mouse.
        let resource_hover = if water_hover.is_none() && region_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && shipyard_hover.is_none() && barracks_hover.is_none() && militia_hover.is_none() && ship_hover.is_none() && person_hover.is_none() && !placing && person_to_place.is_none() && !tool_on {
            let reach = (camera.distance() * 0.01).clamp(CELL_SIZE * 2.5, 40.0);
            ground_at().and_then(|at| resource_hover::deposit_near(&world, at.x, at.y, reach))
        } else {
            None
        };

        set_default_camera();
        // The names of the regions and the rivers, over the map and under everything else.
        if options.show_region_names {
            region_labels::draw(&style, &day, &region_map, &world, &eye);
        }
        if options.show_named_places {
            region_labels::draw_rivers(&style, &day, &water_bodies, &world, &eye);
            region_labels::draw_lakes(&style, &day, &water_bodies, &world, &eye);
            region_labels::draw_places(&style, &day, &world, &eye);
        }
        // A bubble over each unit, in its country's color, while the map is seen at an angle.
        unit_bubbles::draw(&world, &day, &walks, &eye, camera.pitch());
        // A flashing red warning over whatever an alert is about, under the interface.
        alerts::draw_marks(&day, &day_failures, &region_map, &world, &eye, time);
        // While "Population" is hovered, a circle over each city for the number of its people.
        if label_hovered == Some(hud::FooterLabel::Population) {
            population_view::draw(&style, &day, &world, &eye);
        }
        // The header and the footer are there behind a popup too, but the keys are only for when it isn't up.
        let header = hud::header(&style, &day, !modal, camera.yaw(), selected_person);
        if !modal && is_mouse_button_pressed(MouseButton::Left) {
            if header.country.is_some_and(|b| b.contains(mouse)) {
                // The country is a drop-down, to switch to another.
                options_ui.close();
                overlays_ui.close();
                country_dropdown.toggle();
            } else if header.person.is_some_and(|b| b.contains(mouse)) {
                // The person beside it is a drop-down of the people of the country.
                options_ui.close();
                overlays_ui.close();
                country_dropdown.close();
                people_dropdown.toggle();
            } else if header.edit.is_some_and(|b| b.contains(mouse)) {
                // The pencil beside it edits the country.
                options_ui.close();
                overlays_ui.close();
                country_dropdown.close();
                country_menu.open_info(&day);
            } else if header.money.is_some_and(|b| b.contains(mouse)) {
                options_ui.close();
                overlays_ui.close();
                treasury.open(&day);
            } else if header.compass.contains(mouse) {
                // Turns the map to look straight down with north at the top.
                camera.reset_view();
            } else if header.day.contains(mouse) {
                // Ending the day is asked about first.
                options_ui.close();
                overlays_ui.close();
                advance_popup.open(day.date);
            }
        }
        // The country in the header is a drop-down to switch to another country, which is not there behind a popup.
        if modal {
            country_dropdown.close();
            people_dropdown.close();
        }
        let footer = hud::footer(&style);
        label_hovered = hud::footer_labels(&style, footer, !modal);
        autosave.update(&saves, &day, get_time());
        if let Some(message) = autosave.error() {
            hud::save_failed(&style, message);
        }
        if !modal {
            let newsreel = newsreel_panel.show(&style, &day);
            if newsreel_panel.take_add_clicked() {
                options_ui.close();
                overlays_ui.close();
                noting = Some(Popup::note());
            }
            // A unit in the list of those with moves left is selected, and the camera goes to it.
            if let Some(unit) = idle_units_ui.show(&style, &day, country_dropdown.has_mouse(mouse) || people_dropdown.has_mouse(mouse)) {
                options_ui.close();
                overlays_ui.close();
                move_error = None;
                placing_person = None;
                match unit {
                    idle_units::Unit::Militia(id) => {
                        if let Some(militia) = day.militia(id) {
                            selected_ship = None;
                            selected_person = None;
                            selected_militia = Some(id);
                            camera.fly_to(walks.position(militia), REGION_VIEW_DISTANCE);
                        }
                    }
                    idle_units::Unit::Ship(id) => {
                        if let Some(ship) = day.ship(id) {
                            selected_militia = None;
                            selected_person = None;
                            selected_ship = Some(id);
                            camera.fly_to(walks.position_of(id, vec2(ship.x, ship.z)), REGION_VIEW_DISTANCE);
                        }
                    }
                    idle_units::Unit::Person(id) => {
                        if let Some(at) = day.person(id).and_then(|p| person_view::position(&walks, p)) {
                            selected_militia = None;
                            selected_ship = None;
                            selected_person = Some(id);
                            camera.fly_to(at, REGION_VIEW_DISTANCE);
                        }
                    }
                }
            }
            // The person to start putting on the map, if the player asked to, by an alert or the drop-down of people.
            let mut start_placing = None;
            // An alert shows what it is about: the countries menu at a country, or the region where something failed.
            if let Some(target) = alerts_ui.show(&style, &day, &day_failures, newsreel) {
                match target {
                    AlertTarget::Country(country) => {
                        options_ui.close();
                        overlays_ui.close();
                        country_menu.open_at(&day, Some(country));
                    }
                    AlertTarget::PlacePerson(person) => start_placing = Some(person),
                    AlertTarget::Upkeep(country) => {
                        options_ui.close();
                        overlays_ui.close();
                        upkeep_popup.open(country);
                    }
                    AlertTarget::Gatherer(id) => {
                        if let Some(gatherer) = day.gatherer(id) {
                            camera.fly_to(vec2(gatherer.x, gatherer.z), REGION_VIEW_DISTANCE);
                        }
                    }
                    AlertTarget::City(id) => {
                        if let Some(city) = day.city(id) {
                            camera.fly_to(vec2(city.x, city.z), NEW_CITY_VIEW_DISTANCE);
                        }
                    }
                    AlertTarget::Militia(id) => {
                        if let Some(militia) = day.militia(id) {
                            camera.fly_to(vec2(militia.x, militia.z), REGION_VIEW_DISTANCE);
                        }
                    }
                    AlertTarget::Region(number) => {
                        if let Some(center) = RegionId::from_raw(number).and_then(|r| region_map.center(r)) {
                            camera.fly_to(center, REGION_VIEW_DISTANCE);
                        }
                    }
                    AlertTarget::Bridge(id) => {
                        // To the center of the region the bridge stands in, or to the bridge if that isn't known.
                        let bridge = day.bridge(id).map(|b| (b.end(bridge::End::First), b.end(bridge::End::Second)));
                        let center = bridge.and_then(|(a, _)| world.region_at(a.x, a.y)).and_then(|r| region_map.center(r));
                        if let Some(at) = center.or(bridge.map(|(a, b)| (a + b) * 0.5)) {
                            camera.fly_to(at, REGION_VIEW_DISTANCE);
                        }
                    }
                }
            }
            // The drop-down of countries is over the panels at the top left.
            match country_dropdown.show(&style, &day, header.country) {
                Some(Pick::Country(id)) => {
                    // Only keep the change if it is saved too.
                    let mut updated = day.clone();
                    match updated.select_country(id).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => day = updated,
                        Err(message) => {
                            // It is shown in the full list, which is where the player can see why.
                            country_menu.open_at(&day, Some(id));
                            country_menu.fail(message);
                        }
                    }
                }
                Some(Pick::Manage) => country_menu.open_at(&day, day.player_country().map(|c| c.id)),
                None => {}
            }
            // A person picked in the drop-down of people is selected, and the camera goes to them if they are on the map.
            match people_dropdown.show(&style, &day, selected_person, header.person) {
                Some(PeoplePick::Person(id)) => {
                    options_ui.close();
                    overlays_ui.close();
                    move_error = None;
                    selected_militia = None;
                    selected_ship = None;
                    // One who is not on the map yet is put there next, and one who is, is selected where they stand.
                    match day.person(id).and_then(|p| person_view::position(&walks, p)) {
                        Some(at) => {
                            placing_person = None;
                            selected_person = Some(id);
                            camera.fly_to(at, REGION_VIEW_DISTANCE);
                        }
                        None => start_placing = Some(id),
                    }
                }
                Some(PeoplePick::Introduce) => {
                    options_ui.close();
                    overlays_ui.close();
                    introducing = Some(Asking::Person.popup(&day));
                }
                None => {}
            }
            if let Some(id) = start_placing {
                options_ui.close();
                overlays_ui.close();
                toolbar.close();
                // A tool that is on would keep the clicks on the map for itself.
                path_tool.deactivate();
                road_tool.deactivate();
                bridge_tool.deactivate();
                destroy_tool.deactivate();
                gatherer_tools.iter_mut().for_each(|tool| tool.deactivate());
                shipyard_tools.iter_mut().for_each(|tool| tool.deactivate());
                barracks_tools.iter_mut().for_each(|tool| tool.deactivate());
                move_error = None;
                selected_militia = None;
                selected_ship = None;
                selected_person = None;
                placing_person = Some(id);
            }
            if let Some(person) = person_to_place.and_then(|id| day.person(id)) {
                hud::person_prompt(&style, &person.name, placing_error.as_deref().or(person_problem.as_deref()));
            }
            if placing {
                hud::placing_prompt(&style, settling_from.and_then(|from| day.city(from)).map(|city| city.name.as_str()), problem.as_deref());
            }
            if let Some(militia) = selected_militia.and_then(|id| day.militia(id)) {
                let id = militia.id;
                match movement_view::panel(&style, militia.title(), militia.points, militia.daily_points(), move_error.as_deref()) {
                    Some(PanelAction::Details) => militia_popup.open(id),
                    Some(PanelAction::Deselect) => {
                        selected_militia = None;
                        move_error = None;
                    }
                    None => {}
                }
            }
            if let Some(person) = selected_person.and_then(|id| day.person(id)) {
                let id = person.id;
                match movement_view::panel(&style, &person.name, person.points, movement::DAILY_POINTS, move_error.as_deref()) {
                    Some(PanelAction::Details) => person_popup.open(id),
                    Some(PanelAction::Deselect) => {
                        selected_person = None;
                        move_error = None;
                    }
                    None => {}
                }
            }
            if let Some(ship) = selected_ship.and_then(|id| day.ship(id)) {
                let id = ship.id;
                match movement_view::panel(&style, ship.title(), ship.points, movement::DAILY_POINTS, move_error.as_deref()) {
                    Some(PanelAction::Details) => ship_popup.open(id),
                    Some(PanelAction::Deselect) => {
                        selected_ship = None;
                        move_error = None;
                    }
                    None => {}
                }
            }
            if let Some(city) = hovered.and_then(|id| day.city(id)) {
                hud::city_tooltip(&style, &day, city);
            } else if let Some(gatherer) = gatherer_hover.and_then(|id| day.gatherer(id)) {
                gatherer_tool::tooltip(&style, &day, gatherer, gatherer.operating);
            } else if let Some(shipyard) = shipyard_hover.and_then(|id| day.shipyard(id)) {
                shipyard_tool::tooltip(&style, &day, shipyard);
            } else if let Some(barracks) = barracks_hover.and_then(|id| day.find_barracks(id)) {
                barracks_tool::tooltip(&style, &day, barracks);
            } else if let Some(militia) = militia_hover.and_then(|id| day.militia(id)) {
                militia_view::tooltip(&style, &day, militia);
            } else if let Some(ship) = ship_hover.and_then(|id| day.ship(id)) {
                ship_view::tooltip(&style, &day, ship);
            } else if let Some(person) = person_hover.and_then(|id| day.person(id)) {
                person_view::tooltip(&style, &day, person);
            } else if let Some(id) = water_hover {
                if water_bodies.body(id).is_some_and(|b| b.lake) {
                    hud::river_tooltip(&style, day.lake_name(id.0), "lake");
                } else {
                    hud::river_tooltip(&style, day.river_name(id.0), "river");
                }
            } else if let Some(region) = region_hover {
                hud::region_tooltip(&style, day.region_name(region.raw()), region_name::kind_text(region));
            } else if let Some(deposit) = resource_hover {
                hud::resource_tooltip(&style, deposit);
            }
            // Only one of the menus is open at a time: the one that was just opened wins.
            let (options_was_open, overlays_was_open, toolbar_was_open) = (options_ui.is_open(), overlays_ui.is_open(), toolbar.is_open());
            options_ui.show(&style, footer, &mut options, view.has_regions());
            overlays_ui.show(&style, footer, &mut options, view.has_regions());
            // Only one of the tools is on at a time: the one that was just turned on wins.
            let mut was_on = vec![path_tool.is_active(), road_tool.is_active(), bridge_tool.is_active(), destroy_tool.is_active()];
            was_on.extend(gatherer_tools.iter().map(|tool| tool.is_active()));
            was_on.extend(shipyard_tools.iter().map(|tool| tool.is_active()));
            was_on.extend(barracks_tools.iter().map(|tool| tool.is_active()));
            let destroy_button = destroy_tool.show_button(&style, footer, overlays_ui.button().x - 8.0);
            // The Travel and Buildings buttons are lit while a tool in them is on, and the open one has its tools in a
            // panel above it.
            let lit = [path_tool.is_active() || road_tool.is_active() || bridge_tool.is_active(), gatherer_tools.iter().any(|tool| tool.is_active()) || shipyard_tools.iter().any(|tool| tool.is_active()) || barracks_tools.iter().any(|tool| tool.is_active())];
            match toolbar.show(&style, footer, destroy_button.x - 8.0, lit) {
                Some((Menu::Travel, panel)) => {
                    path_tool.show_row(&style, toolbar::row_rect(panel, 0), panel, path_problem.as_ref(), no_regions);
                    road_tool.show_row(&style, toolbar::row_rect(panel, 1), panel, road_problem.as_ref(), no_regions);
                    bridge_tool.show_row(&style, toolbar::row_rect(panel, 2), panel, bridge_problem.as_ref());
                }
                Some((Menu::Buildings, panel)) => {
                    for (n, tool) in gatherer_tools.iter_mut().enumerate() {
                        let problem = day.gatherer_problem(tool.kind());
                        tool.show_row(&style, toolbar::row_rect(panel, n), panel, problem.as_deref());
                    }
                    for (n, tool) in shipyard_tools.iter_mut().enumerate() {
                        let problem = day.shipyard_problem(tool.kind());
                        tool.show_row(&style, toolbar::row_rect(panel, GathererKind::ALL.len() + n), panel, problem.as_deref());
                    }
                    for (n, tool) in barracks_tools.iter_mut().enumerate() {
                        let problem = day.barracks_problem(tool.kind());
                        tool.show_row(&style, toolbar::row_rect(panel, GathererKind::ALL.len() + ShipyardKind::BUILDABLE.len() + n), panel, problem.as_deref());
                    }
                }
                None => {}
            }
            let mut now_on = vec![path_tool.is_active(), road_tool.is_active(), bridge_tool.is_active(), destroy_tool.is_active()];
            now_on.extend(gatherer_tools.iter().map(|tool| tool.is_active()));
            now_on.extend(shipyard_tools.iter().map(|tool| tool.is_active()));
            now_on.extend(barracks_tools.iter().map(|tool| tool.is_active()));
            // Turning a tool on or off puts the menu away, so that the map is clear for it.
            if now_on != was_on {
                toolbar.close();
            }
            if now_on.iter().filter(|on| **on).count() > 1 {
                // The one that was already on gives way.
                if was_on[0] && now_on[0] {
                    path_tool.deactivate();
                }
                if was_on[1] && now_on[1] {
                    road_tool.deactivate();
                }
                if was_on[2] && now_on[2] {
                    bridge_tool.deactivate();
                }
                if was_on[3] && now_on[3] {
                    destroy_tool.deactivate();
                }
                for (n, tool) in gatherer_tools.iter_mut().enumerate() {
                    if was_on[4 + n] && now_on[4 + n] {
                        tool.deactivate();
                    }
                }
                for (n, tool) in shipyard_tools.iter_mut().enumerate() {
                    let at = 4 + GathererKind::ALL.len() + n;
                    if was_on[at] && now_on[at] {
                        tool.deactivate();
                    }
                }
                for (n, tool) in barracks_tools.iter_mut().enumerate() {
                    let at = 4 + GathererKind::ALL.len() + ShipyardKind::BUILDABLE.len() + n;
                    if was_on[at] && now_on[at] {
                        tool.deactivate();
                    }
                }
            }
            path_tool.show_prompt(&style, &day);
            road_tool.show_prompt(&style, &day);
            bridge_tool.show_prompt(&style, &day);
            destroy_tool.show_prompt(&style, &day);
            for tool in &mut gatherer_tools {
                tool.show_prompt(&style, &day);
            }
            for tool in &mut shipyard_tools {
                tool.show_prompt(&style, &day);
            }
            for tool in &mut barracks_tools {
                tool.show_prompt(&style, &day);
            }
            if overlays_ui.is_open() && !overlays_was_open {
                options_ui.close();
                toolbar.close();
            } else if options_ui.is_open() && !options_was_open {
                overlays_ui.close();
                toolbar.close();
            } else if toolbar.is_open() && !toolbar_was_open {
                options_ui.close();
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
                        if matches!(kind, Asking::Person) {
                            placing_person = day.people.last().map(|p| p.id);
                        }
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

        if let Some(region) = claim_prompt.show(&style, &day) {
            // Only keep the claim if it is saved too.
            let mut updated = day.clone();
            match updated.claim_region(region).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => {
                    day = updated;
                    claim_prompt.claimed();
                }
                Err(message) => claim_prompt.fail(message),
            }
        }

        if let Some(transfer) = treasury.show(&style, &day) {
            // Only keep the transfer if it is saved too.
            let mut updated = day.clone();
            match transfer.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => {
                    day = updated;
                    treasury.done();
                }
                Err(message) => treasury.fail(message),
            }
        }

        if advance_popup.show(&style) {
            // The day that ended is saved as it was, and the game goes on to the next.
            match saves.advance(&day) {
                Ok(next) => {
                    // The alerts about the day that ended are cleared, and those about the new day begin.
                    day_failures = alerts::Alert::new_failures(&day, &next);
                    alerts_ui.reset();
                    day = next;
                    autosave.reset(&day, get_time());
                    advance_popup.close();
                }
                Err(message) => advance_popup.fail(message),
            }
        }

        if let Some(change) = city_popup.show(&style, &day, &world) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => city_popup.fail(message),
            }
        }
        if let Some(from) = city_popup.take_settle() {
            settling_from = Some(from);
        }

        if let Some(change) = shipyard_popup.show(&style, &day, &world) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => shipyard_popup.fail(message),
            }
        }

        if let Some(change) = barracks_popup.show(&style, &day, &world) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => barracks_popup.fail(message),
            }
        }

        gatherer_popup.show(&style, &day);

        if let Some(change) = person_popup.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => person_popup.fail(message),
            }
        }

        if let Some(change) = militia_popup.show(&style, &day, &world) {
            // Only keep the change if it is saved too. A militia that is still marching to where it was moved can't be
            // combined or split until it has got there.
            let mut updated = day.clone();
            let marching = change.militia().into_iter().find(|id| walks.is_walking(*id));
            let result = match marching {
                Some(id) if !matches!(change, militia_popup::Change::Country(..) | militia_popup::Change::Siege(..) | militia_popup::Change::EndMoves(..) | militia_popup::Change::Garrison(..)) => Err(format!("{} is still marching.", day.militia(id).map_or("A militia", |m| m.title()))),
                _ => change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)),
            };
            match result {
                Ok(()) => {
                    // A militia that has been combined into another is no longer there to be selected.
                    if selected_militia.is_some_and(|id| updated.militia(id).is_none()) {
                        selected_militia = None;
                    }
                    day = updated;
                }
                Err(message) => militia_popup.fail(message),
            }
        }

        if let Some(change) = ship_popup.show(&style, &day) {
            // Only keep the change if it is saved too. A ship that is still sailing to where it was moved can't be scrapped until it
            // has got there.
            let mut updated = day.clone();
            let sailing = walks.is_walking(change.ship()) && !matches!(change, ship_popup::Change::Country(..) | ship_popup::Change::EndMoves(..) | ship_popup::Change::Garrison(..));
            let result = if sailing {
                Err(format!("{} is still sailing.", day.ship(change.ship()).map_or("A ship", |s| s.title())))
            } else {
                change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated))
            };
            match result {
                Ok(()) => {
                    if selected_ship.is_some_and(|id| updated.ship(id).is_none()) {
                        selected_ship = None;
                    }
                    day = updated;
                }
                Err(message) => ship_popup.fail(message),
            }
        }

        if let Some(change) = upkeep_popup.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => upkeep_popup.fail(message),
            }
        }

        if let Some(id) = ship_popup.take_rename() {
            if let Some(ship) = day.ship(id) {
                naming_ship = Some((id, Popup::ship(ship.title())));
            }
        }
        let mut ship_named = false;
        if let Some((id, popup)) = naming_ship.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the name if it is saved too.
                let mut updated = day.clone();
                match updated.rename_ship(*id, &typed[0]).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        ship_named = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if ship_named {
            naming_ship = None;
        }

        if let Some(id) = militia_popup.take_rename() {
            if let Some(militia) = day.militia(id) {
                naming_militia = Some((id, Popup::militia(militia.title())));
            }
        }
        let mut named = false;
        if let Some((id, popup)) = naming_militia.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the name if it is saved too.
                let mut updated = day.clone();
                match updated.rename_militia(*id, &typed[0]).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        named = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if named {
            naming_militia = None;
        }

        let mut founded = false;
        if let Some((at, popup)) = naming_city.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the city if it is saved too.
                let mut updated = day.clone();
                let region = world.region_at(at.x, at.y).map(|r| r.raw());
                let made = match settling_from {
                    Some(from) => updated.settle_city(from, &typed[0], at.x, at.y, region),
                    None => updated.found_city(&typed[0], at.x, at.y, region),
                };
                match made.and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        settling_from = None;
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

        let mut named = false;
        if let Some((region, popup)) = naming_region.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the name if it is saved too.
                let mut updated = day.clone();
                match updated.set_region_name(*region, &typed[0]).and_then(|()| updated.assign_region(*region, &typed[1])).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        named = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if named {
            naming_region = None;
        }

        let mut named = false;
        if let Some((water, lake, popup)) = naming_water.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the name if it is saved too.
                let mut updated = day.clone();
                let set = if *lake { updated.set_lake_name(*water, &typed[0]) } else { updated.set_river_name(*water, &typed[0]) };
                match set.and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        named = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if named {
            naming_water = None;
        }

        let mut named = false;
        if let Some((existing, at, popup)) = naming_place.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the place if it is saved too.
                let mut updated = day.clone();
                let set = match existing {
                    Some(id) => updated.set_place_name(*id, &typed[0]),
                    None => updated.add_place(*at, &typed[0]).map(|_| ()),
                };
                match set.and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        named = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if named {
            naming_place = None;
        }

        let mut noted = false;
        if let Some(popup) = noting.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the note if it is saved too.
                let mut updated = day.clone();
                match updated.add_note(&typed[0]).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        noted = true;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if noted {
            noting = None;
        }

        let mut introduced = false;
        if let Some(popup) = introducing.as_mut() {
            if let Some(typed) = popup.show(&style) {
                // Only keep the person if they are saved too.
                let mut updated = day.clone();
                match Asking::Person.answer(&mut updated, &typed).and_then(|()| autosave.save_now(&saves, &updated)) {
                    Ok(()) => {
                        day = updated;
                        introduced = true;
                        // They are put on the map next.
                        placing_person = day.people.last().map(|p| p.id);
                        selected_militia = None;
                        selected_ship = None;
                        selected_person = None;
                    }
                    Err(message) => popup.fail(message),
                }
            }
        }
        if introduced {
            introducing = None;
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
