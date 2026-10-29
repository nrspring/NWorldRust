//! A map being edited, and saving, opening and closing it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use macroquad::prelude::*;

use crate::camera::EditorCamera;
use crate::continents::ContinentParams;
use crate::dialogs::{self, Unsaved};
use crate::hills::{HillStroke, Peak};
use crate::props::DensityStroke;
use crate::history::History;
use crate::islands::IslandParams;
use crate::render;
use crate::render_data::RESOURCE_TEXTURES;
use crate::terrain_mesh::TerrainMesh;
use crate::shallows::ShallowStroke;
use crate::storage::{self, SavedMap};
use crate::toast::Toasts;
use crate::ui::TOOLBAR_WIDTH;
use crate::world::{Region, Resource, World};

/// A map being edited, with its camera, the backdrop sized to it, and its surface texture.
pub struct OpenMap {
    pub world: World,
    /// The terrain of the world as meshes.
    pub mesh: TerrainMesh,
    pub cam: EditorCamera,
    pub backdrop: Mesh,
    pub surface: Texture2D,
    /// The resource densities, for the terrain shader.
    pub resources: [Texture2D; RESOURCE_TEXTURES],
    /// The regions, for the terrain shader.
    pub region_texture: Texture2D,
    /// Whether the map has been divided into regions.
    has_regions: bool,
    pub history: History,
    /// Seeds the shape of every hill painted on this map.
    hill_seed: u32,
    /// The hills brush stroke being painted, while the mouse button is held.
    stroke: Option<HillStroke>,
    /// The shallows brush stroke being painted, while the mouse button is held.
    shallow_stroke: Option<ShallowStroke>,
    /// The brush stroke of a resource being painted, while the mouse button is held.
    resource_stroke: Option<(Resource, DensityStroke)>,
    /// Where the first end of a river was picked, while the Rivers tool waits for the second.
    pub river_start: Option<Vec2>,
    /// The points of the pass picked so far, while the Passes tool waits for the rest.
    pub pass_points: Vec<Vec2>,
    /// The file the map was last saved to or opened from.
    path: Option<PathBuf>,
    /// Whether the map has been edited since it was created, saved or opened.
    dirty: bool,
}

/// How an attempt to save went.
pub enum Outcome {
    Saved,
    /// The user cancelled the file dialog.
    Cancelled,
    Failed(String),
}

impl OpenMap {
    /// A new map of the given size in meters: deep ocean everywhere, seen from above.
    pub fn new(width: f32, height: f32) -> Self {
        Self::with_world(World::new(width, height))
    }

    /// A map around `world`.
    fn with_world(world: World) -> Self {
        let cam = EditorCamera::new(&world, TOOLBAR_WIDTH);
        let mut mesh = TerrainMesh::new(&world);
        mesh.update(&world, cam.position(), Duration::from_secs(10));
        let backdrop = render::backdrop_mesh(world.half_extents());
        let surface = render::surface_texture(&world);
        let resources = render::resources_texture(&world);
        let region_texture = render::regions_texture(&world);
        let has_regions = world.regions().iter().any(|&r| r != 0);
        Self {
            world,
            mesh,
            cam,
            backdrop,
            surface,
            resources,
            region_texture,
            has_regions,
            history: History::default(),
            hill_seed: rand::gen_range(1, u32::MAX),
            stroke: None,
            shallow_stroke: None,
            resource_stroke: None,
            river_start: None,
            pass_points: Vec::new(),
            path: None,
            dirty: false,
        }
    }

