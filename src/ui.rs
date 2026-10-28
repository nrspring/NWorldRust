//! Photoshop-style editor UI: a vertical icon toolbar on the left, where each tool opens a
//! flyout panel beside it.

use macroquad::prelude::*;

use crate::continents::ContinentParams;
use crate::hills::Peak;
use crate::islands::IslandParams;
use crate::style::{self, Style, Weight};
use crate::world::{snap_map_size, CHUNK_SIZE, MAX_MAP_SIZE, MIN_MAP_SIZE};

pub const TOOLBAR_WIDTH: f32 = 52.0;
const ICON_SIZE: f32 = 38.0;
const ICON_GAP: f32 = 6.0;
const TOOLBAR_TOP: f32 = 10.0;
const PANEL_GAP: f32 = 8.0;
const PANEL_WIDTH: f32 = 288.0;
const PANEL_RADIUS: f32 = 10.0;
const PAD: f32 = 16.0;
const INNER_WIDTH: f32 = PANEL_WIDTH - PAD * 2.0;
const CONTROL_HEIGHT: f32 = 36.0;
const CONTROL_RADIUS: f32 = 6.0;
const DEFAULT_MAP_WIDTH: u32 = 10000;
const DEFAULT_MAP_HEIGHT: u32 = 6000;
const MAX_CONTINENTS: u32 = 12;
/// Mountains brush width in meters.
const MOUNTAIN_BRUSH_MIN: f32 = 20.0;
const MOUNTAIN_BRUSH_MAX: f32 = 100.0;
const MOUNTAIN_BRUSH_DEFAULT: f32 = 50.0;
/// Lakes brush width in meters, and the random lakes: how many, and how big across.
const LAKE_BRUSH_MIN: f32 = 10.0;
const LAKE_BRUSH_MAX: f32 = 100.0;
const LAKE_BRUSH_DEFAULT: f32 = 40.0;
const LAKE_COUNT_MIN: f32 = 1.0;
const LAKE_COUNT_MAX: f32 = 20.0;
const LAKE_COUNT_DEFAULT: f32 = 3.0;
const LAKE_SIZE_MIN: f32 = 20.0;
const LAKE_SIZE_MAX: f32 = 600.0;
const LAKE_SIZE_DEFAULT: f32 = 200.0;
/// Rivers: channel width in meters, and how much they wind, as a percentage.
const RIVER_WIDTH_MIN: f32 = 8.0;
const RIVER_WIDTH_MAX: f32 = 120.0;
const RIVER_WIDTH_DEFAULT: f32 = 24.0;
const RIVER_WINDING_DEFAULT: f32 = 50.0;
const RIVER_COUNT_MAX: f32 = 10.0;
/// Shallows brush width in meters, and how far out from the coast the shelf of shallows reaches.
const SHALLOW_BRUSH_MIN: f32 = 30.0;
const SHALLOW_BRUSH_MAX: f32 = 600.0;
const SHALLOW_BRUSH_DEFAULT: f32 = 150.0;
const SHELF_WIDTH_MIN: f32 = 10.0;
const SHELF_WIDTH_MAX: f32 = 100.0;
const SHELF_WIDTH_DEFAULT: f32 = 40.0;
/// Random mountain ranges: length along the range and width across it, in meters.
const RANGE_LENGTH_MIN: f32 = 500.0;
const RANGE_LENGTH_MAX: f32 = 4000.0;
const RANGE_LENGTH_DEFAULT: f32 = 1500.0;
const RANGE_WIDTH_MIN: f32 = 60.0;
const RANGE_WIDTH_MAX: f32 = 300.0;
const RANGE_WIDTH_DEFAULT: f32 = 120.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    NewMap,
    Continents,
    Islands,
    Mountains,
    Lakes,
    Rivers,
    Shallows,
}

impl Tool {
    const ALL: [Tool; 7] =
        [Tool::NewMap, Tool::Continents, Tool::Islands, Tool::Mountains, Tool::Lakes, Tool::Rivers, Tool::Shallows];

    fn label(self) -> &'static str {
        match self {
            Tool::NewMap => "New map",
            Tool::Continents => "Continents",
            Tool::Islands => "Islands",
            Tool::Mountains => "Mountains",
            Tool::Lakes => "Lakes",
            Tool::Rivers => "Rivers",
            Tool::Shallows => "Shallows",
        }
    }
}

pub enum UiAction {
    /// Build a new map of this size in meters (already snapped to a buildable size).
    CreateMap { width: f32, height: f32 },
    /// Lay out land and sea on the open map.
    GenerateContinents(ContinentParams),
    /// Sprinkle islands into the open water on the open map.
    AddIslands(IslandParams),
    /// Raise a randomly placed mountain range, this long and wide in meters.
    AddMountainRange { length: f32, width: f32, seed: u32 },
    /// Dig this many randomly placed lakes, about this many meters across.
    AddLakes { count: u32, size: f32, seed: u32 },
    /// Dig this many rivers from random inland points to the sea, with this channel width in
    /// meters and winding from 0 to 1.
    AddRivers { count: u32, width: f32, winding: f32, seed: u32 },
    /// Make the sea this many meters out from every coast shallow.
    AddShallows { width: f32 },
    /// Undo the most recent edit.
    Undo,
    /// Save the map; `save_as` asks for a new file name even if the map already has a file.
    Save { save_as: bool },
    /// Open a map from a file.
    Open,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldId {
    Width,
    Height,
    Seed,
    IslandSeed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SliderId {
    Coverage,
    Roughness,
    Regularity,
    EdgeBuffer,
    IslandCount,
    IslandSize,
    IslandVariety,
    IslandScatter,
    IslandRoughness,
    MountainBrush,
    RangeLength,
    RangeWidth,
    LakeBrush,
    LakeCount,
    LakeSize,
    RiverWidth,
    RiverWinding,
    RiverCount,
    ShallowBrush,
    ShelfWidth,
}

struct NumberField {
    label: &'static str,
    text: String,
    /// Shown inside the field on the right, e.g. "m".
    unit: &'static str,
    min: u32,
    max_digits: usize,
    /// The whole value is selected, so the next digit typed replaces it.
    selected: bool,
    invalid: bool,
}

impl NumberField {
    fn new(label: &'static str, value: u32, unit: &'static str, min: u32, max_digits: usize) -> Self {
        Self {
            label,
            text: value.to_string(),
            unit,
            min,
            max_digits,
            selected: false,
            invalid: false,
        }
    }

    fn value(&self) -> Option<u32> {
        self.text.parse().ok().filter(|&v| v >= self.min)
    }
}

/// A whole-number value picked by dragging along a track.
struct Slider {
    label: &'static str,
    value: f32,
    min: f32,
    max: f32,
    unit: &'static str,
    /// Short captions for the two ends of the track, for sliders whose direction isn't obvious.
    ends: Option<(&'static str, &'static str)>,
}

impl Slider {
    fn fraction(&self) -> f32 {
        (self.value - self.min) / (self.max - self.min)
    }

    fn set_from_x(&mut self, track: Rect, x: f32) {
        let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
        self.value = (self.min + t * (self.max - self.min)).round();
    }
}

/// Where everything in the New map panel sits; shared by input handling and drawing.
struct NewMapLayout {
    panel: Rect,
    width: Rect,
    height: Rect,
    button: Rect,
}

impl NewMapLayout {
    fn new(origin: Vec2) -> Self {
        let field_w = (INNER_WIDTH - 12.0) * 0.5;
        let y = origin.y + 88.0;
        let x = origin.x + PAD;
        let button = Rect::new(x, y + CONTROL_HEIGHT + 40.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            width: Rect::new(x, y, field_w, CONTROL_HEIGHT),
            height: Rect::new(x + field_w + 12.0, y, field_w, CONTROL_HEIGHT),
            button,
        }
    }
}

/// Where everything in the Continents panel sits.
struct ContinentsLayout {
    panel: Rect,
    coverage: Rect,
    roughness: Rect,
    regularity: Rect,
    edge_buffer: Rect,
    count: Rect,
    minus: Rect,
    plus: Rect,
    seed: Rect,
    dice: Rect,
    button: Rect,
}

impl ContinentsLayout {
    fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        // Slider rects are the draggable hit areas; the visible track runs through their middle.
        let slider = |y: f32| Rect::new(x, origin.y + y, INNER_WIDTH, 22.0);
        let coverage = slider(92.0);
        let roughness = slider(154.0);
        let regularity = slider(216.0);
        let edge_buffer = slider(278.0);
        let count = Rect::new(x + INNER_WIDTH - 116.0, origin.y + 322.0, 116.0, 32.0);
        let seed = Rect::new(x, origin.y + 390.0, INNER_WIDTH - CONTROL_HEIGHT - 8.0, CONTROL_HEIGHT);
        let button = Rect::new(x, seed.bottom() + 20.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            coverage,
            roughness,
            regularity,
            edge_buffer,
            count,
            minus: Rect::new(count.x, count.y, 32.0, count.h),
            plus: Rect::new(count.right() - 32.0, count.y, 32.0, count.h),
            seed,
            dice: Rect::new(seed.right() + 8.0, seed.y, CONTROL_HEIGHT, CONTROL_HEIGHT),
            button,
        }
    }
}

/// Where everything in the Islands panel sits.
struct IslandsLayout {
    panel: Rect,
    count: Rect,
    size: Rect,
    variety: Rect,
    scatter: Rect,
    roughness: Rect,
    edge_buffer: Rect,
    seed: Rect,
    dice: Rect,
    button: Rect,
}

impl IslandsLayout {
    fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let slider = |y: f32| Rect::new(x, origin.y + y, INNER_WIDTH, 22.0);
        let seed = Rect::new(x, origin.y + 464.0, INNER_WIDTH - CONTROL_HEIGHT - 8.0, CONTROL_HEIGHT);
        let button = Rect::new(x, seed.bottom() + 20.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            count: slider(92.0),
            size: slider(154.0),
            variety: slider(216.0),
            scatter: slider(278.0),
            roughness: slider(340.0),
            edge_buffer: slider(402.0),
            seed,
            dice: Rect::new(seed.right() + 8.0, seed.y, CONTROL_HEIGHT, CONTROL_HEIGHT),
            button,
        }
    }
}

