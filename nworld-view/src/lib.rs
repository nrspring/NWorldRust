//! Draws an `nworld-map` map with macroquad: the terrain as meshes and textures, the sky and the
//! backdrop around the world, and the trees and rocks of the resources. It also holds the dark,
//! rounded look (`style`) of the interfaces drawn over the map.
//!
//! The editor and the game share this crate, so a map looks the same in both. Most programs only
//! need [`MapView`]. The pieces under it are public for the editor, which updates textures and
//! meshes as the map is painted.

pub mod copper;
pub mod forest;
pub mod iron;
mod map_view;
pub mod oil;
mod picking;
pub mod props;
pub mod render;
pub mod render_data;
pub mod stone;
pub mod style;
pub mod sulphur;
pub mod terrain_mesh;

pub use map_view::{view_distance, MapView};
pub use picking::pick_terrain;

/// The vertical field of view of the 3D camera, in degrees. The terrain's level of detail depends
/// on it, so cameras that draw a `MapView` should use it.
pub const FOV_DEGREES: f32 = 65.0;
