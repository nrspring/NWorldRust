//! What the game draws over the map: the header (the country and the day), the footer (the frame rate
//! and the key help) and the day's newsreel.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::city::City;
use crate::day::DaySave;

/// Distance of the panels from the edges of the window.
const MARGIN: f32 = 16.0;
const NEWSREEL_WIDTH: f32 = 280.0;

const HELP: &[(&str, &str)] = &[
    ("W A S D", "pan"),
    ("Q E", "turn"),
    ("Wheel", "zoom"),
    ("Right-drag", "look"),
    ("Left-drag", "grab"),
    ("Shift", "fast"),
    ("Esc", "quit"),
];

/// How tall the footer is.
pub const FOOTER_HEIGHT: f32 = 48.0;
/// Distance of what is in the footer from its left edge. The options button keeps the same from the right.
pub const FOOTER_PADDING: f32 = 12.0;
/// Room kept at the right end of the footer for the overlays button and the options button.
const BUTTONS_ROOM: f32 = 160.0;

/// The size of the text in the header and the footer.
const BAR_TEXT_SIZE: u16 = 13;
/// How tall the header is.
pub const HEADER_HEIGHT: f32 = 48.0;

/// Where the header is: along the top of the window.
pub fn header_rect() -> Rect {
    Rect::new(0.0, 0.0, screen_width(), HEADER_HEIGHT)
}

/// The header along the top of the window: the player's country, once there is one, which is a button
/// for the countries menu (lit when the mouse is over it, if `interactive`), and the day. Returns
/// where the button is.
pub fn header(style: &Style, day: &DaySave, interactive: bool) -> Option<Rect> {
    let bar = header_rect();
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, style::GLASS);
    draw_line(bar.x, bar.bottom(), bar.right(), bar.bottom(), 1.0, style::BORDER);

    let y = bar.center().y;
    let mut x = bar.x + FOOTER_PADDING;
    let mut button = None;
    if let Some(country) = day.player_country() {
        let name_width = style.measure(&country.name, BAR_TEXT_SIZE, Weight::Semibold).width;
        let pill = Rect::new(x, y - 14.0, name_width + 52.0, 28.0);
        if interactive && pill.contains(Vec2::from(mouse_position())) {
            style::rounded_rect(pill, 8.0, style::HOVER);
        }
        draw_circle(pill.x + 16.0, y, 5.0, country.color.color());
        style.text_mid(&country.name, pill.x + 28.0, y, BAR_TEXT_SIZE, Weight::Semibold, style::TEXT);
        // A small arrow, to say there is more to choose.
        let arrow = pill.right() - 13.0;
        draw_triangle(vec2(arrow - 4.0, y - 1.5), vec2(arrow + 4.0, y - 1.5), vec2(arrow, y + 2.5), style::TEXT_MUTED);
        // A dot between the country and the day.
        draw_circle(pill.right() + 8.0, y, 2.0, style::TEXT_MUTED);
        x = pill.right() + 18.0;
        button = Some(pill);
    }
    style.text_mid(&format!("Day {}", day.date), x, y, BAR_TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
    button
}

/// The footer along the bottom of the window: the frame rate at the left and, when `keys` is set and
/// there is room for them, the keys in the middle. The overlays and options buttons go at the right,
/// which `OverlaysUi` and `OptionsUi` draw. Returns where the footer is.
pub fn footer(style: &Style, keys: bool) -> Rect {
    let bar = Rect::new(0.0, screen_height() - FOOTER_HEIGHT, screen_width(), FOOTER_HEIGHT);
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, style::GLASS);
    draw_line(bar.x, bar.y, bar.right(), bar.y, 1.0, style::BORDER);

    let y = bar.center().y;
    let fps_x = bar.x + FOOTER_PADDING + 8.0;
    style.text_mid(&format!("{} fps", get_fps()), fps_x, y, BAR_TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
    // Room for four digits however many there are, so the keys beside it never move.
    let fps_width = style.measure("8888 fps", BAR_TEXT_SIZE, Weight::Regular).width;

    if keys {
        help(style, bar, fps_x + fps_width + 24.0, bar.right() - BUTTONS_ROOM);
    }
    bar
}

