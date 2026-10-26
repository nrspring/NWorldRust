mod camera;
mod noise;
mod render;
mod ui;
mod world;

use std::time::Duration;

use macroquad::models::draw_mesh;
use macroquad::prelude::*;

use camera::EditorCamera;
use ui::{EditorUi, UiAction, TOOLBAR_WIDTH};
use world::{Frustum, World};

/// Fog distance at sea level; it grows with altitude so the overview shows the whole world.
const BASE_VIEW_DISTANCE: f32 = 4000.0;

const HELP: &[(&str, &str)] = &[
    ("W A S D", "move"),
    ("E / Q", "up / down"),
    ("Wheel", "zoom"),
    ("Right-drag, arrows", "look"),
    ("Shift", "faster"),
    ("T", "top-down view / back"),
    ("R", "reset camera"),
    ("H", "hide controls"),
    ("Esc", "close panel, else quit"),
];

const HUD_TEXT: Color = Color::new(1.0, 1.0, 1.0, 0.85);
const HUD_DIM: Color = Color::new(1.0, 1.0, 1.0, 0.45);
const HUD_SHADOW: Color = Color::new(0.0, 0.0, 0.0, 0.6);
/// Left edge of the HUD, just past the toolbar.
const HUD_LEFT: f32 = TOOLBAR_WIDTH + 10.0;

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

/// A map being edited, with its camera and the backdrop sized to it.
struct OpenMap {
    world: World,
    cam: EditorCamera,
    backdrop: Mesh,
}

impl OpenMap {
    fn new(width: f32, height: f32) -> Self {
        let mut world = World::new(width, height);
        let cam = EditorCamera::new(&world);
        world.update(cam.position(), Duration::from_secs(10));
        let backdrop = render::backdrop_mesh(world.half_extents());
        Self {
            world,
            cam,
            backdrop,
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let terrain_material = render::load_terrain_material();
    let backdrop_material = render::load_backdrop_material();

    let mut ui = EditorUi::new();
    // The app opens with no map; the New map tool creates one.
    let mut map: Option<OpenMap> = None;

    let mut show_help = false;
    let mut time = 0.0f32;

    loop {
        let dt = get_frame_time().min(0.1);
        time += dt;

        if is_key_pressed(KeyCode::Escape) && !ui.escape() {
            break;
        }
        if let Some(UiAction::CreateMap { width, height }) = ui.update() {
            map = Some(OpenMap::new(width, height));
        }

        // Shortcuts and camera keys pause while a text field is being typed into.
        let shortcuts = !ui.wants_keyboard();
        if shortcuts && is_key_pressed(KeyCode::H) {
            show_help = !show_help;
        }

        match &mut map {
            Some(m) => {
                if shortcuts && is_key_pressed(KeyCode::R) {
                    m.cam.reset(&m.world);
                }
                if (shortcuts && is_key_pressed(KeyCode::T)) || overview_button_clicked() {
                    m.cam.toggle_overview(&m.world, TOOLBAR_WIDTH);
                }

                m.cam.update(dt, &m.world, shortcuts, !ui.mouse_over());
                m.world.update(m.cam.position(), Duration::from_millis(6));
                draw_world(m, &terrain_material, &backdrop_material, time);
            }
            None => {
                clear_background(BLACK);
                draw_no_map_label();
            }
        }

        // The HUD goes under the editor UI so an open panel covers it.
        draw_hud(map.as_ref().map(|m| &m.cam), show_help);
        ui.draw();
        next_frame().await;
    }
}

fn draw_world(map: &OpenMap, terrain_material: &Material, backdrop_material: &Material, time: f32) {
    let pos = map.cam.position();
    let view_distance = BASE_VIEW_DISTANCE + pos.y.max(0.0) * 1.5;
    let camera = map.cam.camera(&map.world, view_distance * 1.5);
    set_camera(&camera);
    clear_background(render::background_color());

    let half = map.world.half_extents();
    render::apply_backdrop_uniforms(backdrop_material, pos, view_distance, half);
    gl_use_material(backdrop_material);
    draw_mesh(&map.backdrop);

    render::apply_terrain_uniforms(terrain_material, pos, view_distance, time);
    gl_use_material(terrain_material);
    map.world.draw(&Frustum::from_matrix(camera.matrix()), pos, view_distance);

    gl_use_default_material();
    set_default_camera();
}

/// "No map", centered in the area right of the toolbar.
fn draw_no_map_label() {
    let text = "No map";
    let dims = measure_text(text, None, 48, 1.0);
    let center_x = TOOLBAR_WIDTH + (screen_width() - TOOLBAR_WIDTH) * 0.5;
    draw_text(
        text,
        center_x - dims.width * 0.5,
        (screen_height() + dims.offset_y) * 0.5,
        48.0,
        HUD_TEXT,
    );
}

/// Text with a soft drop shadow so it stays readable over both water and the backdrop.
fn hud_text(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x + 1.0, y + 1.0, size, HUD_SHADOW);
    draw_text(text, x, y, size, color);
}

fn overview_button_rect() -> Rect {
    Rect::new(screen_width() - 118.0, 8.0, 110.0, 26.0)
}

fn overview_button_clicked() -> bool {
    is_mouse_button_pressed(MouseButton::Left)
        && overview_button_rect().contains(mouse_position().into())
}

fn draw_overview_button(cam: &EditorCamera) {
    let r = overview_button_rect();
    let hovered = r.contains(mouse_position().into());
    let fill = if hovered {
        Color::new(0.25, 0.45, 0.75, 0.8)
    } else {
        Color::new(0.0, 0.0, 0.0, 0.35)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color::new(1.0, 1.0, 1.0, 0.3));
    let label = if cam.in_overview() { "Back  (T)" } else { "Top-down  (T)" };
    let size = measure_text(label, None, 18, 1.0);
    draw_text(
        label,
        r.x + (r.w - size.width) * 0.5,
        r.y + (r.h + size.offset_y) * 0.5 - 1.0,
        18.0,
        HUD_TEXT,
    );
}

/// `cam` is the open map's camera, if there is a map.
fn draw_hud(cam: Option<&EditorCamera>, show_help: bool) {
    hud_text(&format!("{} FPS", get_fps()), HUD_LEFT, 24.0, 18.0, HUD_TEXT);
    if let Some(cam) = cam {
        draw_overview_button(cam);
    }

    let bottom = screen_height() - 10.0;
    if show_help {
        let line_h = 18.0;
        let top = bottom - (HELP.len() - 1) as f32 * line_h;
        for (i, (keys, action)) in HELP.iter().enumerate() {
            let y = top + i as f32 * line_h;
            hud_text(keys, HUD_LEFT, y, 16.0, Color::new(0.75, 0.85, 1.0, 0.9));
            hud_text(action, HUD_LEFT + 140.0, y, 16.0, HUD_TEXT);
        }
    } else {
        hud_text("H  controls", HUD_LEFT, bottom, 16.0, HUD_DIM);
    }
}
