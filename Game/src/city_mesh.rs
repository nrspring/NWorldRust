//! Turns a city's buildings into shapes to draw: boxes with gabled or hipped roofs that overhang
//! their walls, on stone footings, with cornices, dormers, chimneys and spires. The shapes are plain
//! vertices and indices, so this needs no window.

use macroquad::models::Vertex;
use macroquad::prelude::*;

use crate::city_layout::{Building, Roof, Walls};

/// What a mesh can hold, because its indices are 16 bits.
pub const MAX_VERTICES: usize = u16::MAX as usize;

/// A chimney is this wide, in meters, and stands this far above the ridge of the roof. Its cap is a
/// little wider and this tall.
const CHIMNEY_WIDTH: f32 = 0.55;
const CHIMNEY_ABOVE_RIDGE: f32 = 0.75;
const CHIMNEY_CAP: (f32, f32) = (0.18, 0.2);
const CHIMNEY_BRICK: [u8; 3] = [140, 82, 66];
const CHIMNEY_TOP: [u8; 3] = [58, 50, 48];

/// A roof reaches this far past its walls, at most, in meters, and comes down a little there.
const EAVES: f32 = 0.28;
/// A stone footing under timber, masonry and stone walls: how tall it is and how far it sticks out.
const PLINTH: f32 = 0.9;
const PLINTH_OUT: f32 = 0.12;
const PLINTH_STONE: [u8; 3] = [136, 131, 122];
const PLINTH_TOP: [u8; 3] = [166, 161, 150];
/// A band of dressed stone under the eaves of masonry walls: how far below the top of the walls it
/// starts, how tall it is and how far it sticks out.
const CORNICE: (f32, f32, f32) = (0.5, 0.36, 0.15);
const CORNICE_STONE: [u8; 3] = [210, 202, 184];
/// A dormer window in a gabled roof: its width along the roof, in meters.
const DORMER_WIDTH: f32 = 1.0;
/// The weathered gold of the finial and cross on a spire.
const GOLD: [u8; 3] = [214, 178, 88];

