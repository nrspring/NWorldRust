//! Photoshop-style editor UI: a vertical icon toolbar on the left, where each tool opens a
//! flyout panel beside it.

use macroquad::prelude::*;

use crate::continents::ContinentParams;
use crate::hills::Peak;
use crate::islands::IslandParams;
use crate::style::{self, Style, Weight};
use crate::world::{snap_map_size, Resource, CHUNK_SIZE, MAX_MAP_SIZE, MIN_MAP_SIZE};

mod constants;
mod defaults;
mod draw;
mod icons;
mod layout;
mod update;
mod widgets;

pub use constants::TOOLBAR_WIDTH;
use constants::*;
use icons::*;
use layout::*;
use widgets::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    NewMap,
    Continents,
    Islands,
    Mountains,
    Lakes,
    Rivers,
    Passes,
    Shallows,
    Forest,
    Stone,
    Iron,
    Copper,
    Sulphur,
    Oil,
    Regions,
}

impl Tool {
    const ALL: [Tool; 15] = [
        Tool::NewMap,
        Tool::Continents,
        Tool::Islands,
        Tool::Mountains,
        Tool::Lakes,
        Tool::Rivers,
        Tool::Passes,
        Tool::Shallows,
        Tool::Forest,
        Tool::Stone,
        Tool::Iron,
        Tool::Copper,
        Tool::Sulphur,
        Tool::Oil,
        Tool::Regions,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::NewMap => "New map",
            Tool::Continents => "Continents",
            Tool::Islands => "Islands",
            Tool::Mountains => "Mountains",
            Tool::Lakes => "Lakes",
            Tool::Rivers => "Rivers",
            Tool::Passes => "Passes",
            Tool::Shallows => "Shallows",
            Tool::Forest => "Forest",
            Tool::Stone => "Stone",
            Tool::Iron => "Iron",
            Tool::Copper => "Copper",
            Tool::Sulphur => "Sulphur",
            Tool::Oil => "Oil",
            Tool::Regions => "Regions",
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
    /// Divide the map into regions about this many meters across.
    BreakIntoRegions { size: f32, seed: u32 },
    /// Take the regions off the map.
    ClearRegions,
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
    ForestBrush,
    ForestDensity,
    StoneBrush,
    StoneDensity,
    IronBrush,
    IronDensity,
    CopperBrush,
    CopperDensity,
    SulphurBrush,
    SulphurDensity,
    OilBrush,
    OilDensity,
    RegionSize,
    PassWidth,
    PassWalls,
    PassWinding,
}

/// What a brush paints.
#[derive(Clone, Copy)]
pub enum Brush {
    /// A resource of this density, 0 to 1, on land (left button) or bare ground (right).
    Resource(Resource, f32),
    Peak(Peak),
    /// Shallow water onto deep sea (left button) or deep water onto shallow (right).
    Shallows,
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
    /// Width of the forest brush in meters, and the density it paints, from 0 to 100.
    forest_brush: Slider,
    forest_density: Slider,
    /// The same for stone.
    stone_brush: Slider,
    stone_density: Slider,
    /// And for iron.
    iron_brush: Slider,
    iron_density: Slider,
    /// And for copper.
    copper_brush: Slider,
    copper_density: Slider,
    /// And for sulphur.
    sulphur_brush: Slider,
    sulphur_density: Slider,
    /// And for oil.
    oil_brush: Slider,
    oil_density: Slider,
    /// The average size of a region in meters.
    region_size: Slider,
    /// A pass's road bed width in meters, how steep its walls are and how much it winds, from 0 to 100.
    pass_width: Slider,
    pass_walls: Slider,
    pass_winding: Slider,
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
    // The tool icons keep clear of the action buttons stacked up from the bottom, closing their
    // gaps (but never overlapping) when the window is short.
    let actions_top = ActionButton::Open.rect().y;
    let room = actions_top - ICON_GAP - TOOLBAR_TOP - ICON_SIZE;
    let step = (room / (Tool::ALL.len() - 1) as f32).clamp(ICON_SIZE, ICON_SIZE + ICON_GAP);
    Rect::new(
        (TOOLBAR_WIDTH - ICON_SIZE) * 0.5,
        TOOLBAR_TOP + index as f32 * step,
        ICON_SIZE,
        ICON_SIZE,
    )
}

fn random_seed() -> u32 {
    rand::gen_range(1, 999_999_999)
}

impl EditorUi {
    fn layout(&self) -> Option<Layout> {
        let tool = self.open_tool?;
        let index = Tool::ALL.iter().position(|&t| t == tool)?;
        let mut origin = vec2(TOOLBAR_WIDTH + PANEL_GAP, icon_rect(index).y);
        // The resource and Regions panels, low in the toolbar, slide up to stay on the screen.
        if matches!(tool, Tool::Forest | Tool::Stone | Tool::Iron | Tool::Copper | Tool::Sulphur | Tool::Oil) {
            origin.y = origin.y.min(screen_height() - RESOURCE_PANEL_HEIGHT - PANEL_GAP).max(PANEL_GAP);
        } else if tool == Tool::Regions {
            origin.y = origin.y.min(screen_height() - REGIONS_PANEL_HEIGHT - PANEL_GAP).max(PANEL_GAP);
        } else if tool == Tool::Passes {
            origin.y = origin.y.min(screen_height() - PASSES_PANEL_HEIGHT - PANEL_GAP).max(PANEL_GAP);
        }
        Some(match tool {
            Tool::NewMap => Layout::NewMap(NewMapLayout::new(origin)),
            Tool::Continents => Layout::Continents(ContinentsLayout::new(origin)),
            Tool::Islands => Layout::Islands(IslandsLayout::new(origin)),
            Tool::Mountains => Layout::Mountains(BrushLayout::new(origin)),
            Tool::Lakes => Layout::Lakes(BrushLayout::new(origin)),
            Tool::Rivers => Layout::Rivers(RiverLayout::new(origin)),
            Tool::Passes => Layout::Passes(PassLayout::new(origin)),
            Tool::Shallows => Layout::Shallows(ShallowLayout::new(origin)),
            Tool::Forest => Layout::Forest(ResourceLayout::new(origin)),
            Tool::Stone => Layout::Stone(ResourceLayout::new(origin)),
            Tool::Iron => Layout::Iron(ResourceLayout::new(origin)),
            Tool::Copper => Layout::Copper(ResourceLayout::new(origin)),
            Tool::Sulphur => Layout::Sulphur(ResourceLayout::new(origin)),
            Tool::Oil => Layout::Oil(ResourceLayout::new(origin)),
            Tool::Regions => Layout::Regions(RegionsLayout::new(origin)),
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
            SliderId::ForestBrush => &mut self.forest_brush,
            SliderId::ForestDensity => &mut self.forest_density,
            SliderId::StoneBrush => &mut self.stone_brush,
            SliderId::StoneDensity => &mut self.stone_density,
            SliderId::IronBrush => &mut self.iron_brush,
            SliderId::IronDensity => &mut self.iron_density,
            SliderId::CopperBrush => &mut self.copper_brush,
            SliderId::CopperDensity => &mut self.copper_density,
            SliderId::SulphurBrush => &mut self.sulphur_brush,
            SliderId::SulphurDensity => &mut self.sulphur_density,
            SliderId::OilBrush => &mut self.oil_brush,
            SliderId::OilDensity => &mut self.oil_density,
            SliderId::RegionSize => &mut self.region_size,
            SliderId::PassWidth => &mut self.pass_width,
            SliderId::PassWalls => &mut self.pass_walls,
            SliderId::PassWinding => &mut self.pass_winding,
        }
    }

    /// Text fields in the open panel, in Tab order.
    fn panel_fields(&self) -> &'static [FieldId] {
        match self.open_tool {
            Some(Tool::NewMap) => &[FieldId::Width, FieldId::Height],
            Some(Tool::Continents) => &[FieldId::Seed],
            Some(Tool::Islands) => &[FieldId::IslandSeed],
            Some(Tool::Mountains) | Some(Tool::Lakes) | Some(Tool::Rivers) | Some(Tool::Passes) | Some(Tool::Shallows) | Some(Tool::Forest) | Some(Tool::Stone) | Some(Tool::Iron) | Some(Tool::Copper) | Some(Tool::Sulphur) | Some(Tool::Oil) | Some(Tool::Regions) | None => &[],
        }
    }

