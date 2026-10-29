mod camera;
mod continents;
mod copper;
mod dialogs;
mod forest;
mod gen_common;
mod hills;
mod history;
mod iron;
mod islands;
mod noise;
mod oil;
mod open_map;
mod props;
mod regions;
mod render;
mod rivers;
mod shallows;
mod stone;
mod storage;
mod sulphur;
mod style;
mod toast;
mod topo;
mod ui;
mod world;

use std::time::Duration;

use macroquad::models::draw_mesh;
use macroquad::prelude::*;

use camera::EditorCamera;
use continents::ContinentParams;
use islands::IslandParams;
use open_map::{can_discard, open_map, save_map, OpenMap};
use style::{Style, Weight};
use toast::Toasts;
use ui::{Brush, EditorUi, UiAction, TOOLBAR_WIDTH};

/// Fog distance at sea level; it grows with altitude so the overview shows the whole world.
const BASE_VIEW_DISTANCE: f32 = 4000.0;

const HELP: &[(&[&str], &str)] = &[
    (&["W", "A", "S", "D"], "Move"),
    (&["E", "Q"], "Up / down"),
    (&["Wheel"], "Zoom"),
    (&["Right-drag"], "Look (not with brushes)"),
    (&["Left-drag"], "Pan (no tool open)"),
    (&["Left-drag"], "Paint with the open tool"),
    (&["Click"], "Rivers: pick both ends"),
    (&["Right-drag"], "Lower / deepen / clear"),
    (&["[", "]"], "Brush size"),
    (&["Arrows"], "Look"),
    (&["Shift"], "Faster"),
    (&["T"], "Top-down view / back"),
    (&["V"], "Angled view (20 degrees)"),
    (&["R"], "Reset camera"),
    (&["Ctrl", "Z"], "Undo"),
    (&["Ctrl", "S"], "Save"),
    (&["Ctrl", "Shift", "S"], "Save as"),
    (&["Ctrl", "O"], "Open"),
    (&["H"], "Hide controls"),
    (&["Esc"], "Close panel, else quit"),
];

/// Distance of HUD elements from the edges of the 3D view.
const HUD_MARGIN: f32 = 12.0;
/// Left edge of the HUD, just past the toolbar.
const HUD_LEFT: f32 = TOOLBAR_WIDTH + HUD_MARGIN;
const HUD_RADIUS: f32 = 8.0;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: "NWorld - World Creator".to_owned(),
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

/// A generation run asked for by the UI.
enum Generation {
    Continents(ContinentParams),
    Islands(IslandParams),
}

/// What to tell the user after adding islands.
fn islands_message(asked: u32, added: usize) -> String {
    let plural = |n: usize| if n == 1 { "island" } else { "islands" };
    match added {
        0 => "No room for islands there. Try fewer or smaller ones, or a smaller edge buffer.".to_owned(),
        n if n as u32 >= asked => format!("Added {n} {}", plural(n)),
        n => format!("Added {n} of {asked} islands. There wasn't room for the rest."),
    }
}

fn ctrl_down() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

fn shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

