//! Seeded 2D Perlin noise, used for natural-looking color variation.

pub struct Perlin {
    perm: [u8; 512],
}

impl Perlin {
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

    /// Classic gradient noise, roughly in [-1, 1].
    pub fn noise(&self, x: f32, y: f32) -> f32 {
        let xf = x.floor();
        let yf = y.floor();
        let xi = (xf as i32 & 255) as usize;
        let yi = (yf as i32 & 255) as usize;
        let x = x - xf;
        let y = y - yf;
        let u = fade(x);
        let v = fade(y);

        let p = &self.perm;
        let aa = p[p[xi] as usize + yi];
        let ab = p[p[xi] as usize + yi + 1];
        let ba = p[p[xi + 1] as usize + yi];
        let bb = p[p[xi + 1] as usize + yi + 1];

        let x1 = lerp(u, grad(aa, x, y), grad(ba, x - 1.0, y));
        let x2 = lerp(u, grad(ab, x, y - 1.0), grad(bb, x - 1.0, y - 1.0));
        lerp(v, x1, x2) * 1.4
    }
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

fn grad(hash: u8, x: f32, y: f32) -> f32 {
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
