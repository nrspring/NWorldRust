//! NWorld, the game. It loads the map and lets you fly over it, and puts countries, people and cities
//! on it. Roads and units will be laid over the map from here too; none of that belongs to
//! `nworld-map`.

mod advance_popup;
mod alerts;
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
mod country_menu;
mod day;
mod failure;
mod format;
mod ground;
mod hud;
mod ids;
mod newsreel;
mod resource_hover;
mod transfer;
mod treasury;
mod options;
mod path;
mod path_tool;
mod path_view;
mod overlays;
mod person;
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
mod title_type;

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
use autosave::Autosave;
use camera::GameCamera;
use city_popup::CityPopup;
use city_view::CityRenderer;
use day::DaySave;
use country_menu::CountryMenu;
use treasury::Treasury;
use options::{Options, OptionsUi};
use bridge::BRIDGE_COST;
use bridge_tool::BridgeTool;
use claim_prompt::ClaimPrompt;
use destroy_tool::DestroyTool;
use gatherer::GathererKind;
use gatherer_popup::GathererPopup;
use gatherer_tool::GathererTool;
use bridge_view::BridgeRenderer;
use path::Kind;
use path_tool::{PathTool, Scene};
use path_view::PathRenderer;
use overlays::OverlaysUi;
use placement::ClickTracker;
use popup::Popup;
use rgb::Rgb;
use saves::Saves;