#[macroquad::main(window_conf)]
async fn main() {
    let terrain = render::TerrainRenderer::new();
    // The props of each resource, in the order of `Resource::ALL`.
    let mut props = [
        props::PropRenderer::new(forest::kind()),
        props::PropRenderer::new(stone::kind()),
        props::PropRenderer::new(iron::kind()),
        props::PropRenderer::new(copper::kind()),
        props::PropRenderer::new(sulphur::kind()),
        props::PropRenderer::new(oil::kind()),
    ];
    let backdrop_material = render::load_backdrop_material();
    let sky_material = render::load_sky_material();
    let style = Style::load();

    // Closing the window goes through the same unsaved-changes check as Esc.
    prevent_quit();

    let mut ui = EditorUi::new();
    let mut toasts = Toasts::default();
    // The app opens with no map; the New map tool creates one.
    let mut map: Option<OpenMap> = None;

    // Generation requested last frame; it runs a frame late so "Generating..." gets drawn first.
    let mut pending: Option<Generation> = None;

    let mut show_help = false;
    let mut show_topo = false;
    let mut show_regions = false;
    let mut contour_labels = topo::ContourLabels::default();
    let mut time = 0.0f32;

    loop {
        let dt = get_frame_time().min(0.1);
        time += dt;

        if let Some(generation) = pending.take() {
            if let Some(m) = &mut map {
                match generation {
                    Generation::Continents(params) => m.generate_continents(&params),
                    Generation::Islands(params) => {
                        let added = m.add_islands(&params);
                        toasts.info(islands_message(params.count, added));
                    }
                }
            }
            ui.finish_generating();
        }

        // Esc drops the first end of a river, if one is picked, before it closes the panel.
        let escape = is_key_pressed(KeyCode::Escape);
        let dropped_end = escape && map.as_mut().is_some_and(|m| m.river_start.take().is_some());
        let quit = is_quit_requested() || (escape && !dropped_end && !ui.escape());
        if quit && can_discard(map.as_mut(), "close NWorld", &mut toasts) {
            break;
        }

        let undo_steps = map.as_ref().map_or(0, |m| m.history.steps());
        let mut action = ui.update(map.is_some(), undo_steps);

        // Shortcuts and camera keys pause while a text field is being typed into.
        let shortcuts = !ui.wants_keyboard();
        if shortcuts && ctrl_down() {
            if is_key_pressed(KeyCode::Z) {
                action = Some(UiAction::Undo);
            } else if is_key_pressed(KeyCode::S) && map.is_some() {
                action = Some(UiAction::Save { save_as: shift_down() });
            } else if is_key_pressed(KeyCode::O) {
                action = Some(UiAction::Open);
            }
        }
        if shortcuts && is_key_pressed(KeyCode::H) {
            show_help = !show_help;
        }

        // Any other edit ends a hills stroke first, so it lands as its own undo step.
        if action.is_some() {
            if let Some(m) = &mut map {
                m.end_stroke();
            }
        }

        match action {
            Some(UiAction::CreateMap { width, height }) => {
                if can_discard(map.as_mut(), "create a new map", &mut toasts) {
                    map = Some(OpenMap::new(width, height));
                }
            }
            Some(UiAction::GenerateContinents(params)) => pending = Some(Generation::Continents(params)),
            Some(UiAction::AddIslands(params)) => pending = Some(Generation::Islands(params)),
            Some(UiAction::AddLakes { count, size, seed }) => {
                if let Some(m) = &mut map {
                    match m.add_lakes(count, size, seed) {
                        0 => toasts.info("No room for lakes there. Try fewer or smaller ones, or add more land first."),
                        n if n as u32 >= count => toasts.info(format!("Added {n} {}", if n == 1 { "lake" } else { "lakes" })),
                        n => toasts.info(format!("Added {n} of {count} lakes. There wasn't room for the rest.")),
                    }
                }
            }
            Some(UiAction::AddRivers { count, width, winding, seed }) => {
                if let Some(m) = &mut map {
                    match m.add_rivers(count, width, winding, seed) {
                        0 => toasts.info("No room for a river. It needs land at least 150 m from the sea."),
                        n if n as u32 >= count => toasts.info(format!("Added {n} {}", if n == 1 { "river" } else { "rivers" })),
                        n => toasts.info(format!("Added {n} of {count} rivers. There wasn't room for the rest.")),
                    }
                }
            }
            Some(UiAction::BreakIntoRegions { size, seed }) => {
                if let Some(m) = &mut map {
                    let count = m.generate_regions(size, seed);
                    toasts.info(format!("Divided the map into {count} {}", if count == 1 { "region" } else { "regions" }));
                }
            }
            Some(UiAction::ClearRegions) => {
                if let Some(m) = &mut map {
                    if m.clear_regions() {
                        toasts.info("Removed the regions");
                    } else {
                        toasts.info("The map has no regions.");
                    }
                }
            }
            Some(UiAction::AddShallows { width }) => {
                if let Some(m) = &mut map {
                    if m.add_shallows(width) {
                        toasts.info("Added shallows along the coasts");
                    } else {
                        toasts.info("No deep water near land to make shallow.");
                    }
                }
            }
            Some(UiAction::AddMountainRange { length, width, seed }) => {
                if let Some(m) = &mut map {
                    if m.add_mountain_range(length, width, seed) {
                        toasts.info("Added a mountain range");
                    } else {
                        toasts.info("No room for a mountain range. Try a shorter one, or add more land first.");
                    }
                }
            }
            Some(UiAction::Undo) => {
                if let Some(m) = &mut map {
                    m.undo();
                }
            }
            Some(UiAction::Save { save_as }) => {
                if let Some(m) = &mut map {
                    save_map(m, save_as, &mut toasts);
                }
            }
            Some(UiAction::Open) => open_map(&mut map, &mut toasts),
            None => {}
        }

        let pills = hud_pills(&style, map.as_ref());
        if is_mouse_button_pressed(MouseButton::Left) && pills.topo.is_some_and(|r| r.contains(mouse_position().into())) {
            show_topo = !show_topo;
        }
        if is_mouse_button_pressed(MouseButton::Left) && pills.regions.is_some_and(|r| r.contains(mouse_position().into())) {
            show_regions = !show_regions;
        }

        match &mut map {
            Some(m) => {
                if shortcuts && is_key_pressed(KeyCode::R) {
                    m.cam.reset(&m.world);
                }
                if (shortcuts && is_key_pressed(KeyCode::T)) || overview_button_clicked(&style) {
                    m.cam.toggle_overview(&m.world, TOOLBAR_WIDTH);
                }
                if (shortcuts && is_key_pressed(KeyCode::V)) || angled_button_clicked(&style) {
                    m.cam.angled_view(&m.world);
                }

                let pan = !ui.tool_open() && !pills.covers(mouse_position().into(), &style);
                m.cam.update(dt, &m.world, shortcuts, !ui.mouse_over(), ui.brush().is_none(), pan);
                m.world.update(m.cam.position(), Duration::from_millis(6));
                let brush = paint_hills(m, &ui, &style);
                let river = place_river(m, &ui, &style, &mut toasts);
                let highlight = region_under_mouse(m, &ui, &style, &pills, show_regions);
                draw_world(m, &terrain, &mut props, &sky_material, &backdrop_material, time, show_topo, ui.regions_open(), highlight, brush, river);
                if show_topo {
                    contour_labels.refresh(&m.world, get_time());
                    contour_labels.draw(&style, &m.world, &m.cam.camera(&m.world, 40_000.0), TOOLBAR_WIDTH);
                }
            }
            None => {
                clear_background(BLACK);
                draw_no_map_label(&style);
            }
        }

        // The HUD goes under the editor UI so an open panel covers it.
        draw_hud(&style, &pills, map.as_ref(), show_help, show_topo, show_regions);
        ui.draw(&style, map.is_some(), map.as_ref().map_or(0, |m| m.history.steps()));
        toasts.draw(&style, viewport());
        next_frame().await;
    }
}

