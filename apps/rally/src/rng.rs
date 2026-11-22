#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = ((self.0 >> 18) ^ self.0) >> 27;
        (x as u32).rotate_right((self.0 >> 59) as u32)
    }

    pub fn float(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.float()
    }

    pub fn below(&mut self, limit: u32) -> u32 {
        ((self.next_u32() as u64 * limit as u64) >> 32) as u32
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.float() < probability
    }

    pub fn pick_weighted(&mut self, weights: &[f32]) -> usize {
        let total: f32 = weights.iter().sum();
        let mut target = self.float() * total;
        for (index, weight) in weights.iter().enumerate() {
            if target < *weight {
                return index;
            }
            target -= weight;
        }
        weights.len() - 1
    }
}

pub fn hash(seed: u64, a: u64, b: u64) -> u64 {
    let mut x =
        seed ^ a.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ b.wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

pub fn hash_float(seed: u64, a: u64, b: u64) -> f32 {
    (hash(seed, a, b) >> 40) as f32 / (1u64 << 24) as f32
}

pub fn value_noise(seed: u64, x: f32) -> f32 {
    let cell = x.floor();
    let t = x - cell;
    let smooth = t * t * (3.0 - 2.0 * t);
    let a = hash_float(seed, cell as i64 as u64, 0);
    let b = hash_float(seed, (cell as i64 + 1) as u64, 0);
    a + (b - a) * smooth
}

pub fn value_noise_2d(seed: u64, x: f32, y: f32) -> f32 {
    let (cx, cy) = (x.floor(), y.floor());
    let (tx, ty) = (x - cx, y - cy);
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (smooth(tx), smooth(ty));
    let corner =
        |dx: i64, dy: i64| hash_float(seed, (cx as i64 + dx) as u64, (cy as i64 + dy) as u64);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}