    /// A map read from the file at `path`; the error is a message fit to show the user.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let saved = storage::load(&path)?;
        let mut map = Self::with_world(World::from_saved(saved)?);
        map.path = Some(path);
        Ok(map)
    }

    fn to_saved(&self) -> SavedMap {
        self.world.to_saved()
    }

    /// The file name shown to the user.
    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| "Untitled".to_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// Whether the map has been edited since it was created, saved or opened.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Runs one building step on the world as a single undoable change, and redraws whatever the
    /// step changed.
    fn build<T>(&mut self, step: impl FnOnce(&mut World) -> T) -> T {
        let (heights, water, regions) = (self.world.heights().to_vec(), self.world.water().to_vec(), self.world.regions().to_vec());
        let version = self.world.revision().1;
        let out = step(&mut self.world);
        self.history.record(&heights, &water, self.world.heights(), self.world.water());
        self.history.record_regions(&regions, self.world.regions());
        if self.world.revision().1 != version {
            self.redraw();
            self.dirty = true;
        }
        if regions != self.world.regions() {
            self.regions_changed();
            self.dirty = true;
        }
        out
    }

    /// Adds continents to the land already on the map, as one undoable step.
    pub fn generate_continents(&mut self, params: &ContinentParams) {
        self.build(|world| world.generate_continents(params));
    }

    /// Sprinkles islands into the open water, as one undoable step, and returns how many were
    /// added, which can be fewer than asked for if there is no room.
    pub fn add_islands(&mut self, params: &IslandParams) -> usize {
        self.build(|world| world.add_islands(params))
    }

    /// Raises a randomly placed mountain range, as one undoable step. Returns false if there is
    /// no land with room for one.
    pub fn add_mountain_range(&mut self, length: f32, width: f32, seed: u32) -> bool {
        self.build(|world| world.add_mountain_range(length, width, seed))
    }

    /// Digs randomly placed lakes, about `size` meters across, as one undoable step, and returns
    /// how many fit, which can be fewer than asked for or none.
    pub fn add_lakes(&mut self, count: u32, size: f32, seed: u32) -> usize {
        self.build(|world| world.add_lakes(count, size, seed))
    }

    /// Cuts a pass through the points (world x and z, at least two) as one undoable step; see
    /// [`World::add_pass`].
    pub fn add_pass(&mut self, points: &[Vec2], width: f32, steepness: f32, winding: f32, seed: u32) -> Result<(), &'static str> {
        self.build(|world| world.add_pass(points, width, steepness, winding, seed))
    }

    /// Digs a river between two points (world x and z) as one undoable step; see
    /// [`World::add_river`].
    pub fn add_river(&mut self, from: Vec2, to: Vec2, width: f32, winding: f32, seed: u32) -> Result<(), &'static str> {
        self.build(|world| world.add_river(from, to, width, winding, seed))
    }

    /// Digs up to `count` rivers from random inland points down to the sea, as one undoable step,
    /// and returns how many fit.
    pub fn add_rivers(&mut self, count: u32, width: f32, winding: f32, seed: u32) -> usize {
        self.build(|world| world.add_rivers(count, width, winding, seed))
    }

    /// Makes the sea within `width` meters of land shallow, as one undoable step. Returns false if
    /// there was nowhere to: no land, or the sea there is already shallow.
    pub fn add_shallows(&mut self, width: f32) -> bool {
        self.build(|world| world.add_shallows(width))
    }

    /// Divides the map into regions about `size` meters across on average, as one undoable step,
    /// replacing any made before. Returns how many regions there are.
    pub fn generate_regions(&mut self, size: f32, seed: u32) -> usize {
        self.build(|world| world.divide_into_regions(size, seed))
    }

    /// Paints `kind` of `density` (0 to 1) onto dry land with a brush of `radius` meters at `center`
    /// (world x and z), or with `erase` clears it, continuing the current stroke or starting one
    /// (`kind`, `erase` and `density` only matter when starting). `dt` is the seconds since the
    /// last call. [`end_stroke`](Self::end_stroke) finishes it.
    pub fn paint_resource(&mut self, kind: Resource, center: Vec2, radius: f32, erase: bool, density: f32, dt: f32) {
        let (nx, nz) = self.world.samples();
        let half = self.world.half_extents();
        let (kind, stroke) = self.resource_stroke.get_or_insert_with(|| {
            let stroke = DensityStroke::begin(self.world.resource(kind), self.world.heights(), self.world.water(), nx, nz, erase, density);
            (kind, stroke)
        });
        if let Some(region) = stroke.paint(self.world.resource_mut(*kind), half, center, radius, dt) {
            self.world.resource_changed(*kind, region);
            render::update_resources(&self.resources, &self.world, *kind, region);
        }
    }

    /// Paints shallow water onto deep sea with a brush of `radius` meters at `center` (world x
    /// and z), or with `deepen` paints deep water back, continuing the current stroke or starting
    /// one (`deepen` only matters when starting). [`end_stroke`](Self::end_stroke) finishes it.
    pub fn paint_shallows(&mut self, center: Vec2, radius: f32, deepen: bool, dt: f32) {
        let (nx, nz) = self.world.samples();
        let half = self.world.half_extents();
        let stroke = self
            .shallow_stroke
            .get_or_insert_with(|| ShallowStroke::begin(self.world.heights(), self.world.water(), nx, nz, deepen));
        if let Some(region) = stroke.paint(self.world.heights_mut(), half, center, radius, dt) {
            self.refresh(region);
        }
    }

    /// Grows `peak`s with a brush of `radius` meters at `center` (world x and z), or with `erase`
    /// wears the ground down, continuing the current stroke or starting one (`erase` and `peak`
    /// only matter when starting). `dt` is the seconds since the last call, which sets how much
    /// the brush changes. [`end_stroke`](Self::end_stroke) finishes it.
    pub fn paint_peaks(&mut self, center: Vec2, radius: f32, erase: bool, peak: Peak, dt: f32) {
        let (nx, nz) = self.world.samples();
        let half = self.world.half_extents();
        let seed = self.hill_seed;
        let stroke = self
            .stroke
            .get_or_insert_with(|| HillStroke::begin_on(self.world.heights(), self.world.water(), nx, nz, seed, erase, peak));
        if let Some(region) = stroke.paint(self.world.heights_mut(), half, center, radius, dt) {
            copy_water(stroke.water(), self.world.water_mut(), nx, region);
            self.refresh(region);
        }
    }

    /// Finishes the hills or mountains stroke being painted: smooths its rim and records it as one undoable
    /// step.
    pub fn end_stroke(&mut self) {
        if let Some((kind, stroke)) = self.resource_stroke.take() {
            if stroke.base() != self.world.resource(kind) {
                self.history.record_resource(kind, stroke.base(), self.world.resource(kind));
                self.dirty = true;
            }
        }
        if let Some(stroke) = self.shallow_stroke.take() {
            if stroke.base() != self.world.heights() {
                self.history.record(stroke.base(), self.world.water(), self.world.heights(), self.world.water());
                self.dirty = true;
            }
        }
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        if let Some(region) = stroke.finish(self.world.heights_mut()) {
            self.refresh(region);
        }
        if stroke.base() != self.world.heights() || stroke.base_water() != self.world.water() {
            self.history.record(stroke.base(), stroke.base_water(), self.world.heights(), self.world.water());
            self.dirty = true;
        }
    }

    /// The hills stroke being painted, if any: `Some(true)` if it lowers the ground.
    pub fn stroke_erases(&self) -> Option<bool> {
        self.stroke
            .as_ref()
            .map(HillStroke::erases)
            .or_else(|| self.shallow_stroke.as_ref().map(ShallowStroke::deepens))
            .or_else(|| self.resource_stroke.as_ref().map(|(_, stroke)| stroke.erases()))
    }

    /// Redraws the part of the map that changed, without touching the rest.
    fn refresh(&mut self, region: Region) {
        let texels = self.world.refresh_region(region);
        render::update_surface(&self.surface, &self.world, texels);
        // The chunks the change touches are rebuilt now, not over the next frames.
        self.mesh.update(&self.world, self.cam.position(), Duration::from_millis(30));
    }

    /// Undoes the most recent edit. Returns false if there was nothing to undo.
    pub fn undo(&mut self) -> bool {
        let mut heights = self.world.heights().to_vec();
        let mut water = self.world.water().to_vec();
        let mut resources = Resource::ALL.map(|kind| self.world.resource(kind).to_vec());
        let mut regions = self.world.regions().to_vec();
        if !self.history.undo(&mut heights, &mut water, &mut resources, &mut regions) {
            return false;
        }
        self.apply_terrain(heights, water);
        if regions != self.world.regions() {
            self.apply_regions(regions);
        }
        if Resource::ALL.iter().any(|&kind| resources[kind.index()] != self.world.resource(kind)) {
            self.apply_resources(resources);
        }
        self.dirty = true;
        true
    }

    /// Puts new resource densities (in the order of [`Resource::ALL`]) on the map and refreshes everything drawn
    /// from them.
    fn apply_resources(&mut self, resources: [Vec<u8>; Resource::COUNT]) {
        for (kind, layer) in Resource::ALL.into_iter().zip(resources) {
            self.world.set_resource(kind, layer);
        }
        self.resources = render::resources_texture(&self.world);
    }

    /// Puts new regions on the map and refreshes everything drawn from them.
    fn apply_regions(&mut self, regions: Vec<u16>) {
        self.world.set_regions(regions);
        self.regions_changed();
    }

    /// Refreshes everything drawn from the map's regions after they changed.
    fn regions_changed(&mut self) {
        self.has_regions = self.world.regions().iter().any(|&r| r != 0);
        render::update_regions(&self.region_texture, &self.world);
    }

    /// Whether the map has been divided into regions.
    pub fn has_regions(&self) -> bool {
        self.has_regions
    }

    /// Takes the regions off the map, as one undoable step. Returns false if there were none.
    pub fn clear_regions(&mut self) -> bool {
        if self.world.regions().iter().all(|&r| r == 0) {
            return false;
        }
        let none = vec![0; self.world.regions().len()];
        self.history.record_regions(self.world.regions(), &none);
        self.apply_regions(none);
        self.dirty = true;
        true
    }

    /// Puts new heights and water levels on the map and refreshes everything drawn from them.
    fn apply_terrain(&mut self, heights: Vec<f32>, water: Vec<f32>) {
        self.world.set_terrain(heights, water);
        self.redraw();
    }

    /// Redraws the whole map after its terrain was replaced.
    fn redraw(&mut self) {
        self.surface = render::surface_texture(&self.world);
        // Rebuild every chunk now rather than streaming them in over the next frames.
        self.mesh.update(&self.world, self.cam.position(), Duration::from_secs(10));
    }

    /// Saves the map to its file, asking where first if it has none yet or `save_as` is set.
    pub fn save(&mut self, save_as: bool) -> Outcome {
        let path = match (&self.path, save_as) {
            (Some(path), false) => path.clone(),
            _ => match dialogs::pick_save(self.path.as_deref()) {
                Some(path) => path,
                None => return Outcome::Cancelled,
            },
        };
        self.save_to(path)
    }

    /// Saves the map to `path`, which becomes the map's file.
    pub fn save_to(&mut self, path: PathBuf) -> Outcome {
        match storage::save(&path, &self.to_saved()) {
            Ok(()) => {
                self.path = Some(path);
                self.dirty = false;
                Outcome::Saved
            }
            Err(e) => Outcome::Failed(e),
        }
    }
}