/// Slowly raises hills or mountains with the left button, or lowers the ground with the right,
/// while the Mountains tool is open and the mouse is held over the map. Returns where the brush ring should be drawn: its center and radius.
fn paint_hills(map: &mut OpenMap, ui: &EditorUi, style: &Style) -> Option<(Vec2, f32)> {
    let Some((radius, brush)) = ui.brush() else {
        map.end_stroke();
        return None;
    };
    let mouse: Vec2 = mouse_position().into();
    let over_map = !ui.mouse_over() && !hud_pills(style, Some(&*map)).covers(mouse, style);
    // A stroke can only start on the map, but once started it follows the mouse anywhere, for
    // as long as the button that started it is held.
    let erase = map.stroke_erases().or_else(|| {
        let pressed = |b| is_mouse_button_pressed(b) && over_map;
        if pressed(MouseButton::Left) {
            Some(false)
        } else {
            pressed(MouseButton::Right).then_some(true)
        }
    });
    let painting = erase.is_some_and(|erase| {
        is_mouse_button_down(if erase { MouseButton::Right } else { MouseButton::Left })
    });
    if !painting {
        map.end_stroke();
    }

    let camera = map.cam.camera(&map.world, 40_000.0);
    let center = (over_map || painting)
        .then(|| hills::pick_terrain(&camera, &map.world, mouse))
        .flatten();
    if let (Some(erase), true, Some(center)) = (erase, painting, center) {
        let dt = get_frame_time().min(0.1);
        match brush {
            Brush::Peak(peak) => map.paint_peaks(center, radius, erase, peak, dt),
            Brush::Shallows => map.paint_shallows(center, radius, erase, dt),
            Brush::Resource(kind, density) => map.paint_resource(kind, center, radius, erase, density, dt),
        }
    }
    center.filter(|_| over_map).map(|c| (c, radius))
}

