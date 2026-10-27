//! Visual style shared by the editor UI and the HUD: fonts, colors and drawing helpers for a
//! flat, modern dark theme.

use std::path::PathBuf;

use macroquad::prelude::*;

pub const SURFACE: Color = Color::new(0.118, 0.122, 0.133, 1.0);
pub const PANEL: Color = Color::new(0.165, 0.169, 0.184, 1.0);
pub const FIELD: Color = Color::new(0.106, 0.110, 0.122, 1.0);
pub const BORDER: Color = Color::new(1.0, 1.0, 1.0, 0.08);
pub const BORDER_STRONG: Color = Color::new(1.0, 1.0, 1.0, 0.16);
pub const HOVER: Color = Color::new(1.0, 1.0, 1.0, 0.07);
pub const TEXT: Color = Color::new(0.925, 0.929, 0.941, 1.0);
pub const TEXT_MUTED: Color = Color::new(0.600, 0.612, 0.647, 1.0);
pub const ACCENT: Color = Color::new(0.302, 0.553, 1.0, 1.0);
pub const ACCENT_HOVER: Color = Color::new(0.416, 0.635, 1.0, 1.0);
pub const ACCENT_SOFT: Color = Color::new(0.302, 0.553, 1.0, 0.20);
pub const ERROR: Color = Color::new(0.937, 0.365, 0.365, 1.0);
/// Translucent fill for controls floating over the 3D view.
pub const GLASS: Color = Color::new(0.098, 0.102, 0.114, 0.78);

#[derive(Clone, Copy)]
pub enum Weight {
    Regular,
    Semibold,
}

/// UI fonts, loaded from the system. Inter is preferred, then Segoe UI; if neither is
/// installed, macroquad's built-in font is used.
pub struct Style {
    regular: Option<Font>,
    semibold: Option<Font>,
}

impl Style {
    pub fn load() -> Self {
        Self {
            regular: load_system_font(&["Inter-Regular.ttf", "segoeui.ttf"]),
            semibold: load_system_font(&["Inter-SemiBold.ttf", "seguisb.ttf", "segoeui.ttf"]),
        }
    }

    fn font(&self, weight: Weight) -> Option<&Font> {
        match weight {
            Weight::Regular => self.regular.as_ref(),
            Weight::Semibold => self.semibold.as_ref().or(self.regular.as_ref()),
        }
    }

    pub fn measure(&self, text: &str, size: u16, weight: Weight) -> TextDimensions {
        measure_text(text, self.font(weight), size, 1.0)
    }

    /// Draws text with its baseline at `y`, snapped to whole pixels so glyphs stay crisp.
    pub fn text(&self, text: &str, x: f32, y: f32, size: u16, weight: Weight, color: Color) {
        draw_text_ex(
            text,
            x.round(),
            y.round(),
            TextParams {
                font: self.font(weight),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    /// Draws text vertically centered on `center_y`, starting at `x`.
    pub fn text_mid(&self, text: &str, x: f32, center_y: f32, size: u16, weight: Weight, color: Color) {
        let baseline = center_y + self.cap_height(size, weight) * 0.5;
        self.text(text, x, baseline, size, weight, color);
    }

    /// Draws text centered in `r`.
    pub fn text_centered(&self, text: &str, r: Rect, size: u16, weight: Weight, color: Color) {
        let width = self.measure(text, size, weight).width;
        self.text_mid(text, r.x + (r.w - width) * 0.5, r.center().y, size, weight, color);
    }

    /// Height of capital letters, used to center text optically regardless of its content.
    fn cap_height(&self, size: u16, weight: Weight) -> f32 {
        self.measure("H", size, weight).offset_y
    }

    /// A keyboard key drawn as a small outlined cap; returns its width.
    pub fn keycap(&self, key: &str, x: f32, center_y: f32) -> f32 {
        let size = 12;
        let w = (self.measure(key, size, Weight::Semibold).width + 10.0).max(18.0);
        let r = Rect::new(x, center_y - 9.0, w, 18.0);
        rounded_rect(r, 4.0, Color::new(1.0, 1.0, 1.0, 0.06));
        rounded_rect_lines(r, 4.0, 1.0, BORDER_STRONG);
        self.text_centered(key, r, size, Weight::Semibold, TEXT_MUTED);
        w
    }
}

fn load_system_font(names: &[&str]) -> Option<Font> {
    // Fonts can be installed for all users or just the current one.
    let dirs: Vec<PathBuf> = [
        std::env::var("WINDIR").ok().map(|w| PathBuf::from(w).join("Fonts")),
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|l| PathBuf::from(l).join("Microsoft/Windows/Fonts")),
    ]
    .into_iter()
    .flatten()
    .collect();

    names.iter().find_map(|name| {
        dirs.iter().find_map(|dir| {
            let bytes = std::fs::read(dir.join(name)).ok()?;
            load_ttf_font_from_bytes(&bytes).ok()
        })
    })
}

/// Outline of a rectangle with rounded corners, clockwise from the top-left arc.
fn rounded_outline(r: Rect, radius: f32) -> Vec<Vec2> {
    const SEGMENTS: usize = 6;
    let radius = radius.min(r.w * 0.5).min(r.h * 0.5).max(0.0);
    let corners = [
        (vec2(r.x + radius, r.y + radius), std::f32::consts::PI),
        (vec2(r.right() - radius, r.y + radius), std::f32::consts::PI * 1.5),
        (vec2(r.right() - radius, r.bottom() - radius), 0.0),
        (vec2(r.x + radius, r.bottom() - radius), std::f32::consts::FRAC_PI_2),
    ];
    let mut points = Vec::with_capacity(4 * (SEGMENTS + 1));
    for (center, start) in corners {
        for s in 0..=SEGMENTS {
            let a = start + std::f32::consts::FRAC_PI_2 * s as f32 / SEGMENTS as f32;
            points.push(center + vec2(a.cos(), a.sin()) * radius);
        }
    }
    points
}

/// Fills a rounded rectangle. Drawn as a triangle fan so translucent fills blend evenly.
pub fn rounded_rect(r: Rect, radius: f32, color: Color) {
    let points = rounded_outline(r, radius);
    let center = r.center();
    for (i, &p) in points.iter().enumerate() {
        draw_triangle(center, p, points[(i + 1) % points.len()], color);
    }
}

pub fn rounded_rect_lines(r: Rect, radius: f32, thickness: f32, color: Color) {
    // Inset by half the stroke so the line sits inside the shape, like a CSS border.
    let half = thickness * 0.5;
    let inner = Rect::new(r.x + half, r.y + half, r.w - thickness, r.h - thickness);
    let points = rounded_outline(inner, (radius - half).max(0.0));
    for (i, &p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        draw_line(p.x, p.y, q.x, q.y, thickness, color);
    }
}

/// A soft drop shadow under a floating card.
pub fn shadow(r: Rect, radius: f32) {
    for i in 1..=6 {
        let spread = i as f32 * 2.0;
        let s = Rect::new(r.x - spread, r.y - spread + 4.0, r.w + spread * 2.0, r.h + spread * 2.0);
        rounded_rect(s, radius + spread, Color::new(0.0, 0.0, 0.0, 0.05));
    }
}

/// A floating card: shadow, fill and hairline border.
pub fn card(r: Rect, radius: f32, fill: Color) {
    shadow(r, radius);
    rounded_rect(r, radius, fill);
    rounded_rect_lines(r, radius, 1.0, BORDER);
}
