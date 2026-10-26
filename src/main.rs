mod flight;
mod noise;
mod sky;
mod terrain;

use std::time::Duration;

use macroquad::models::draw_mesh;
use macroquad::prelude::*;

use flight::Flight;
use sky::Environment;
use terrain::{Frustum, Terrain};

const VIEW_DISTANCE: f32 = 2400.0;
const SUN_AZIMUTH: f32 = 2.4;

const HELP: &[(&str, &str)] = &[
    ("W / S  or  Up / Down", "pitch up / down"),
    ("A / D  or  Left / Right", "turn left / right"),
    ("Shift / Ctrl  (or + / -)", "throttle up / down"),
    ("Space (hold)", "boost x3"),
    ("Tab", "autopilot: terrain follow"),
    ("I", "invert pitch"),
    ("[  /  ]", "time of day"),
    ("N", "new random world"),
    ("R", "reset position"),
    ("F", "toggle fullscreen"),
    ("H", "hide / show help"),
    ("Esc", "quit"),
];

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: "NWorld - Terrain Flyover".to_owned(),
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

#[macroquad::main(window_conf)]
async fn main() {
    let sky_material = sky::load_sky_material();
    let terrain_material = sky::load_terrain_material();

    rand::srand(macroquad::miniquad::date::now() as u64);
    let mut terrain = Terrain::new(1337, VIEW_DISTANCE);
    let mut flight = Flight::new(&terrain.field);
    terrain.update(flight.pos, Duration::from_secs(10));

    let mut sun_elevation: f32 = 0.6;
    let mut show_help = true;
    let mut fullscreen = false;
    let mut time = 0.0f32;

    loop {
        let dt = get_frame_time().min(0.1);
        time += dt;

        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::H) {
            show_help = !show_help;
        }
        if is_key_pressed(KeyCode::Tab) {
            flight.toggle_autopilot(&terrain.field);
        }
        if is_key_pressed(KeyCode::I) {
            flight.invert_pitch = !flight.invert_pitch;
        }
        if is_key_pressed(KeyCode::F) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        if is_key_pressed(KeyCode::R) {
            flight = Flight::new(&terrain.field);
        }
        if is_key_pressed(KeyCode::N) {
            terrain.reseed(rand::gen_range(1, 1_000_000));
            flight = Flight::new(&terrain.field);
            terrain.update(flight.pos, Duration::from_secs(10));
        }
        let sun_input = is_key_down(KeyCode::RightBracket) as i32 - is_key_down(KeyCode::LeftBracket) as i32;
        sun_elevation = (sun_elevation + sun_input as f32 * 0.4 * dt).clamp(-0.3, 1.5);

        flight.update(dt, &terrain.field);
        terrain.update(flight.pos, Duration::from_millis(6));

        let env = Environment::new(sun_elevation, SUN_AZIMUTH);
        let camera = flight.camera(VIEW_DISTANCE * 1.5);
        set_camera(&camera);
        clear_background(Color::new(env.horizon.x, env.horizon.y, env.horizon.z, 1.0));

        env.apply_to_sky(&sky_material, flight.pos);
        gl_use_material(&sky_material);
        draw_mesh(&sky::sky_box(flight.pos));

        env.apply_to_terrain(&terrain_material, flight.pos, VIEW_DISTANCE, time);
        gl_use_material(&terrain_material);
        let drawn = terrain.draw(&Frustum::from_matrix(camera.matrix()));

        gl_use_default_material();
        set_default_camera();
        draw_hud(&flight, &terrain, drawn, show_help);

        next_frame().await;
    }
}

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(1.0, 1.0, 1.0, 0.25));
}

fn draw_hud(flight: &Flight, terrain: &Terrain, drawn: usize, show_help: bool) {
    let mode = if flight.autopilot {
        format!("AUTOPILOT  holding {:.0} m AGL", flight.target_agl)
    } else {
        "MANUAL".to_owned()
    };
    let speed = if flight.boosting {
        format!("{:.0} m/s  (BOOST)", flight.current_speed())
    } else {
        format!("{:.0} m/s", flight.current_speed())
    };
    let lines = [
        format!("FPS {}   chunks {} / drawn {}", get_fps(), terrain.chunk_count(), drawn),
        format!("Speed     {speed}"),
        format!(
            "Altitude  {:.0} m   (AGL {:.0} m)",
            flight.pos.y,
            flight.agl(&terrain.field)
        ),
        format!(
            "Heading   {:03.0}   Pitch {:+.0}",
            flight.heading_degrees(),
            flight.pitch.to_degrees()
        ),
        format!("Mode      {mode}"),
        format!(
            "Seed      {}{}",
            terrain.field.seed,
            if flight.invert_pitch { "   (pitch inverted)" } else { "" }
        ),
    ];

    let line_h = 22.0;
    panel(12.0, 12.0, 400.0, lines.len() as f32 * line_h + 14.0);
    for (i, line) in lines.iter().enumerate() {
        draw_text(line, 24.0, 34.0 + i as f32 * line_h, 22.0, WHITE);
    }

    // Small reticle so the flight direction is easy to judge.
    let (cx, cy) = (screen_width() * 0.5, screen_height() * 0.5);
    let reticle = Color::new(1.0, 1.0, 1.0, 0.6);
    draw_line(cx - 14.0, cy, cx - 5.0, cy, 2.0, reticle);
    draw_line(cx + 5.0, cy, cx + 14.0, cy, 2.0, reticle);
    draw_line(cx, cy + 5.0, cx, cy + 12.0, 2.0, reticle);

    if show_help {
        let h = HELP.len() as f32 * line_h + 44.0;
        let y0 = screen_height() - h - 12.0;
        panel(12.0, y0, 470.0, h);
        draw_text("CONTROLS", 24.0, y0 + 26.0, 22.0, YELLOW);
        for (i, (keys, action)) in HELP.iter().enumerate() {
            let y = y0 + 50.0 + i as f32 * line_h;
            draw_text(keys, 24.0, y, 20.0, Color::new(0.75, 0.85, 1.0, 1.0));
            draw_text(action, 270.0, y, 20.0, WHITE);
        }
    } else {
        draw_text("H: help", 16.0, screen_height() - 16.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.6));
    }
}