/// Where everything in the Mountains and Lakes panels sits: a brush size, then two sliders and a
/// button for placing them at random.
struct BrushLayout {
    panel: Rect,
    brush: Rect,
    /// Where the line above the random placement controls sits.
    divider: f32,
    first: Rect,
    second: Rect,
    button: Rect,
}

impl BrushLayout {
    fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let brush = Rect::new(x, origin.y + 92.0, INNER_WIDTH, 22.0);
        let divider = brush.bottom() + 110.0;
        let first = Rect::new(x, divider + 54.0, INNER_WIDTH, 22.0);
        let second = Rect::new(x, first.y + 62.0, INNER_WIDTH, 22.0);
        let button = Rect::new(x, second.bottom() + 28.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            brush,
            divider,
            first,
            second,
            button,
        }
    }
}

/// Where everything in the Rivers panel sits: a width and winding slider, notes, and the
/// random rivers: a count and a button.
struct RiverLayout {
    panel: Rect,
    width: Rect,
    winding: Rect,
    /// Where the line above the random rivers sits.
    divider: f32,
    count: Rect,
    button: Rect,
}

impl RiverLayout {
    fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let slider = |y: f32| Rect::new(x, origin.y + y, INNER_WIDTH, 22.0);
        let divider = origin.y + 312.0;
        let count = slider(divider - origin.y + 102.0);
        let button = Rect::new(x, count.bottom() + 28.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            width: slider(92.0),
            winding: slider(154.0),
            divider,
            count,
            button,
        }
    }
}

/// Where everything in the Shallows panel sits: a brush size and notes, then the shelf along
/// coasts: a width and a button.
struct ShallowLayout {
    panel: Rect,
    brush: Rect,
    /// Where the line above the shelf controls sits.
    divider: f32,
    width: Rect,
    button: Rect,
}

impl ShallowLayout {
    fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let brush = Rect::new(x, origin.y + 92.0, INNER_WIDTH, 22.0);
        let divider = brush.bottom() + 110.0;
        let width = Rect::new(x, divider + 84.0, INNER_WIDTH, 22.0);
        let button = Rect::new(x, width.bottom() + 28.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, button.bottom() + PAD - origin.y),
            brush,
            divider,
            width,
            button,
        }
    }
}

/// What a brush paints.
#[derive(Clone, Copy)]
pub enum Brush {
    Peak(Peak),
    /// Shallow water onto deep sea (left button) or deep water onto shallow (right).
    Shallows,
}

enum Layout {
    NewMap(NewMapLayout),
    Continents(ContinentsLayout),
    Islands(IslandsLayout),
    Mountains(BrushLayout),
    Lakes(BrushLayout),
    Rivers(RiverLayout),
    Shallows(ShallowLayout),
}

impl Layout {
    fn panel(&self) -> Rect {
        match self {
            Layout::NewMap(l) => l.panel,
            Layout::Continents(l) => l.panel,
            Layout::Islands(l) => l.panel,
            Layout::Mountains(l) | Layout::Lakes(l) => l.panel,
            Layout::Rivers(l) => l.panel,
            Layout::Shallows(l) => l.panel,
        }
    }
}

pub struct EditorUi {
    open_tool: Option<Tool>,
    width: NumberField,
    height: NumberField,
    coverage: Slider,
    roughness: Slider,
    regularity: Slider,
    edge_buffer: Slider,
    continent_count: u32,
    seed: NumberField,
    island_count: Slider,
    island_size: Slider,
    island_variety: Slider,
    island_scatter: Slider,
    island_roughness: Slider,
    island_seed: NumberField,
    /// Width of the mountains brush in meters.
    mountain_brush: Slider,
    /// Length and width of a random mountain range, in meters.
    range_length: Slider,
    range_width: Slider,
    /// Width of the lakes brush in meters, and the random lakes' number and width.
    lake_brush: Slider,
    lake_count: Slider,
    lake_size: Slider,
    /// Width of a river's channel in meters, and how much it winds, from 0 to 100.
    river_width: Slider,
    river_winding: Slider,
    /// How many random rivers to add.
    river_count: Slider,
    /// Width of the shallows brush in meters, and how far out from coasts the shelf reaches.
    shallow_brush: Slider,
    shelf_width: Slider,
    focused: Option<FieldId>,
    dragging: Option<SliderId>,
    /// Continents are being generated; the Generate button waits until they're done.
    busy: bool,
}

/// The buttons for actions rather than tools, stacked up from the bottom of the toolbar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionButton {
    Undo,
    Save,
    Open,
}

impl ActionButton {
    const ALL: [ActionButton; 3] = [ActionButton::Open, ActionButton::Save, ActionButton::Undo];

    fn rect(self) -> Rect {
        let slot = match self {
            ActionButton::Undo => 0.0,
            ActionButton::Save => 1.0,
            ActionButton::Open => 2.0,
        };
        Rect::new(
            (TOOLBAR_WIDTH - ICON_SIZE) * 0.5,
            screen_height() - TOOLBAR_TOP - ICON_SIZE - slot * (ICON_SIZE + ICON_GAP),
            ICON_SIZE,
            ICON_SIZE,
        )
    }
}

fn icon_rect(index: usize) -> Rect {
    Rect::new(
        (TOOLBAR_WIDTH - ICON_SIZE) * 0.5,
        TOOLBAR_TOP + index as f32 * (ICON_SIZE + ICON_GAP),
        ICON_SIZE,
        ICON_SIZE,
    )
}

