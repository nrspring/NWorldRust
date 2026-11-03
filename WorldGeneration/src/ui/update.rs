//! Reading the mouse and keyboard: what the toolbar and the open panel do with them.

use super::*;

impl EditorUi {
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
        let mut clear_regions = false;
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
            Layout::Forest(l) | Layout::Stone(l) | Layout::Iron(l) | Layout::Copper(l) | Layout::Sulphur(l) | Layout::Oil(l) => {
                let (brush, density) = match self.open_tool {
                    Some(Tool::Stone) => (SliderId::StoneBrush, SliderId::StoneDensity),
                    Some(Tool::Iron) => (SliderId::IronBrush, SliderId::IronDensity),
                    Some(Tool::Copper) => (SliderId::CopperBrush, SliderId::CopperDensity),
                    Some(Tool::Sulphur) => (SliderId::SulphurBrush, SliderId::SulphurDensity),
                    Some(Tool::Oil) => (SliderId::OilBrush, SliderId::OilDensity),
                    _ => (SliderId::ForestBrush, SliderId::ForestDensity),
                };
                let sliders = [(l.brush, brush), (l.density, density)];
                if clicked {
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    }
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
            Layout::Passes(l) => {
                let sliders = [
                    (l.width, SliderId::PassWidth),
                    (l.walls, SliderId::PassWalls),
                    (l.winding, SliderId::PassWinding),
                ];
                if clicked {
                    if let Some((_, id)) = sliders.iter().find(|(r, _)| r.contains(mouse)) {
                        self.dragging = Some(*id);
                    }
                }
                if let Some(id) = self.dragging {
                    if let Some((track, _)) = sliders.iter().find(|(_, s)| *s == id) {
                        self.slider_mut(id).set_from_x(*track, mouse.x);
                    }
                }
            }
            Layout::Regions(l) => {
                if clicked {
                    if l.size.contains(mouse) {
                        self.dragging = Some(SliderId::RegionSize);
                    }
                    submit = l.button.contains(mouse);
                    clear_regions = l.clear.contains(mouse);
                }
                if self.dragging == Some(SliderId::RegionSize) {
                    self.region_size.set_from_x(l.size, mouse.x);
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

        if clear_regions {
            return has_map.then_some(UiAction::ClearRegions);
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
            Layout::Passes(_) => None,
            Layout::Regions(_) => has_map.then(|| UiAction::BreakIntoRegions { size: self.region_size.value, seed: random_seed() }),
            Layout::Forest(_) | Layout::Stone(_) | Layout::Iron(_) | Layout::Copper(_) | Layout::Sulphur(_) | Layout::Oil(_) => None,
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
}
