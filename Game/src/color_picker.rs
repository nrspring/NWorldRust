//! A color picker, in a popup: a square to choose how rich and how bright, a bar to choose the hue, and
//! a row of ready-made colors. Used to change the color of a country.

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::ids::CountryId;
use crate::popup::{self, Icon};
use crate::rgb::Rgb;

const CARD_WIDTH: f32 = 360.0;
const PADDING: f32 = 24.0;
const SQUARE_HEIGHT: f32 = 176.0;
const BAR_HEIGHT: f32 = 16.0;
const SWATCH: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;
/// How many strips the square is drawn in, each way. The colors in it are blended between its corners
/// and a few strips keep that smooth.
const STRIPS: usize = 16;

/// Colors to start from, which are easier to land on than the right place in the square.
const PRESETS: [&str; 10] = ["#e5484d", "#f76b15", "#f5d90a", "#30a46c", "#12a594", "#3e63dd", "#8e4ec6", "#d6409f", "#a18072", "#8b8d98"];

/// What the player decided.
pub enum Outcome {
    Save(Rgb),
    Cancel,
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Square,
    Bar,
}

pub struct ColorPicker {
    country: CountryId,
    name: String,
    /// The color, as a hue in degrees, and how rich and how bright it is, each 0 to 1. Kept this way, and
    /// not as red, green and blue, so the hue stays where it is when the color goes grey or black.
    hue: f32,
    saturation: f32,
    value: f32,
    drag: Option<Drag>,
    opened_at: f64,
}

impl ColorPicker {
    /// A picker for the color of the country called `name`, with the color it has now.
    pub fn new(country: CountryId, name: &str, color: Rgb) -> Self {
        let (hue, saturation, value) = color.to_hsv();
        Self { country, name: name.to_owned(), hue, saturation, value, drag: None, opened_at: get_time() }
    }

    pub fn country(&self) -> CountryId {
        self.country
    }

    fn color(&self) -> Rgb {
        Rgb::from_hsv(self.hue, self.saturation, self.value)
    }

    fn pick(&mut self, color: Rgb) {
        (self.hue, self.saturation, self.value) = color.to_hsv();
    }

