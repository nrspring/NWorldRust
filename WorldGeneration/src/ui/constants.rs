//! Sizes, limits and starting values for the toolbar, the panels and their sliders.

pub const TOOLBAR_WIDTH: f32 = 52.0;
pub(super) const ICON_SIZE: f32 = 38.0;
pub(super) const ICON_GAP: f32 = 6.0;
pub(super) const TOOLBAR_TOP: f32 = 10.0;
pub(super) const PANEL_GAP: f32 = 8.0;
pub(super) const PANEL_WIDTH: f32 = 288.0;
pub(super) const PANEL_RADIUS: f32 = 10.0;
pub(super) const PAD: f32 = 16.0;
pub(super) const INNER_WIDTH: f32 = PANEL_WIDTH - PAD * 2.0;
pub(super) const CONTROL_HEIGHT: f32 = 36.0;
pub(super) const CONTROL_RADIUS: f32 = 6.0;
pub(super) const DEFAULT_MAP_WIDTH: u32 = 10000;
pub(super) const DEFAULT_MAP_HEIGHT: u32 = 6000;
pub(super) const MAX_CONTINENTS: u32 = 12;
/// Mountains brush width in meters.
pub(super) const MOUNTAIN_BRUSH_MIN: f32 = 20.0;
pub(super) const MOUNTAIN_BRUSH_MAX: f32 = 100.0;
pub(super) const MOUNTAIN_BRUSH_DEFAULT: f32 = 50.0;
/// Lakes brush width in meters, and the random lakes: how many, and how big across.
pub(super) const LAKE_BRUSH_MIN: f32 = 10.0;
pub(super) const LAKE_BRUSH_MAX: f32 = 100.0;
pub(super) const LAKE_BRUSH_DEFAULT: f32 = 40.0;
pub(super) const LAKE_COUNT_MIN: f32 = 1.0;
pub(super) const LAKE_COUNT_MAX: f32 = 20.0;
pub(super) const LAKE_COUNT_DEFAULT: f32 = 3.0;
pub(super) const LAKE_SIZE_MIN: f32 = 20.0;
pub(super) const LAKE_SIZE_MAX: f32 = 600.0;
pub(super) const LAKE_SIZE_DEFAULT: f32 = 200.0;
/// Rivers: channel width in meters, and how much they wind, as a percentage.
pub(super) const RIVER_WIDTH_MIN: f32 = 8.0;
pub(super) const RIVER_WIDTH_MAX: f32 = 120.0;
pub(super) const RIVER_WIDTH_DEFAULT: f32 = 24.0;
pub(super) const RIVER_WINDING_DEFAULT: f32 = 50.0;
pub(super) const RIVER_COUNT_MAX: f32 = 10.0;
/// Shallows brush width in meters, and how far out from the coast the shelf of shallows reaches.
pub(super) const SHALLOW_BRUSH_MIN: f32 = 30.0;
pub(super) const SHALLOW_BRUSH_MAX: f32 = 600.0;
pub(super) const SHALLOW_BRUSH_DEFAULT: f32 = 150.0;
pub(super) const SHELF_WIDTH_MIN: f32 = 10.0;
pub(super) const SHELF_WIDTH_MAX: f32 = 100.0;
pub(super) const SHELF_WIDTH_DEFAULT: f32 = 40.0;
/// Forest brush width in meters, and the density it paints, as a percentage.
pub(super) const FOREST_BRUSH_MIN: f32 = 20.0;
pub(super) const FOREST_BRUSH_MAX: f32 = 400.0;
pub(super) const FOREST_BRUSH_DEFAULT: f32 = 120.0;
pub(super) const FOREST_DENSITY_MIN: f32 = 10.0;
pub(super) const FOREST_DENSITY_DEFAULT: f32 = 80.0;
/// Stone brush width in meters, and the density it paints, as a percentage.
pub(super) const STONE_BRUSH_MIN: f32 = 20.0;
pub(super) const STONE_BRUSH_MAX: f32 = 400.0;
pub(super) const STONE_BRUSH_DEFAULT: f32 = 80.0;
pub(super) const STONE_DENSITY_DEFAULT: f32 = 70.0;
/// Iron brush width in meters, and the density it paints, as a percentage.
pub(super) const IRON_BRUSH_MIN: f32 = 20.0;
pub(super) const IRON_BRUSH_MAX: f32 = 400.0;
pub(super) const IRON_BRUSH_DEFAULT: f32 = 60.0;
pub(super) const IRON_DENSITY_DEFAULT: f32 = 70.0;
/// Copper brush width in meters, and the density it paints, as a percentage.
pub(super) const COPPER_BRUSH_MIN: f32 = 20.0;
pub(super) const COPPER_BRUSH_MAX: f32 = 400.0;
pub(super) const COPPER_BRUSH_DEFAULT: f32 = 60.0;
pub(super) const COPPER_DENSITY_DEFAULT: f32 = 70.0;
/// Sulphur and oil brush widths in meters, and the density they paint, as a percentage.
pub(super) const SULPHUR_BRUSH_MIN: f32 = 20.0;
pub(super) const SULPHUR_BRUSH_MAX: f32 = 400.0;
pub(super) const SULPHUR_BRUSH_DEFAULT: f32 = 50.0;
pub(super) const SULPHUR_DENSITY_DEFAULT: f32 = 70.0;
pub(super) const OIL_BRUSH_MIN: f32 = 20.0;
pub(super) const OIL_BRUSH_MAX: f32 = 400.0;
pub(super) const OIL_BRUSH_DEFAULT: f32 = 90.0;
pub(super) const OIL_DENSITY_DEFAULT: f32 = 70.0;
/// Average region size in meters across.
pub(super) const REGION_SIZE_MIN: f32 = 150.0;
/// Passes: the width of the road bed in meters, how steep its walls are as a percentage (100 is a
/// 45 degree slope), and how much it winds as a percentage.
pub(super) const PASS_WIDTH_MIN: f32 = 20.0;
pub(super) const PASS_WIDTH_MAX: f32 = 200.0;
pub(super) const PASS_WIDTH_DEFAULT: f32 = 40.0;
pub(super) const PASS_WALLS_MIN: f32 = 30.0;
pub(super) const PASS_WALLS_MAX: f32 = 200.0;
pub(super) const PASS_WALLS_DEFAULT: f32 = 100.0;
pub(super) const PASS_WINDING_DEFAULT: f32 = 20.0;
/// Height of the Passes panel.
pub(super) const PASSES_PANEL_HEIGHT: f32 = 392.0;
pub(super) const REGION_SIZE_MAX: f32 = 3000.0;
pub(super) const REGION_SIZE_DEFAULT: f32 = 600.0;
/// Height of the Regions panel.
pub(super) const REGIONS_PANEL_HEIGHT: f32 = 398.0;
/// Height of the panels of the resource tools.
pub(super) const RESOURCE_PANEL_HEIGHT: f32 = 312.0;
/// Random mountain ranges: length along the range and width across it, in meters.
pub(super) const RANGE_LENGTH_MIN: f32 = 500.0;
pub(super) const RANGE_LENGTH_MAX: f32 = 4000.0;
pub(super) const RANGE_LENGTH_DEFAULT: f32 = 1500.0;
pub(super) const RANGE_WIDTH_MIN: f32 = 60.0;
pub(super) const RANGE_WIDTH_MAX: f32 = 300.0;
pub(super) const RANGE_WIDTH_DEFAULT: f32 = 120.0;
