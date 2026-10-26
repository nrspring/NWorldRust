//! Photoshop-style editor UI: a vertical icon toolbar on the left, where each tool opens a
//! flyout panel beside it.

use macroquad::prelude::*;

use crate::world::{snap_map_size, CHUNK_SIZE, MAX_MAP_SIZE, MIN_MAP_SIZE};

pub const TOOLBAR_WIDTH: f32 = 44.0;
const ICON_SIZE: f32 = 34.0;
const ICON_GAP: f32 = 5.0;
const PANEL_GAP: f32 = 4.0;
const PANEL_WIDTH: f32 = 250.0;
const NEW_MAP_PANEL_HEIGHT: f32 = 184.0;
const DEFAULT_MAP_SIZE: u32 = 4096;
const MAX_DIGITS: usize = 5;

const TOOLBAR_BG: Color = Color::new(0.11, 0.11, 0.12, 1.0);
const PANEL_BG: Color = Color::new(0.15, 0.15, 0.16, 1.0);
const BORDER: Color = Color::new(1.0, 1.0, 1.0, 0.12);
const ICON: Color = Color::new(0.85, 0.85, 0.87, 1.0);
const HOVER_BG: Color = Color::new(1.0, 1.0, 1.0, 0.10);
const ACCENT: Color = Color::new(0.24, 0.47, 0.85, 1.0);
const ACCENT_HOVER: Color = Color::new(0.32, 0.56, 0.95, 1.0);
const TEXT: Color = Color::new(0.92, 0.92, 0.94, 1.0);
const TEXT_DIM: Color = Color::new(0.62, 0.62, 0.66, 1.0);
const FIELD_BG: Color = Color::new(0.08, 0.08, 0.09, 1.0);
const SELECTION: Color = Color::new(0.24, 0.47, 0.85, 0.55);
const ERROR: Color = Color::new(0.90, 0.30, 0.30, 1.0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    NewMap,
}

impl Tool {
    const ALL: [Tool; 1] = [Tool::NewMap];

    fn label(self) -> &'static str {
        match self {
            Tool::NewMap => "New map",
        }
    }
}

pub enum UiAction {
    /// Build a new map of this size in meters (already snapped to a buildable size).
    CreateMap { width: f32, height: f32 },
}

struct NumberField {
    label: &'static str,
    text: String,
    /// The whole value is selected, so the next digit typed replaces it.
    selected: bool,
    invalid: bool,
}

impl NumberField {
    fn new(label: &'static str, value: u32) -> Self {
        Self {
            label,
            text: value.to_string(),
            selected: false,
            invalid: false,
        }
    }

    fn value(&self) -> Option<u32> {
        self.text.parse().ok().filter(|&v| v > 0)
    }
}

pub struct EditorUi {
    open_tool: Option<Tool>,
    /// The New map panel's width and height fields.
    fields: [NumberField; 2],
    focused: Option<usize>,
}

fn icon_rect(index: usize) -> Rect {
    Rect::new(
        (TOOLBAR_WIDTH - ICON_SIZE) * 0.5,
        ICON_GAP + index as f32 * (ICON_SIZE + ICON_GAP),
        ICON_SIZE,
        ICON_SIZE,
    )
}

fn field_rect(panel: Rect, index: usize) -> Rect {
    Rect::new(panel.x + 76.0, panel.y + 44.0 + index as f32 * 36.0, 120.0, 28.0)
}