/// A warning at the top middle that the game couldn't be saved, and why.
pub fn save_failed(style: &Style, message: &str) {
    let title = "Couldn't save the game. Trying again soon.";
    let width = style.measure(title, 14, Weight::Semibold).width.max(style.measure(message, 12, Weight::Regular).width) + 32.0;
    let width = width.min(screen_width() - MARGIN * 2.0);
    let panel = Rect::new((screen_width() - width) * 0.5, HEADER_HEIGHT + MARGIN, width, 52.0);
    style::card(panel, 12.0, style::GLASS);
    style::rounded_rect_lines(panel, 12.0, 1.5, style::ERROR);
    style.text_mid(title, panel.x + 16.0, panel.y + 18.0, 14, Weight::Semibold, style::ERROR);
    style.text_mid(message, panel.x + 16.0, panel.y + 38.0, 12, Weight::Regular, style::TEXT_MUTED);
}

/// A whole number with a comma between each three digits: 12,345.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What there is to know of `city`, next to the mouse: its name, its country and its population.
pub fn city_tooltip(style: &Style, day: &DaySave, city: &City) {
    let rows = [("Country", day.country_name(city.country).to_owned()), ("Population", thousands(city.population))];
    let label_w = rows.iter().map(|(label, _)| style.measure(label, 13, Weight::Regular).width).fold(0.0, f32::max);
    let value_w = rows.iter().map(|(_, value)| style.measure(value, 14, Weight::Semibold).width).fold(0.0, f32::max);
    let name_w = style.measure(&city.name, 17, Weight::Semibold).width;
    let width = name_w.max(label_w + 18.0 + value_w) + 32.0;
    let height = 16.0 + 24.0 + 12.0 + rows.len() as f32 * 22.0 + 6.0;

    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - MARGIN), (mouse.y + 20.0).min(screen_height() - height - FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    style.text_mid(&city.name, panel.x + 16.0, panel.y + 16.0 + 12.0, 17, Weight::Semibold, style::TEXT);
    draw_line(panel.x + 16.0, panel.y + 16.0 + 24.0 + 2.0, panel.right() - 16.0, panel.y + 16.0 + 24.0 + 2.0, 1.0, style::BORDER);
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = panel.y + 16.0 + 24.0 + 12.0 + 11.0 + 22.0 * k as f32;
        style.text_mid(label, panel.x + 16.0, y, 13, Weight::Regular, style::TEXT_MUTED);
        let value_x = panel.right() - 16.0 - style.measure(value, 14, Weight::Semibold).width;
        style.text_mid(value, value_x, y, 14, Weight::Semibold, style::TEXT);
    }
}

/// The prompt for placing the first city, above the footer. `problem` says why the city can't go
/// where the mouse is, if it can't.
pub fn placing_prompt(style: &Style, problem: Option<&str>) {
    let panel = Rect::new((screen_width() - 460.0) * 0.5, screen_height() - FOOTER_HEIGHT - MARGIN - 72.0, 460.0, 72.0);
    style::card(panel, 12.0, style::GLASS);
    style.text_mid("Found your first city", panel.x + 20.0, panel.y + 24.0, 16, Weight::Semibold, style::TEXT);
    match problem {
        Some(problem) => style.text_mid(problem, panel.x + 20.0, panel.y + 48.0, 14, Weight::Regular, style::ERROR),
        None => style.text_mid(
            &format!("Click on dry land to place it. It starts with {} people.", crate::city::INITIAL_POPULATION),
            panel.x + 20.0,
            panel.y + 48.0,
            14,
            Weight::Regular,
            style::TEXT_MUTED,
        ),
    }
}

