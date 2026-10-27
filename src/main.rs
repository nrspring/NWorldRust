mod camera;
mod continents;
mod dialogs;
mod gen_common;
mod hills;
mod history;
mod islands;
mod noise;
mod open_map;
mod render;
mod storage;
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
use ui::{EditorUi, UiAction, TOOLBAR_WIDTH};

/// Fog distance at sea level; it grows with altitude so the overview shows the whole world.
const BASE_VIEW_DISTANCE: f32 = 4000.0;

const HELP: &[(&[&str], &str)] = &[
    (&["W", "A", "S", "D"], "Move"),
    (&["E", "Q"], "Up / down"),
    (&["Wheel"], "Zoom"),
    (&["Right-drag"], "Look (not with brushes)"),
    (&["Left-drag"], "Raise hills / mountains"),
    (&["Right-drag"], "Lower ground"),
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
    let backdrop_material = render::load_backdrop_material();
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

        let quit = is_quit_requested() || (is_key_pressed(KeyCode::Escape) && !ui.escape());
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

                m.cam.update(dt, &m.world, shortcuts, !ui.mouse_over(), ui.brush().is_none());
                m.world.update(m.cam.position(), Duration::from_millis(6));
                let brush = paint_hills(m, &ui, &style);
                draw_world(m, &terrain, &backdrop_material, time, show_topo, brush);
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
        draw_hud(&style, &pills, map.as_ref(), show_help, show_topo);
        ui.draw(&style, map.is_some(), map.as_ref().map_or(0, |m| m.history.steps()));
        toasts.draw(&style, viewport());
        next_frame().await;
    }
}

/// Slowly raises hills or mountains with the left button, or lowers the ground with the right,
/// while the Mountains tool is open and the mouse is held over the map. Returns where the brush ring should be drawn: its center and radius.
fn paint_hills(map: &mut OpenMap, ui: &EditorUi, style: &Style) -> Option<(Vec2, f32)> {
    let Some((radius, peak)) = ui.brush() else {
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
        map.paint_peaks(center, radius, erase, peak, get_frame_time().min(0.1));
    }
    center.filter(|_| over_map).map(|c| (c, radius))
}

fn draw_world(map: &mut OpenMap, terrain: &render::TerrainRenderer, backdrop_material: &Material, time: f32, topo: bool, brush: Option<(Vec2, f32)>) {
    let pos = map.cam.position();
    let view_distance = BASE_VIEW_DISTANCE + pos.y.max(0.0) * 1.5;
    let camera = map.cam.camera(&map.world, view_distance * 1.5);
    set_camera(&camera);
    clear_background(render::background_color());

    let half = map.world.half_extents();
    render::apply_backdrop_uniforms(backdrop_material, pos, view_distance, half);
    gl_use_material(backdrop_material);
    draw_mesh(&map.backdrop);

    terrain.draw(&mut map.world, &map.surface, &camera, pos, view_distance, time, topo);

    gl_use_default_material();
    if let Some((center, radius)) = brush {
        draw_brush_ring(&map.world, pos, center, radius);
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

/// The pills along the top left of the HUD: the frame rate, the map's name and the switch for
/// the topographic view.
struct HudPills {
    fps: Rect,
    name: Option<Rect>,
    topo: Option<Rect>,
}

impl HudPills {
    /// Whether the pointer is over a HUD control, rather than the map beneath.
    fn covers(&self, mouse: Vec2, style: &Style) -> bool {
        self.topo.is_some_and(|r| r.contains(mouse))
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
        return HudPills { fps, name: None, topo: None };
    };
    let name_w = style.measure(&map_label(map), 12, Weight::Regular).width + 20.0;
    let name = Rect::new(fps.right() + 6.0, HUD_MARGIN, name_w, PILL_HEIGHT);
    let label_w = style.measure("Topographic", 12, Weight::Regular).width;
    let topo = Rect::new(name.right() + 6.0, HUD_MARGIN, 12.0 + label_w + 10.0 + SWITCH_WIDTH + 10.0, PILL_HEIGHT);
    HudPills { fps, name: Some(name), topo: Some(topo) }
}

/// "Topographic" and an on/off switch.
fn draw_topo_switch(style: &Style, r: Rect, on: bool) {
    glass(r, r.contains(mouse_position().into()));
    let mid = r.center().y;
    style.text_mid("Topographic", r.x + 12.0, mid, 12, Weight::Regular, style::TEXT_MUTED);
    let track = Rect::new(r.right() - 10.0 - SWITCH_WIDTH, mid - 8.0, SWITCH_WIDTH, 16.0);
    let fill = if on { style::ACCENT } else { Color::new(1.0, 1.0, 1.0, 0.18) };
    style::rounded_rect(track, 8.0, fill);
    let knob_x = if on { track.right() - 8.0 } else { track.x + 8.0 };
    draw_circle(knob_x, mid, 6.0, WHITE);
}

fn draw_hud(style: &Style, pills: &HudPills, map: Option<&OpenMap>, show_help: bool, show_topo: bool) {
    let fps = format!("{} FPS", get_fps());
    glass(pills.fps, false);
    style.text_centered(&fps, pills.fps, 12, Weight::Semibold, style::TEXT_MUTED);

    if let (Some(map), Some(name_rect), Some(topo_rect)) = (map, pills.name, pills.topo) {
        glass(name_rect, false);
        style.text_centered(&map_label(map), name_rect, 12, Weight::Regular, style::TEXT_MUTED);
        draw_topo_switch(style, topo_rect, show_topo);

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