fn create_button_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 140.0, panel.w - 24.0, 32.0)
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            open_tool: None,
            fields: [
                NumberField::new("Width", DEFAULT_MAP_SIZE),
                NumberField::new("Height", DEFAULT_MAP_SIZE),
            ],
            focused: None,
        }
    }

    fn panel_rect(&self) -> Option<Rect> {
        let tool = self.open_tool?;
        let index = Tool::ALL.iter().position(|&t| t == tool)?;
        let height = match tool {
            Tool::NewMap => NEW_MAP_PANEL_HEIGHT,
        };
        Some(Rect::new(
            TOOLBAR_WIDTH + PANEL_GAP,
            icon_rect(index).y,
            PANEL_WIDTH,
            height,
        ))
    }

    /// True while a text field has focus, so keyboard shortcuts and camera keys should pause.
    pub fn wants_keyboard(&self) -> bool {
        self.focused.is_some()
    }

    /// True while the pointer is over the toolbar or an open panel.
    pub fn mouse_over(&self) -> bool {
        let mouse: Vec2 = mouse_position().into();
        mouse.x < TOOLBAR_WIDTH || self.panel_rect().is_some_and(|p| p.contains(mouse))
    }

    /// Handles Esc: drops field focus first, then closes the open panel. Returns false when
    /// there was nothing to close.
    pub fn escape(&mut self) -> bool {
        if self.focused.take().is_some() {
            return true;
        }
        self.open_tool.take().is_some()
    }

    fn focus(&mut self, index: Option<usize>) {
        self.focused = index;
        if let Some(i) = index {
            self.fields[i].selected = true;
        }
    }

    pub fn update(&mut self) -> Option<UiAction> {
        // Always drain typed characters so keys pressed outside a field never show up later.
        let mut typed = Vec::new();
        while let Some(c) = get_char_pressed() {
            typed.push(c);
        }
        let mouse: Vec2 = mouse_position().into();
        let clicked = is_mouse_button_pressed(MouseButton::Left);

        if clicked {
            if let Some(i) = (0..Tool::ALL.len()).find(|&i| icon_rect(i).contains(mouse)) {
                let tool = Tool::ALL[i];
                let opening = self.open_tool != Some(tool);
                self.open_tool = opening.then_some(tool);
                // Opening New map puts the cursor straight into the width field.
                self.focus(opening.then_some(0));
                return None;
            }
        }

        let panel = self.panel_rect()?;
        let mut submit = false;
        if clicked {
            let hit = (0..self.fields.len()).find(|&i| field_rect(panel, i).contains(mouse));
            self.focus(hit);
            submit = create_button_rect(panel).contains(mouse);
        }

        if let Some(i) = self.focused {
            let field = &mut self.fields[i];
            for c in typed.into_iter().filter(char::is_ascii_digit) {
                if field.selected {
                    field.text.clear();
                    field.selected = false;
                }
                if field.text.len() < MAX_DIGITS {
                    field.text.push(c);
                }
                field.invalid = false;
            }
            if is_key_pressed(KeyCode::Backspace) {
                if field.selected {
                    field.text.clear();
                    field.selected = false;
                } else {
                    field.text.pop();
                }
                field.invalid = false;
            }
            if is_key_pressed(KeyCode::Tab) {
                self.focus(Some((i + 1) % self.fields.len()));
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                submit = true;
            }
        }

        if submit { self.submit() } else { None }
    }

    fn submit(&mut self) -> Option<UiAction> {
        for field in &mut self.fields {
            field.invalid = field.value().is_none();
        }
        let width = snap_map_size(self.fields[0].value()? as f32);
        let height = snap_map_size(self.fields[1].value()? as f32);
        // Show the size actually built, in case it was rounded or clamped.
        self.fields[0].text = format!("{width}");
        self.fields[1].text = format!("{height}");
        self.open_tool = None;
        self.focused = None;
        Some(UiAction::CreateMap { width, height })
    }

    pub fn draw(&self) {
        let mouse: Vec2 = mouse_position().into();

        draw_rectangle(0.0, 0.0, TOOLBAR_WIDTH, screen_height(), TOOLBAR_BG);
        draw_line(TOOLBAR_WIDTH, 0.0, TOOLBAR_WIDTH, screen_height(), 1.0, BORDER);

        for (i, &tool) in Tool::ALL.iter().enumerate() {
            let r = icon_rect(i);
            let hovered = r.contains(mouse);
            let active = self.open_tool == Some(tool);
            if active {
                draw_rectangle(r.x, r.y, r.w, r.h, ACCENT);
            } else if hovered {
                draw_rectangle(r.x, r.y, r.w, r.h, HOVER_BG);
            }
            draw_tool_icon(tool, r);
            if hovered && !active {
                draw_tooltip(tool.label(), r.right() + 10.0, r.center().y);
            }
        }

        if let Some(panel) = self.panel_rect() {
            draw_rectangle(panel.x, panel.y, panel.w, panel.h, PANEL_BG);
            draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.0, BORDER);
            self.draw_new_map_panel(panel, mouse);
        }
    }

    fn draw_new_map_panel(&self, panel: Rect, mouse: Vec2) {
        draw_text("New map", panel.x + 12.0, panel.y + 26.0, 22.0, TEXT);

        for (i, field) in self.fields.iter().enumerate() {
            let r = field_rect(panel, i);
            let focused = self.focused == Some(i);
            draw_text(field.label, panel.x + 12.0, r.y + 19.0, 20.0, TEXT_DIM);

            draw_rectangle(r.x, r.y, r.w, r.h, FIELD_BG);
            let outline = if field.invalid {
                ERROR
            } else if focused {
                ACCENT
            } else {
                BORDER
            };
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if focused { 2.0 } else { 1.0 }, outline);

            let text_x = r.x + 8.0;
            let baseline = r.y + 19.0;
            let text_w = measure_text(&field.text, None, 20, 1.0).width;
            if focused && field.selected && !field.text.is_empty() {
                draw_rectangle(text_x - 2.0, r.y + 5.0, text_w + 4.0, r.h - 10.0, SELECTION);
            }
            draw_text(&field.text, text_x, baseline, 20.0, TEXT);
            let caret_on = (get_time() * 2.0) as i64 % 2 == 0;
            if focused && !field.selected && caret_on {
                let cx = text_x + text_w + 1.0;
                draw_line(cx, r.y + 6.0, cx, r.bottom() - 6.0, 1.5, TEXT);
            }
            draw_text("m", r.right() + 8.0, baseline, 20.0, TEXT_DIM);
        }

        let hint = format!(
            "{:.0} - {:.0} m, rounded to {:.0} m",
            MIN_MAP_SIZE, MAX_MAP_SIZE, CHUNK_SIZE
        );
        draw_text(&hint, panel.x + 12.0, panel.y + 128.0, 16.0, TEXT_DIM);

        let b = create_button_rect(panel);
        let fill = if b.contains(mouse) { ACCENT_HOVER } else { ACCENT };
        draw_rectangle(b.x, b.y, b.w, b.h, fill);
        let label = "Create map";
        let size = measure_text(label, None, 20, 1.0);
        draw_text(
            label,
            b.x + (b.w - size.width) * 0.5,
            b.y + (b.h + size.offset_y) * 0.5 - 1.0,
            20.0,
            TEXT,
        );
    }
}