fn random_seed() -> u32 {
    rand::gen_range(1, 999_999_999)
}

impl EditorUi {
    pub fn new() -> Self {
        rand::srand(macroquad::miniquad::date::now() as u64);
        Self {
            open_tool: None,
            width: NumberField::new("Width", DEFAULT_MAP_WIDTH, "m", 1, 5),
            height: NumberField::new("Height", DEFAULT_MAP_HEIGHT, "m", 1, 5),
            coverage: Slider {
                label: "Land to add",
                value: 30.0,
                min: 5.0,
                max: 95.0,
                unit: "%",
                ends: None,
            },
            roughness: Slider {
                label: "Coastline roughness",
                value: 50.0,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Smooth", "Ragged")),
            },
            // 0 gives stretched, lobed continents full of bays and peninsulas; 100 gives
            // compact, rounded ones. The default is well short of the rounded end.
            regularity: Slider {
                label: "Shape regularity",
                value: 35.0,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Irregular", "Regular")),
            },
            // Share of the map's shorter side kept as open water around the edge.
            edge_buffer: Slider {
                label: "Edge buffer",
                value: 6.0,
                // Never zero: continents must not touch the map edge.
                min: 2.0,
                max: 25.0,
                unit: "%",
                ends: None,
            },
            continent_count: 4,
            seed: NumberField::new("Seed", random_seed(), "", 0, 9),
            island_count: Slider {
                label: "Islands",
                value: 20.0,
                min: 1.0,
                max: 200.0,
                unit: "",
                ends: None,
            },
            // Typical island width as a share of the map's shorter side.
            island_size: Slider {
                label: "Island size",
                value: 5.0,
                min: 1.0,
                max: 15.0,
                unit: "%",
                ends: None,
            },
            island_variety: Slider {
                label: "Size variety",
                value: 50.0,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Even", "Varied")),
            },
            island_scatter: Slider {
                label: "Scatter",
                value: 60.0,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Clustered", "Scattered")),
            },
            island_roughness: Slider {
                label: "Coastline roughness",
                value: 60.0,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Smooth", "Ragged")),
            },
            island_seed: NumberField::new("Seed", random_seed(), "", 0, 9),
            mountain_brush: Slider {
                label: "Brush size",
                value: MOUNTAIN_BRUSH_DEFAULT,
                min: MOUNTAIN_BRUSH_MIN,
                max: MOUNTAIN_BRUSH_MAX,
                unit: " m",
                ends: None,
            },
            range_length: Slider {
                label: "Length",
                value: RANGE_LENGTH_DEFAULT,
                min: RANGE_LENGTH_MIN,
                max: RANGE_LENGTH_MAX,
                unit: " m",
                ends: None,
            },
            range_width: Slider {
                label: "Width",
                value: RANGE_WIDTH_DEFAULT,
                min: RANGE_WIDTH_MIN,
                max: RANGE_WIDTH_MAX,
                unit: " m",
                ends: None,
            },
            lake_brush: Slider {
                label: "Brush size",
                value: LAKE_BRUSH_DEFAULT,
                min: LAKE_BRUSH_MIN,
                max: LAKE_BRUSH_MAX,
                unit: " m",
                ends: None,
            },
            lake_count: Slider {
                label: "Lakes",
                value: LAKE_COUNT_DEFAULT,
                min: LAKE_COUNT_MIN,
                max: LAKE_COUNT_MAX,
                unit: "",
                ends: None,
            },
            lake_size: Slider {
                label: "Lake size",
                value: LAKE_SIZE_DEFAULT,
                min: LAKE_SIZE_MIN,
                max: LAKE_SIZE_MAX,
                unit: " m",
                ends: None,
            },
            river_width: Slider {
                label: "Width",
                value: RIVER_WIDTH_DEFAULT,
                min: RIVER_WIDTH_MIN,
                max: RIVER_WIDTH_MAX,
                unit: " m",
                ends: None,
            },
            river_winding: Slider {
                label: "Winding",
                value: RIVER_WINDING_DEFAULT,
                min: 0.0,
                max: 100.0,
                unit: "",
                ends: Some(("Straight", "Winding")),
            },
            river_count: Slider {
                label: "Rivers",
                value: 1.0,
                min: 1.0,
                max: RIVER_COUNT_MAX,
                unit: "",
                ends: None,
            },
            shallow_brush: Slider {
                label: "Brush size",
                value: SHALLOW_BRUSH_DEFAULT,
                min: SHALLOW_BRUSH_MIN,
                max: SHALLOW_BRUSH_MAX,
                unit: " m",
                ends: None,
            },
            shelf_width: Slider {
                label: "Shallows reach",
                value: SHELF_WIDTH_DEFAULT,
                min: SHELF_WIDTH_MIN,
                max: SHELF_WIDTH_MAX,
                unit: " m",
                ends: None,
            },
            focused: None,
            dragging: None,
            busy: false,
        }
    }

    fn layout(&self) -> Option<Layout> {
        let tool = self.open_tool?;
        let index = Tool::ALL.iter().position(|&t| t == tool)?;
        let origin = vec2(TOOLBAR_WIDTH + PANEL_GAP, icon_rect(index).y);
        Some(match tool {
            Tool::NewMap => Layout::NewMap(NewMapLayout::new(origin)),
            Tool::Continents => Layout::Continents(ContinentsLayout::new(origin)),
            Tool::Islands => Layout::Islands(IslandsLayout::new(origin)),
            Tool::Mountains => Layout::Mountains(BrushLayout::new(origin)),
            Tool::Lakes => Layout::Lakes(BrushLayout::new(origin)),
            Tool::Rivers => Layout::Rivers(RiverLayout::new(origin)),
            Tool::Shallows => Layout::Shallows(ShallowLayout::new(origin)),
        })
    }

    fn field(&self, id: FieldId) -> &NumberField {
        match id {
            FieldId::Width => &self.width,
            FieldId::Height => &self.height,
            FieldId::Seed => &self.seed,
            FieldId::IslandSeed => &self.island_seed,
        }
    }

    fn field_mut(&mut self, id: FieldId) -> &mut NumberField {
        match id {
            FieldId::Width => &mut self.width,
            FieldId::Height => &mut self.height,
            FieldId::Seed => &mut self.seed,
            FieldId::IslandSeed => &mut self.island_seed,
        }
    }

    fn slider_mut(&mut self, id: SliderId) -> &mut Slider {
        match id {
            SliderId::Coverage => &mut self.coverage,
            SliderId::Roughness => &mut self.roughness,
            SliderId::Regularity => &mut self.regularity,
            SliderId::EdgeBuffer => &mut self.edge_buffer,
            SliderId::IslandCount => &mut self.island_count,
            SliderId::IslandSize => &mut self.island_size,
            SliderId::IslandVariety => &mut self.island_variety,
            SliderId::IslandScatter => &mut self.island_scatter,
            SliderId::IslandRoughness => &mut self.island_roughness,
            SliderId::MountainBrush => &mut self.mountain_brush,
            SliderId::RangeLength => &mut self.range_length,
            SliderId::RangeWidth => &mut self.range_width,
            SliderId::LakeBrush => &mut self.lake_brush,
            SliderId::LakeCount => &mut self.lake_count,
            SliderId::LakeSize => &mut self.lake_size,
            SliderId::RiverWidth => &mut self.river_width,
            SliderId::RiverWinding => &mut self.river_winding,
            SliderId::RiverCount => &mut self.river_count,
            SliderId::ShallowBrush => &mut self.shallow_brush,
            SliderId::ShelfWidth => &mut self.shelf_width,
        }
    }