/// The keys, in the middle of the `footer`, as long as they fit between `left` and `right`.
fn help(style: &Style, footer: Rect, left: f32, right: f32) {
    let item_gap = 18.0;
    let measured: Vec<(f32, f32)> = HELP
        .iter()
        .map(|(key, what)| ((style.measure(key, 12, Weight::Semibold).width + 10.0).max(18.0), style.measure(what, 13, Weight::Regular).width))
        .collect();
    let total: f32 = measured.iter().map(|(k, w)| k + 6.0 + w).sum::<f32>() + item_gap * (HELP.len() - 1) as f32;
    if total > right - left {
        return;
    }
    // In the middle of the window if that is clear of the rest, and otherwise as near as it can be.
    let mut x = ((footer.w - total) * 0.5).clamp(left, right - total);
    for ((key, what), (key_w, what_w)) in HELP.iter().zip(&measured) {
        style.keycap(key, x, footer.center().y);
        style.text_mid(what, x + key_w + 6.0, footer.center().y, 13, Weight::Regular, style::TEXT_MUTED);
        x += key_w + 6.0 + what_w + item_gap;
    }
}

/// The day's newsreel, at the top right.
pub fn newsreel(style: &Style, day: &DaySave) {
    const TEXT_SIZE: u16 = 12;
    const LINE: f32 = 16.0;
    const ITEM_GAP: f32 = 7.0;
    let text_width = NEWSREEL_WIDTH - 24.0 - 16.0;

    let items: Vec<Vec<String>> = day.newsreel.iter().map(|n| wrap(style, &n.headline(day), TEXT_SIZE, text_width)).collect();
    let body: f32 = if items.is_empty() {
        LINE
    } else {
        items.iter().map(|lines| lines.len() as f32 * LINE).sum::<f32>() + ITEM_GAP * (items.len() - 1) as f32
    };
    let panel = Rect::new(screen_width() - NEWSREEL_WIDTH - MARGIN, HEADER_HEIGHT + MARGIN, NEWSREEL_WIDTH, 12.0 + 14.0 + 10.0 + body + 12.0);
    style::card(panel, 10.0, style::GLASS);

    // The header: a red "live" dot, the name, and the day.
    let header_y = panel.y + 12.0 + 7.0;
    draw_circle(panel.x + 16.0, header_y, 3.0, Color::new(0.93, 0.27, 0.27, 1.0));
    style.text_mid("NEWSREEL", panel.x + 26.0, header_y, 10, Weight::Semibold, style::TEXT);
    let date = format!("Day {}", day.date);
    let date_w = style.measure(&date, 10, Weight::Regular).width;
    style.text_mid(&date, panel.right() - 12.0 - date_w, header_y, 10, Weight::Regular, style::TEXT_MUTED);
    draw_line(panel.x + 12.0, header_y + 11.0, panel.right() - 12.0, header_y + 11.0, 1.0, style::BORDER);

    let mut y = header_y + 11.0 + 10.0;
    if items.is_empty() {
        style.text_mid("Nothing to report yet.", panel.x + 12.0, y + LINE * 0.5, TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
        return;
    }
    for lines in &items {
        draw_circle(panel.x + 16.0, y + LINE * 0.5, 2.5, style::ACCENT);
        for (k, line) in lines.iter().enumerate() {
            style.text_mid(line, panel.x + 28.0, y + LINE * (k as f32 + 0.5), TEXT_SIZE, Weight::Regular, style::TEXT);
        }
        y += lines.len() as f32 * LINE + ITEM_GAP;
    }
}

/// Breaks `s` into lines no wider than `width`, at the spaces.
fn wrap(style: &Style, s: &str, size: u16, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(line) if style.measure(&format!("{line} {word}"), size, Weight::Regular).width <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_get_commas() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(100), "100");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(12_345), "12,345");
        assert_eq!(thousands(100_000), "100,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