/// The style, in the fourth part of a normal, of a roof slope, which the shader draws courses on.
pub const ROOF_STYLE: f32 = 4.0;

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
    /// with nothing drawn on it, and otherwise how a wall or roof looks.
    fn face(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4], normal: Vec3, style: f32, color: [u8; 3]) {
        let first = self.vertices.len() as u16;
        for (corner, uv) in corners.into_iter().zip(uvs) {
            self.vertices.push(Vertex { position: corner, uv, color: [color[0], color[1], color[2], 255], normal: normal.extend(style) });
        }
        self.indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    fn triangle(&mut self, corners: [Vec3; 3], uvs: [Vec2; 3], normal: Vec3, style: f32, color: [u8; 3]) {
        let first = self.vertices.len() as u16;
        for (corner, uv) in corners.into_iter().zip(uvs) {
            self.vertices.push(Vertex { position: corner, uv, color: [color[0], color[1], color[2], 255], normal: normal.extend(style) });
        }
        self.indices.extend([first, first + 1, first + 2]);
    }

    /// A triangle of wall, like the end of a gabled roof, with surface coordinates as on a wall.
    /// Its surface coordinates go on up from the wall below it, which starts at height `base`.
    fn wall_triangle(&mut self, corners: [Vec3; 3], base: f32, length: f32, normal: Vec3, walls: Walls, color: [u8; 3]) {
        let (low, high) = (corners[0].y - base, corners[2].y - base);
        self.triangle(corners, [vec2(0.0, low), vec2(length, low), vec2(length * 0.5, high)], normal, walls.flag(), color);
    }

    /// A sloping roof face. Its normal comes from its corners, and always points upward. Its surface
    /// coordinates are meters along its first edge and up the slope, for the shader's courses.
    fn slope(&mut self, corners: [Vec3; 4], color: [u8; 3]) {
        let (normal, across, down) = roof_axes(corners[0], corners[1], corners[2]);
        self.face(corners, corners.map(|c| roof_uv(c - corners[0], across, down)), normal, ROOF_STYLE, color);
    }

    fn slope_triangle(&mut self, corners: [Vec3; 3], color: [u8; 3]) {
        let (normal, across, down) = roof_axes(corners[0], corners[1], corners[2]);
        self.triangle(corners, corners.map(|c| roof_uv(c - corners[0], across, down)), normal, ROOF_STYLE, color);
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

        // A roof reaches past the walls, and comes down there as much as its slope takes it.
        let eaves = EAVES.min(0.25 * b.width);
        let eave = |sx: f32, sz: f32, drop: f32| centre + right * (sx * (half_w + eaves)) + along * (sz * (half_d + eaves)) + Vec3::Y * (b.top - drop);
        let drop = |across: f32| eaves * b.roof.rise() / across;
        match b.roof {
            Roof::Flat => self.quad([corner(1.0, 1.0, b.top), corner(-1.0, 1.0, b.top), corner(-1.0, -1.0, b.top), corner(1.0, -1.0, b.top)], Vec3::Y, b.roof_color),
            Roof::Gable(rise) => {
                // Two slopes with the ridge along the depth, and a wall triangle, a gable, at each end.
                let drop = drop(half_w);
                let ridge = |end: f32| centre + along * (end * (half_d + eaves)) + Vec3::Y * (b.top + rise);
                for side in [1.0f32, -1.0] {
                    self.slope([eave(side, 1.0, drop), eave(side, -1.0, drop), ridge(-1.0), ridge(1.0)], b.roof_color);
                }
                for end in [1.0f32, -1.0] {
                    let apex = centre + along * (end * half_d) + Vec3::Y * (b.top + rise);
                    self.wall_triangle([corner(1.0, end, b.top), corner(-1.0, end, b.top), apex], b.base, b.width, along * end, b.walls, b.wall);
                }
            }
            Roof::Hip(rise) => {
                // The ridge is shorter than the roof, so the ends slope too; on a square building it is
                // a point.
                let inset = (half_d - half_w).max(0.0);
                let ridge = |end: f32| centre + along * (end * inset) + Vec3::Y * (b.top + rise);
                let drop = drop(half_w.min(half_d));
                self.slope([eave(1.0, 1.0, drop), eave(1.0, -1.0, drop), ridge(-1.0), ridge(1.0)], b.roof_color);
                self.slope([eave(-1.0, -1.0, drop), eave(-1.0, 1.0, drop), ridge(1.0), ridge(-1.0)], b.roof_color);
                self.slope_triangle([eave(-1.0, 1.0, drop), eave(1.0, 1.0, drop), ridge(1.0)], b.roof_color);
                self.slope_triangle([eave(1.0, -1.0, drop), eave(-1.0, -1.0, drop), ridge(-1.0)], b.roof_color);
            }
        }

        // What is built on and around the walls and roof. These are boxes of their own, with nothing
        // drawn on their walls, so they are never the first faces of a building.
        if matches!(b.walls, Walls::Timber | Walls::Masonry | Walls::Church) {
            self.footing(b, PLINTH_OUT, b.base, b.base + PLINTH, PLINTH_STONE, PLINTH_TOP);
        }
        if b.walls == Walls::Masonry && b.top - b.base > 5.0 {
            self.footing(b, CORNICE.2, b.top - CORNICE.0, b.top - CORNICE.0 + CORNICE.1, CORNICE_STONE, CORNICE_STONE);
        }
        self.dormers(b, centre, right, along);
        self.chimneys(b, centre, right, along);
        if let (Walls::Church, Roof::Hip(rise)) = (b.walls, b.roof) {
            if (b.width - b.depth).abs() < 1e-3 && rise > 3.0 {
                self.finial(b, b.top + rise);
            }
        }
    }

    /// A plain box around `b`'s walls, `out` meters wider all round, from height `base` to `top`.
    fn footing(&mut self, b: &Building, out: f32, base: f32, top: f32, wall: [u8; 3], roof_color: [u8; 3]) {
        self.building(&Building { width: b.width + 2.0 * out, depth: b.depth + 2.0 * out, base, top, roof: Roof::Flat, walls: Walls::Plain, wall, roof_color, chimneys: 0, dormers: 0, ..b.clone() });
    }

    /// Chimneys, one at each end of the roof, rising from inside it to above the ridge, with a cap.
    fn chimneys(&mut self, b: &Building, centre: Vec3, right: Vec3, along: Vec3) {
        for k in 0..b.chimneys {
            let side = if k == 0 { 1.0 } else { -1.0 };
            let at = centre + right * (side * 0.2 * b.width) + along * (side * 0.28 * b.depth);
            let top = b.top + b.roof.rise() + CHIMNEY_ABOVE_RIDGE;
            let stack = Building { x: at.x, z: at.z, width: CHIMNEY_WIDTH, depth: CHIMNEY_WIDTH, yaw: b.yaw, base: b.top, top, roof: Roof::Flat, walls: Walls::Plain, wall: CHIMNEY_BRICK, roof_color: CHIMNEY_TOP, chimneys: 0, dormers: 0 };
            self.building(&stack);
            self.building(&Building { width: CHIMNEY_WIDTH + CHIMNEY_CAP.0, depth: CHIMNEY_WIDTH + CHIMNEY_CAP.0, base: top - CHIMNEY_CAP.1, wall: CHIMNEY_TOP, ..stack });
        }
    }

    /// Dormer windows on the slopes of a gabled roof, each with a little roof of its own across the
    /// slope. The roof of one reaches four fifths of the way up the main roof's rise, so it always meets
    /// the slope before it could reach the ridge.
    fn dormers(&mut self, b: &Building, centre: Vec3, right: Vec3, along: Vec3) {
        let Roof::Gable(rise) = b.roof else { return };
        let half_w = b.width * 0.5;
        if b.dormers == 0 || half_w < 1.6 || rise < 1.4 {
            return;
        }
        let (out, front_rise, roof_rise) = (0.8 * half_w, 0.4 * rise, 0.2 * rise);
        let depth = 0.6 * half_w + 0.15;
        let roof_at_front = b.top + rise * (1.0 - out / half_w);
        for side in [1.0f32, -1.0] {
            // Away from the end of the roof with a chimney on this side.
            let at = centre + right * (side * (out - depth * 0.5)) + along * (-side * 0.22 * b.depth);
            self.building(&Building {
                x: at.x,
                z: at.z,
                width: DORMER_WIDTH,
                depth,
                // Turned a quarter way round so that its front faces out over the eaves.
                yaw: b.yaw - side * std::f32::consts::FRAC_PI_2,
                base: roof_at_front - 0.15,
                top: roof_at_front + front_rise,
                roof: Roof::Gable(roof_rise),
                walls: Walls::Dormer,
                wall: b.wall,
                roof_color: b.roof_color,
                chimneys: 0,
                dormers: 0,
            });
        }
    }

    /// A gilded finial and cross on the point of the spire of `b`, which ends at `apex`.
    fn finial(&mut self, b: &Building, apex: f32) {
        let pole = Building { width: 0.2, depth: 0.2, base: apex - 0.3, top: apex + 1.4, roof: Roof::Flat, walls: Walls::Plain, wall: GOLD, roof_color: GOLD, chimneys: 0, dormers: 0, ..b.clone() };
        self.building(&pole);
        self.building(&Building { width: 0.75, depth: 0.12, base: apex + 0.75, top: apex + 0.9, ..pole });
    }
}