    /// Text fields in the open panel, in Tab order.
    fn panel_fields(&self) -> &'static [FieldId] {
        match self.open_tool {
            Some(Tool::NewMap) => &[FieldId::Width, FieldId::Height],
            Some(Tool::Continents) => &[FieldId::Seed],
            Some(Tool::Islands) => &[FieldId::IslandSeed],
            Some(Tool::Mountains) | Some(Tool::Lakes) | Some(Tool::Rivers) | Some(Tool::Shallows) | None => &[],
        }
    }

    /// The channel width in meters and the winding, from 0 to 1, of the river the Rivers tool
    /// digs while it is open, else `None`.
    pub fn river(&self) -> Option<(f32, f32)> {
        (self.open_tool == Some(Tool::Rivers)).then(|| (self.river_width.value, self.river_winding.value / 100.0))
    }

    /// The radius in meters and what it paints while the Mountains, Lakes or Shallows tool is
    /// open, else `None`.
    pub fn brush(&self) -> Option<(f32, Brush)> {
        match self.open_tool {
            Some(Tool::Mountains) => Some((self.mountain_brush.value * 0.5, Brush::Peak(Peak::Mountains))),
            Some(Tool::Lakes) => Some((self.lake_brush.value * 0.5, Brush::Peak(Peak::Lakes))),
            Some(Tool::Shallows) => Some((self.shallow_brush.value * 0.5, Brush::Shallows)),
            _ => None,
        }
    }

    /// Whether a tool's panel is open.
    pub fn tool_open(&self) -> bool {
        self.open_tool.is_some()
    }

    /// True while a text field has focus, so keyboard shortcuts and camera keys should pause.
    pub fn wants_keyboard(&self) -> bool {
        self.focused.is_some()
    }

    /// True while the pointer is over the toolbar or an open panel, or dragging a slider.
    pub fn mouse_over(&self) -> bool {
        let mouse: Vec2 = mouse_position().into();
        self.dragging.is_some()
            || mouse.x < TOOLBAR_WIDTH
            || self.layout().is_some_and(|l| l.panel().contains(mouse))
    }

    /// Call once a requested continent generation has been applied.
    pub fn finish_generating(&mut self) {
        self.busy = false;
    }

    /// Handles Esc: drops field focus first, then closes the open panel. Returns false when
    /// there was nothing to close.
    pub fn escape(&mut self) -> bool {
        if self.focused.take().is_some() {
            return true;
        }
        self.open_tool.take().is_some()
    }

    fn focus(&mut self, id: Option<FieldId>) {
        self.focused = id;
        if let Some(id) = id {
            self.field_mut(id).selected = true;
        }
    }

    /// `has_map` enables the tools that work on an open map; `undo_steps` is how many edits
    /// can be undone.
    pub fn update(&mut self, has_map: bool, undo_steps: usize) -> Option<UiAction> {
        // Always drain typed characters so keys pressed outside a field never show up later.
        let mut typed = Vec::new();
        while let Some(c) = get_char_pressed() {
            typed.push(c);
        }
        let mouse: Vec2 = mouse_position().into();
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        if !is_mouse_button_down(MouseButton::Left) {
            self.dragging = None;
        }

        if clicked {
            if ActionButton::Undo.rect().contains(mouse) {
                return (undo_steps > 0).then_some(UiAction::Undo);
            }
            if ActionButton::Save.rect().contains(mouse) {
                let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                return has_map.then_some(UiAction::Save { save_as: shift });
            }
            if ActionButton::Open.rect().contains(mouse) {
                return Some(UiAction::Open);
            }
        }

        if clicked {
            if let Some(i) = (0..Tool::ALL.len()).find(|&i| icon_rect(i).contains(mouse)) {
                let tool = Tool::ALL[i];
                let opening = self.open_tool != Some(tool);
                self.open_tool = opening.then_some(tool);
                // Opening New map puts the cursor straight into the width field.
                self.focus((opening && tool == Tool::NewMap).then_some(FieldId::Width));
                return None;
            }
        }

        let layout = self.layout()?;
        let mut submit = false;
        match &layout {
            Layout::Mountains(l) | Layout::Lakes(l) => {
                let (brush, first, second) = if self.open_tool == Some(Tool::Lakes) {
                    (SliderId::LakeBrush, SliderId::LakeCount, SliderId::LakeSize)
                } else {
                    (SliderId::MountainBrush, SliderId::RangeLength, SliderId::RangeWidth)
                };
                let sliders = [(l.brush, brush), (l.first, first), (l.second, second)];
                if clicked {
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    }
                    submit = l.button.contains(mouse);
                }
                if let Some(id) = self.dragging {
                    if let Some((track, _)) = sliders.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
                // [ and ] resize the brush without leaving the map.
                let slider = self.slider_mut(brush);
                let notch = (slider.max - slider.min) / 58.0;
                for (key, sign) in [(KeyCode::LeftBracket, -1.0), (KeyCode::RightBracket, 1.0)] {
                    if is_key_pressed(key) {
                        slider.value = (slider.value + sign * notch).round().clamp(slider.min, slider.max);
                    }
                }
            }
            Layout::NewMap(l) => {
                if clicked {
                    let hit = [(l.width, FieldId::Width), (l.height, FieldId::Height)]
                        .into_iter()
                        .find(|(r, _)| r.contains(mouse));
                    self.focus(hit.map(|(_, id)| id));
                    submit = l.button.contains(mouse);
                }
            }
            Layout::Continents(l) => {
                if clicked {
                    self.focus(l.seed.contains(mouse).then_some(FieldId::Seed));
                    let slider_hit = [
                        (l.coverage, SliderId::Coverage),
                        (l.roughness, SliderId::Roughness),
                        (l.regularity, SliderId::Regularity),
                        (l.edge_buffer, SliderId::EdgeBuffer),
                    ]
                    .into_iter()
                    .find(|(r, _)| r.contains(mouse));
                    if let Some((_, id)) = slider_hit {
                        self.dragging = Some(id);
                    } else if l.minus.contains(mouse) {
                        self.continent_count = (self.continent_count - 1).max(1);
                    } else if l.plus.contains(mouse) {
                        self.continent_count = (self.continent_count + 1).min(MAX_CONTINENTS);
                    } else if l.dice.contains(mouse) {
                        self.seed.text = random_seed().to_string();
                        self.seed.invalid = false;
                    }
                    submit = l.button.contains(mouse);
                }
                if let Some(id) = self.dragging {
                    let tracks = [
                        (l.coverage, SliderId::Coverage),
                        (l.roughness, SliderId::Roughness),
                        (l.regularity, SliderId::Regularity),
                        (l.edge_buffer, SliderId::EdgeBuffer),
                    ];
                    if let Some((track, _)) = tracks.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
            }
            Layout::Shallows(l) => {
                let sliders = [(l.brush, SliderId::ShallowBrush), (l.width, SliderId::ShelfWidth)];
                if clicked {
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    }
                    submit = l.button.contains(mouse);
                }
                if let Some(id) = self.dragging {
                    if let Some((track, _)) = sliders.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
                // [ and ] resize the brush without leaving the map.
                let slider = self.slider_mut(SliderId::ShallowBrush);
                let notch = (slider.max - slider.min) / 58.0;
                for (key, sign) in [(KeyCode::LeftBracket, -1.0), (KeyCode::RightBracket, 1.0)] {
                    if is_key_pressed(key) {
                        slider.value = (slider.value + sign * notch).round().clamp(slider.min, slider.max);
                    }
                }
            }
            Layout::Rivers(l) => {
                let sliders = [
                    (l.width, SliderId::RiverWidth),
                    (l.winding, SliderId::RiverWinding),
                    (l.count, SliderId::RiverCount),
                ];
                if clicked {
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    }
                    submit = l.button.contains(mouse);
                }
                if let Some(id) = self.dragging {
                    if let Some((track, _)) = sliders.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
            }
            Layout::Islands(l) => {
                let sliders = [
                    (l.count, SliderId::IslandCount),
                    (l.size, SliderId::IslandSize),
                    (l.variety, SliderId::IslandVariety),
                    (l.scatter, SliderId::IslandScatter),
                    (l.roughness, SliderId::IslandRoughness),
                    (l.edge_buffer, SliderId::EdgeBuffer),
                ];
                if clicked {
                    self.focus(l.seed.contains(mouse).then_some(FieldId::IslandSeed));
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    } else if l.dice.contains(mouse) {
                        self.island_seed.text = random_seed().to_string();
                        self.island_seed.invalid = false;
                    }
                    submit = l.button.contains(mouse);
                }
                if let Some(id) = self.dragging {
                    if let Some((track, _)) = sliders.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
            }
        }

        if let Some(id) = self.focused {
            let field = self.field_mut(id);
            for c in typed.into_iter().filter(char::is_ascii_digit) {
                if field.selected {
                    field.text.clear();
                    field.selected = false;
                }
                if field.text.len() < field.max_digits {
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
                let order = self.panel_fields();
                let next = order.iter().position(|&f| f == id).map_or(0, |p| (p + 1) % order.len());
                self.focus(Some(order[next]));
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                submit = true;
            }
        }

        if !submit {
            return None;
        }
        match layout {
            Layout::NewMap(_) => self.submit_new_map(),
            Layout::Continents(_) => self.submit_continents(has_map),
            Layout::Islands(_) => self.submit_islands(has_map),
            Layout::Mountains(_) => has_map.then(|| UiAction::AddMountainRange {
                length: self.range_length.value,
                width: self.range_width.value,
                seed: random_seed(),
            }),
            Layout::Lakes(_) => has_map.then(|| UiAction::AddLakes {
                count: self.lake_count.value as u32,
                size: self.lake_size.value,
                seed: random_seed(),
            }),
            Layout::Shallows(_) => has_map.then(|| UiAction::AddShallows { width: self.shelf_width.value }),
            Layout::Rivers(_) => has_map.then(|| UiAction::AddRivers {
                count: self.river_count.value as u32,
                width: self.river_width.value,
                winding: self.river_winding.value / 100.0,
                seed: random_seed(),
            }),
        }
    }

    fn submit_new_map(&mut self) -> Option<UiAction> {
        self.width.invalid = self.width.value().is_none();
        self.height.invalid = self.height.value().is_none();
        let width = snap_map_size(self.width.value()? as f32);
        let height = snap_map_size(self.height.value()? as f32);
        // Show the size actually built, in case it was rounded or clamped.
        self.width.text = format!("{width}");
        self.height.text = format!("{height}");
        self.open_tool = None;
        self.focused = None;
        Some(UiAction::CreateMap { width, height })
    }

    fn submit_continents(&mut self, has_map: bool) -> Option<UiAction> {
        if !has_map || self.busy {
            return None;
        }
        self.seed.invalid = self.seed.value().is_none();
        let seed = self.seed.value()?;
        self.focused = None;
        self.busy = true;
        // The panel stays open so settings can be tweaked and regenerated.
        Some(UiAction::GenerateContinents(ContinentParams {
            coverage: self.coverage.value / 100.0,
            count: self.continent_count,
            roughness: self.roughness.value / 100.0,
            regularity: self.regularity.value / 100.0,
            edge_buffer: self.edge_buffer.value / 100.0,
            seed,
        }))
    }

    fn submit_islands(&mut self, has_map: bool) -> Option<UiAction> {
        if !has_map || self.busy {
            return None;
        }
        self.island_seed.invalid = self.island_seed.value().is_none();
        let seed = self.island_seed.value()?;
        self.focused = None;
        self.busy = true;
        Some(UiAction::AddIslands(IslandParams {
            count: self.island_count.value as u32,
            size: self.island_size.value / 100.0,
            variety: self.island_variety.value / 100.0,
            scatter: self.island_scatter.value / 100.0,
            roughness: self.island_roughness.value / 100.0,
            edge_buffer: self.edge_buffer.value / 100.0,
            seed,
        }))
    }

    pub fn draw(&self, style: &Style, has_map: bool, undo_steps: usize) {
        let mouse: Vec2 = mouse_position().into();

        draw_rectangle(0.0, 0.0, TOOLBAR_WIDTH, screen_height(), style::SURFACE);
        draw_line(TOOLBAR_WIDTH - 0.5, 0.0, TOOLBAR_WIDTH - 0.5, screen_height(), 1.0, style::BORDER);

        for (i, &tool) in Tool::ALL.iter().enumerate() {
            let r = icon_rect(i);
            let hovered = r.contains(mouse);
            let active = self.open_tool == Some(tool);
            if active {
                style::rounded_rect(r, 8.0, style::ACCENT_SOFT);
                // Accent bar on the toolbar's edge marks the active tool.
                style::rounded_rect(Rect::new(0.0, r.y + 8.0, 3.0, r.h - 16.0), 1.5, style::ACCENT);
            } else if hovered {
                style::rounded_rect(r, 8.0, style::HOVER);
            }
            let color = if active || hovered { style::TEXT } else { style::TEXT_MUTED };
            draw_tool_icon(tool, r, color);
            if hovered && !active {
                draw_tooltip(style, tool.label(), r.right() + 10.0, r.center().y);
            }
        }

        for button in ActionButton::ALL {
            self.draw_action_button(style, mouse, button, has_map, undo_steps);
        }

        if let Some(layout) = self.layout() {
            style::card(layout.panel(), PANEL_RADIUS, style::PANEL);
            match &layout {
                Layout::NewMap(l) => self.draw_new_map_panel(style, l, mouse),
                Layout::Continents(l) => self.draw_continents_panel(style, l, mouse, has_map),
                Layout::Islands(l) => self.draw_islands_panel(style, l, mouse, has_map),
                Layout::Mountains(l) => self.draw_paint_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    PaintPanel {
                        title: "Mountains",
                        brush: (&self.mountain_brush, SliderId::MountainBrush),
                        lines: [
                            "Hold left to grow mountains (26 to 50 m).",
                            "Hills form around them as foothills.",
                            "Hold right to lower ground, down to 1 m.",
                            "[ and ] resize the brush.",
                        ],
                        random_title: "Random mountain range",
                        random: [(&self.range_length, SliderId::RangeLength), (&self.range_width, SliderId::RangeWidth)],
                        button: "Add mountain range",
                    },
                ),
                Layout::Lakes(l) => self.draw_paint_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    PaintPanel {
                        title: "Lakes",
                        brush: (&self.lake_brush, SliderId::LakeBrush),
                        lines: [
                            "Hold left to dig a lake into the land.",
                            "It joins any lake it touches, otherwise it",
                            "fills to the lowest ground under the brush.",
                            "[ and ] resize the brush.",
                        ],
                        random_title: "Random lakes",
                        random: [(&self.lake_count, SliderId::LakeCount), (&self.lake_size, SliderId::LakeSize)],
                        button: "Add lakes",
                    },
                ),
                Layout::Rivers(l) => self.draw_river_panel(style, l, mouse, has_map),
                Layout::Shallows(l) => self.draw_shallow_panel(style, l, mouse, has_map),
            }
        }
    }

    fn draw_action_button(&self, style: &Style, mouse: Vec2, button: ActionButton, has_map: bool, undo_steps: usize) {
        let r = button.rect();
        let enabled = match button {
            ActionButton::Undo => undo_steps > 0,
            ActionButton::Save => has_map,
            ActionButton::Open => true,
        };
        let hovered = r.contains(mouse);
        if enabled && hovered {
            style::rounded_rect(r, 8.0, style::HOVER);
        }
        let color = if !enabled {
            Color::new(1.0, 1.0, 1.0, 0.22)
        } else if hovered {
            style::TEXT
        } else {
            style::TEXT_MUTED
        };
        match button {
            ActionButton::Undo => draw_undo_icon(r, color),
            ActionButton::Save => draw_save_icon(r, color),
            ActionButton::Open => draw_open_icon(r, color),
        }
        if hovered {
            let text = match button {
                ActionButton::Undo => match undo_steps {
                    0 => "Nothing to undo".to_owned(),
                    1 => "Undo (Ctrl+Z), 1 step left".to_owned(),
                    n => format!("Undo (Ctrl+Z), {n} steps left"),
                },
                ActionButton::Save if has_map => "Save (Ctrl+S), Save as (Ctrl+Shift+S)".to_owned(),
                ActionButton::Save => "Nothing to save".to_owned(),
                ActionButton::Open => "Open map (Ctrl+O)".to_owned(),
            };
            draw_tooltip(style, &text, r.right() + 10.0, r.center().y);
        }
    }

    fn draw_header(&self, style: &Style, panel: Rect, title: &str, subtitle: &str) {
        let x = panel.x + PAD;
        style.text(title, x, panel.y + 32.0, 16, Weight::Semibold, style::TEXT);
        style.text(subtitle, x, panel.y + 52.0, 13, Weight::Regular, style::TEXT_MUTED);
    }

    fn draw_new_map_panel(&self, style: &Style, l: &NewMapLayout, mouse: Vec2) {
        self.draw_header(style, l.panel, "New map", "Starts as deep ocean.");
        for (r, id) in [(l.width, FieldId::Width), (l.height, FieldId::Height)] {
            let field = self.field(id);
            style.text(field.label, r.x, r.y - 8.0, 12, Weight::Semibold, style::TEXT_MUTED);
            draw_field(style, r, field, self.focused == Some(id), mouse);
        }
        let hint = format!(
            "{:.0} to {:.0} m, in steps of {:.0} m",
            MIN_MAP_SIZE, MAX_MAP_SIZE, CHUNK_SIZE
        );
        style.text(&hint, l.panel.x + PAD, l.width.bottom() + 22.0, 12, Weight::Regular, style::TEXT_MUTED);
        draw_primary_button(style, l.button, "Create map", true, mouse);
    }

    /// The Mountains and Lakes panels: a brush size and notes on how it works, then sliders and a
    /// button for placing them at random.
    fn draw_paint_panel(&self, style: &Style, l: &BrushLayout, mouse: Vec2, has_map: bool, panel: PaintPanel) {
        let subtitle = if has_map {
            "Paint on land, or place at random."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, panel.title, subtitle);
        let (brush, brush_id) = panel.brush;
        draw_slider(style, l.brush, brush, self.dragging == Some(brush_id), mouse);

        let x = l.panel.x + PAD;
        let mut y = l.brush.bottom() + 30.0;
        for line in panel.lines {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }

        draw_line(x, l.divider, l.panel.right() - PAD, l.divider, 1.0, style::BORDER);
        style.text(panel.random_title, x, l.divider + 28.0, 14, Weight::Semibold, style::TEXT);
        for (r, (slider, id)) in [l.first, l.second].into_iter().zip(panel.random) {
            draw_slider(style, r, slider, self.dragging == Some(id), mouse);
        }
        draw_primary_button(style, l.button, panel.button, has_map, mouse);
    }

    fn draw_shallow_panel(&self, style: &Style, l: &ShallowLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Paint shallow water onto the sea."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Shallows", subtitle);
        draw_slider(style, l.brush, &self.shallow_brush, self.dragging == Some(SliderId::ShallowBrush), mouse);

        let x = l.panel.x + PAD;
        let mut y = l.brush.bottom() + 30.0;
        for line in [
            "Hold left to paint shallow water.",
            "Hold right to paint deep water.",
            "Land, lakes and rivers are left alone.",
            "[ and ] resize the brush.",
        ] {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }

        draw_line(x, l.divider, l.panel.right() - PAD, l.divider, 1.0, style::BORDER);
        style.text("Along the coasts", x, l.divider + 28.0, 14, Weight::Semibold, style::TEXT);
        style.text("Shallows beside every coast, easing", x, l.divider + 48.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text("to deep water this far out.", x, l.divider + 64.0, 12, Weight::Regular, style::TEXT_MUTED);
        draw_slider(style, l.width, &self.shelf_width, self.dragging == Some(SliderId::ShelfWidth), mouse);
        draw_primary_button(style, l.button, "Add shallows", has_map, mouse);
    }

    fn draw_river_panel(&self, style: &Style, l: &RiverLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Click where it starts and ends."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Rivers", subtitle);
        for (r, slider, id) in [
            (l.width, &self.river_width, SliderId::RiverWidth),
            (l.winding, &self.river_winding, SliderId::RiverWinding),
        ] {
            draw_slider(style, r, slider, self.dragging == Some(id), mouse);
        }

        let x = l.panel.x + PAD;
        let mut y = l.winding.bottom() + 46.0;
        for line in [
            "Click the two ends of the river.",
            "It flows from the higher one to the lower,",
            "cutting through land where it must.",
            "It stops at the sea or a lake it reaches.",
            "Esc forgets the first point.",
        ] {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }

        draw_line(x, l.divider, l.panel.right() - PAD, l.divider, 1.0, style::BORDER);
        style.text("Random rivers", x, l.divider + 28.0, 14, Weight::Semibold, style::TEXT);
        style.text("Each starts inland and runs to the sea,", x, l.divider + 48.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text("with the width and winding set above.", x, l.divider + 64.0, 12, Weight::Regular, style::TEXT_MUTED);
        draw_slider(style, l.count, &self.river_count, self.dragging == Some(SliderId::RiverCount), mouse);
        draw_primary_button(style, l.button, "Add rivers", has_map, mouse);
    }

    fn draw_islands_panel(&self, style: &Style, l: &IslandsLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Sprinkles islands into open water."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Islands", subtitle);

        for (r, slider, id) in [
            (l.count, &self.island_count, SliderId::IslandCount),
            (l.size, &self.island_size, SliderId::IslandSize),
            (l.variety, &self.island_variety, SliderId::IslandVariety),
            (l.scatter, &self.island_scatter, SliderId::IslandScatter),
            (l.roughness, &self.island_roughness, SliderId::IslandRoughness),
            (l.edge_buffer, &self.edge_buffer, SliderId::EdgeBuffer),
        ] {
            draw_slider(style, r, slider, self.dragging == Some(id), mouse);
        }

        style.text("Seed", l.seed.x, l.seed.y - 8.0, 12, Weight::Semibold, style::TEXT_MUTED);
        draw_field(style, l.seed, &self.island_seed, self.focused == Some(FieldId::IslandSeed), mouse);
        draw_icon_button(l.dice, mouse);
        draw_dice(l.dice.center(), style::TEXT_MUTED);
        if l.dice.contains(mouse) {
            draw_tooltip(style, "Random seed", l.dice.right() + 10.0, l.dice.center().y);
        }

        let label = if self.busy { "Generating..." } else { "Add islands" };
        draw_primary_button(style, l.button, label, has_map && !self.busy, mouse);
    }

    fn draw_continents_panel(&self, style: &Style, l: &ContinentsLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Adds to any land already there."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Continents", subtitle);

        for (r, slider, id) in [
            (l.coverage, &self.coverage, SliderId::Coverage),
            (l.roughness, &self.roughness, SliderId::Roughness),
            (l.regularity, &self.regularity, SliderId::Regularity),
            (l.edge_buffer, &self.edge_buffer, SliderId::EdgeBuffer),
        ] {
            draw_slider(style, r, slider, self.dragging == Some(id), mouse);
        }

        style.text_mid("Continents", l.panel.x + PAD, l.count.center().y, 13, Weight::Regular, style::TEXT);
        draw_stepper(style, l, self.continent_count, mouse);

        style.text("Seed", l.seed.x, l.seed.y - 8.0, 12, Weight::Semibold, style::TEXT_MUTED);
        draw_field(style, l.seed, &self.seed, self.focused == Some(FieldId::Seed), mouse);
        draw_icon_button(l.dice, mouse);
        draw_dice(l.dice.center(), style::TEXT_MUTED);
        if l.dice.contains(mouse) {
            draw_tooltip(style, "Random seed", l.dice.right() + 10.0, l.dice.center().y);
        }

        let label = if self.busy { "Generating..." } else { "Add continents" };
        draw_primary_button(style, l.button, label, has_map && !self.busy, mouse);
    }
}

