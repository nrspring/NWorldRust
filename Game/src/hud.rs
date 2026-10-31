//! What the game draws over the map: the header (the country and the day), the footer (the frame rate
//! and the key help) and the day's newsreel. The alerts panel under the newsreel is in `alerts`.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use nworld_map::world::Resource;

use crate::city::City;
use crate::country::{resource_name, Stock};
use crate::day::DaySave;
use crate::format::{money, thousands, thousands_u64};

/// Distance of the panels from the edges of the window.
const MARGIN: f32 = 16.0;
const NEWSREEL_WIDTH: f32 = 280.0;

const HELP: &[(&str, &str)] = &[
    ("W A S D", "pan"),
    ("Q E", "turn"),
    ("N", "north up"),
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
/// Room kept at the right end of the footer for its buttons, before any reason that one of them is off.
pub const BUTTONS_ROOM: f32 = 430.0;

/// The size of the text in the header and the footer.
const BAR_TEXT_SIZE: u16 = 13;
/// How tall the header is.
pub const HEADER_HEIGHT: f32 = 48.0;

/// Where the header is: along the top of the window.
pub fn header_rect() -> Rect {
    Rect::new(0.0, 0.0, screen_width(), HEADER_HEIGHT)
}

/// The buttons of the header, where they are.
pub struct HeaderButtons {
    /// The player's country, for the countries menu, once there is one.
    pub country: Option<Rect>,
    /// The day, for ending it.
    pub day: Rect,
    /// The compass at the right, for turning the map to look straight down with north up.
    pub compass: Rect,
}

/// The header along the top of the window: the player's country, once there is one, which is a button
/// for the countries menu, and the day, which is a button for ending it (each lit when the mouse is over
/// it, if `interactive`), and the money of the country. At the right is a compass, whose needle points to north as
/// the camera (looking in the direction `yaw`) sees it, which is a button for turning the map north up. Returns
/// where the buttons are.
pub fn header(style: &Style, day: &DaySave, interactive: bool, yaw: f32) -> HeaderButtons {
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
        x = pill.right() + 28.0;
        button = Some(pill);
    }
    let day_text = format!("Day {}", day.date);
    let day_width = style.measure(&day_text, BAR_TEXT_SIZE, Weight::Regular).width;
    let day_button = Rect::new(x - 8.0, y - 14.0, day_width + 16.0, 28.0);
    let day_hovered = interactive && day_button.contains(Vec2::from(mouse_position()));
    if day_hovered {
        style::rounded_rect(day_button, 8.0, style::HOVER);
    }
    style.text_mid(&day_text, x, y, BAR_TEXT_SIZE, Weight::Regular, if day_hovered { style::TEXT } else { style::TEXT_MUTED });
    // The player's country's money, after the day.
    if let Some(country) = day.player_country() {
        x += day_width;
        draw_circle(x + 14.0, y, 2.0, style::TEXT_MUTED);
        draw_coin(vec2(x + 31.0, y), 6.5);
        let color = if country.money < 0 { style::ERROR } else { style::TEXT };
        style.text_mid(&money(country.money), x + 43.0, y, BAR_TEXT_SIZE, Weight::Semibold, color);
    }
    let compass = Rect::new(bar.right() - FOOTER_PADDING - 32.0, y - 14.0, 32.0, 28.0);
    // Then its stock of each resource, if there is room before the compass.
    if let Some(country) = day.player_country() {
        let coin_end = x + 43.0 + style.measure(&money(country.money), BAR_TEXT_SIZE, Weight::Semibold).width;
        draw_stock(style, &country.stock, coin_end + 22.0, y, compass.x - 16.0);
    }
    draw_compass(compass, yaw, interactive && compass.contains(Vec2::from(mouse_position())));
    HeaderButtons { country: button, day: day_button, compass }
}

/// What `resource` is drawn as in the header: a dot of this color, like the resource looks on the map.
pub fn resource_color(resource: Resource) -> Color {
    match resource {
        Resource::Forest => Color::new(0.35, 0.66, 0.30, 1.0),
        Resource::Stone => Color::new(0.62, 0.64, 0.68, 1.0),
        Resource::Iron => Color::new(0.72, 0.38, 0.28, 1.0),
        Resource::Copper => Color::new(0.86, 0.55, 0.28, 1.0),
        Resource::Sulphur => Color::new(0.93, 0.84, 0.28, 1.0),
        Resource::Oil => Color::new(0.30, 0.31, 0.38, 1.0),
    }
}

