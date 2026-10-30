//! Everything needed to draw one map: [`MapView`].

use std::time::Duration;

use macroquad::models::{draw_mesh, Mesh};
use macroquad::prelude::*;
use nworld_map::world::World;

use crate::props::{PropKind, PropRenderer};
use crate::render::{self, TerrainRenderer};
use crate::render_data::RESOURCE_TEXTURES;
use crate::terrain_mesh::TerrainMesh;
use crate::{copper, forest, iron, oil, stone, sulphur};

/// How far you can see from the ground, in meters.
const BASE_VIEW_DISTANCE: f32 = 4000.0;

/// How far you can see from a camera at `height` meters, in meters. Higher up shows more.
/// A camera drawing a [`MapView`] should put its far plane at `view_distance(height) * 1.5`.
pub fn view_distance(height: f32) -> f32 {
    BASE_VIEW_DISTANCE + height.max(0.0) * 1.5
}

/// Time spent each frame building terrain meshes that are out of date or newly in range.
const MESH_BUDGET: Duration = Duration::from_millis(6);

/// A map as something to look at. It holds the GPU meshes, textures and shaders of one map, so it
/// needs a window: create it after macroquad has started.
///
/// It draws the map as it was when the view was made. A program that edits the map keeps the
/// pieces in step itself, as the editor does.
pub struct MapView {
    mesh: TerrainMesh,
    terrain: TerrainRenderer,
    props: Vec<PropRenderer>,
    backdrop: Mesh,
    backdrop_material: Material,
    sky_material: Material,
    surface: Texture2D,
    resources: [Texture2D; RESOURCE_TEXTURES],
    regions: Texture2D,
}

impl MapView {
    /// A view of `world`, with its terrain built for a camera at `from`.
    pub fn new(world: &World, from: Vec3) -> Self {
        let mut mesh = TerrainMesh::new(world);
        mesh.update(world, from, Duration::from_secs(10));
        // In the order of `Resource::ALL`.
        let kinds: [PropKind; 6] = [forest::kind(), stone::kind(), iron::kind(), copper::kind(), sulphur::kind(), oil::kind()];
        Self {
            mesh,
            terrain: TerrainRenderer::new(),
            props: kinds.into_iter().map(PropRenderer::new).collect(),
            backdrop: render::backdrop_mesh(world.half_extents()),
            backdrop_material: render::load_backdrop_material(),
            sky_material: render::load_sky_material(),
            surface: render::surface_texture(world),
            resources: render::resources_texture(world),
            regions: render::regions_texture(world),
        }
    }

    /// The camera for looking at the map from `position` towards `target`, with the field of view
    /// and clip planes the view is made for.
    pub fn camera(position: Vec3, target: Vec3, up: Vec3, z_near: f32) -> Camera3D {
        Camera3D {
            position,
            target,
            up,
            fovy: crate::FOV_DEGREES.to_radians(),
            z_near,
            z_far: view_distance(position.y) * 1.5,
            ..Default::default()
        }
    }

    /// Draws the sky, the terrain and its props as seen by `camera`. `time` is in seconds and
    /// moves the water. The caller draws anything of its own over it with `camera` still set,
    /// then calls `set_default_camera` for its interface.
    pub fn draw(&mut self, world: &World, camera: &Camera3D, time: f32) {
        let pos = camera.position;
        let view_distance = view_distance(pos.y);
        self.mesh.update(world, pos, MESH_BUDGET);

        set_default_camera();
        clear_background(render::background_color());
        render::draw_sky(&self.sky_material, camera.matrix(), pos, time);
        set_camera(camera);

        render::apply_backdrop_uniforms(&self.backdrop_material, pos, view_distance, world.half_extents());
        gl_use_material(&self.backdrop_material);
        draw_mesh(&self.backdrop);

        self.terrain.draw(&mut self.mesh, world, &self.surface, &self.resources, &self.regions, false, None, camera, pos, view_distance, time, false);
        for kind in &mut self.props {
            kind.draw(world, camera, pos, view_distance, time);
        }
        gl_use_default_material();
    }
}