/// What differs between the Mountains and Lakes panels.
struct PaintPanel<'a> {
    title: &'static str,
    brush: (&'a Slider, SliderId),
    lines: [&'static str; 4],
    random_title: &'static str,
    random: [(&'a Slider, SliderId); 2],
    button: &'static str,
}

fn draw_field(style: &Style, r: Rect, field: &NumberField, focused: bool, mouse: Vec2) {
    if focused {
        // Soft focus ring around the field.
        let ring = Rect::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0);
        style::rounded_rect(ring, CONTROL_RADIUS + 3.0, style::ACCENT_SOFT);
    }
    style::rounded_rect(r, CONTROL_RADIUS, style::FIELD);
    let border = if field.invalid {
        style::ERROR
    } else if focused {
        style::ACCENT
    } else if r.contains(mouse) {
        style::BORDER_STRONG
    } else {
        style::BORDER
    };
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, border);

    let text_x = r.x + 12.0;
    let mid = r.center().y;
    let text_w = style.measure(&field.text, 14, Weight::Regular).width;
    if focused && field.selected && !field.text.is_empty() {
        let sel = Rect::new(text_x - 3.0, mid - 10.0, text_w + 6.0, 20.0);
        style::rounded_rect(sel, 3.0, Color::new(0.302, 0.553, 1.0, 0.45));
    }
    style.text_mid(&field.text, text_x, mid, 14, Weight::Regular, style::TEXT);
    let caret_on = (get_time() * 2.0) as i64 % 2 == 0;
    if focused && !field.selected && caret_on {
        let cx = (text_x + text_w + 1.5).round();
        draw_line(cx, mid - 8.0, cx, mid + 8.0, 1.5, style::TEXT);
    }
    if !field.unit.is_empty() {
        let unit_w = style.measure(field.unit, 13, Weight::Regular).width;
        style.text_mid(field.unit, r.right() - 12.0 - unit_w, mid, 13, Weight::Regular, style::TEXT_MUTED);
    }
}

/// Label and value above, track and knob below; `r` is the draggable area.
fn draw_slider(style: &Style, r: Rect, slider: &Slider, dragging: bool, mouse: Vec2) {
    let label_y = r.y - 6.0;
    style.text(slider.label, r.x, label_y, 13, Weight::Regular, style::TEXT);
    let value = format!("{:.0}{}", slider.value, slider.unit);
    let value_w = style.measure(&value, 13, Weight::Semibold).width;
    style.text(&value, r.right() - value_w, label_y, 13, Weight::Semibold, style::TEXT);

    let mid = r.center().y;
    let track = Rect::new(r.x, mid - 2.0, r.w, 4.0);
    style::rounded_rect(track, 2.0, Color::new(1.0, 1.0, 1.0, 0.12));
    let knob_x = r.x + r.w * slider.fraction();
    style::rounded_rect(Rect::new(r.x, track.y, knob_x - r.x, 4.0), 2.0, style::ACCENT);

    let hovered = dragging || r.contains(mouse);
    if hovered {
        draw_circle(knob_x, mid, 11.0, style::ACCENT_SOFT);
    }
    draw_circle(knob_x, mid, 7.0, WHITE);
    draw_circle_lines(knob_x, mid, 7.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.25));

    if let Some((left, right)) = slider.ends {
        // Right under the track, well clear of the next slider's label.
        let y = r.bottom() + 5.0;
        style.text(left, r.x, y, 11, Weight::Regular, style::TEXT_MUTED);
        let w = style.measure(right, 11, Weight::Regular).width;
        style.text(right, r.right() - w, y, 11, Weight::Regular, style::TEXT_MUTED);
    }
}