/// Draws how much of each resource there is in `stock`, from `x` along the line at `y`, each as a dot of its color, its
/// name and the amount, as many as fit before `limit`.
fn draw_stock(style: &Style, stock: &Stock, mut x: f32, y: f32, limit: f32) {
    for resource in Resource::ALL {
        let text = format!("{} {}", resource_name(resource), thousands_u64(stock.get(resource)));
        let width = style.measure(&text, BAR_TEXT_SIZE, Weight::Regular).width;
        if x + 14.0 + width > limit {
            return;
        }
        draw_circle(x + 4.0, y, 4.0, resource_color(resource));
        style.text_mid(&text, x + 14.0, y, BAR_TEXT_SIZE, Weight::Regular, if stock.get(resource) == 0 { style::TEXT_MUTED } else { style::TEXT });
        x += 14.0 + width + 18.0;
    }
}

/// A compass in `rect`: a ring with a needle whose red end points to north on the screen, as seen by a camera
/// looking in the direction `yaw`. North is towards -z, so looking in the direction of [`crate::camera::NORTH`]
/// has it straight up.
fn draw_compass(rect: Rect, yaw: f32, hovered: bool) {
    if hovered {
        style::rounded_rect(rect, 8.0, style::HOVER);
    }
    let c = rect.center();
    draw_circle_lines(c.x, c.y, 11.0, 1.5, if hovered { style::TEXT } else { style::TEXT_MUTED });
    let north = vec2(-yaw.sin(), yaw.cos());
    let across = vec2(-north.y, north.x);
    draw_triangle(c + north * 10.0, c + across * 3.5, c - across * 3.5, Color::new(0.93, 0.30, 0.30, 1.0));
    draw_triangle(c - north * 9.0, c + across * 3.5, c - across * 3.5, Color::new(0.80, 0.82, 0.88, 0.85));
}

