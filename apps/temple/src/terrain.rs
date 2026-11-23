use crate::rng::hash_float;

pub const TEMPLE_Z: f32 = 40.0;
pub const POOL_CENTER: (f32, f32) = (-31.0, 14.0);
pub const POOL_RADII: (f32, f32) = (9.5, 10.0);
pub const WATER_Y: f32 = -0.6;
pub const CLIFF_X: f32 = -40.0;
pub const CLIFF_TOP: f32 = 16.0;
pub const FALLS_Z: f32 = 14.0;
pub const FALLS_HALF_WIDTH: f32 = 2.5;
pub const RIVER_Y: f32 = 16.15;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn value_noise(x: f32, z: f32, seed: u64) -> f32 {
    let (ix, iz) = (x.floor(), z.floor());
    let (fx, fz) = (x - ix, z - iz);
    let corner =
        |dx: f32, dz: f32| hash_float(seed, (ix + dx) as i64 as u64, (iz + dz) as i64 as u64);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    lerp(
        lerp(corner(0.0, 0.0), corner(1.0, 0.0), sx),
        lerp(corner(0.0, 1.0), corner(1.0, 1.0), sx),
        sz,
    )
}

pub fn pool_distance(x: f32, z: f32) -> f32 {
    ((x - POOL_CENTER.0) / POOL_RADII.0).hypot((z - POOL_CENTER.1) / POOL_RADII.1)
}

pub fn ground_height(x: f32, z: f32) -> f32 {
    let mut height = (value_noise(x / 9.0, z / 9.0, 11) - 0.5) * 1.8
        + (value_noise(x / 3.5, z / 3.5, 12) - 0.5) * 0.5;
    let temple = x.abs().max((z - TEMPLE_Z).abs());
    height *= smoothstep(15.0, 24.0, temple);
    height += smoothstep(45.0, 80.0, x.hypot(z - 20.0)) * 4.0;
    let bowl = 1.0 - smoothstep(0.55, 1.15, pool_distance(x, z));
    lerp(height, -2.6, bowl)
}

pub fn plateau_height(x: f32, z: f32) -> f32 {
    let inland = smoothstep(CLIFF_X, CLIFF_X - 8.0, x);
    let river = 1.0 - smoothstep(2.0, 4.0, (z - FALLS_Z).abs());
    let height = CLIFF_TOP + (value_noise(x / 7.0, z / 7.0, 13) - 0.3) * 2.5 * inland;
    lerp(height, CLIFF_TOP - 0.4, river)
}

pub fn cliff_x(z: f32, y: f32) -> f32 {
    if y >= CLIFF_TOP - 0.01 {
        return CLIFF_X;
    }
    let fade = 1.0 - ((y + 3.0) / (CLIFF_TOP + 3.0)).powi(4);
    let bulge = value_noise(z / 5.0, y / 4.0, 14) * 2.2 + value_noise(z / 1.7, y / 1.7, 15) * 0.6;
    let recess = 1.0
        - smoothstep(
            FALLS_HALF_WIDTH,
            FALLS_HALF_WIDTH + 2.5,
            (z - FALLS_Z).abs(),
        );
    CLIFF_X + bulge * fade * (1.0 - recess) - 0.6 * recess * fade
}