/// A "−  value  +" control for small whole numbers.
fn draw_stepper(style: &Style, l: &ContinentsLayout, value: u32, mouse: Vec2) {
    let r = l.count;
    style::rounded_rect(r, CONTROL_RADIUS, style::FIELD);
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, style::BORDER);
    for (b, enabled) in [(l.minus, value > 1), (l.plus, value < MAX_CONTINENTS)] {
        if enabled && b.contains(mouse) {
            style::rounded_rect(b, CONTROL_RADIUS, style::HOVER);
        }
        let color = if enabled { style::TEXT } else { Color::new(1.0, 1.0, 1.0, 0.2) };
        let c = b.center();
        draw_line(c.x - 5.0, c.y, c.x + 5.0, c.y, 1.6, color);
        if b == l.plus {
            draw_line(c.x, c.y - 5.0, c.x, c.y + 5.0, 1.6, color);
        }
    }
    style.text_centered(&value.to_string(), r, 14, Weight::Semibold, style::TEXT);
}

fn draw_primary_button(style: &Style, r: Rect, label: &str, enabled: bool, mouse: Vec2) {
    let (fill, text) = if !enabled {
        (Color::new(1.0, 1.0, 1.0, 0.08), Color::new(1.0, 1.0, 1.0, 0.35))
    } else if r.contains(mouse) {
        (style::ACCENT_HOVER, WHITE)
    } else {
        (style::ACCENT, WHITE)
    };
    style::rounded_rect(r, CONTROL_RADIUS, fill);
    style.text_centered(label, r, 14, Weight::Semibold, text);
}