/// What the Rivers tool shows on the map: where the first end was picked, where the mouse
/// points, and half the width of the channel.
struct RiverPreview {
    start: Option<Vec2>,
    cursor: Option<Vec2>,
    radius: f32,
}

/// Digs rivers with the Rivers tool open: the first click on the map picks one end of the
/// river, the second picks the other and digs it. Returns what to show on the map.
fn place_river(map: &mut OpenMap, ui: &EditorUi, style: &Style, toasts: &mut Toasts) -> Option<RiverPreview> {
    let Some((width, winding)) = ui.river() else {
        map.river_start = None;
        return None;
    };
    let mouse: Vec2 = mouse_position().into();
    let over_map = !ui.mouse_over() && !hud_pills(style, Some(&*map)).covers(mouse, style);
    let camera = map.cam.camera(&map.world, 40_000.0);
    let cursor = over_map.then(|| hills::pick_terrain(&camera, &map.world, mouse)).flatten();
    if let (true, Some(point)) = (is_mouse_button_pressed(MouseButton::Left), cursor) {
        match map.river_start.take() {
            None => map.river_start = Some(point),
            Some(start) => {
                map.end_stroke();
                if let Err(message) = map.add_river(start, point, width, winding, rand::gen_range(1, 999_999_999)) {
                    // Keep the first end, so another second one can be tried.
                    map.river_start = Some(start);
                    toasts.info(message);
                }
            }
        }
    }
    Some(RiverPreview { start: map.river_start, cursor, radius: width * 0.5 })
}

/// Rings for the river's ends, a post to find the first one by, and a line between them.
fn draw_river_preview(world: &world::World, cam: Vec3, river: &RiverPreview) {
    const SEGMENTS: usize = 64;
    if let Some(cursor) = river.cursor {
        draw_brush_ring(world, cam, cursor, river.radius);
    }
    let Some(start) = river.start else {
        return;
    };
    draw_brush_ring(world, cam, start, river.radius);
    let base = vec3(start.x, world.surface_height(start.x, start.y), start.y);
    draw_line_3d(base, base + vec3(0.0, 40.0, 0.0), WHITE);
    if let Some(cursor) = river.cursor {
        let point = |t: f32| {
            let p = start.lerp(cursor, t);
            let lift = (cam.distance(vec3(p.x, 0.0, p.y)) * 0.003).max(0.6);
            vec3(p.x, world.surface_height(p.x, p.y) + lift, p.y)
        };
        for k in 0..SEGMENTS {
            draw_line_3d(point(k as f32 / SEGMENTS as f32), point((k + 1) as f32 / SEGMENTS as f32), Color::new(1.0, 1.0, 1.0, 0.7));
        }
    }
}

