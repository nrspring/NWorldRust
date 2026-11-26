//! Turning the game grey except for some regions, as it is while the player holds the mouse over a label in the footer that lights up
//! regions: what is in a lit region keeps its color, and everything else is shown in shades of grey, so that the lit regions stand out.
//! The ground, the water and what grows are greyed by the map view (see [`nworld_view::MapView::set_greyscale`]). This is for what the
//! game draws on it: the pieces, the cities, the paths, the roads and the bridges. Which regions are lit is kept here, for each of
//! those to ask about, since it is one thing for all of them for the whole frame.

use std::cell::RefCell;
use std::collections::BTreeSet;

use macroquad::models::{Mesh, Vertex};
use nworld_map::regions::RegionId;
use nworld_map::world::World;

#[derive(Default)]
struct State {
    /// The numbers of the regions that keep their color, while greying.
    lit: Option<BTreeSet<u16>>,
    /// Changes whenever `lit` does.
    version: u64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// Greys everything but what is in the regions numbered `lit`, which may be none of them, or with `None` leaves everything in color.
pub fn set(lit: Option<&[RegionId]>) {
    let lit: Option<BTreeSet<u16>> = lit.map(|regions| regions.iter().map(|r| r.raw()).collect());
    STATE.with_borrow_mut(|state| {
        if state.lit != lit {
            state.lit = lit;
            state.version += 1;
        }
    });
}

/// While greying, a number that changes whenever what is greyed does, for keeping what was made grey until then. `None` while
/// everything is in color.
pub fn version() -> Option<u64> {
    STATE.with_borrow(|state| state.lit.is_some().then_some(state.version))
}

/// Whether what stands at (`x`, `z`) is shown in grey: greying is on, and it isn't in one of the lit regions.
pub fn greyed(world: &World, x: f32, z: f32) -> bool {
    STATE.with_borrow(|state| state.lit.as_ref().is_some_and(|lit| !world.region_at(x, z).is_some_and(|r| lit.contains(&r.raw()))))
}

/// The shade of grey as bright as the color `[red, green, blue, alpha]`, as solid as it was.
pub fn grey([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    let shade = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
    [shade, shade, shade, a]
}

/// A copy of `mesh` with the color of each of its corners that `is_greyed` (given its x and z) made grey. For what is drawn in its own
/// colors, with no light of its own that would color it again.
pub fn mesh(mesh: &Mesh, is_greyed: impl Fn(f32, f32) -> bool) -> Mesh {
    let vertices = mesh.vertices.iter().map(|v| Vertex { color: if is_greyed(v.position.x, v.position.z) { grey(v.color) } else { v.color }, ..*v }).collect();
    Mesh { vertices, indices: mesh.indices.clone(), texture: mesh.texture.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn nothing_is_greyed_until_it_is_set_and_then_all_but_the_lit_regions() {
        let world = test_support::regioned_continent();
        let spot = test_support::land_spots(&world, 2)[0];
        let here = world.region_at(spot.x, spot.y).unwrap();
        assert!(!greyed(&world, spot.x, spot.y));
        assert_eq!(version(), None);

        set(Some(&[]));
        assert!(greyed(&world, spot.x, spot.y), "no region is lit, so all are grey");
        let first = version().unwrap();
        set(Some(&[here]));
        assert!(!greyed(&world, spot.x, spot.y));
        let second = version().unwrap();
        assert_ne!(first, second);
        set(Some(&[here]));
        assert_eq!(version(), Some(second), "the same again changes nothing");
        // Somewhere with no region at all is not in a lit one.
        assert!(greyed(&world, world.half_extents().x * 3.0, 0.0));
        set(None);
        assert!(!greyed(&world, world.half_extents().x * 3.0, 0.0));
    }

    #[test]
    fn grey_is_as_bright_as_the_color_and_as_solid() {
        assert_eq!(grey([255, 255, 255, 200]), [255, 255, 255, 200]);
        assert_eq!(grey([0, 0, 0, 255]), [0, 0, 0, 255]);
        assert_eq!(grey([255, 0, 0, 255]), [76, 76, 76, 255]);
        let mesh_of = Mesh { vertices: vec![Vertex { position: macroquad::math::vec3(1.0, 0.0, 2.0), uv: Default::default(), color: [255, 0, 0, 255], normal: Default::default() }; 3], indices: vec![0, 1, 2], texture: None };
        assert_eq!(mesh(&mesh_of, |x, z| (x, z) == (1.0, 2.0)).vertices[0].color, [76, 76, 76, 255]);
        assert_eq!(mesh(&mesh_of, |_, _| false).vertices[0].color, [255, 0, 0, 255]);
    }
}
