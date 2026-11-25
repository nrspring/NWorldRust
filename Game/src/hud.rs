//! What the game draws over the map: the header (the country and the day), the footer (the frame rate) and the day's newsreel. The alerts panel under the newsreel is in `alerts`.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use nworld_map::world::Resource;

use crate::city::City;
use crate::country_menu::fit;
use crate::country::{resource_name, Stock};
use crate::resource_hover::Deposit;
use crate::day::DaySave;
use crate::format::{money, thousands, thousands_u64};
use crate::ids::PersonId;
use crate::people_dropdown;

/// Distance of the panels from the edges of the window.
const MARGIN: f32 = 16.0;
const NEWSREEL_WIDTH: f32 = 280.0;

/// How tall the footer is.
pub const FOOTER_HEIGHT: f32 = 48.0;
/// Distance of what is in the footer from its left edge. The options button keeps the same from the right.
pub const FOOTER_PADDING: f32 = 12.0;

/// The size of the text in the header and the footer.
const BAR_TEXT_SIZE: u16 = 13;
/// The widest the name of a person is shown in the header before it is cut short.
const PERSON_WIDTH: f32 = 150.0;
/// How tall the header is.
pub const HEADER_HEIGHT: f32 = 48.0;

/// Where the header is: along the top of the window.
pub fn header_rect() -> Rect {
    Rect::new(0.0, 0.0, screen_width(), HEADER_HEIGHT)
}

/// The buttons of the header, where they are.
pub struct HeaderButtons {
    /// The player's country, for the drop-down to switch to another, once there is one.
    pub country: Option<Rect>,
    /// The small pencil beside it, for editing the country, once there is one.
    pub edit: Option<Rect>,
    /// The person picked, for the drop-down of the people of the country, once there is a country.
    pub person: Option<Rect>,
    /// The day, for ending it.
    pub day: Rect,
    /// The player's country's money, for its treasury, once there is a country.
    pub money: Option<Rect>,
    /// The compass at the right, for turning the map to look straight down with north up.
    pub compass: Rect,
}

/// The header along the top of the window: the player's country, once there is one, which is a button
/// for the countries menu, the person of it who is `selected`, which is a button for the list of its people, and the day, which is a button for ending it (each lit when the mouse is over
/// it, if `interactive`), and the money of the country. At the right is a compass, whose needle points to north as
/// the camera (looking in the direction `yaw`) sees it, which is a button for turning the map north up. Returns
/// where the buttons are.
pub fn header(style: &Style, day: &DaySave, interactive: bool, yaw: f32, selected: Option<PersonId>) -> HeaderButtons {
    let bar = header_rect();
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, style::GLASS);
    draw_line(bar.x, bar.bottom(), bar.right(), bar.bottom(), 1.0, style::BORDER);

    let y = bar.center().y;
    let mut x = bar.x + FOOTER_PADDING;
    let mut button = None;
    let mut edit_button = None;
    let mut person_button = None;
    let mut money_button = None;
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
        // A small pencil beside it, to edit the country.
        let edit = Rect::new(pill.right() + 2.0, y - 13.0, 26.0, 26.0);
        let edit_hovered = interactive && edit.contains(Vec2::from(mouse_position()));
        if edit_hovered {
            style::rounded_rect(edit, 8.0, style::HOVER);
        }
        draw_pencil(edit.center(), if edit_hovered { style::TEXT } else { style::TEXT_MUTED });
        // A dot, and then the person of the country who is picked, which is a drop-down of its people.
        draw_circle(edit.right() + 8.0, y, 2.0, style::TEXT_MUTED);
        let picked = people_dropdown::shown(day, selected).is_some();
        let weight = if picked { Weight::Semibold } else { Weight::Regular };
        let name = fit(style, people_dropdown::label(day, selected), BAR_TEXT_SIZE, weight, PERSON_WIDTH);
        let person = Rect::new(edit.right() + 18.0, y - 14.0, style.measure(&name, BAR_TEXT_SIZE, weight).width + 52.0, 28.0);
        if interactive && person.contains(Vec2::from(mouse_position())) {
            style::rounded_rect(person, 8.0, style::HOVER);
        }
        let color = if picked { style::TEXT } else { style::TEXT_MUTED };
        draw_person(vec2(person.x + 16.0, y), color);
        style.text_mid(&name, person.x + 28.0, y, BAR_TEXT_SIZE, weight, color);
        let arrow = person.right() - 13.0;
        draw_triangle(vec2(arrow - 4.0, y - 1.5), vec2(arrow + 4.0, y - 1.5), vec2(arrow, y + 2.5), style::TEXT_MUTED);
        // A dot between the person and the day.
        draw_circle(person.right() + 8.0, y, 2.0, style::TEXT_MUTED);
        x = person.right() + 28.0;
        button = Some(pill);
        person_button = Some(person);
        edit_button = Some(edit);
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
        // The coin and the amount are a button, lit when the mouse is over it.
        let amount_width = style.measure(&money(country.money), BAR_TEXT_SIZE, Weight::Semibold).width;
        let pill = Rect::new(x + 22.0, y - 14.0, 43.0 - 22.0 + amount_width + 8.0, 28.0);
        if interactive && pill.contains(Vec2::from(mouse_position())) {
            style::rounded_rect(pill, 8.0, style::HOVER);
        }
        money_button = Some(pill);
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
    HeaderButtons { country: button, edit: edit_button, person: person_button, day: day_button, money: money_button, compass }
}