/// The number of the region under the mouse, while the switch for highlighting regions is on, the
/// map has regions and the mouse is over the map rather than a control.
fn region_under_mouse(map: &OpenMap, ui: &EditorUi, style: &Style, pills: &HudPills, on: bool) -> Option<u16> {
    if !on || !map.has_regions() {
        return None;
    }
    let mouse: Vec2 = mouse_position().into();
    if ui.mouse_over() || pills.covers(mouse, style) {
        return None;
    }
    let camera = map.cam.camera(&map.world, 40_000.0);
    let at = hills::pick_terrain(&camera, &map.world, mouse)?;
    map.world.region_at(at.x, at.y).map(|id| id.raw())
}

fn draw_world(
    map: &mut OpenMap,
    terrain: &render::TerrainRenderer,
    props: &mut [props::PropRenderer],
    sky_material: &Material,
    backdrop_material: &Material,
    time: f32,
    topo: bool,
    regions: bool,
    highlight: Option<u16>,
    brush: Option<(Vec2, f32)>,
    river: Option<RiverPreview>,
) {
    let pos = map.cam.position();
    let view_distance = BASE_VIEW_DISTANCE + pos.y.max(0.0) * 1.5;
    let camera = map.cam.camera(&map.world, view_distance * 1.5);
    set_default_camera();
    clear_background(render::background_color());
    render::draw_sky(sky_material, camera.matrix(), pos, time);
    set_camera(&camera);

    let half = map.world.half_extents();
    render::apply_backdrop_uniforms(backdrop_material, pos, view_distance, half);
    gl_use_material(backdrop_material);
    draw_mesh(&map.backdrop);

    terrain.draw(&mut map.world, &map.surface, &map.resources, &map.region_texture, regions, highlight, &camera, pos, view_distance, time, topo);
    if !topo {
        for kind in props.iter_mut() {
            kind.draw(&map.world, &camera, pos, view_distance, time);
        }
    }

    gl_use_default_material();
    if let Some((center, radius)) = brush {
        draw_brush_ring(&map.world, pos, center, radius);
    }
    if let Some(river) = &river {
        draw_river_preview(&map.world, pos, river);
    }
    set_default_camera();
}

/// A ring on the ground showing where the hills brush will paint.
fn draw_brush_ring(world: &world::World, cam: Vec3, center: Vec2, radius: f32) {
    const SEGMENTS: usize = 72;
    let point = |a: f32| {
        let (x, z) = (center.x + a.cos() * radius, center.y + a.sin() * radius);
        // Float just above the ground, higher the farther away so it doesn't flicker.
        let lift = (cam.distance(vec3(x, 0.0, z)) * 0.003).max(0.6);
        vec3(x, world.surface_height(x, z) + lift, z)
    };
    for k in 0..SEGMENTS {
        let a0 = std::f32::consts::TAU * k as f32 / SEGMENTS as f32;
        let a1 = std::f32::consts::TAU * (k + 1) as f32 / SEGMENTS as f32;
        draw_line_3d(point(a0), point(a1), Color::new(1.0, 1.0, 1.0, 0.9));
    }
}

/// The area right of the toolbar, where the map is shown.
fn viewport() -> Rect {
    Rect::new(TOOLBAR_WIDTH, 0.0, screen_width() - TOOLBAR_WIDTH, screen_height())
}

/// "No map", centered in the viewport.
fn draw_no_map_label(style: &Style) {
    style.text_centered("No map", viewport(), 24, Weight::Regular, style::TEXT_MUTED);
}

/// Translucent rounded backing for HUD elements floating over the 3D view.
fn glass(r: Rect, hovered: bool) {
    let fill = if hovered {
        Color::new(0.16, 0.17, 0.19, 0.88)
    } else {
        style::GLASS
    };
    style::rounded_rect(r, HUD_RADIUS, fill);
    style::rounded_rect_lines(r, HUD_RADIUS, 1.0, style::BORDER);
}