/// The footer along the bottom of the window: the frame rate at the left and, when `keys` is set and
/// there is room for them (the right `buttons_room` pixels are kept for the buttons), the keys in the middle.
/// The bridge, road, path, overlays and options buttons go at the right, which `BridgeTool`, `PathTool` (for paths
/// and roads), `OverlaysUi` and
/// `OptionsUi` draw.
/// Returns where the footer is.
pub fn footer(style: &Style, keys: bool, buttons_room: f32) -> Rect {
    let bar = Rect::new(0.0, screen_height() - FOOTER_HEIGHT, screen_width(), FOOTER_HEIGHT);
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, style::GLASS);
    draw_line(bar.x, bar.y, bar.right(), bar.y, 1.0, style::BORDER);

    let y = bar.center().y;
    let fps_x = bar.x + FOOTER_PADDING + 8.0;
    style.text_mid(&format!("{} fps", get_fps()), fps_x, y, BAR_TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
    // Room for four digits however many there are, so the keys beside it never move.
    let fps_width = style.measure("8888 fps", BAR_TEXT_SIZE, Weight::Regular).width;

    if keys {
        help(style, bar, fps_x + fps_width + 24.0, bar.right() - buttons_room);
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

/// A gold coin, about `radius` pixels across.
pub fn draw_coin(center: Vec2, radius: f32) {
    draw_circle(center.x, center.y, radius, Color::new(0.80, 0.58, 0.12, 1.0));
    draw_circle(center.x, center.y, radius * 0.78, Color::new(0.97, 0.78, 0.28, 1.0));
    draw_circle(center.x, center.y, radius * 0.42, Color::new(0.80, 0.58, 0.12, 1.0));
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

/// A small note next to the mouse about the region it is over, when the region names are showing: its name, or that it
/// has none, and that clicking it names it.
pub fn region_tooltip(style: &Style, name: Option<&str>, kind: &str) {
    name_tooltip(style, name.unwrap_or("Unnamed region"), name.is_some(), &format!("A region of {kind}"));
}

/// The same for a river or lake: its name, or that it has none (`what` is "river" or "lake").
pub fn river_tooltip(style: &Style, name: Option<&str>, what: &str) {
    let unnamed = format!("Unnamed {what}");
    name_tooltip(style, name.unwrap_or(&unnamed), name.is_some(), &format!("A {what}"));
}

/// A note next to the mouse with a `title` (muted if it isn't a real name), what it is, and that clicking it names
/// it.
fn name_tooltip(style: &Style, title: &str, named: bool, what: &str) {
    let lines = [what.to_owned(), "Click to name it".to_owned()];
    let width = lines.iter().map(|l| style.measure(l, 12, Weight::Regular).width).fold(style.measure(title, 15, Weight::Semibold).width, f32::max) + 32.0;
    let height = 14.0 + 22.0 + lines.len() as f32 * 18.0 + 8.0;
    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - MARGIN), (mouse.y + 20.0).min(screen_height() - height - FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    style.text_mid(title, panel.x + 16.0, panel.y + 14.0 + 10.0, 15, Weight::Semibold, if named { style::TEXT } else { style::TEXT_MUTED });
    for (k, line) in lines.iter().enumerate() {
        style.text_mid(line, panel.x + 16.0, panel.y + 14.0 + 22.0 + 18.0 * (k as f32 + 0.5), 12, Weight::Regular, style::TEXT_MUTED);
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

/// The day's newsreel, at the top right. It is as tall as its news, up to a limit, and past that it shows
/// as many items as fit and the mouse wheel scrolls through the rest, an item at a time.
#[derive(Default)]
pub struct NewsreelPanel {
    /// The first item showing.
    first: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
}

impl NewsreelPanel {
    /// Whether the mouse is over it, and so is not for the map.
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.rect.contains(mouse)
    }

    /// Draws it and scrolls it if the player turns the wheel over it. Returns where it is.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Rect {
        const TEXT_SIZE: u16 = 12;
        const LINE: f32 = 16.0;
        const ITEM_GAP: f32 = 7.0;
        const NOTE_HEIGHT: f32 = 26.0;
        let text_width = NEWSREEL_WIDTH - 24.0 - 16.0;

        let items: Vec<Vec<String>> = day.newsreel.iter().map(|n| wrap(style, &n.headline(day), TEXT_SIZE, text_width)).collect();
        let heights: Vec<f32> = items.iter().map(|lines| lines.len() as f32 * LINE).collect();
        // The most the body of the panel may take: a good part of the window, and room left for what is under it.
        let room = screen_height() - HEADER_HEIGHT - FOOTER_HEIGHT - MARGIN * 2.0 - 110.0;
        let max_body = room.min(screen_height() * 0.45).max(80.0);

        // The wheel scrolls it while the mouse is over it.
        if self.rect.contains(Vec2::from(mouse_position())) && mouse_wheel().1 != 0.0 {
            self.first = if mouse_wheel().1 > 0.0 { self.first.saturating_sub(1) } else { self.first + 1 };
        }
        // Scrolled no further than where the last item is at the bottom of a full panel.
        let mut used = 0.0;
        let mut last_page = items.len();
        for i in (0..items.len()).rev() {
            let need = heights[i] + if i + 1 < items.len() { ITEM_GAP } else { 0.0 };
            if used + need > max_body {
                break;
            }
            used += need;
            last_page = i;
        }
        self.first = self.first.min(last_page.min(items.len().saturating_sub(1)));

        // The items that fit from there.
        let mut shown = 0;
        let mut body = 0.0;
        for h in &heights[self.first..] {
            let need = h + if shown > 0 { ITEM_GAP } else { 0.0 };
            if body + need > max_body && shown > 0 {
                break;
            }
            body += need;
            shown += 1;
        }
        let (before, after) = (self.first, items.len() - self.first - shown);
        if items.is_empty() {
            body = LINE;
        }
        let note = if before + after > 0 { NOTE_HEIGHT } else { 0.0 };
        let panel = Rect::new(screen_width() - NEWSREEL_WIDTH - MARGIN, HEADER_HEIGHT + MARGIN, NEWSREEL_WIDTH, 12.0 + 14.0 + 10.0 + body + note + 12.0);
        self.rect = panel;
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
            return panel;
        }
        for lines in &items[self.first..self.first + shown] {
            draw_circle(panel.x + 16.0, y + LINE * 0.5, 2.5, style::ACCENT);
            for (k, line) in lines.iter().enumerate() {
                style.text_mid(line, panel.x + 28.0, y + LINE * (k as f32 + 0.5), TEXT_SIZE, Weight::Regular, style::TEXT);
            }
            y += lines.len() as f32 * LINE + ITEM_GAP;
        }
        // What is out of sight, and that the wheel brings it in.
        if before + after > 0 {
            let more = match (before, after) {
                (0, after) => format!("{after} more below. Scroll for them."),
                (before, 0) => format!("{before} earlier above. Scroll up for them."),
                (before, after) => format!("{before} above and {after} below. Scroll for them."),
            };
            style.text_mid(&more, panel.x + 14.0, panel.bottom() - 12.0 - NOTE_HEIGHT * 0.5, 10, Weight::Regular, style::TEXT_MUTED);
        }
        panel
    }
}

/// Breaks `s` into lines no wider than `width`, at the spaces.
pub fn wrap(style: &Style, s: &str, size: u16, width: f32) -> Vec<String> {
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