fn draw_tooltip(text: &str, x: f32, center_y: f32) {
    let size = measure_text(text, None, 18, 1.0);
    let (w, h) = (size.width + 16.0, 24.0);
    let y = center_y - h * 0.5;
    draw_rectangle(x, y, w, h, Color::new(0.05, 0.05, 0.06, 0.95));
    draw_rectangle_lines(x, y, w, h, 1.0, BORDER);
    draw_text(text, x + 8.0, y + 17.0, 18.0, TEXT);
}

fn draw_tool_icon(tool: Tool, r: Rect) {
    let c = r.center();
    match tool {
        Tool::NewMap => {
            // A sheet of paper with a folded corner and a plus: "new document".
            let (w, h, fold) = (16.0, 20.0, 5.0);
            let (left, top) = (c.x - w * 0.5, c.y - h * 0.5);
            let (right, bottom) = (left + w, top + h);
            let t = 1.5;
            draw_line(left, top, right - fold, top, t, ICON);
            draw_line(right - fold, top, right, top + fold, t, ICON);
            draw_line(right, top + fold, right, bottom, t, ICON);
            draw_line(right, bottom, left, bottom, t, ICON);
            draw_line(left, bottom, left, top, t, ICON);
            draw_line(right - fold, top, right - fold, top + fold, t, ICON);
            draw_line(right - fold, top + fold, right, top + fold, t, ICON);
            let (px, py) = (c.x - 1.0, c.y + 2.0);
            draw_line(px - 4.0, py, px + 4.0, py, t, ICON);
            draw_line(px, py - 4.0, px, py + 4.0, t, ICON);
        }
    }
}
