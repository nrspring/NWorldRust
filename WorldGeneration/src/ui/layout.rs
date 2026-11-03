//! Where everything sits in each tool's panel; shared by input handling and drawing.

use super::*;

/// Where everything in the New map panel sits; shared by input handling and drawing.
pub(super) struct NewMapLayout {
    pub(super) panel: Rect,
    pub(super) width: Rect,
    pub(super) height: Rect,
    pub(super) button: Rect,
}

impl NewMapLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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
pub(super) struct ContinentsLayout {
    pub(super) panel: Rect,
    pub(super) coverage: Rect,
    pub(super) roughness: Rect,
    pub(super) regularity: Rect,
    pub(super) edge_buffer: Rect,
    pub(super) count: Rect,
    pub(super) minus: Rect,
    pub(super) plus: Rect,
    pub(super) seed: Rect,
    pub(super) dice: Rect,
    pub(super) button: Rect,
}

impl ContinentsLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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
pub(super) struct IslandsLayout {
    pub(super) panel: Rect,
    pub(super) count: Rect,
    pub(super) size: Rect,
    pub(super) variety: Rect,
    pub(super) scatter: Rect,
    pub(super) roughness: Rect,
    pub(super) edge_buffer: Rect,
    pub(super) seed: Rect,
    pub(super) dice: Rect,
    pub(super) button: Rect,
}

impl IslandsLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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
pub(super) struct BrushLayout {
    pub(super) panel: Rect,
    pub(super) brush: Rect,
    /// Where the line above the random placement controls sits.
    pub(super) divider: f32,
    pub(super) first: Rect,
    pub(super) second: Rect,
    pub(super) button: Rect,
}

impl BrushLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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
pub(super) struct RiverLayout {
    pub(super) panel: Rect,
    pub(super) width: Rect,
    pub(super) winding: Rect,
    /// Where the line above the random rivers sits.
    pub(super) divider: f32,
    pub(super) count: Rect,
    pub(super) button: Rect,
}

impl RiverLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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
pub(super) struct ShallowLayout {
    pub(super) panel: Rect,
    pub(super) brush: Rect,
    /// Where the line above the shelf controls sits.
    pub(super) divider: f32,
    pub(super) width: Rect,
    pub(super) button: Rect,
}

impl ShallowLayout {
    pub(super) fn new(origin: Vec2) -> Self {
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

/// Where everything in the Passes panel sits: three sliders and notes.
pub(super) struct PassLayout {
    pub(super) panel: Rect,
    pub(super) width: Rect,
    pub(super) walls: Rect,
    pub(super) winding: Rect,
}

impl PassLayout {
    pub(super) fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let slider = |y: f32| Rect::new(x, origin.y + y, INNER_WIDTH, 22.0);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, PASSES_PANEL_HEIGHT),
            width: slider(92.0),
            walls: slider(154.0),
            winding: slider(216.0),
        }
    }
}

/// Where everything in the Regions panel sits: an average size, a key to the kinds of region,
/// and two buttons.
pub(super) struct RegionsLayout {
    pub(super) panel: Rect,
    pub(super) size: Rect,
    pub(super) legend_top: f32,
    pub(super) button: Rect,
    pub(super) clear: Rect,
}

impl RegionsLayout {
    pub(super) fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let size = Rect::new(x, origin.y + 92.0, INNER_WIDTH, 22.0);
        let legend_top = size.bottom() + 86.0;
        let button = Rect::new(x, legend_top + 100.0, INNER_WIDTH, CONTROL_HEIGHT);
        let clear = Rect::new(x, button.bottom() + 10.0, INNER_WIDTH, CONTROL_HEIGHT);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, REGIONS_PANEL_HEIGHT),
            size,
            legend_top,
            button,
            clear,
        }
    }
}

/// Where everything in the resource panels sits: a brush size and a density slider, and
/// notes.
pub(super) struct ResourceLayout {
    pub(super) panel: Rect,
    pub(super) brush: Rect,
    pub(super) density: Rect,
}

impl ResourceLayout {
    pub(super) fn new(origin: Vec2) -> Self {
        let x = origin.x + PAD;
        let slider = |y: f32| Rect::new(x, origin.y + y, INNER_WIDTH, 22.0);
        Self {
            panel: Rect::new(origin.x, origin.y, PANEL_WIDTH, RESOURCE_PANEL_HEIGHT),
            brush: slider(92.0),
            density: slider(154.0),
        }
    }
}

pub(super) enum Layout {
    NewMap(NewMapLayout),
    Continents(ContinentsLayout),
    Islands(IslandsLayout),
    Mountains(BrushLayout),
    Lakes(BrushLayout),
    Rivers(RiverLayout),
    Shallows(ShallowLayout),
    Forest(ResourceLayout),
    Stone(ResourceLayout),
    Iron(ResourceLayout),
    Copper(ResourceLayout),
    Sulphur(ResourceLayout),
    Oil(ResourceLayout),
    Regions(RegionsLayout),
    Passes(PassLayout),
}

impl Layout {
    pub(super) fn panel(&self) -> Rect {
        match self {
            Layout::NewMap(l) => l.panel,
            Layout::Continents(l) => l.panel,
            Layout::Islands(l) => l.panel,
            Layout::Mountains(l) | Layout::Lakes(l) => l.panel,
            Layout::Rivers(l) => l.panel,
            Layout::Shallows(l) => l.panel,
            Layout::Forest(l) | Layout::Stone(l) | Layout::Iron(l) | Layout::Copper(l) | Layout::Sulphur(l) | Layout::Oil(l) => l.panel,
            Layout::Regions(l) => l.panel,
            Layout::Passes(l) => l.panel,
        }
    }
}