/// A small head and shoulders, about 12 pixels tall, centered on `c`.
fn draw_person(c: Vec2, color: Color) {
    draw_circle(c.x, c.y - 3.0, 3.2, color);
    style::rounded_rect(Rect::new(c.x - 5.5, c.y + 1.0, 11.0, 5.5), 2.7, color);
}

/// A small pencil, about 12 pixels across, centered on `c`.
fn draw_pencil(c: Vec2, color: Color) {
    draw_line(c.x - 3.0, c.y + 3.0, c.x + 4.0, c.y - 4.0, 3.6, color);
    draw_triangle(vec2(c.x - 6.0, c.y + 6.0), vec2(c.x - 6.0, c.y + 1.5), vec2(c.x - 1.5, c.y + 6.0), color);
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

/// The footer along the bottom of the window: the frame rate at the left.
/// The Travel, Buildings, destroy, overlays and options buttons go at the right, which `Toolbar` (for the first two),
/// `DestroyTool`, `OverlaysUi` and `OptionsUi` draw.
/// Returns where the footer is.
pub fn footer(style: &Style) -> Rect {
    let bar = Rect::new(0.0, screen_height() - FOOTER_HEIGHT, screen_width(), FOOTER_HEIGHT);
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, style::GLASS);
    draw_line(bar.x, bar.y, bar.right(), bar.y, 1.0, style::BORDER);

    let y = bar.center().y;
    let fps_x = bar.x + FOOTER_PADDING + 8.0;
    // The number is right-aligned in room for four digits, so the label after it never moves when it gains a digit.
    let number = get_fps().to_string();
    let room = style.measure("8888", BAR_TEXT_SIZE, Weight::Regular).width;
    let number_width = style.measure(&number, BAR_TEXT_SIZE, Weight::Regular).width;
    style.text_mid(&number, fps_x + room - number_width, y, BAR_TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
    style.text_mid("fps", fps_x + room + 5.0, y, BAR_TEXT_SIZE, Weight::Regular, style::TEXT_MUTED);
    bar
}

/// The color a region glows under "Units" when a unit of the country in it still has moves left, green like the ring on its bubble.
pub const MOVES_GLOW: [u8; 3] = [89, 255, 89];

/// A label in the footer that does something while the mouse is on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterLabel {
    /// Lights up the regions that have units of the country played in them.
    Units,
    /// Lights up the regions that have an alert about something in them.
    Alerts,
    /// Lights up the regions that have units of other countries in them.
    Enemy,
}

