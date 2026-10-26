//! Procedural height field and a streaming, level-of-detail chunk grid around the camera.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

use crate::noise::{smoothstep, Perlin};

pub const SEA_LEVEL: f32 = 0.0;
pub const CHUNK_SIZE: f32 = 64.0;
/// Grid cells per chunk side at the finest LOD; each coarser LOD halves it.
const BASE_CELLS: usize = 16;
/// Distance thresholds (world units) for LOD 0, 1 and 2; anything farther is LOD 3.
const LOD_DISTANCES: [f32; 3] = [360.0, 800.0, 1500.0];

pub struct HeightField {
    perlin: Perlin,
    pub seed: u32,
}

impl HeightField {
    pub fn new(seed: u32) -> Self {
        Self {
            perlin: Perlin::new(seed),
            seed,
        }
    }

    /// Terrain elevation before water is applied; negative values are sea floor.
    pub fn raw_height(&self, x: f32, z: f32) -> f32 {
        let p = &self.perlin;

        // Warp the sampling domain so coastlines and ridges meander instead of looking grid-aligned.
        let wx = x + p.fbm(x * 0.0015 + 5.2, z * 0.0015 + 1.3, 3) * 140.0;
        let wz = z + p.fbm(x * 0.0015 - 7.7, z * 0.0015 + 9.1, 3) * 140.0;

        let continent = p.fbm(wx * 0.00045 + 100.0, wz * 0.00045 - 50.0, 4) + 0.18;
        let land = smoothstep(-0.05, 0.45, continent);
        let ridges = p.ridged(wx * 0.0021, wz * 0.0021, 6);
        let hills = p.fbm(wx * 0.006 + 30.0, wz * 0.006 + 30.0, 4);
        let detail = p.fbm(x * 0.04, z * 0.04, 2);

        continent * 140.0 + land * (ridges * ridges * 520.0 + hills * 28.0) + detail * 1.5 - 12.0
    }

    /// Height of the visible surface (land or water) at a point.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.raw_height(x, z).max(SEA_LEVEL)
    }

    fn land_color(&self, h: f32, slope_y: f32, x: f32, z: f32) -> Vec3 {
        let v = self.perlin.noise(x * 0.03 + 400.0, z * 0.03 - 400.0) * 0.5;
        let hv = h + v * 25.0;

        let sand = vec3(0.76, 0.70, 0.50);
        let grass_lo = vec3(0.30, 0.50, 0.19);
        let grass_hi = vec3(0.20, 0.34, 0.15);
        let rock = vec3(0.42, 0.38, 0.35);
        let snow = vec3(0.95, 0.96, 1.0);

        let mut c = grass_lo.lerp(grass_hi, smoothstep(10.0, 140.0, hv));
        c = sand.lerp(c, smoothstep(0.5, 2.5, h + v * 4.0));
        let rockiness = smoothstep(0.80, 0.65, slope_y).max(smoothstep(150.0, 210.0, hv));
        c = c.lerp(rock, rockiness);
        let snowiness = smoothstep(185.0, 235.0, hv) * smoothstep(0.35, 0.6, slope_y);
        c = c.lerp(snow, snowiness);
        c * (1.0 + v * 0.15)
    }
}

fn water_color(depth: f32) -> Vec3 {
    vec3(0.10, 0.42, 0.48).lerp(vec3(0.02, 0.10, 0.26), smoothstep(0.0, 45.0, depth))
}

fn rgba8(c: Vec3) -> [u8; 4] {
    let c = c.clamp(Vec3::ZERO, Vec3::ONE) * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}

pub struct Chunk {
    lod: u8,
    mesh: Mesh,
    center: Vec3,
    radius: f32,
}