/// How far from a new city the camera flies to look at it, in meters.
const NEW_CITY_VIEW_DISTANCE: f32 = 190.0;
/// How far from a region the camera is taken to look at it, in meters.
const REGION_VIEW_DISTANCE: f32 = 350.0;

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
    // The popup for the name of the region that was clicked while the region names are showing, and its number.
    let mut naming_region: Option<(u16, Popup)> = None;
    // The same for a river or a lake: its number, whether it is a lake, and the popup.
    let mut naming_water: Option<(u32, bool, Popup)> = None;
    // The same for a place: its number if it is already there, where it goes if it is new, and the popup.
    let mut naming_place: Option<(Option<u32>, Vec2, Popup)> = None;
    // The popup for a note to add to the newsreel.
    let mut noting: Option<Popup> = None;
    // The treasury of the country played, which is also where it sends money and resources to another.
    let mut treasury = Treasury::default();
    // The question of whether to claim a region that something was just placed in.
    let mut claim_prompt = ClaimPrompt::default();
    let mut options = Options::default();
    let mut options_ui = OptionsUi::default();
    let mut overlays_ui = OverlaysUi::default();
    let mut alerts_ui = AlertsUi::default();
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
    let mut bridges = BridgeRenderer::default();
    // The popup that asks whether to end the day.
    let mut advance_popup = AdvancePopup::default();
    let mut country_menu = CountryMenu::default();
    // The popup for the city that was clicked.
    let mut city_popup = CityPopup::default();
    // The popup for the gatherer that was clicked.
    let mut gatherer_popup = GathererPopup::default();
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
            } else if country_menu.is_open() {
                country_menu.escape();
            } else if options_ui.is_open() || overlays_ui.is_open() {
                options_ui.close();
                overlays_ui.close();
            } else if asking.as_mut().is_some_and(|(_, popup)| popup.close_dropdown()) {
                // A drop-down list was open, and is closed.
            } else if naming_city.is_some() {
                naming_city = None;
            } else if naming_region.is_some() {
                naming_region = None;
            } else if naming_water.is_some() {
                naming_water = None;
            } else if naming_place.is_some() {
                naming_place = None;
            } else if noting.is_some() {
                noting = None;
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
            } else {
                break;
            }
        }
        let dt = get_frame_time();
        time += dt;
        // A gatherer that has been cut off from a city, or has got a route to one, since last looked at is news. This is
        // not saved at once: the autosave finds the change.
        day.update_gatherer_operation();

        if asking.is_none() {
            asking = Asking::of(&day).map(|kind| {
                let popup = kind.popup(&day);
                (kind, popup)
            });
        }

        // While a popup is up, typing is for it and not for the camera.
        let modal = asking.is_some() || naming_city.is_some() || naming_region.is_some() || naming_water.is_some() || naming_place.is_some() || noting.is_some() || claim_prompt.is_open() || treasury.is_open() || country_menu.is_open() || city_popup.is_open() || gatherer_popup.is_open() || advance_popup.is_open();
        // The mouse is over the interface, and so not for the map, when it is on the header or the
        // options (which has the footer).
        let mouse = Vec2::from(mouse_position());
        let over_interface = hud::header_rect().contains(mouse) || options_ui.has_mouse(mouse) || overlays_ui.has_mouse(mouse) || alerts_ui.has_mouse(mouse) || newsreel_panel.has_mouse(mouse) || path_tool.has_mouse(mouse) || road_tool.has_mouse(mouse) || bridge_tool.has_mouse(mouse) || destroy_tool.has_mouse(mouse) || gatherer_tools.iter().any(|tool| tool.has_mouse(mouse));
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
        // Whether one of the tools is on, and so the map is not for anything else.
        let tool_on = path_tool.is_active() || road_tool.is_active() || bridge_tool.is_active() || destroy_tool.is_active() || gatherer_tools.iter().any(|tool| tool.is_active());
        let placing = !modal && day.cities.is_empty() && !tool_on;
        let site = if placing && !over_interface { pick_terrain(&eye, &world, mouse) } else { None };
        // Why the city can't go there: the ground, or the region belonging to another country.
        let problem = site.and_then(|at| {
            city::site_problem(&world, at.x, at.y).map(str::to_owned).or_else(|| {
                let country = day.player_country()?.id;
                day.city_region_problem(country, world.region_at(at.x, at.y).map(|r| r.raw()))
            })
        });
        cities.draw(&world, &day.cities, &eye);
        paths.draw(&world, &region_map, &day);
        roads.draw(&world, &region_map, &day);
        bridges.draw(&world, &day);
        gatherer_view::draw(&world, &day);
        if let Some(at) = site {
            cities.draw_site(&world, at, problem.is_none(), &eye);
        }
        // The city under the mouse, if there is one, which the tooltip tells about.
        let hovered = if modal || naming_city.is_some() || over_interface || tool_on { None } else { cities.city_under(mouse_ray(&eye, mouse), &day.cities) };

        // The gatherer under the mouse, if there is one and no city is in front of it, which the tooltip tells about.
        let gatherer_hover = if modal || over_interface || tool_on || hovered.is_some() { None } else { gatherer_view::under_ray(mouse_ray(&eye, mouse), &world, &day.gatherers) };

        // The label of a place under the mouse while the named places are showing.
        let place_hover = if options.show_named_places && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && !placing && !tool_on {
            region_labels::place_under(&style, &day, &world, &eye, mouse)
        } else {
            None
        };
        // The river or lake under the mouse while the named places are showing, with a little give so that a thin river can be
        // hit when the camera is far away.
        let water_hover = if options.show_named_places && place_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && !placing && !tool_on {
            let reach = (camera.distance() * 0.01).clamp(CELL_SIZE * 1.5, 40.0);
            pick_terrain(&eye, &world, mouse).and_then(|at| water_bodies.near(at.x, at.y, reach))
        } else {
            None
        };

        // A click on the map (a drag is for the camera) places the first city, and after that opens the
        // popup of the city clicked.
        let clicked = clicks.update(!modal && !over_interface);
        // With the path tool or the road tool on, clicks are for choosing the ends of a segment of it.
        if !modal && (path_tool.is_active() || road_tool.is_active()) {
            let tool = if path_tool.is_active() { &mut path_tool } else { &mut road_tool };
            let kind = tool.kind();
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface };
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
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface };
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
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface };
            if let Some(tool) = gatherer_tools.iter_mut().find(|tool| tool.is_active()) {
                let kind = tool.kind();
                if let Some((at, region)) = tool.update(&scene, &day, &deposits, clicked) {
                    // Only keep the gatherer if it is saved too.
                    let mut updated = day.clone();
                    match updated.build_gatherer(kind, at.x, at.y, region).and_then(|()| autosave.save_now(&saves, &updated)) {
                        Ok(()) => {
                            day = updated;
                            tool.built();
                            claim_prompt.placed_in(&day, &[region]);
                        }
                        Err(message) => tool.fail(message),
                    }
                }
            }
        } else if !modal && destroy_tool.is_active() {
            // With the destroy tool on, a click is for destroying the path, road, bridge or gatherer the mouse is on.
            let scene = Scene { world: &world, regions: &region_map, eye: &eye, mouse, over_map: !over_interface };
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
            if placing {
                if let (Some(at), None) = (site, &problem) {
                    // The city is founded once it has a name.
                    naming_city = Some((at, Popup::city()));
                }
            } else if let Some(id) = hovered {
                options_ui.close();
                overlays_ui.close();
                city_popup.open(id);
            } else if let Some(id) = gatherer_hover {
                options_ui.close();
                overlays_ui.close();
                gatherer_popup.open(id);
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
                if let Some(region) = pick_terrain(&eye, &world, mouse).and_then(|at| world.region_at(at.x, at.y)) {
                    options_ui.close();
                    overlays_ui.close();
                    let owner = day.region_owner(region.raw()).map_or("", |id| day.country_name(id));
                    let countries = day.countries.iter().map(|c| c.name.clone()).collect();
                    naming_region = Some((region.raw(), Popup::region(day.region_name(region.raw()).unwrap_or(""), region_name::kind_text(region), owner, countries, day.region_lock(region.raw()).as_deref())));
                }
            } else if options.show_named_places {
                // Anywhere else on the map, with the named places showing, a click is for adding a place there.
                if let Some(at) = pick_terrain(&eye, &world, mouse) {
                    options_ui.close();
                    overlays_ui.close();
                    naming_place = Some((None, at, Popup::place(None)));
                }
            }
        }

        // The region under the mouse, for a note about it while the region names are showing, unless it is a river or
        // lake that is under the mouse and the named places are showing.
        let region_hover = if options.show_region_names && water_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && !placing && !tool_on {
            pick_terrain(&eye, &world, mouse).and_then(|at| world.region_at(at.x, at.y))
        } else {
            None
        };
        // The resource under the mouse, for a note about what it is, when nothing else has the mouse.
        let resource_hover = if water_hover.is_none() && region_hover.is_none() && !modal && !over_interface && hovered.is_none() && gatherer_hover.is_none() && !placing && !tool_on {
            let reach = (camera.distance() * 0.01).clamp(CELL_SIZE * 2.5, 40.0);
            pick_terrain(&eye, &world, mouse).and_then(|at| resource_hover::deposit_near(&world, at.x, at.y, reach))
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
        // A flashing red warning over whatever an alert is about, under the interface.
        alerts::draw_marks(&day, &day_failures, &region_map, &world, &eye, time);
        // The header and the footer are there behind a popup too, but the keys are only for when it isn't up.
        let header = hud::header(&style, &day, !modal, camera.yaw());
        if !modal && is_mouse_button_pressed(MouseButton::Left) {
            if header.country.is_some_and(|b| b.contains(mouse)) {
                options_ui.close();
                overlays_ui.close();
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
        // The room the footer keeps for its buttons grows by the reason the paths button is off, when it is.
        // Why a tool button is off, for beside the leftmost of them. Paths cost the least, then roads, then
        // bridges, so each one that can't be built means the ones after it can't be either.
        let coins = day.player_country().map(|c| c.money);
        let tool_reason: Option<String> = match coins {
            None => Some("No country to pay for paths, roads or bridges".to_owned()),
            _ if no_regions => Some("This map has no regions for paths or roads".to_owned()),
            Some(coins) if coins < path::PATH_COST => Some(format!("Not enough coin (paths {}, roads {}, bridges {}; has {coins})", path::PATH_COST, path::ROAD_COST, format::money(BRIDGE_COST))),
            Some(coins) if coins < path::ROAD_COST => Some(format!("Not enough coin for roads or bridges ({}, {}; has {coins})", path::ROAD_COST, format::money(BRIDGE_COST))),
            Some(_) => bridge_problem.as_ref().map(|problem| problem.short()),
        };
        let buttons_room = hud::BUTTONS_ROOM + if tool_reason.is_some() { 300.0 } else { 0.0 };
        let footer = hud::footer(&style, !modal, buttons_room);
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
            // An alert shows what it is about: the countries menu at a country, or the region where something failed.
            if let Some(target) = alerts_ui.show(&style, &day, &day_failures, newsreel) {
                match target {
                    AlertTarget::Country(country) => {
                        options_ui.close();
                        overlays_ui.close();
                        country_menu.open_at(&day, Some(country));
                    }
                    AlertTarget::Gatherer(id) => {
                        if let Some(gatherer) = day.gatherer(id) {
                            camera.fly_to(vec2(gatherer.x, gatherer.z), REGION_VIEW_DISTANCE);
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
            if placing {
                hud::placing_prompt(&style, problem.as_deref());
            }
            if let Some(city) = hovered.and_then(|id| day.city(id)) {
                hud::city_tooltip(&style, &day, city);
            } else if let Some(gatherer) = gatherer_hover.and_then(|id| day.gatherer(id)) {
                gatherer_tool::tooltip(&style, &day, gatherer, day.gatherer_operating(gatherer.id));
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
            // Only one of the two menus is open at a time: the one that was just opened wins.
            let (options_was_open, overlays_was_open) = (options_ui.is_open(), overlays_ui.is_open());
            options_ui.show(&style, footer, &mut options, view.has_regions());
            overlays_ui.show(&style, footer, &mut options, view.has_regions());
            // Only one of the tools is on at a time: the one that was just turned on wins.
            let mut was_on = vec![path_tool.is_active(), road_tool.is_active(), bridge_tool.is_active(), destroy_tool.is_active()];
            was_on.extend(gatherer_tools.iter().map(|tool| tool.is_active()));
            let destroy_button = destroy_tool.show_button(&style, footer, overlays_ui.button().x - 8.0);
            // The gatherer buttons, the last kind nearest the destroy button so that they read in order.
            let mut gatherers_left = destroy_button.x - 8.0;
            for tool in gatherer_tools.iter_mut().rev() {
                let problem = day.gatherer_problem(tool.kind());
                gatherers_left = tool.show_button(&style, footer, gatherers_left, problem.as_deref()).x - 8.0;
            }
            let path_button = path_tool.show_button(&style, footer, gatherers_left, path_problem.as_ref(), no_regions);
            let road_button = road_tool.show_button(&style, footer, path_button.x - 8.0, road_problem.as_ref(), no_regions);
            bridge_tool.show_button(&style, footer, road_button.x - 8.0, bridge_problem.as_ref(), tool_reason.as_deref());
            let mut now_on = vec![path_tool.is_active(), road_tool.is_active(), bridge_tool.is_active(), destroy_tool.is_active()];
            now_on.extend(gatherer_tools.iter().map(|tool| tool.is_active()));
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
            }
            path_tool.show_prompt(&style, &day);
            road_tool.show_prompt(&style, &day);
            bridge_tool.show_prompt(&style, &day);
            destroy_tool.show_prompt(&style, &day);
            for tool in &mut gatherer_tools {
                tool.show_prompt(&style, &day);
            }
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

        if let Some(change) = city_popup.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => city_popup.fail(message),
            }
        }

        if let Some(change) = gatherer_popup.show(&style, &day) {
            // Only keep the change if it is saved too.
            let mut updated = day.clone();
            match change.apply(&mut updated).and_then(|()| autosave.save_now(&saves, &updated)) {
                Ok(()) => day = updated,
                Err(message) => gatherer_popup.fail(message),
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