const OVERVIEW_LABELS: [&str; 2] = ["Top-down", "Back"];

/// Sized for the longer label so the button doesn't jump when it toggles.
fn overview_button_rect(style: &Style) -> Rect {
    let label_w = OVERVIEW_LABELS
        .iter()
        .map(|l| style.measure(l, 13, Weight::Semibold).width)
        .fold(0.0, f32::max);
    let w = 12.0 + label_w + 10.0 + 18.0 + 8.0;
    Rect::new(screen_width() - HUD_MARGIN - w, HUD_MARGIN, w, 32.0)
}

fn overview_button_clicked(style: &Style) -> bool {
    is_mouse_button_pressed(MouseButton::Left)
        && overview_button_rect(style).contains(mouse_position().into())
}

/// The "Angled" button, just left of the overview button.
fn angled_button_rect(style: &Style) -> Rect {
    let label_w = style.measure("Angled", 13, Weight::Semibold).width;
    let w = 12.0 + label_w + 10.0 + 18.0 + 8.0;
    let overview = overview_button_rect(style);
    Rect::new(overview.x - 6.0 - w, overview.y, w, overview.h)
}

fn angled_button_clicked(style: &Style) -> bool {
    is_mouse_button_pressed(MouseButton::Left)
        && angled_button_rect(style).contains(mouse_position().into())
}

fn draw_angled_button(style: &Style) {
    let r = angled_button_rect(style);
    glass(r, r.contains(mouse_position().into()));
    let mid = r.center().y;
    style.text_mid("Angled", r.x + 12.0, mid, 13, Weight::Semibold, style::TEXT);
    style.keycap("V", r.right() - 8.0 - 18.0, mid);
}

fn draw_overview_button(style: &Style, cam: &EditorCamera) {
    let r = overview_button_rect(style);
    glass(r, r.contains(mouse_position().into()));
    let label = OVERVIEW_LABELS[cam.in_overview() as usize];
    let mid = r.center().y;
    style.text_mid(label, r.x + 12.0, mid, 13, Weight::Semibold, style::TEXT);
    style.keycap("T", r.right() - 8.0 - 18.0, mid);
}

const PILL_HEIGHT: f32 = 26.0;
const SWITCH_WIDTH: f32 = 30.0;

/// The pills along the top left of the HUD: the frame rate, the map's name, the switch for
/// the topographic view and, once the map has regions, the switch for lighting up the region
/// under the pointer.
struct HudPills {
    fps: Rect,
    name: Option<Rect>,
    topo: Option<Rect>,
    regions: Option<Rect>,
}

impl HudPills {
    /// Whether the pointer is over a HUD control, rather than the map beneath.
    fn covers(&self, mouse: Vec2, style: &Style) -> bool {
        self.topo.is_some_and(|r| r.contains(mouse))
            || self.regions.is_some_and(|r| r.contains(mouse))
            || (self.name.is_some()
                && (overview_button_rect(style).contains(mouse) || angled_button_rect(style).contains(mouse)))
    }
}

/// The map's file name, with a star while it has edits that aren't saved.
fn map_label(map: &OpenMap) -> String {
    format!("{}{}", map.name(), if map.is_dirty() { " *" } else { "" })
}

