//! Seeded 2D simplex noise and fractal sums of it, used to shape continents.

/// Skew and unskew factors between the square grid and the simplex (triangle) grid.
const F2: f32 = 0.366_025_42; // (sqrt(3) - 1) / 2
const G2: f32 = 0.211_324_87; // (3 - sqrt(3)) / 6

const GRADIENTS: [(f32, f32); 8] = [
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
];

pub struct Simplex {
    perm: [u8; 512],
}

impl Simplex {
    pub fn new(seed: u32) -> Self {
        let mut table: [u8; 256] = std::array::from_fn(|i| i as u8);

        // Fisher-Yates shuffle driven by a small xorshift generator.
        let mut state = seed.wrapping_mul(0x9E37_79B9) | 1;
        for i in (1..256).rev() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let j = (state % (i as u32 + 1)) as usize;
            table.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = table[i & 255];
        }
        Self { perm }
    }

    /// 2D simplex noise in roughly [-1, 1]. Smoother and less grid-aligned than Perlin noise.
    pub fn noise(&self, x: f32, y: f32) -> f32 {
        // Find the simplex cell containing the point and the point's offset from its corners.
        let s = (x + y) * F2;
        let (i, j) = ((x + s).floor(), (y + s).floor());
        let t = (i + j) * G2;
        let (x0, y0) = (x - (i - t), y - (j - t));
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let corners = [
            (x0, y0, 0, 0),
            (x0 - i1 as f32 + G2, y0 - j1 as f32 + G2, i1, j1),
            (x0 - 1.0 + 2.0 * G2, y0 - 1.0 + 2.0 * G2, 1, 1),
        ];

        let (ii, jj) = ((i as i32 & 255) as usize, (j as i32 & 255) as usize);
        let p = &self.perm;
        let sum: f32 = corners
            .iter()
            .map(|&(dx, dy, ci, cj)| {
                let falloff = 0.5 - dx * dx - dy * dy;
                if falloff <= 0.0 {
                    return 0.0;
                }
                let g = GRADIENTS[(p[ii + ci + p[jj + cj] as usize] & 7) as usize];
                falloff.powi(4) * (g.0 * dx + g.1 * dy)
            })
            .sum();
        70.0 * sum
    }

    /// Fractal Brownian motion, normalized to roughly [-1, 1]. `persistence` is how much each
    /// octave's amplitude is scaled relative to the one before; higher means more fine detail.
    pub fn fbm(&self, x: f32, y: f32, octaves: u32, persistence: f32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            // Offset each octave so their patterns don't line up.
            let off = o as f32 * 17.31;
            sum += self.noise(x * freq + off, y * freq - off) * amp;
            norm += amp;
            amp *= persistence;
            freq *= 2.03;
        }
        sum / norm
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