fn draw_icon_button(r: Rect, mouse: Vec2) {
    let fill = if r.contains(mouse) { Color::new(1.0, 1.0, 1.0, 0.10) } else { style::FIELD };
    style::rounded_rect(r, CONTROL_RADIUS, fill);
    style::rounded_rect_lines(r, CONTROL_RADIUS, 1.0, style::BORDER);
}

fn draw_tooltip(style: &Style, text: &str, x: f32, center_y: f32) {
    let w = style.measure(text, 13, Weight::Regular).width + 20.0;
    let r = Rect::new(x, center_y - 14.0, w, 28.0);
    style::card(r, 6.0, Color::new(0.07, 0.07, 0.08, 1.0));
    style.text_mid(text, r.x + 10.0, r.center().y, 13, Weight::Regular, style::TEXT);
}

/// Draws a closed outline through `points`.
fn draw_outline(points: &[Vec2], color: Color) {
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        draw_line(p.x, p.y, q.x, q.y, 1.6, color);
    }
}

/// An open folder: "open".
fn draw_open_icon(r: Rect, color: Color) {
    let c = r.center();
    draw_outline(
        &[
            c + vec2(-9.0, 8.0),
            c + vec2(-9.0, -7.0),
            c + vec2(-3.0, -7.0),
            c + vec2(-1.0, -4.0),
            c + vec2(9.0, -4.0),
            c + vec2(9.0, 8.0),
        ],
        color,
    );
}

/// A floppy disk: "save".
fn draw_save_icon(r: Rect, color: Color) {
    let c = r.center();
    draw_outline(
        &[
            c + vec2(-9.0, -9.0),
            c + vec2(5.0, -9.0),
            c + vec2(9.0, -5.0),
            c + vec2(9.0, 9.0),
            c + vec2(-9.0, 9.0),
        ],
        color,
    );
    // The metal shutter at the top and the label at the bottom.
    draw_outline(
        &[c + vec2(-5.0, -9.0), c + vec2(-5.0, -3.5), c + vec2(3.0, -3.5), c + vec2(3.0, -9.0)],
        color,
    );
    draw_outline(
        &[c + vec2(-6.0, 9.0), c + vec2(-6.0, 2.0), c + vec2(6.0, 2.0), c + vec2(6.0, 9.0)],
        color,
    );
}