/// The unit normal of the roof face through `a`, `b` and `c` (turned to point upward), and the unit
/// directions along its edge from `a` to `b` and across it, in its plane.
fn roof_axes(a: Vec3, b: Vec3, c: Vec3) -> (Vec3, Vec3, Vec3) {
    let normal = up((b - a).cross(c - a).normalize());
    let across = (b - a).normalize();
    (normal, across, normal.cross(across))
}

/// Surface coordinates, in meters, of the point `from` the first corner of a roof face.
fn roof_uv(from: Vec3, across: Vec3, down: Vec3) -> Vec2 {
    vec2(from.dot(across), from.dot(down))
}

/// `normal`, turned over if it points down.
fn up(normal: Vec3) -> Vec3 {
    if normal.y < 0.0 { -normal } else { normal }
}

/// How many vertices a mesh is filled to before the next building starts another. A building takes
/// at most about three hundred, so a mesh stays well under [`MAX_VERTICES`].
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
        Building { x: 100.0, z: -40.0, width: 10.0, depth: 12.0, yaw: 0.7, base: 4.2, top: 11.0, roof: Roof::Gable(3.0), walls: Walls::Timber, wall: [200, 190, 170], roof_color: [150, 70, 60], chimneys: 0, dormers: 0 }
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
            let slopes: Vec<_> = g.vertices.iter().filter(|v| v.normal.w == ROOF_STYLE).collect();
            assert!(!slopes.is_empty());
            assert!(slopes.iter().all(|v| v.normal.y > 0.0), "{:?}", slopes[0].normal);
        }
    }

    #[test]
    fn a_roof_reaches_past_its_walls_and_comes_down_there() {
        let b = house();
        let g = one(&[b.clone()]);
        // The first slope's eave corners are the first two of the roof, after the four walls.
        let (near, far) = (g.vertices[16].position, g.vertices[17].position);
        for eave in [near, far] {
            let local = (eave - vec3(b.x, eave.y, b.z)).dot(vec3(b.yaw.cos(), 0.0, b.yaw.sin()));
            assert!(local.abs() > b.width * 0.5 + 0.2, "{local}");
            assert!(eave.y < b.top && eave.y > b.top - 1.0, "{}", eave.y);
        }
    }

    #[test]
    fn a_roof_slope_has_surface_coordinates_in_meters() {
        let g = one(&[house()]);
        let slope = &g.vertices[16..20];
        assert_eq!(slope[0].uv, Vec2::ZERO);
        // The eave is as long as the roof, and the slope is longer than the rise it climbs.
        assert!((slope[1].uv.x.abs() - (12.0 + 2.0 * 0.28)).abs() < 1e-3, "{}", slope[1].uv);
        assert!(slope[3].uv.y.abs() > 3.0);
    }

    #[test]
    fn masonry_and_timber_stand_on_a_footing_and_masonry_has_a_cornice() {
        let with_footing = one(&[Building { walls: Walls::Timber, ..house() }]).vertices.len();
        let plain = one(&[Building { walls: Walls::Plain, ..house() }]).vertices.len();
        assert_eq!(with_footing, plain + 20);
        let masonry = one(&[Building { walls: Walls::Masonry, top: 20.0, ..house() }]).vertices.len();
        let low_masonry = one(&[Building { walls: Walls::Masonry, top: 7.0, ..house() }]).vertices.len();
        assert_eq!(masonry, low_masonry + 20);
        // The footing is as tall as it should be, from the base of the walls.
        let g = one(&[house()]);
        let footing = g.vertices.iter().filter(|v| v.normal.w == 0.0 && v.normal.y.abs() < 1e-4);
        assert!(footing.map(|v| v.position.y).fold(f32::MIN, f32::max) <= 4.2 + PLINTH + 1e-4);
    }

    #[test]
    fn a_gabled_house_with_dormers_has_a_window_in_each_slope() {
        let plain = one(&[Building { roof: Roof::Gable(2.2), ..house() }]);
        let with = one(&[Building { roof: Roof::Gable(2.2), width: 4.4, dormers: 1, ..house() }]);
        let without = one(&[Building { roof: Roof::Gable(2.2), width: 4.4, ..house() }]);
        assert!(plain.vertices.len() < with.vertices.len());
        // Each dormer is a box with a gabled roof: four walls, two slopes and two gables.
        assert_eq!(with.vertices.len(), without.vertices.len() + 2 * 30);
        let dormer_walls = with.vertices.iter().filter(|v| v.normal.w == Walls::Dormer.flag()).count();
        assert_eq!(dormer_walls, 2 * (16 + 6));
        // None of it reaches higher than the ridge of the roof.
        let ridge = 11.0 + 2.2;
        assert!(with.vertices.iter().all(|v| v.position.y <= ridge + 1e-4));
    }

    #[test]
    fn a_spire_ends_in_a_gilded_finial_and_cross() {
        let g = one(&[spire()]);
        let gold = g.vertices.iter().filter(|v| v.color[..3] == GOLD).count();
        assert_eq!(gold, 2 * 20);
        let high = g.vertices.iter().map(|v| v.position.y).fold(f32::MIN, f32::max);
        // The point of the spire is at 42, and the finial rises above it.
        assert!(high > 43.0, "{high}");
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
        // Two chimneys on top of the building itself, each a stack and a cap of 20 vertices.
        let without = one(&[Building { chimneys: 0, ..b }]);
        assert_eq!(g.vertices.len(), without.vertices.len() + 80);
    }

    #[test]
    fn walls_are_marked_with_their_style_and_roofs_and_footings_are_not() {
        let g = one(&[house()]);
        assert!(g.vertices[..16].iter().all(|v| v.normal.w == 2.0));
        // The roof's two slopes are drawn as roof; the gables at its ends are wall, and have the wall's.
        assert!(g.vertices[16..24].iter().all(|v| v.normal.w == ROOF_STYLE));
        assert!(g.vertices[24..30].iter().all(|v| v.normal.w == 2.0));
        // And the footing has nothing drawn on it.
        assert!(g.vertices[30..].iter().all(|v| v.normal.w == 0.0));

        assert!(one(&[spire()]).vertices[..16].iter().all(|v| v.normal.w == 3.0));
        let with_chimneys = one(&[block()]);
        assert!(with_chimneys.vertices[..16].iter().all(|v| v.normal.w == 1.0));
        assert!(with_chimneys.vertices[with_chimneys.vertices.len() - 80..].iter().all(|v| v.normal.w == 0.0));
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
        // Splitting loses nothing: the same shapes, made one building at a time, have as many vertices.
        let separate: usize = buildings
            .iter()
            .map(|b| {
                let mut one = Geometry::default();
                one.building(b);
                one.vertices.len()
            })
            .sum();
        assert_eq!(meshes.iter().map(|m| m.vertices.len()).sum::<usize>(), separate);    }

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
