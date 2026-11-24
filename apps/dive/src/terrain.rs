use crate::rng::hash_float;

pub const SURFACE: f32 = 0.0;
pub const BOUNDS: (f32, f32, f32, f32) = (-50.0, 62.0, -56.0, 190.0);

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

pub fn reef(x: f32, z: f32) -> f32 {
    smoothstep(-20.0, -34.0, x + (value_noise(z / 9.0, 0.0, 5) - 0.5) * 6.0)
}

pub fn seabed(x: f32, z: f32) -> f32 {
    let shelf = lerp(-12.0, -42.0, smoothstep(-30.0, 52.0, z));
    let trench = -30.0 * smoothstep(114.0, 150.0, z) * (1.0 - smoothstep(175.5, 190.0, z));
    let dunes = (value_noise(x / 9.0, z / 9.0, 11) - 0.5) * 1.8
        + (value_noise(x / 3.0, z / 3.0, 12) - 0.5) * 0.4;
    let reef_rise = reef(x, z) * (26.0 + (value_noise(x / 4.0, z / 4.0, 13) - 0.5) * 9.0);
    let east = smoothstep(38.0, 56.0, x) * (24.0 + (value_noise(x / 5.0, z / 5.0, 14) - 0.5) * 6.0);
    let south = smoothstep(-38.0, -52.0, z) * 6.0;
    let north = smoothstep(176.0, 188.0, z) * 40.0;
    let rocky = (reef(x, z) + smoothstep(38.0, 56.0, x)).min(1.0);
    (shelf + trench + dunes * (1.0 - rocky) + reef_rise + east + south + north).min(-2.5)
}

pub fn normal(x: f32, z: f32) -> [f32; 3] {
    let step = 0.6;
    let dx = seabed(x + step, z) - seabed(x - step, z);
    let dz = seabed(x, z + step) - seabed(x, z - step);
    let length = (dx * dx + dz * dz + 4.0 * step * step).sqrt();
    [-dx / length, 2.0 * step / length, -dz / length]
}