/// A curved arrow pointing back: "undo".
fn draw_undo_icon(r: Rect, color: Color) {
    let c = r.center() + vec2(0.0, 2.0);
    let radius = 7.5;
    let (start, end) = (-2.4f32, 1.35f32);
    let point = |a: f32| c + vec2(a.cos(), a.sin()) * radius;
    let steps = 18;
    for k in 0..steps {
        let a0 = start + (end - start) * k as f32 / steps as f32;
        let a1 = start + (end - start) * (k + 1) as f32 / steps as f32;
        let (p0, p1) = (point(a0), point(a1));
        draw_line(p0.x, p0.y, p1.x, p1.y, 1.8, color);
    }
    // Arrowhead at the start of the arc, pointing the way the arc runs backwards.
    let tip = point(start);
    let heading = vec2(start.sin(), -start.cos());
    let tip = tip + heading * 2.0;
    for wing in [2.55f32, -2.55] {
        let (s, co) = wing.sin_cos();
        let dir = vec2(heading.x * co - heading.y * s, heading.x * s + heading.y * co);
        let end = tip + dir * 5.0;
        draw_line(tip.x, tip.y, end.x, end.y, 1.8, color);
    }
}

/// A die showing five pips: "random".
fn draw_dice(c: Vec2, color: Color) {
    let r = Rect::new(c.x - 8.0, c.y - 8.0, 16.0, 16.0);
    style::rounded_rect_lines(r, 3.5, 1.5, color);
    for (dx, dy) in [(-4.0, -4.0), (4.0, -4.0), (0.0, 0.0), (-4.0, 4.0), (4.0, 4.0)] {
        draw_circle(c.x + dx, c.y + dy, 1.4, color);
    }
}

fn draw_tool_icon(tool: Tool, r: Rect, color: Color) {
    let c = r.center();
    let t = 1.6;
    match tool {
        Tool::NewMap => {
            // A folded map with a plus badge: "new map".
            let (left, right) = (c.x - 10.0, c.x + 10.0);
            let (top, bottom) = (c.y - 8.0, c.y + 8.0);
            let (f1, f2) = (c.x - 3.3, c.x + 3.3);
            let outline = [
                (vec2(left, top + 2.0), vec2(f1, top)),
                (vec2(f1, top), vec2(f2, top + 2.0)),
                (vec2(f2, top + 2.0), vec2(right, top)),
                (vec2(right, top), vec2(right, bottom - 2.0)),
                (vec2(right, bottom - 2.0), vec2(f2, bottom)),
                (vec2(f2, bottom), vec2(f1, bottom - 2.0)),
                (vec2(f1, bottom - 2.0), vec2(left, bottom)),
                (vec2(left, bottom), vec2(left, top + 2.0)),
                (vec2(f1, top), vec2(f1, bottom - 2.0)),
                (vec2(f2, top + 2.0), vec2(f2, bottom)),
            ];
            for (a, b) in outline {
                draw_line(a.x, a.y, b.x, b.y, t, color);
            }
            let badge = vec2(right, bottom);
            draw_circle(badge.x, badge.y, 7.0, style::SURFACE);
            draw_circle(badge.x, badge.y, 5.5, style::ACCENT);
            draw_line(badge.x - 3.0, badge.y, badge.x + 3.0, badge.y, 1.5, WHITE);
            draw_line(badge.x, badge.y - 3.0, badge.x, badge.y + 3.0, 1.5, WHITE);
        }
        Tool::Mountains => {
            // A tall peak with a snow cap, and a smaller one beside it.
            let ground = c.y + 9.0;
            let (peak_x, peak_y) = (c.x - 2.0, c.y - 10.0);
            draw_line(c.x - 12.0, ground, peak_x, peak_y, t, color);
            draw_line(peak_x, peak_y, c.x + 7.0, ground, t, color);
            draw_line(c.x + 3.0, c.y - 1.0, c.x + 11.0, ground, t, color);
            draw_line(c.x - 12.0, ground, c.x + 11.0, ground, t, color);
            // The snow line, zigzagging across the peak.
            let line = [vec2(peak_x - 3.7, peak_y + 6.0), vec2(peak_x - 1.5, peak_y + 4.0), vec2(peak_x + 0.5, peak_y + 6.0), vec2(peak_x + 2.5, peak_y + 4.0), vec2(peak_x + 3.7, peak_y + 5.5)];
            for pair in line.windows(2) {
                draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 1.3, color);
            }
        }
        Tool::Lakes => {
            // A pond with two ripples across it.
            draw_ellipse_lines(c.x, c.y, 11.0, 7.5, 0.0, t, color);
            for (dx, dy, w) in [(-3.5, -1.5, 3.0), (2.0, 2.5, 3.5)] {
                draw_line(c.x + dx - w, c.y + dy, c.x + dx + w, c.y + dy, 1.2, color);
            }
        }
        Tool::Shallows => {
            // Waves over water that is shallower toward the shore: three wavy lines, the
            // topmost the faintest.
            for (k, dy) in [-7.0f32, 0.0, 7.0].into_iter().enumerate() {
                let mut shade = color;
                shade.a *= 0.45 + 0.275 * k as f32;
                let steps = 16;
                for s in 0..steps {
                    let (x0, x1) = (c.x - 10.0 + 20.0 * s as f32 / steps as f32, c.x - 10.0 + 20.0 * (s + 1) as f32 / steps as f32);
                    let wave = |x: f32| c.y + dy + 2.2 * ((x - c.x) * 0.55 + k as f32).sin();
                    draw_line(x0, wave(x0), x1, wave(x1), t, shade);
                }
            }
        }
        Tool::Rivers => {
            // A river winding across the icon, widening toward its mouth.
            let steps = 24;
            let point = |k: usize| {
                let t = k as f32 / steps as f32;
                vec2(c.x - 11.0 + 22.0 * t, c.y + 5.5 * (t * std::f32::consts::TAU * 1.1).sin())
            };
            for k in 0..steps {
                let (p0, p1) = (point(k), point(k + 1));
                draw_line(p0.x, p0.y, p1.x, p1.y, 1.0 + 1.6 * k as f32 / steps as f32, color);
            }
        }
        Tool::Islands => {
            // Three islands of different sizes.
            draw_ellipse_lines(c.x - 3.5, c.y + 2.0, 7.5, 5.0, 0.3, t, color);
            draw_ellipse_lines(c.x + 6.0, c.y - 5.5, 3.6, 2.6, -0.4, t, color);
            draw_ellipse_lines(c.x + 6.5, c.y + 6.0, 2.6, 1.9, 0.2, t, color);
        }
        Tool::Continents => {
            // A globe: outline, a meridian ellipse and two parallels.
            let radius = 10.0;
            draw_circle_lines(c.x, c.y, radius, t, color);
            let steps = 24;
            for k in 0..steps {
                let a0 = std::f32::consts::TAU * k as f32 / steps as f32;
                let a1 = std::f32::consts::TAU * (k + 1) as f32 / steps as f32;
                let p = |a: f32| vec2(c.x + a.cos() * radius * 0.42, c.y + a.sin() * radius);
                let (p0, p1) = (p(a0), p(a1));
                draw_line(p0.x, p0.y, p1.x, p1.y, t, color);
            }
            draw_line(c.x - radius, c.y, c.x + radius, c.y, t, color);
            for dy in [-5.0f32, 5.0] {
                let half = (radius * radius - dy * dy).sqrt();
                draw_line(c.x - half, c.y + dy, c.x + half, c.y + dy, 1.2, color);
            }
        }
    }
}
