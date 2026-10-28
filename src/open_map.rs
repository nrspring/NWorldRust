//! A map being edited, and saving, opening and closing it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use macroquad::prelude::*;

use crate::camera::EditorCamera;
use crate::continents::{self, ContinentParams};
use crate::dialogs::{self, Unsaved};
use crate::hills::{self, HillStroke, Peak};
use crate::history::History;
use crate::islands::{self, IslandParams};
use crate::render;
use crate::storage::{self, SavedMap};
use crate::toast::Toasts;
use crate::ui::TOOLBAR_WIDTH;
use crate::world::{Region, World, CELL_SIZE};

/// A map being edited, with its camera, the backdrop sized to it, and its surface texture.
pub struct OpenMap {
    pub world: World,
    pub cam: EditorCamera,
    pub backdrop: Mesh,
    pub surface: Texture2D,
    /// How many times continents have been generated on this map, undone or not.
    generations: u32,
    pub history: History,
    /// Seeds the shape of every hill painted on this map.
    hill_seed: u32,
    /// The hills brush stroke being painted, while the mouse button is held.
    stroke: Option<HillStroke>,
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
        let mut world = World::new(width, height);
        let cam = EditorCamera::new(&world, TOOLBAR_WIDTH);
        world.update(cam.position(), Duration::from_secs(10));
        let backdrop = render::backdrop_mesh(world.half_extents());
        let surface = render::surface_texture(&world);
        Self {
            world,
            cam,
            backdrop,
            surface,
            generations: 0,
            history: History::default(),
            hill_seed: rand::gen_range(1, u32::MAX),
            stroke: None,
            path: None,
            dirty: false,
        }
    }

    /// A map read from the file at `path`; the error is a message fit to show the user.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let saved = storage::load(&path)?;
        let mut map = Self::new(saved.cells_x as f32 * CELL_SIZE, saved.cells_z as f32 * CELL_SIZE);
        if map.world.cells() != (saved.cells_x, saved.cells_z) {
            return Err("The map size in this file isn't supported.".to_owned());
        }
        map.apply_terrain(saved.heights, saved.water);
        map.generations = saved.generations;
        map.path = Some(path);
        Ok(map)
    }

    fn to_saved(&self) -> SavedMap {
        let (cells_x, cells_z) = self.world.cells();
        SavedMap {
            cells_x,
            cells_z,
            generations: self.generations,
            heights: self.world.heights().to_vec(),
            water: self.world.water().to_vec(),
        }
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

    /// Adds continents to the land already on the map, as one undoable step.
    pub fn generate_continents(&mut self, params: &ContinentParams) {
        let (cells_x, cells_z) = self.world.cells();
        let params = ContinentParams {
            seed: self.next_seed(params.seed),
            ..*params
        };
        let heights = continents::generate(&params, cells_x, cells_z, self.world.heights());
        self.history.record(self.world.heights(), self.world.water(), &heights, self.world.water());
        self.apply_heights(heights);
        self.dirty = true;
    }

    /// Sprinkles islands into the open water, as one undoable step, and returns how many were
    /// added, which can be fewer than asked for if there is no room. Seeded like
    /// [`generate_continents`](Self::generate_continents).
    pub fn add_islands(&mut self, params: &IslandParams) -> usize {
        let (cells_x, cells_z) = self.world.cells();
        let params = IslandParams {
            seed: self.next_seed(params.seed),
            ..*params
        };
        let (heights, added) = islands::generate(&params, cells_x, cells_z, self.world.heights());
        if heights != self.world.heights() {
            self.history.record(self.world.heights(), self.world.water(), &heights, self.world.water());
            self.apply_heights(heights);
            self.dirty = true;
        }
        added
    }

    /// Raises a randomly placed mountain range, as one undoable step. Returns false if there is
    /// no land with room for one. Seeded like [`generate_continents`](Self::generate_continents).
    pub fn add_mountain_range(&mut self, length: f32, width: f32, seed: u32) -> bool {
        let (nx, nz) = self.world.samples();
        let seed = self.next_seed(seed);
        let Some(heights) = hills::mountain_range(self.world.heights(), nx, nz, self.world.half_extents(), seed, length, width)
        else {
            return false;
        };
        self.history.record(self.world.heights(), self.world.water(), &heights, self.world.water());
        self.apply_heights(heights);
        self.dirty = true;
        true
    }

    /// Digs randomly placed lakes, about `size` meters across, as one undoable step, and returns
    /// how many fit, which can be fewer than asked for or none. Seeded like
    /// [`generate_continents`](Self::generate_continents).
    pub fn add_lakes(&mut self, count: u32, size: f32, seed: u32) -> usize {
        let (nx, nz) = self.world.samples();
        let seed = self.next_seed(seed);
        let Some((heights, water, added)) =
            hills::scatter_lakes(self.world.heights(), self.world.water(), nx, nz, self.world.half_extents(), seed, count, size)
        else {
            return 0;
        };
        self.history.record(self.world.heights(), self.world.water(), &heights, &water);
        self.apply_terrain(heights, water);
        self.dirty = true;
        added
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
        self.stroke.as_ref().map(HillStroke::erases)
    }

    /// Redraws the part of the map that changed, without touching the rest.
    fn refresh(&mut self, region: Region) {
        let texels = self.world.refresh_region(region, self.cam.position());
        render::update_surface(&self.surface, &self.world, texels);
    }

    /// The seed for the next generation run: the one the user chose, mixed with how many runs
    /// there have been on this map, so pressing the button again adds something different, even
    /// after an undo. The same map, settings, seed and number of earlier runs give the same result.
    fn next_seed(&mut self, seed: u32) -> u32 {
        let salted = seed.wrapping_add(self.generations.wrapping_mul(0x9E37_79B9));
        self.generations += 1;
        salted
    }

    /// Undoes the most recent edit. Returns false if there was nothing to undo.
    pub fn undo(&mut self) -> bool {
        let mut heights = self.world.heights().to_vec();
        let mut water = self.world.water().to_vec();
        if !self.history.undo(&mut heights, &mut water) {
            return false;
        }
        self.apply_terrain(heights, water);
        self.dirty = true;
        true
    }

    /// Puts new heights on the map, leaving the water levels as they are, and refreshes
    /// everything drawn from them.
    fn apply_heights(&mut self, heights: Vec<f32>) {
        self.world.set_heights(heights);
        self.redraw();
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
        self.world.update(self.cam.position(), Duration::from_secs(10));
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