impl FooterLabel {
    /// Every label, in the order they are drawn, from the left.
    const ALL: [FooterLabel; 3] = [FooterLabel::Units, FooterLabel::Alerts, FooterLabel::Enemy];

    fn text(self) -> &'static str {
        match self {
            Self::Units => "Units",
            Self::Alerts => "Alerts",
            Self::Enemy => "Enemy",
        }
    }

    /// The color the regions it lights up glow, red, green and blue. (Units that still have moves left glow [`MOVES_GLOW`] instead.)
    pub fn glow(self) -> [u8; 3] {
        match self {
            Self::Units => [255, 102, 255],
            Self::Alerts => [245, 176, 51],
            Self::Enemy => [255, 71, 71],
        }
    }
}

/// Draws the labels in the footer `bar`, after the frame rate, and returns the one the mouse is on, if it is on one and `interactive`.
pub fn footer_labels(style: &Style, bar: Rect, interactive: bool) -> Option<FooterLabel> {
    let mouse = Vec2::from(mouse_position());
    let y = bar.center().y;
    // After the frame rate (see `footer`), with room between.
    let fps = bar.x + FOOTER_PADDING + 8.0 + style.measure("8888", BAR_TEXT_SIZE, Weight::Regular).width + 5.0 + style.measure("fps", BAR_TEXT_SIZE, Weight::Regular).width;
    let mut x = fps + 24.0;
    let mut hovered = None;
    for label in FooterLabel::ALL {
        let width = style.measure(label.text(), BAR_TEXT_SIZE, Weight::Semibold).width;
        let pill = Rect::new(x, y - 14.0, width + 20.0, 28.0);
        let over = interactive && pill.contains(mouse);
        if over {
            style::rounded_rect(pill, 8.0, style::HOVER);
            hovered = Some(label);
        }
        style.text_mid(label.text(), pill.x + 10.0, y, BAR_TEXT_SIZE, Weight::Semibold, if over { style::TEXT } else { style::TEXT_MUTED });
        x = pill.right() + 6.0;
    }
    hovered
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

/// A small note next to the mouse about the resource it is over: what it is, with a dot of its color, and how rich the
/// deposit is.
pub fn resource_tooltip(style: &Style, deposit: Deposit) {
    let name = resource_name(deposit.resource);
    let title = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
    let what = format!("{} deposit", deposit.richness());
    let width = style.measure(&what, 12, Weight::Regular).width.max(style.measure(&title, 15, Weight::Semibold).width + 18.0) + 32.0;
    let height = 14.0 + 22.0 + 18.0 + 8.0;
    let mouse = Vec2::from(mouse_position());
    let panel = Rect::new((mouse.x + 20.0).min(screen_width() - width - MARGIN), (mouse.y + 20.0).min(screen_height() - height - FOOTER_HEIGHT - 8.0), width, height);
    style::card(panel, 10.0, style::GLASS);
    draw_circle(panel.x + 16.0 + 4.0, panel.y + 14.0 + 10.0, 4.0, resource_color(deposit.resource));
    style.text_mid(&title, panel.x + 16.0 + 18.0, panel.y + 14.0 + 10.0, 15, Weight::Semibold, style::TEXT);
    style.text_mid(&what, panel.x + 16.0, panel.y + 14.0 + 22.0 + 9.0, 12, Weight::Regular, style::TEXT_MUTED);
}

/// The prompt for placing a city, above the footer: the first city, or a new one settled with people from the city called
/// `settling_from`. `problem` says why the city can't go where the mouse is, if it can't.
pub fn placing_prompt(style: &Style, settling_from: Option<&str>, problem: Option<&str>) {
    let title = settling_from.map_or_else(|| "Found your first city".to_owned(), |from| format!("Settle a new city from {from}"));
    let hint = match settling_from {
        Some(_) => format!("Click on dry land to place it. It takes {} people. Esc to cancel.", crate::city::SETTLERS),
        None => format!("Click on dry land to place it. It starts with {} people.", crate::city::INITIAL_POPULATION),
    };
    prompt(style, &title, &hint, problem);
}

/// The prompt while the person called `name` is being put on the map, saying why they can't go under the mouse, if they can't.
pub fn person_prompt(style: &Style, name: &str, problem: Option<&str>) {
    prompt(style, &format!("Place {name} on the map"), "Click on dry land to put them there. Esc to do it later.", problem);
}

/// A card above the footer saying what is being placed: `title`, and under it `hint`, or `problem` in its stead if there is one.
fn prompt(style: &Style, title: &str, hint: &str, problem: Option<&str>) {
    let panel = Rect::new((screen_width() - 460.0) * 0.5, screen_height() - FOOTER_HEIGHT - MARGIN - 72.0, 460.0, 72.0);
    style::card(panel, 12.0, style::GLASS);
    style.text_mid(title, panel.x + 20.0, panel.y + 24.0, 16, Weight::Semibold, style::TEXT);
    let (text, color) = problem.map_or((hint, style::TEXT_MUTED), |problem| (problem, style::ERROR));
    style.text_mid(text, panel.x + 20.0, panel.y + 48.0, 14, Weight::Regular, color);
}

/// The day's newsreel, at the top right, with the newest news on top. It is as tall as its news, up to a limit (about a fifth of the
/// window's height), and past that it shows as many items as fit and the mouse wheel scrolls through the rest, an item at a time.
#[derive(Default)]
pub struct NewsreelPanel {
    /// The first item showing.
    first: usize,
    /// Where it was drawn last, so the mouse can be told it is over it.
    rect: Rect,
    /// Whether the player clicked the button to add a note since it was last asked.
    add_clicked: bool,
    /// The headlines as they were last shown, and each broken into lines, which is only done again when they change: it
    /// measures the text a word at a time, which is too slow to do for every item every frame.
    wrapped: (Vec<String>, Vec<Vec<String>>),
}

impl NewsreelPanel {
    /// Whether the player has clicked the button to add a note, which it then forgets.
    pub fn take_add_clicked(&mut self) -> bool {
        std::mem::take(&mut self.add_clicked)
    }

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

        // The newest first.
        let headlines: Vec<String> = day.newsreel.iter().rev().map(|n| n.headline(day)).collect();
        if headlines != self.wrapped.0 {
            let lines = headlines.iter().map(|headline| wrap(style, headline, TEXT_SIZE, text_width)).collect();
            self.wrapped = (headlines, lines);
        }
        let items = &self.wrapped.1;
        let heights: Vec<f32> = items.iter().map(|lines| lines.len() as f32 * LINE).collect();
        // The most the body of the panel may take: a good part of the window, and room left for what is under it.
        let room = screen_height() - HEADER_HEIGHT - FOOTER_HEIGHT - MARGIN * 2.0 - 110.0;
        let max_body = (room.min(screen_height() * 0.45) * 0.5).max(60.0);

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
        // A quiet plus for adding a note, left of the day: no box until the mouse is over it.
        let add = Rect::new(panel.right() - 12.0 - date_w - 8.0 - 18.0, header_y - 9.0, 18.0, 18.0);
        let over_add = add.contains(Vec2::from(mouse_position()));
        if over_add {
            style::rounded_rect(add, 5.0, style::HOVER);
        }
        let plus_color = if over_add { style::TEXT } else { style::TEXT_MUTED };
        draw_line(add.center().x - 4.5, add.center().y, add.center().x + 4.5, add.center().y, 1.5, plus_color);
        draw_line(add.center().x, add.center().y - 4.5, add.center().x, add.center().y + 4.5, 1.5, plus_color);
        if over_add && is_mouse_button_pressed(MouseButton::Left) {
            self.add_clicked = true;
        }
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
                (0, after) => format!("{after} earlier below. Scroll for them."),
                (before, 0) => format!("{before} newer above. Scroll up for them."),
                (before, after) => format!("{before} newer above and {after} earlier below. Scroll for them."),
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