fn build_chunk(field: &HeightField, cx: i32, cz: i32, lod: u8) -> Chunk {
    let n = BASE_CELLS >> lod;
    let step = CHUNK_SIZE / n as f32;
    let x0 = cx as f32 * CHUNK_SIZE;
    let z0 = cz as f32 * CHUNK_SIZE;

    // Sample one extra ring around the chunk so normals at the edges match the neighbours.
    let g = n + 3;
    let mut raw = vec![0.0f32; g * g];
    for j in 0..g {
        for i in 0..g {
            let x = x0 + (i as f32 - 1.0) * step;
            let z = z0 + (j as f32 - 1.0) * step;
            raw[j * g + i] = field.raw_height(x, z);
        }
    }
    let surf = |i: usize, j: usize| raw[j * g + i].max(SEA_LEVEL);

    let row = n + 1;
    let mut vertices = Vec::with_capacity(row * row + 4 * row);
    let mut indices: Vec<u16> = Vec::with_capacity(n * n * 6 + 4 * n * 6);
    let (mut y_min, mut y_max) = (f32::MAX, f32::MIN);

    for j in 0..=n {
        for i in 0..=n {
            let (gi, gj) = (i + 1, j + 1);
            let x = x0 + i as f32 * step;
            let z = z0 + j as f32 * step;
            let r = raw[gj * g + gi];
            let y = r.max(SEA_LEVEL);

            // normal.w carries a "water" flag the fragment shader uses for ripples and glints.
            let (normal, color, water) = if r < SEA_LEVEL {
                (Vec3::Y, water_color(SEA_LEVEL - r), 1.0)
            } else {
                let nrm = vec3(
                    surf(gi - 1, gj) - surf(gi + 1, gj),
                    2.0 * step,
                    surf(gi, gj - 1) - surf(gi, gj + 1),
                )
                .normalize();
                (nrm, field.land_color(r, nrm.y, x, z), 0.0)
            };

            y_min = y_min.min(y);
            y_max = y_max.max(y);
            vertices.push(Vertex {
                position: vec3(x, y, z),
                uv: Vec2::ZERO,
                color: rgba8(color),
                normal: normal.extend(water),
            });
        }
    }

    for j in 0..n {
        for i in 0..n {
            let a = (j * row + i) as u16;
            let b = a + 1;
            let c = a + row as u16;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    // Skirts: a curtain hanging down from every edge hides cracks between chunks of different LOD.
    let skirt = step * 1.5 + 6.0;
    let edges: [Vec<usize>; 4] = [
        (0..=n).collect(),
        (0..=n).map(|i| n * row + i).collect(),
        (0..=n).map(|j| j * row).collect(),
        (0..=n).map(|j| j * row + n).collect(),
    ];
    for edge in &edges {
        let base = vertices.len();
        for &k in edge {
            let mut v = vertices[k].clone();
            v.position.y -= skirt;
            vertices.push(v);
        }
        for t in 0..n {
            let a = edge[t] as u16;
            let b = edge[t + 1] as u16;
            let sa = (base + t) as u16;
            let sb = sa + 1;
            indices.extend_from_slice(&[a, sa, b, b, sa, sb]);
        }
    }

    let half = CHUNK_SIZE * 0.5;
    let half_h = (y_max - (y_min - skirt)) * 0.5;
    Chunk {
        lod,
        mesh: Mesh {
            vertices,
            indices,
            texture: None,
        },
        center: vec3(x0 + half, y_min - skirt + half_h, z0 + half),
        radius: (half * half * 2.0 + half_h * half_h).sqrt(),
    }
}

fn lod_for(distance: f32) -> u8 {
    LOD_DISTANCES
        .iter()
        .position(|&d| distance < d)
        .unwrap_or(LOD_DISTANCES.len()) as u8
}

fn chunk_distance(cx: i32, cz: i32, cam: Vec3) -> f32 {
    let center = vec2(
        (cx as f32 + 0.5) * CHUNK_SIZE,
        (cz as f32 + 0.5) * CHUNK_SIZE,
    );
    center.distance(vec2(cam.x, cam.z))
}

pub struct Terrain {
    pub field: HeightField,
    pub view_distance: f32,
    chunks: HashMap<(i32, i32), Chunk>,
}

impl Terrain {
    pub fn new(seed: u32, view_distance: f32) -> Self {
        Self {
            field: HeightField::new(seed),
            view_distance,
            chunks: HashMap::new(),
        }
    }

    pub fn reseed(&mut self, seed: u32) {
        self.field = HeightField::new(seed);
        self.chunks.clear();
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Streams chunks in and out around the camera, spending at most `budget` building meshes.
    /// Missing chunks closest to the camera are built first; LOD swaps come after.
    pub fn update(&mut self, cam: Vec3, budget: Duration) {
        let start = Instant::now();
        let keep = self.view_distance + CHUNK_SIZE * 2.0;
        self.chunks
            .retain(|&(x, z), _| chunk_distance(x, z, cam) <= keep);

        let ccx = (cam.x / CHUNK_SIZE).floor() as i32;
        let ccz = (cam.z / CHUNK_SIZE).floor() as i32;
        let r = (self.view_distance / CHUNK_SIZE).ceil() as i32 + 1;

        let mut work: Vec<(f32, i32, i32, u8)> = Vec::new();
        for z in ccz - r..=ccz + r {
            for x in ccx - r..=ccx + r {
                let d = chunk_distance(x, z, cam);
                if d > self.view_distance + CHUNK_SIZE {
                    continue;
                }
                let lod = lod_for(d);
                match self.chunks.get(&(x, z)) {
                    Some(c) if c.lod == lod => {}
                    Some(_) => work.push((d + 2000.0, x, z, lod)),
                    None => work.push((d, x, z, lod)),
                }
            }
        }
        work.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (_, x, z, lod) in work {
            if start.elapsed() > budget {
                break;
            }
            self.chunks.insert((x, z), build_chunk(&self.field, x, z, lod));
        }
    }

    /// Draws every chunk that intersects the view frustum; returns how many were drawn.
    pub fn draw(&self, frustum: &Frustum) -> usize {
        let mut drawn = 0;
        for chunk in self.chunks.values() {
            if frustum.sphere_visible(chunk.center, chunk.radius) {
                draw_mesh(&chunk.mesh);
                drawn += 1;
            }
        }
        drawn
    }
}

/// View frustum side and near planes, extracted from a view-projection matrix.
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    pub fn from_matrix(m: Mat4) -> Self {
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2]
            .map(|p| p / p.truncate().length());
        Self { planes }
    }

    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p.truncate().dot(center) + p.w >= -radius)
    }
}