    /// Takes the keyboard and mouse for this frame and draws the picker over everything.
    pub fn show(&mut self, style: &Style) -> Option<Outcome> {
        let mouse = Vec2::from(mouse_position());
        let pressed = is_mouse_button_pressed(MouseButton::Left);

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let height = 84.0 + SQUARE_HEIGHT + 14.0 + BAR_HEIGHT + 16.0 + 28.0 + 12.0 + SWATCH + 20.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside its icon.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Flag, badge);
        let text_x = badge.right() + 14.0;
        style.text("Country color", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text(&format!("Choose the color of {}.", self.name), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let square = Rect::new(inner_x, card.y + 84.0, inner_w, SQUARE_HEIGHT);
        let bar = Rect::new(inner_x, square.bottom() + 14.0, inner_w, BAR_HEIGHT);
        let info_y = bar.bottom() + 16.0;
        let presets_y = info_y + 28.0 + 12.0;
        let button = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let gap = (inner_w - SWATCH * PRESETS.len() as f32) / (PRESETS.len() - 1) as f32;
        let preset_rects: Vec<Rect> = (0..PRESETS.len()).map(|i| Rect::new(inner_x + i as f32 * (SWATCH + gap), presets_y, SWATCH, SWATCH)).collect();

        // A press on the square or the bar starts a drag, which goes on until the button is let go, even
        // off the edge of what was pressed.
        if pressed {
            self.drag = if square.contains(mouse) {
                Some(Drag::Square)
            } else if bar.contains(mouse) {
                Some(Drag::Bar)
            } else {
                None
            };
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.drag = None;
        }
        match self.drag {
            Some(Drag::Square) => {
                self.saturation = ((mouse.x - square.x) / square.w).clamp(0.0, 1.0);
                self.value = 1.0 - ((mouse.y - square.y) / square.h).clamp(0.0, 1.0);
            }
            Some(Drag::Bar) => self.hue = ((mouse.x - bar.x) / bar.w).clamp(0.0, 1.0) * 360.0,
            None => {}
        }
        if pressed {
            if let Some(i) = preset_rects.iter().position(|r| r.contains(mouse)) {
                if let Some(color) = Rgb::from_hex(PRESETS[i]) {
                    self.pick(color);
                }
            }
        }

        // The square: pale to full across, bright to black down. Then the bar of hues.
        draw_square(square, self.hue);
        draw_rectangle_lines(square.x, square.y, square.w, square.h, 1.0, style::BORDER_STRONG);
        let marker = vec2(square.x + self.saturation * square.w, square.y + (1.0 - self.value) * square.h);
        draw_circle_lines(marker.x, marker.y, 7.0, 3.0, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_circle_lines(marker.x, marker.y, 7.0, 1.8, WHITE);
        draw_bar(bar);
        let at = bar.x + self.hue / 360.0 * bar.w;
        style::rounded_rect(Rect::new(at - 3.0, bar.y - 3.0, 6.0, bar.h + 6.0), 3.0, WHITE);
        style::rounded_rect_lines(Rect::new(at - 3.0, bar.y - 3.0, 6.0, bar.h + 6.0), 3.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.5));

        // The color so far, and its code.
        let color = self.color();
        let preview = Rect::new(inner_x, info_y, 28.0, 28.0);
        style::rounded_rect(preview, 7.0, color.color());
        style::rounded_rect_lines(preview, 7.0, 1.0, style::BORDER_STRONG);
        style.text_mid(&color.to_hex().to_uppercase(), preview.right() + 12.0, preview.center().y, 14, Weight::Semibold, style::TEXT);

        for (rect, code) in preset_rects.iter().zip(PRESETS) {
            let preset = Rgb::from_hex(code).unwrap_or(color);
            style::rounded_rect(*rect, 6.0, preset.color());
            if preset == color {
                style::rounded_rect_lines(Rect::new(rect.x - 2.0, rect.y - 2.0, SWATCH + 4.0, SWATCH + 4.0), 8.0, 1.5, WHITE);
            } else if rect.contains(mouse) {
                style::rounded_rect_lines(*rect, 6.0, 1.5, style::BORDER_STRONG);
            }
        }

        // The button.
        let hovered = button.contains(mouse);
        style::rounded_rect(button, 8.0, if hovered { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("Save color", button, 14, Weight::Semibold, WHITE);
        let cap = Rect::new(button.right() - 60.0, button.center().y - 9.0, 44.0, 18.0);
        style::rounded_rect(cap, 5.0, Color::new(1.0, 1.0, 1.0, 0.22));
        style.text_centered("Enter", cap, 11, Weight::Semibold, WHITE);

        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if (pressed && hovered) || enter {
            return Some(Outcome::Save(color));
        }
        // A click on the dimmed window outside the card gives up.
        if pressed && !card.contains(mouse) {
            return Some(Outcome::Cancel);
        }
        None
    }
}

/// The square of colors of one hue: white to the hue across the top, and darkening to black down.
fn draw_square(rect: Rect, hue: f32) {
    let mut vertices = Vec::with_capacity((STRIPS + 1) * (STRIPS + 1));
    for row in 0..=STRIPS {
        for col in 0..=STRIPS {
            let (s, v) = (col as f32 / STRIPS as f32, 1.0 - row as f32 / STRIPS as f32);
            let c = Rgb::from_hsv(hue, s, v);
            vertices.push(Vertex {
                position: vec3(rect.x + s * rect.w, rect.y + (1.0 - v) * rect.h, 0.0),
                uv: Vec2::ZERO,
                color: [c.r, c.g, c.b, 255],
                normal: Vec4::ZERO,
            });
        }
    }
    let mut indices = Vec::with_capacity(STRIPS * STRIPS * 6);
    let at = |row: usize, col: usize| (row * (STRIPS + 1) + col) as u16;
    for row in 0..STRIPS {
        for col in 0..STRIPS {
            indices.extend([at(row, col), at(row, col + 1), at(row + 1, col), at(row, col + 1), at(row + 1, col + 1), at(row + 1, col)]);
        }
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

/// The bar of hues, from red round to red again.
fn draw_bar(rect: Rect) {
    const STOPS: usize = 12;
    let mut vertices = Vec::with_capacity((STOPS + 1) * 2);
    for i in 0..=STOPS {
        let c = Rgb::from_hsv(i as f32 / STOPS as f32 * 360.0, 1.0, 1.0);
        let x = rect.x + i as f32 / STOPS as f32 * rect.w;
        for y in [rect.y, rect.bottom()] {
            vertices.push(Vertex { position: vec3(x, y, 0.0), uv: Vec2::ZERO, color: [c.r, c.g, c.b, 255], normal: Vec4::ZERO });
        }
    }
    let mut indices = Vec::new();
    for i in 0..STOPS as u16 {
        let top = i * 2;
        indices.extend([top, top + 2, top + 1, top + 2, top + 3, top + 1]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}
