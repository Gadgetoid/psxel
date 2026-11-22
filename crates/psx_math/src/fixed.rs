use std::ops::{Add, Mul, Neg, Sub};
use std::sync::OnceLock;

pub const ONE: i32 = 1 << 12;
pub const FULL_TURN: i32 = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fx12(pub i32);

impl Fx12 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(ONE);

    pub const fn from_int(value: i32) -> Self {
        Self(value << 12)
    }

    pub const fn to_int(self) -> i32 {
        self.0 >> 12
    }

    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl Add for Fx12 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }
}

impl Sub for Fx12 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }
}

impl Neg for Fx12 {
    type Output = Self;
    fn neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }
}

impl Mul for Fx12 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self(((self.0 as i64 * other.0 as i64) >> 12) as i32)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vec3i16 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl Vec3i16 {
    pub const fn new(x: i16, y: i16, z: i16) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [i16; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vec3i32 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Vec3i32 {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mat3Fx12(pub [[i16; 3]; 3]);

impl Mat3Fx12 {
    pub const IDENTITY: Self = Self([[ONE as i16, 0, 0], [0, ONE as i16, 0], [0, 0, ONE as i16]]);

    pub fn rotation_x(angle: i32) -> Self {
        let (sin, cos) = (sin12(angle) as i16, cos12(angle) as i16);
        Self([[ONE as i16, 0, 0], [0, cos, -sin], [0, sin, cos]])
    }

    pub fn rotation_y(angle: i32) -> Self {
        let (sin, cos) = (sin12(angle) as i16, cos12(angle) as i16);
        Self([[cos, 0, sin], [0, ONE as i16, 0], [-sin, 0, cos]])
    }

    pub fn rotation_z(angle: i32) -> Self {
        let (sin, cos) = (sin12(angle) as i16, cos12(angle) as i16);
        Self([[cos, -sin, 0], [sin, cos, 0], [0, 0, ONE as i16]])
    }

    pub fn transpose(self) -> Self {
        let m = self.0;
        Self([
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ])
    }

    pub fn apply(self, v: Vec3i32) -> Vec3i32 {
        let row = |r: [i16; 3]| {
            ((r[0] as i64 * v.x as i64 + r[1] as i64 * v.y as i64 + r[2] as i64 * v.z as i64) >> 12)
                as i32
        };
        Vec3i32::new(row(self.0[0]), row(self.0[1]), row(self.0[2]))
    }
}

impl Mul for Mat3Fx12 {
    type Output = Self;
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul(self, other: Self) -> Self {
        let mut result = [[0i16; 3]; 3];
        for (row, result_row) in result.iter_mut().enumerate() {
            for (column, cell) in result_row.iter_mut().enumerate() {
                let sum: i32 = (0..3)
                    .map(|k| self.0[row][k] as i32 * other.0[k][column] as i32)
                    .sum();
                *cell = (sum >> 12).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            }
        }
        Self(result)
    }
}

fn sine_table() -> &'static [i16; 1025] {
    static TABLE: OnceLock<[i16; 1025]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0i16; 1025];
        for (index, entry) in table.iter_mut().enumerate() {
            let radians = index as f64 * std::f64::consts::TAU / FULL_TURN as f64;
            *entry = (radians.sin() * ONE as f64).round() as i16;
        }
        table
    })
}

pub fn sin12(angle: i32) -> i32 {
    let table = sine_table();
    let angle = angle.rem_euclid(FULL_TURN) as usize;
    match angle / 1024 {
        0 => table[angle] as i32,
        1 => table[2048 - angle] as i32,
        2 => -(table[angle - 2048] as i32),
        _ => -(table[4096 - angle] as i32),
    }
}

pub fn cos12(angle: i32) -> i32 {
    sin12(angle + 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx12_conversions() {
        assert_eq!(Fx12::from_int(3).raw(), 3 * 4096);
        assert_eq!(Fx12(-1).to_int(), -1);
        assert_eq!((Fx12::from_int(3) * Fx12(2048)).raw(), 6144);
    }

    #[test]
    fn sine_quadrants() {
        assert_eq!(sin12(0), 0);
        assert_eq!(sin12(1024), 4096);
        assert_eq!(sin12(2048), 0);
        assert_eq!(sin12(3072), -4096);
        assert_eq!(sin12(512), 2896);
        assert_eq!(sin12(-512), -2896);
        assert_eq!(cos12(0), 4096);
        assert_eq!(cos12(2048), -4096);
        assert_eq!(sin12(4096 + 100), sin12(100));
    }

    #[test]
    fn rotation_by_quarter_turn() {
        let rotated = Mat3Fx12::rotation_y(1024).apply(Vec3i32::new(100, 0, 0));
        assert_eq!(rotated, Vec3i32::new(0, 0, -100));
        let product = Mat3Fx12::rotation_z(700) * Mat3Fx12::rotation_z(700).transpose();
        for (row, identity_row) in product.0.iter().zip(Mat3Fx12::IDENTITY.0) {
            for (value, expected) in row.iter().zip(identity_row) {
                assert!((value - expected).abs() <= 2, "{product:?}");
            }
        }
    }
}
