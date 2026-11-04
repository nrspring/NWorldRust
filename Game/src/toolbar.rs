//! The tool menus in the footer: Travel, for paths, roads and bridges, and Buildings, for the gatherers. Each is a button
//! that opens a panel above it with a row for every tool in it, and picking one turns that tool on and closes the panel.
//! The destroy tool has a button of its own, beside them.
//!
//! The rows are drawn by the tools themselves (see [`draw_row`]), since each knows when it can't be used and what to say
//! about it; the panel they go in is drawn here.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::gatherer::GathererKind;
use crate::shipyard::ShipyardKind;
use crate::hud;

const BUTTON_HEIGHT: f32 = 36.0;
const PANEL_WIDTH: f32 = 300.0;
const HEADER_HEIGHT: f32 = 44.0;
const ROW_HEIGHT: f32 = 40.0;
const GAP: f32 = 8.0;
const TEXT_SIZE: u16 = 13;

/// A menu of tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Travel,
    Buildings,
}

impl Menu {
    /// Every menu, in the order the footer has them from the left.
    pub const ALL: [Menu; 2] = [Menu::Travel, Menu::Buildings];

    fn label(self) -> &'static str {
        match self {
            Self::Travel => "Travel",
            Self::Buildings => "Buildings",
        }
    }

    /// Where it is in [`ALL`](Self::ALL).
    fn index(self) -> usize {
        match self {
            Self::Travel => 0,
            Self::Buildings => 1,
        }
    }

    /// How many tools it has: a path, a road and a bridge, or one for each kind of gatherer and the one kind of shipyard that can be built.
    fn rows(self) -> usize {
        match self {
            Self::Travel => 3,
            Self::Buildings => GathererKind::ALL.len() + ShipyardKind::BUILDABLE.len(),
        }
    }
}

/// The two menu buttons, and the panel of the one that is open.
#[derive(Default)]
pub struct Toolbar {
    open: Option<Menu>,
    /// Where the panel was drawn last, so the mouse can be told it is over it.
    panel: Option<Rect>,
}

impl Toolbar {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn close(&mut self) {
        self.open = None;
        self.panel = None;
    }

    /// Whether the mouse is over the panel, and so is not for the map. (The buttons are in the footer, which is the
    /// options button's to say.)
    pub fn has_mouse(&self, mouse: Vec2) -> bool {
        self.panel.is_some_and(|p| p.contains(mouse))
    }

    /// Draws the buttons in the `footer`, with the right edge of the last at `right`, and the panel above the one that is
    /// open. A menu's button is lit while it is open and while a tool in it is on (`lit` says, for each of [`Menu::ALL`]).
    /// Returns the open menu and where its panel is, for the caller to draw the tools' rows in with [`row_rect`].
    pub fn show(&mut self, style: &Style, footer: Rect, right: f32, lit: [bool; 2]) -> Option<(Menu, Rect)> {
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        let mut right = right;
        let mut buttons = [Rect::default(); 2];
        // From the right, so that they read Travel and then Buildings from the left.
        for menu in Menu::ALL.into_iter().rev() {
            let label = menu.label();
            let width = style.measure(label, TEXT_SIZE, Weight::Semibold).width + 14.0 + 12.0 + 8.0 + 8.0;
            let button = Rect::new(right - width, footer.center().y - BUTTON_HEIGHT * 0.5, width, BUTTON_HEIGHT);
            buttons[menu.index()] = button;
            right = button.x - GAP;

            let hovered = button.contains(mouse);
            if hovered && clicked {
                self.open = if self.open == Some(menu) { None } else { Some(menu) };
            }
            let open = self.open == Some(menu);
            if open || lit[menu.index()] {
                style::rounded_rect(button, 10.0, style::ACCENT_SOFT);
            } else if hovered {
                style::rounded_rect(button, 10.0, style::HOVER);
            }
            let color = if open || lit[menu.index()] { style::ACCENT_HOVER } else { style::TEXT };
            style.text_mid(label, button.x + 14.0, button.center().y, TEXT_SIZE, Weight::Semibold, color);
            // A small arrow, pointing up as the panel opens upward from here.
            let arrow = vec2(button.right() - 16.0, button.center().y);
            draw_triangle(vec2(arrow.x - 4.0, arrow.y + 2.0), vec2(arrow.x + 4.0, arrow.y + 2.0), vec2(arrow.x, arrow.y - 3.0), color);
        }

        let Some(menu) = self.open else {
            self.panel = None;
            return None;
        };
        let button = buttons[menu.index()];
        let height = HEADER_HEIGHT + ROW_HEIGHT * menu.rows() as f32 + GAP;
        let panel = Rect::new((button.right() - PANEL_WIDTH).max(GAP), footer.y - GAP - height, PANEL_WIDTH, height);
        self.panel = Some(panel);
        style::card(panel, 14.0, style::GLASS);
        style.text_mid(menu.label(), panel.x + 16.0, panel.y + HEADER_HEIGHT * 0.5, 16, Weight::Semibold, style::TEXT);
        draw_line(panel.x + 16.0, panel.y + HEADER_HEIGHT - 1.0, panel.right() - 16.0, panel.y + HEADER_HEIGHT - 1.0, 1.0, style::BORDER);
        Some((menu, panel))
    }
}

/// Where the row for the `n`th tool of the menu with `panel` is.
pub fn row_rect(panel: Rect, n: usize) -> Rect {
    Rect::new(panel.x + 8.0, panel.y + HEADER_HEIGHT + ROW_HEIGHT * n as f32, panel.w - 16.0, ROW_HEIGHT)
}

/// What the player did with a row.
pub struct Row {
    /// Whether the mouse is over it.
    pub hovered: bool,
    /// Whether it was just clicked while it could be.
    pub pressed: bool,
}

/// Draws the row of a tool in `rect`: its icon (drawn by `icon` centered at the point it is given, in the color it is
/// given), its `label` and, at the right, what it costs in coins. It is lit when the tool is `active` and dimmed when
/// `disabled`, and then it can't be pressed.
pub fn draw_row(style: &Style, rect: Rect, label: &str, price: i64, active: bool, disabled: bool, icon: impl FnOnce(Vec2, Color)) -> Row {
    let hovered = rect.contains(Vec2::from(mouse_position()));
    let pressed = hovered && !disabled && is_mouse_button_pressed(MouseButton::Left);
    if active {
        style::rounded_rect(rect, 8.0, style::ACCENT_SOFT);
    } else if hovered && !disabled {
        style::rounded_rect(rect, 8.0, style::HOVER);
    }
    let color = match (disabled, active) {
        (true, _) => Color::new(1.0, 1.0, 1.0, 0.30),
        (false, true) => style::ACCENT_HOVER,
        (false, false) => style::TEXT,
    };
    icon(vec2(rect.x + 12.0 + 14.0, rect.center().y), color);
    style.text_mid(label, rect.x + 12.0 + 28.0 + 8.0, rect.center().y, TEXT_SIZE, Weight::Semibold, color);

    let amount = crate::format::money(price);
    let amount_width = style.measure(&amount, 12, Weight::Regular).width;
    let amount_x = rect.right() - 12.0 - amount_width;
    let muted = if disabled { Color::new(1.0, 1.0, 1.0, 0.30) } else { style::TEXT_MUTED };
    style.text_mid(&amount, amount_x, rect.center().y, 12, Weight::Regular, muted);
    hud::draw_coin(vec2(amount_x - 12.0, rect.center().y), 5.5);
    Row { hovered, pressed }
}