/// Copies the water levels of the samples in `region` from `from` to `to`, which are laid out
/// `nx` samples to the row.
fn copy_water(from: &[f32], to: &mut [f32], nx: usize, region: Region) {
    for j in region.j0..=region.j1 {
        let row = j * nx + region.i0..=j * nx + region.i1;
        to[row.clone()].copy_from_slice(&from[row]);
    }
}

/// Saves the map and reports the result. Returns true if it was saved.
pub fn save_map(map: &mut OpenMap, save_as: bool, toasts: &mut Toasts) -> bool {
    match map.save(save_as) {
        Outcome::Saved => {
            toasts.info(format!("Saved {}", map.name()));
            true
        }
        Outcome::Cancelled => false,
        Outcome::Failed(e) => {
            toasts.error(e);
            false
        }
    }
}

/// Whether it is fine to throw `map` away for `action` ("create a new map", "close NWorld"):
/// true if it has no unsaved edits, or the user chose to save them (and that worked) or to
/// discard them. Asks the user when there are unsaved edits.
pub fn can_discard(map: Option<&mut OpenMap>, action: &str, toasts: &mut Toasts) -> bool {
    let Some(map) = map.filter(|m| m.is_dirty()) else {
        return true;
    };
    match dialogs::ask_unsaved(&map.name(), action) {
        Unsaved::Save => save_map(map, false, toasts),
        Unsaved::Discard => true,
        Unsaved::Cancel => false,
    }
}

/// Lets the user pick a map file and opens it in place of the current map.
pub fn open_map(map: &mut Option<OpenMap>, toasts: &mut Toasts) {
    if !can_discard(map.as_mut(), "open another map", toasts) {
        return;
    }
    let Some(path) = dialogs::pick_open() else {
        return;
    };
    let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    match OpenMap::open(path) {
        Ok(opened) => {
            toasts.info(format!("Opened {}", opened.name()));
            *map = Some(opened);
        }
        Err(e) => toasts.error(format!("Couldn't open {name}. {e}")),
    }
}