fn hud_pills(style: &Style, map: Option<&OpenMap>) -> HudPills {
    // Sized for the widest reading so the pills beside it don't shift as the number changes.
    let fps_w = style.measure("888 FPS", 12, Weight::Semibold).width + 20.0;
    let fps = Rect::new(HUD_LEFT, HUD_MARGIN, fps_w, PILL_HEIGHT);
    let Some(map) = map else {
        return HudPills { fps, name: None, topo: None, regions: None };
    };
    let name_w = style.measure(&map_label(map), 12, Weight::Regular).width + 20.0;
    let name = Rect::new(fps.right() + 6.0, HUD_MARGIN, name_w, PILL_HEIGHT);
    let label_w = style.measure("Topographic", 12, Weight::Regular).width;
    let topo = Rect::new(name.right() + 6.0, HUD_MARGIN, 12.0 + label_w + 10.0 + SWITCH_WIDTH + 10.0, PILL_HEIGHT);
    let label_w = style.measure("Highlight regions", 12, Weight::Regular).width;
    let regions = map
        .has_regions()
        .then(|| Rect::new(topo.right() + 6.0, HUD_MARGIN, 12.0 + label_w + 10.0 + SWITCH_WIDTH + 10.0, PILL_HEIGHT));
    HudPills { fps, name: Some(name), topo: Some(topo), regions }
}

/// A label and an on/off switch.
fn draw_switch(style: &Style, r: Rect, label: &str, on: bool) {
    glass(r, r.contains(mouse_position().into()));
    let mid = r.center().y;
    style.text_mid(label, r.x + 12.0, mid, 12, Weight::Regular, style::TEXT_MUTED);
    let track = Rect::new(r.right() - 10.0 - SWITCH_WIDTH, mid - 8.0, SWITCH_WIDTH, 16.0);
    let fill = if on { style::ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.18) };
    style::rounded_rect(track, 8.0, fill);
    let knob_x = if on { track.right() - 8.0 } else { track.x + 8.0 };
    draw_circle(knob_x, mid, 6.0, WHITE);
}

fn draw_hud(style: &Style, pills: &HudPills, map: Option<&OpenMap>, show_help: bool, show_topo: bool, show_regions: bool) {
    let fps = format!("{} FPS", get_fps());
    glass(pills.fps, false);
    style.text_centered(&fps, pills.fps, 12, Weight::Semibold, style::TEXT_MUTED);

    if let (Some(map), Some(name_rect), Some(topo_rect)) = (map, pills.name, pills.topo) {
        glass(name_rect, false);
        style.text_centered(&map_label(map), name_rect, 12, Weight::Regular, style::TEXT_MUTED);
        draw_switch(style, topo_rect, "Topographic", show_topo);
        if let Some(regions_rect) = pills.regions {
            draw_switch(style, regions_rect, "Highlight regions", show_regions);
        }

        draw_overview_button(style, &map.cam);
        draw_angled_button(style);
    }

    let bottom = screen_height() - HUD_MARGIN;
    if show_help {
        draw_help_card(style, bottom);
    } else {
        // A small "H Controls" pill hinting at the controls card.
        let label_w = style.measure("Controls", 13, Weight::Regular).width;
        let r = Rect::new(HUD_LEFT, bottom - 30.0, 8.0 + 18.0 + 8.0 + label_w + 12.0, 30.0);
        glass(r, false);
        let mid = r.center().y;
        let key_w = style.keycap("H", r.x + 8.0, mid);
        style.text_mid("Controls", r.x + 8.0 + key_w + 8.0, mid, 13, Weight::Regular, style::TEXT_MUTED);
    }
}

fn draw_help_card(style: &Style, bottom: f32) {
    const ROW_H: f32 = 26.0;
    const KEYS_W: f32 = 120.0;
    let h = 44.0 + HELP.len() as f32 * ROW_H + 6.0;
    let r = Rect::new(HUD_LEFT, bottom - h, 300.0, h);
    glass(r, false);
    style.text("Controls", r.x + 14.0, r.y + 26.0, 13, Weight::Semibold, style::TEXT);

    for (i, (keys, action)) in HELP.iter().enumerate() {
        let mid = r.y + 44.0 + ROW_H * (i as f32 + 0.5);
        let mut x = r.x + 14.0;
        for key in keys.iter() {
            x += style.keycap(key, x, mid) + 4.0;
        }
        style.text_mid(action, r.x + 14.0 + KEYS_W, mid, 13, Weight::Regular, style::TEXT_MUTED);
    }
}
