//! Drawing the toolbar and the open panel.

use super::*;

/// What differs between the Forest and Stone panels.
struct ResourcePanel<'a> {
    title: &'static str,
    subtitle: &'static str,
    brush: (&'a Slider, SliderId),
    density: (&'a Slider, SliderId),
    lines: [&'static str; 5],
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

impl EditorUi {
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
                Layout::Regions(l) => self.draw_regions_panel(style, l, mouse, has_map),
                Layout::Passes(l) => self.draw_passes_panel(style, l, mouse, has_map),
                Layout::Rivers(l) => self.draw_river_panel(style, l, mouse, has_map),
                Layout::Shallows(l) => self.draw_shallow_panel(style, l, mouse, has_map),
                Layout::Forest(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Forest",
                        subtitle: "Paint forest onto the land.",
                        brush: (&self.forest_brush, SliderId::ForestBrush),
                        density: (&self.forest_density, SliderId::ForestDensity),
                        lines: [
                            "Hold left to paint forest.",
                            "Hold right to clear it.",
                            "Trees keep off water, beaches, cliffs",
                            "and the high rock.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
                Layout::Stone(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Stone",
                        subtitle: "Paint stone onto the land.",
                        brush: (&self.stone_brush, SliderId::StoneBrush),
                        density: (&self.stone_density, SliderId::StoneDensity),
                        lines: [
                            "Hold left to paint stone.",
                            "Hold right to clear it.",
                            "Boulders and loose rock lie in patches,",
                            "wherever the ground is dry.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
                Layout::Iron(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Iron",
                        subtitle: "Paint iron ore onto the land.",
                        brush: (&self.iron_brush, SliderId::IronBrush),
                        density: (&self.iron_density, SliderId::IronDensity),
                        lines: [
                            "Hold left to paint iron.",
                            "Hold right to clear it.",
                            "Rusty outcrops show in veins,",
                            "wherever the ground is dry.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
                Layout::Copper(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Copper",
                        subtitle: "Paint copper ore onto the land.",
                        brush: (&self.copper_brush, SliderId::CopperBrush),
                        density: (&self.copper_density, SliderId::CopperDensity),
                        lines: [
                            "Hold left to paint copper.",
                            "Hold right to clear it.",
                            "Green-streaked nuggets lie in pockets,",
                            "wherever the ground is dry.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
                Layout::Sulphur(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Sulphur",
                        subtitle: "Paint sulphur onto the land.",
                        brush: (&self.sulphur_brush, SliderId::SulphurBrush),
                        density: (&self.sulphur_density, SliderId::SulphurDensity),
                        lines: [
                            "Hold left to paint sulphur.",
                            "Hold right to clear it.",
                            "Yellow lumps gather around vents,",
                            "wherever the ground is dry.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
                Layout::Oil(l) => self.draw_resource_panel(
                    style,
                    l,
                    mouse,
                    has_map,
                    ResourcePanel {
                        title: "Oil",
                        subtitle: "Paint oil onto the land.",
                        brush: (&self.oil_brush, SliderId::OilBrush),
                        density: (&self.oil_density, SliderId::OilDensity),
                        lines: [
                            "Hold left to paint oil.",
                            "Hold right to clear it.",
                            "Tar pools collect in broad fields,",
                            "on gentle dry ground.",
                            "[ and ] resize the brush.",
                        ],
                    },
                ),
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

    /// The Forest and Stone panels: a brush size and a density, and notes on how it works.
    fn draw_resource_panel(&self, style: &Style, l: &ResourceLayout, mouse: Vec2, has_map: bool, panel: ResourcePanel) {
        let subtitle = if has_map { panel.subtitle } else { "Create a map first." };
        self.draw_header(style, l.panel, panel.title, subtitle);
        let ((brush, brush_id), (density, density_id)) = (panel.brush, panel.density);
        draw_slider(style, l.brush, brush, self.dragging == Some(brush_id), mouse);
        draw_slider(style, l.density, density, self.dragging == Some(density_id), mouse);

        let x = l.panel.x + PAD;
        let mut y = l.density.bottom() + 46.0;
        for line in panel.lines {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }
    }

    fn draw_passes_panel(&self, style: &Style, l: &PassLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Click the points it runs through."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Passes", subtitle);
        for (r, slider, id) in [
            (l.width, &self.pass_width, SliderId::PassWidth),
            (l.walls, &self.pass_walls, SliderId::PassWalls),
            (l.winding, &self.pass_winding, SliderId::PassWinding),
        ] {
            draw_slider(style, r, slider, self.dragging == Some(id), mouse);
        }

        let x = l.panel.x + PAD;
        let mut y = l.winding.bottom() + 46.0;
        for line in [
            "Click each point the road runs through.",
            "Enter, or a click on the last point again,",
            "cuts it level through any mountain and",
            "builds it up across valleys. Backspace",
            "takes back a point; Esc forgets them all.",
        ] {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }
    }

    fn draw_regions_panel(&self, style: &Style, l: &RegionsLayout, mouse: Vec2, has_map: bool) {
        let subtitle = if has_map {
            "Divide the map into areas."
        } else {
            "Create a map first."
        };
        self.draw_header(style, l.panel, "Regions", subtitle);
        draw_slider(style, l.size, &self.region_size, self.dragging == Some(SliderId::RegionSize), mouse);

        let x = l.panel.x + PAD;
        let mut y = l.size.bottom() + 30.0;
        for line in [
            "Each region holds one kind of ground,",
            "and sizes vary a lot around the average.",
            "Lakes and rivers count as land.",
        ] {
            style.text(line, x, y, 12, Weight::Regular, style::TEXT_MUTED);
            y += 18.0;
        }
        let kinds = [
            ("Deep water", Color::new(0.10, 0.22, 0.85, 1.0)),
            ("Shallow water", Color::new(0.10, 0.85, 0.90, 1.0)),
            ("Mountain", Color::new(0.75, 0.55, 0.45, 1.0)),
            ("Land", Color::new(0.35, 0.85, 0.25, 1.0)),
        ];
        for (k, (label, color)) in kinds.into_iter().enumerate() {
            let row = l.legend_top + k as f32 * 22.0;
            style::rounded_rect(Rect::new(x, row, 14.0, 14.0), 3.0, color);
            style.text(label, x + 24.0, row + 12.0, 13, Weight::Regular, style::TEXT);
        }
        draw_primary_button(style, l.button, "Break into regions", has_map, mouse);
        draw_secondary_button(style, l.clear, "Clear regions", has_map, mouse);
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
