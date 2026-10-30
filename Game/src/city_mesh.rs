//! Turns a city's buildings into shapes to draw: boxes with gabled or hipped roofs, spires and
//! chimneys. The shapes are plain vertices and indices, so this needs no window.

use macroquad::models::Vertex;
use macroquad::prelude::*;

use crate::city_layout::{Building, Roof, Walls};

/// What a mesh can hold, because its indices are 16 bits.
pub const MAX_VERTICES: usize = u16::MAX as usize;

/// A chimney is this wide, in meters, and stands this far above the ridge of the roof.
const CHIMNEY_WIDTH: f32 = 0.55;
const CHIMNEY_ABOVE_RIDGE: f32 = 0.75;
const CHIMNEY_BRICK: [u8; 3] = [140, 82, 66];
const CHIMNEY_TOP: [u8; 3] = [58, 50, 48];

#[derive(Default)]
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Geometry {
    /// A flat face with the given corners, in order around it, lit as facing `normal`.
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [u8; 3]) {
        self.face(corners, [Vec2::ZERO; 4], normal, 0.0, color);
    }

    /// A wall: a quad whose surface coordinates are meters along it and up it, and which is marked
    /// with how it looks, as [`Walls::flag`]. `length` is the wall's width, from the first corner to
    /// the second.
    fn wall(&mut self, corners: [Vec3; 4], length: f32, normal: Vec3, walls: Walls, color: [u8; 3]) {
        let height = corners[3].y - corners[0].y;
        self.face(corners, [vec2(0.0, 0.0), vec2(length, 0.0), vec2(length, height), vec2(0.0, height)], normal, walls.flag(), color);
    }

    /// `style` rides along in the fourth part of the normal, which nothing else uses: 0 for a surface
    /// with nothing drawn on it, and otherwise how a wall looks.
    fn face(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], normal: Vec3, style: f32, color: [u8; 3]) {
        let first = self.vertices.len() as u16;
        for (corner, uv) in corners.into_iter().zip(uvs) {
            self.vertices.push(Vertex { position: corner, uv, color: [color[0], color[1], color[2], 255], normal: normal.extend(style) });
        }
        self.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    fn triangle(&mut self, corners: [Vec3; 3], normal: Vec3, color: [u8; 3]) {
        let first = self.vertices.len() as u16;
        for corner in corners {
            self.vertices.push(Vertex { position: corner, uv: Vec2::ZERO, color: [color[0], color[1], color[2], 255], normal: normal.extend(0.0) });
        }
        self.indices.extend([first, first + 1, first + 2]);
    }

    /// A triangle of wall, like the end of a gabled roof, with surface coordinates as on a wall.
    fn wall_triangle(&mut self, corners: [Vec3; 3], length: f32, normal: Vec3, walls: Walls, color: [u8; 3]) {
        let first = self.vertices.len() as u16;
        let rise = corners[2].y - corners[0].y;
        for (corner, uv) in corners.into_iter().zip([vec2(0.0, 0.0), vec2(length, 0.0), vec2(length * 0.5, rise)]) {
            self.vertices.push(Vertex { position: corner, uv, color: [color[0], color[1], color[2], 255], normal: normal.extend(walls.flag()) });
        }
        self.indices.extend([first, first + 1, first + 2]);
    }

    /// A sloping roof face. Its normal comes from its corners, and always points upward.
    fn slope(&mut self, corners: [Vec3; 4], color: [u8; 3]) {
        let normal = up((corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize());
        self.quad(corners, normal, color);
    }

    fn slope_triangle(&mut self, corners: [Vec3; 3], color: [u8; 3]) {
        let normal = up((corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize());
        self.triangle(corners, normal, color);
    }

    fn building(&mut self, b: &Building) {
        let (sin, cos) = b.yaw.sin_cos();
        // The building's own axes on the ground: across its width, and along its depth.
        let right = vec3(cos, 0.0, sin);
        let along = vec3(-sin, 0.0, cos);
        let centre = vec3(b.x, 0.0, b.z);
        let (half_w, half_d) = (b.width * 0.5, b.depth * 0.5);
        let corner = |sx: f32, sz: f32, y: f32| centre + right * (sx * half_w) + along * (sz * half_d) + Vec3::Y * y;

        // The four walls, each lit by the way it faces.
        for (a, b2, normal, length) in [((1.0, 1.0), (-1.0, 1.0), along, b.width), ((-1.0, 1.0), (-1.0, -1.0), -right, b.depth), ((-1.0, -1.0), (1.0, -1.0), -along, b.width), ((1.0, -1.0), (1.0, 1.0), right, b.depth)] {
            self.wall([corner(a.0, a.1, b.base), corner(b2.0, b2.1, b.base), corner(b2.0, b2.1, b.top), corner(a.0, a.1, b.top)], length, normal, b.walls, b.wall);
        }

        match b.roof {
            Roof::Flat => self.quad([corner(1.0, 1.0, b.top), corner(-1.0, 1.0, b.top), corner(-1.0, -1.0, b.top), corner(1.0, -1.0, b.top)], Vec3::Y, b.roof_color),
            Roof::Gable(rise) => {
                // Two slopes with the ridge along the depth, and a wall triangle, a gable, at each end.
                let ridge = |end: f32| centre + along * (end * half_d) + Vec3::Y * (b.top + rise);
                for side in [1.0f32, -1.0] {
                    self.slope([corner(side, 1.0, b.top), corner(side, -1.0, b.top), ridge(-1.0), ridge(1.0)], b.roof_color);
                }
                for end in [1.0f32, -1.0] {
                    self.wall_triangle([corner(1.0, end, b.top), corner(-1.0, end, b.top), ridge(end)], b.width, along * end, b.walls, b.wall);
                }
            }
            Roof::Hip(rise) => {
                // The ridge is shorter than the roof, so the ends slope too; on a square building it is
                // a point.
                let inset = (half_d - half_w).max(0.0);
                let ridge = |end: f32| centre + along * (end * inset) + Vec3::Y * (b.top + rise);
                self.slope([corner(1.0, 1.0, b.top), corner(1.0, -1.0, b.top), ridge(-1.0), ridge(1.0)], b.roof_color);
                self.slope([corner(-1.0, -1.0, b.top), corner(-1.0, 1.0, b.top), ridge(1.0), ridge(-1.0)], b.roof_color);
                self.slope_triangle([corner(-1.0, 1.0, b.top), corner(1.0, 1.0, b.top), ridge(1.0)], b.roof_color);
                self.slope_triangle([corner(1.0, -1.0, b.top), corner(-1.0, -1.0, b.top), ridge(-1.0)], b.roof_color);
            }
        }

        // Chimneys, one at each end of the roof, rising from inside it to above the ridge.
        for k in 0..b.chimneys {
            let side = if k == 0 { 1.0 } else { -1.0 };
            let at = centre + right * (side * 0.2 * b.width) + along * (side * 0.28 * b.depth);
            self.building(&Building {
                x: at.x,
                z: at.z,
                width: CHIMNEY_WIDTH,
                depth: CHIMNEY_WIDTH,
                yaw: b.yaw,
                base: b.top,
                top: b.top + b.roof.rise() + CHIMNEY_ABOVE_RIDGE,
                roof: Roof::Flat,
                walls: Walls::Plain,
                wall: CHIMNEY_BRICK,
                roof_color: CHIMNEY_TOP,
                chimneys: 0,
            });
        }
    }
}

/// `normal`, turned over if it points down.
fn up(normal: Vec3) -> Vec3 {
    if normal.y < 0.0 { -normal } else { normal }
}

/// How many vertices a mesh is filled to before the next building starts another. A building takes
/// at most about a hundred, so a mesh stays well under [`MAX_VERTICES`].
const MESH_VERTICES: usize = 60_000;

/// The shapes of all of `buildings`, in as many meshes as it takes to hold them.
pub fn build(buildings: &[Building]) -> Vec<Geometry> {
    let mut meshes = vec![Geometry::default()];
    for b in buildings {
        let mesh = meshes.last_mut().expect("there is always a mesh");
        mesh.building(b);
        debug_assert!(mesh.vertices.len() < MAX_VERTICES, "a mesh holds too many vertices");
        if mesh.vertices.len() >= MESH_VERTICES {
            meshes.push(Geometry::default());
        }
    }
    if meshes.last().is_some_and(|mesh| mesh.vertices.is_empty()) && meshes.len() > 1 {
        meshes.pop();
    }
    meshes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn house() -> Building {
        Building { x: 100.0, z: -40.0, width: 10.0, depth: 12.0, yaw: 0.7, base: 4.2, top: 11.0, roof: Roof::Gable(3.0), walls: Walls::Timber, wall: [200, 190, 170], roof_color: [150, 70, 60], chimneys: 0 }
    }

    fn block() -> Building {
        Building { roof: Roof::Hip(3.5), walls: Walls::Masonry, chimneys: 2, top: 20.0, ..house() }
    }

    fn spire() -> Building {
        Building { width: 6.0, depth: 6.0, roof: Roof::Hip(12.0), walls: Walls::Church, top: 30.0, ..house() }
    }

    /// The shapes of `buildings`, which are few enough for one mesh.
    fn one(buildings: &[Building]) -> Geometry {
        let mut meshes = build(buildings);
        assert_eq!(meshes.len(), 1);
        meshes.remove(0)
    }

    fn flat() -> Building {
        Building { roof: Roof::Flat, walls: Walls::Masonry, top: 70.0, ..house() }
    }

    #[test]
    fn every_index_points_at_a_vertex_and_faces_are_whole() {
        for b in [house(), block(), spire(), flat()] {
            let g = one(&[b]);
            assert!(!g.vertices.is_empty());
            assert_eq!(g.indices.len() % 3, 0);
            assert!(g.indices.iter().all(|&i| (i as usize) < g.vertices.len()));
        }
    }

    #[test]
    fn normals_are_unit_length_and_walls_face_outward() {
        for b in [house(), block(), spire()] {
            let g = one(&[b.clone()]);
            for v in &g.vertices {
                assert!((v.normal.truncate().length() - 1.0).abs() < 1e-4, "{:?}", v.normal);
            }
            // Each wall's normal points away from the building's middle.
            for wall in g.vertices[..16].chunks(4) {
                let middle = vec3(b.x, wall[0].position.y, b.z);
                let face_centre = (wall[0].position + wall[2].position) * 0.5;
                assert!((face_centre - middle).dot(wall[0].normal.truncate()) > 0.0);
            }
        }
    }

    #[test]
    fn roof_slopes_face_upward() {
        for b in [house(), block(), spire()] {
            let g = one(&[Building { chimneys: 0, ..b }]);
            // After the four walls come the roof faces, and then, on a gable, its end walls.
            for v in &g.vertices[16..] {
                if v.normal.w == 0.0 {
                    assert!(v.normal.y > 0.0, "{:?}", v.normal);
                }
            }
        }
    }

    #[test]
    fn a_house_stays_between_its_base_and_its_ridge() {
        let g = one(&[house()]);
        let (low, high) = g.vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.position.y), hi.max(v.position.y)));
        assert!((low - 4.2).abs() < 1e-4 && (high - 14.0).abs() < 1e-4, "{low} {high}");
    }

    #[test]
    fn chimneys_rise_above_the_ridge() {
        let b = block();
        let g = one(&[b.clone()]);
        let high = g.vertices.iter().map(|v| v.position.y).fold(f32::MIN, f32::max);
        assert!((high - (b.top + 3.5 + CHIMNEY_ABOVE_RIDGE)).abs() < 1e-4, "{high}");
        // Two chimneys, 20 vertices each, on top of the building itself.
        let without = one(&[Building { chimneys: 0, ..b }]);
        assert_eq!(g.vertices.len(), without.vertices.len() + 40);
    }

    #[test]
    fn walls_are_marked_with_their_style_and_roofs_and_chimneys_are_not() {
        let g = one(&[house()]);
        assert!(g.vertices[..16].iter().all(|v| v.normal.w == 2.0));
        // The roof's two slopes have no style; the gables at its ends are wall, and have the wall's.
        assert!(g.vertices[16..24].iter().all(|v| v.normal.w == 0.0));
        assert!(g.vertices[24..].iter().all(|v| v.normal.w == 2.0));

        assert!(one(&[spire()]).vertices[..16].iter().all(|v| v.normal.w == 3.0));
        let with_chimneys = one(&[block()]);
        assert!(with_chimneys.vertices[..16].iter().all(|v| v.normal.w == 1.0));
        assert!(with_chimneys.vertices[with_chimneys.vertices.len() - 40..].iter().all(|v| v.normal.w == 0.0));
    }

    #[test]
    fn a_wall_has_surface_coordinates_in_meters() {
        let g = one(&[house()]);
        let wall = &g.vertices[..4];
        // 10 m wide, and from the base at 4.2 up to the top at 11.0.
        assert_eq!(wall[1].uv.x, 10.0);
        assert!((wall[2].uv.y - 6.8).abs() < 1e-4);
    }

    #[test]
    fn a_square_hipped_roof_comes_to_a_point() {
        let g = one(&[spire()]);
        let apex: Vec<_> = g.vertices.iter().filter(|v| (v.position.y - 42.0).abs() < 1e-4).collect();
        assert!(!apex.is_empty());
        assert!(apex.iter().all(|v| (v.position - apex[0].position).length() < 1e-4));
    }

    #[test]
    fn a_flat_roof_has_a_top_that_faces_up() {
        let g = one(&[flat()]);
        let top = &g.vertices[16..20];
        assert!(top.iter().all(|v| (v.position.y - 70.0).abs() < 1e-4 && v.normal.y > 0.99));
    }

    #[test]
    fn a_big_city_is_split_so_that_no_mesh_is_too_full() {
        use crate::city::{City, MAX_POPULATION};
        use crate::ids::{CityId, CountryId};
        use crate::test_support;
        let world = test_support::small_continent();
        let at = test_support::inland_spot(&world, 270.0);
        let city = City { id: CityId::from_u128(1), country: CountryId::from_u128(1), name: "Test".to_owned(), x: at.x, z: at.y, region: None, population: MAX_POPULATION, under_siege: false };
        let buildings = crate::city_layout::buildings(&city, &world);
        let meshes = build(&buildings);
        assert!(meshes.iter().all(|m| !m.vertices.is_empty() && m.vertices.len() < MAX_VERTICES));
        // Splitting loses nothing: the same shapes, made in one go, have as many vertices.
        let mut whole = Geometry::default();
        for b in &buildings {
            whole.building(b);
        }
        assert_eq!(meshes.iter().map(|m| m.vertices.len()).sum::<usize>(), whole.vertices.len());
    }

    #[test]
    fn many_buildings_go_into_more_than_one_mesh() {
        let buildings: Vec<Building> = (0..2_000).map(|k| Building { x: k as f32 * 20.0, ..block() }).collect();
        let meshes = build(&buildings);
        assert!(meshes.len() >= 2);
        for mesh in &meshes {
            assert!(mesh.vertices.len() < MAX_VERTICES);
            assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
        }
    }
}