    /// The channel width in meters and the winding, from 0 to 1, of the river the Rivers tool
    /// digs while it is open, else `None`.
    pub fn river(&self) -> Option<(f32, f32)> {
        (self.open_tool == Some(Tool::Rivers)).then(|| (self.river_width.value, self.river_winding.value / 100.0))
    }

    /// The road bed width in meters, the steepness of the walls (rise over run) and the winding,
    /// from 0 to 1, of the pass the Passes tool cuts while it is open, else `None`.
    pub fn pass(&self) -> Option<(f32, f32, f32)> {
        (self.open_tool == Some(Tool::Passes))
            .then(|| (self.pass_width.value, self.pass_walls.value / 100.0, self.pass_winding.value / 100.0))
    }

    /// The radius in meters and what it paints while the Mountains, Lakes or Shallows tool is
    /// open, else `None`.
    pub fn brush(&self) -> Option<(f32, Brush)> {
        match self.open_tool {
            Some(Tool::Mountains) => Some((self.mountain_brush.value * 0.5, Brush::Peak(Peak::Mountains))),
            Some(Tool::Lakes) => Some((self.lake_brush.value * 0.5, Brush::Peak(Peak::Lakes))),
            Some(Tool::Shallows) => Some((self.shallow_brush.value * 0.5, Brush::Shallows)),
            Some(Tool::Forest) => {
                Some((self.forest_brush.value * 0.5, Brush::Resource(Resource::Forest, self.forest_density.value / 100.0)))
            }
            Some(Tool::Stone) => {
                Some((self.stone_brush.value * 0.5, Brush::Resource(Resource::Stone, self.stone_density.value / 100.0)))
            }
            Some(Tool::Iron) => {
                Some((self.iron_brush.value * 0.5, Brush::Resource(Resource::Iron, self.iron_density.value / 100.0)))
            }
            Some(Tool::Copper) => {
                Some((self.copper_brush.value * 0.5, Brush::Resource(Resource::Copper, self.copper_density.value / 100.0)))
            }
            Some(Tool::Sulphur) => {
                Some((self.sulphur_brush.value * 0.5, Brush::Resource(Resource::Sulphur, self.sulphur_density.value / 100.0)))
            }
            Some(Tool::Oil) => Some((self.oil_brush.value * 0.5, Brush::Resource(Resource::Oil, self.oil_density.value / 100.0))),
            _ => None,
        }
    }

    /// Whether the Regions panel is open, which shows the regions over the map.
    pub fn regions_open(&self) -> bool {
        self.open_tool == Some(Tool::Regions)
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

}
