use crate::division::{unr_divide, DIVISION_OVERFLOW};
use crate::fixed::{Mat3Fx12, Vec3i16, Vec3i32};

pub mod flag {
    pub const ERROR: u32 = 1 << 31;
    pub const MAC1_POSITIVE: u32 = 1 << 30;
    pub const MAC2_POSITIVE: u32 = 1 << 29;
    pub const MAC3_POSITIVE: u32 = 1 << 28;
    pub const MAC1_NEGATIVE: u32 = 1 << 27;
    pub const MAC2_NEGATIVE: u32 = 1 << 26;
    pub const MAC3_NEGATIVE: u32 = 1 << 25;
    pub const IR1_SATURATED: u32 = 1 << 24;
    pub const IR2_SATURATED: u32 = 1 << 23;
    pub const IR3_SATURATED: u32 = 1 << 22;
    pub const COLOR_R_SATURATED: u32 = 1 << 21;
    pub const COLOR_G_SATURATED: u32 = 1 << 20;
    pub const COLOR_B_SATURATED: u32 = 1 << 19;
    pub const SZ_SATURATED: u32 = 1 << 18;
    pub const DIVIDE_OVERFLOW: u32 = 1 << 17;
    pub const MAC0_POSITIVE: u32 = 1 << 16;
    pub const MAC0_NEGATIVE: u32 = 1 << 15;
    pub const SX_SATURATED: u32 = 1 << 14;
    pub const SY_SATURATED: u32 = 1 << 13;
    pub const IR0_SATURATED: u32 = 1 << 12;

    pub const ERROR_MASK: u32 = 0x7f87_e000;
}

const MAC_POSITIVE: [u32; 3] = [
    flag::MAC1_POSITIVE,
    flag::MAC2_POSITIVE,
    flag::MAC3_POSITIVE,
];
const MAC_NEGATIVE: [u32; 3] = [
    flag::MAC1_NEGATIVE,
    flag::MAC2_NEGATIVE,
    flag::MAC3_NEGATIVE,
];
const IR_SATURATED: [u32; 3] = [
    flag::IR1_SATURATED,
    flag::IR2_SATURATED,
    flag::IR3_SATURATED,
];

pub const SCREEN_MIN: i32 = -0x400;
pub const SCREEN_MAX: i32 = 0x3ff;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenXy {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Projected {
    pub sx: i16,
    pub sy: i16,
    pub sz: u16,
}

#[derive(Clone, Debug, Default)]
pub struct Gte {
    pub rotation: Mat3Fx12,
    pub translation: Vec3i32,
    pub light: Mat3Fx12,
    pub light_color: Mat3Fx12,
    pub background_color: Vec3i32,
    pub far_color: Vec3i32,
    pub ofx: i32,
    pub ofy: i32,
    pub h: u16,
    pub dqa: i16,
    pub dqb: i32,
    pub zsf3: i16,
    pub zsf4: i16,

    pub sxy: [ScreenXy; 3],
    pub sz: [u16; 4],
    pub ir: [i16; 4],
    pub mac: [i32; 4],
    pub otz: u16,
    pub rgbc: [u8; 4],
    pub rgb_fifo: [[u8; 4]; 3],
    pub flag: u32,
}

fn sign_extend_44(value: i64) -> i64 {
    (value << 20) >> 20
}

impl Gte {
    fn begin(&mut self) {
        self.flag = 0;
    }

    fn end(&mut self) {
        if self.flag & flag::ERROR_MASK != 0 {
            self.flag |= flag::ERROR;
        }
    }

    fn accumulate(&mut self, index: usize, value: i64) -> i64 {
        if value > 0x7ff_ffff_ffff {
            self.flag |= MAC_POSITIVE[index];
        } else if value < -0x800_0000_0000 {
            self.flag |= MAC_NEGATIVE[index];
        }
        sign_extend_44(value)
    }

    fn check_mac0(&mut self, value: i64) -> i64 {
        if value > i32::MAX as i64 {
            self.flag |= flag::MAC0_POSITIVE;
        } else if value < i32::MIN as i64 {
            self.flag |= flag::MAC0_NEGATIVE;
        }
        value
    }

    fn saturate_ir(&mut self, index: usize, value: i32, flag_value: i32, lm: bool) -> i16 {
        let min = if lm { 0 } else { -0x8000 };
        if flag_value < min || flag_value > 0x7fff {
            self.flag |= IR_SATURATED[index];
        }
        value.clamp(min, 0x7fff) as i16
    }

    fn rotate_translate(&mut self, v: Vec3i16, sf: bool) -> [i64; 3] {
        let shift = if sf { 12 } else { 0 };
        let vector = v.to_array();
        let translation = self.translation.to_array();
        let mut result = [0i64; 3];
        for (axis, out) in result.iter_mut().enumerate() {
            let row = self.rotation.0[axis];
            let mut sum = self.accumulate(axis, (translation[axis] as i64) << 12);
            for (element, component) in row.iter().zip(vector) {
                sum = self.accumulate(axis, sum + *element as i64 * component as i64);
            }
            self.mac[axis + 1] = (sum >> shift) as i32;
            *out = sum;
        }
        result
    }

    pub fn rot_trans(&mut self, v: Vec3i16, sf: bool, lm: bool) -> [i16; 3] {
        self.begin();
        self.rotate_translate(v, sf);
        for axis in 0..3 {
            let mac = self.mac[axis + 1];
            self.ir[axis + 1] = self.saturate_ir(axis, mac, mac, lm);
        }
        self.end();
        [self.ir[1], self.ir[2], self.ir[3]]
    }

    fn push_sz(&mut self, value: i64) {
        let clamped = if !(0..=0xffff).contains(&value) {
            self.flag |= flag::SZ_SATURATED;
            value.clamp(0, 0xffff)
        } else {
            value
        };
        self.sz = [self.sz[1], self.sz[2], self.sz[3], clamped as u16];
    }

    fn push_sxy(&mut self, x: i64, y: i64) {
        let mut clamp_screen = |value: i64, saturated: u32| {
            if value < SCREEN_MIN as i64 || value > SCREEN_MAX as i64 {
                self.flag |= saturated;
            }
            value.clamp(SCREEN_MIN as i64, SCREEN_MAX as i64) as i16
        };
        let x = clamp_screen(x, flag::SX_SATURATED);
        let y = clamp_screen(y, flag::SY_SATURATED);
        self.sxy = [self.sxy[1], self.sxy[2], ScreenXy { x, y }];
    }

    fn divide(&mut self) -> i64 {
        match unr_divide(self.h, self.sz[3]) {
            Some(quotient) => quotient as i64,
            None => {
                self.flag |= flag::DIVIDE_OVERFLOW;
                DIVISION_OVERFLOW as i64
            }
        }
    }

    fn set_depth_cue(&mut self, quotient: i64) {
        let mac0 = self.check_mac0(quotient * self.dqa as i64 + self.dqb as i64);
        self.mac[0] = mac0 as i32;
        let ir0 = mac0 >> 12;
        if !(0..=0x1000).contains(&ir0) {
            self.flag |= flag::IR0_SATURATED;
        }
        self.ir[0] = ir0.clamp(0, 0x1000) as i16;
    }

    fn rtp(&mut self, v: Vec3i16, sf: bool, lm: bool, last: bool) {
        let sums = self.rotate_translate(v, sf);
        for axis in 0..2 {
            let mac = self.mac[axis + 1];
            self.ir[axis + 1] = self.saturate_ir(axis, mac, mac, lm);
        }
        let mac3 = self.mac[3];
        let ir3_flag_value = if sf { mac3 } else { (sums[2] >> 12) as i32 };
        self.ir[3] = self.saturate_ir(2, mac3, ir3_flag_value, lm);
        self.push_sz(sums[2] >> 12);

        let quotient = self.divide();
        let x = self.check_mac0(quotient * self.ir[1] as i64 + self.ofx as i64);
        let y = self.check_mac0(quotient * self.ir[2] as i64 + self.ofy as i64);
        self.mac[0] = y as i32;
        self.push_sxy(x >> 16, y >> 16);
        if last {
            self.set_depth_cue(quotient);
        }
    }

    pub fn rtps(&mut self, v: Vec3i16) -> Projected {
        self.begin();
        self.rtp(v, true, false, true);
        self.end();
        let screen = self.sxy[2];
        Projected {
            sx: screen.x,
            sy: screen.y,
            sz: self.sz[3],
        }
    }

    pub fn rtpt(&mut self, vertices: [Vec3i16; 3]) -> [Projected; 3] {
        self.begin();
        for (index, vertex) in vertices.into_iter().enumerate() {
            self.rtp(vertex, true, false, index == 2);
        }
        self.end();
        [0, 1, 2].map(|index| Projected {
            sx: self.sxy[index].x,
            sy: self.sxy[index].y,
            sz: self.sz[index + 1],
        })
    }

    pub fn nclip(&mut self) -> i32 {
        self.begin();
        let [a, b, c] = self.sxy.map(|p| (p.x as i64, p.y as i64));
        let area = a.0 * b.1 + b.0 * c.1 + c.0 * a.1 - a.0 * c.1 - b.0 * a.1 - c.0 * b.1;
        self.mac[0] = self.check_mac0(area) as i32;
        self.end();
        self.mac[0]
    }

    fn set_otz(&mut self, mac0: i64) -> u16 {
        let mac0 = self.check_mac0(mac0);
        self.mac[0] = mac0 as i32;
        let otz = mac0 >> 12;
        if !(0..=0xffff).contains(&otz) {
            self.flag |= flag::SZ_SATURATED;
        }
        self.otz = otz.clamp(0, 0xffff) as u16;
        self.otz
    }

    pub fn avsz3(&mut self) -> u16 {
        self.begin();
        let sum = self.sz[1] as i64 + self.sz[2] as i64 + self.sz[3] as i64;
        let otz = self.set_otz(self.zsf3 as i64 * sum);
        self.end();
        otz
    }

    pub fn avsz4(&mut self) -> u16 {
        self.begin();
        let sum: i64 = self.sz.iter().map(|&z| z as i64).sum();
        let otz = self.set_otz(self.zsf4 as i64 * sum);
        self.end();
        otz
    }

    pub fn depth_cue(&mut self, sz: u16) -> i16 {
        self.begin();
        let quotient = match unr_divide(self.h, sz) {
            Some(quotient) => quotient as i64,
            None => {
                self.flag |= flag::DIVIDE_OVERFLOW;
                DIVISION_OVERFLOW as i64
            }
        };
        self.set_depth_cue(quotient);
        self.end();
        self.ir[0]
    }
}

const COLOR_SATURATED: [u32; 3] = [
    flag::COLOR_R_SATURATED,
    flag::COLOR_G_SATURATED,
    flag::COLOR_B_SATURATED,
];

impl Gte {
    fn matrix_vector(
        &mut self,
        matrix: Mat3Fx12,
        translation: [i64; 3],
        vector: [i16; 3],
        lm: bool,
    ) {
        for axis in 0..3 {
            let mut sum = self.accumulate(axis, translation[axis] << 12);
            for (element, component) in matrix.0[axis].iter().zip(vector) {
                sum = self.accumulate(axis, sum + *element as i64 * component as i64);
            }
            let mac = (sum >> 12) as i32;
            self.mac[axis + 1] = mac;
            self.ir[axis + 1] = self.saturate_ir(axis, mac, mac, lm);
        }
    }

    fn light(&mut self, normal: Vec3i16) {
        self.matrix_vector(self.light, [0; 3], normal.to_array(), true);
        let background = self.background_color.to_array().map(|c| c as i64);
        let intensity = [self.ir[1], self.ir[2], self.ir[3]];
        self.matrix_vector(self.light_color, background, intensity, true);
    }

    fn multiply_vertex_color(&mut self) {
        for axis in 0..3 {
            let sum = (self.rgbc[axis] as i64 * self.ir[axis + 1] as i64) << 4;
            self.mac[axis + 1] = self.accumulate(axis, sum) as i32;
        }
    }

    fn interpolate_to_far_color(&mut self, mac: [i64; 3]) {
        let far = self.far_color.to_array();
        for axis in 0..3 {
            let difference = self.accumulate(axis, ((far[axis] as i64) << 12) - mac[axis]) >> 12;
            let ir = self.saturate_ir(axis, difference as i32, difference as i32, false);
            let sum = self.accumulate(axis, ir as i64 * self.ir[0] as i64 + mac[axis]);
            self.mac[axis + 1] = (sum >> 12) as i32;
        }
    }

    fn push_color(&mut self, lm: bool) -> [u8; 3] {
        let mut color = [0u8; 4];
        for axis in 0..3 {
            let mac = self.mac[axis + 1];
            self.ir[axis + 1] = self.saturate_ir(axis, mac, mac, lm);
            let value = mac >> 4;
            if !(0..=0xff).contains(&value) {
                self.flag |= COLOR_SATURATED[axis];
            }
            color[axis] = value.clamp(0, 0xff) as u8;
        }
        color[3] = self.rgbc[3];
        self.rgb_fifo = [self.rgb_fifo[1], self.rgb_fifo[2], color];
        [color[0], color[1], color[2]]
    }

    pub fn ncs(&mut self, normal: Vec3i16) -> [u8; 3] {
        self.begin();
        self.light(normal);
        let color = self.push_color(true);
        self.end();
        color
    }

    pub fn nccs(&mut self, normal: Vec3i16) -> [u8; 3] {
        self.begin();
        self.light(normal);
        self.multiply_vertex_color();
        for axis in 0..3 {
            self.mac[axis + 1] >>= 12;
        }
        let color = self.push_color(true);
        self.end();
        color
    }

    pub fn ncds(&mut self, normal: Vec3i16) -> [u8; 3] {
        self.begin();
        self.light(normal);
        let mac = [0, 1, 2].map(|axis| (self.rgbc[axis] as i64 * self.ir[axis + 1] as i64) << 4);
        self.interpolate_to_far_color(mac);
        let color = self.push_color(true);
        self.end();
        color
    }

    pub fn dpcs(&mut self, color: [u8; 3]) -> [u8; 3] {
        self.begin();
        let mac = color.map(|c| (c as i64) << 16);
        self.interpolate_to_far_color(mac);
        let color = self.push_color(false);
        self.end();
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn camera() -> Gte {
        Gte {
            rotation: Mat3Fx12::IDENTITY,
            ofx: 160 << 16,
            ofy: 120 << 16,
            h: 256,
            zsf3: 4096 / 3,
            zsf4: 4096 / 4,
            ..Default::default()
        }
    }

    fn lit() -> Gte {
        let mut gte = camera();
        gte.light = Mat3Fx12([[0, 0, -4096], [0; 3], [0; 3]]);
        gte.light_color = Mat3Fx12([[4096, 0, 0], [4096, 0, 0], [2048, 0, 0]]);
        gte.background_color = Vec3i32::new(256, 256, 256);
        gte.rgbc = [128, 128, 128, 0x30];
        gte
    }

    #[test]
    fn ncs_facing_and_backfacing() {
        let mut gte = lit();
        assert_eq!(gte.ncs(Vec3i16::new(0, 0, -4096)), [255, 255, 144]);
        assert_ne!(gte.flag & flag::COLOR_R_SATURATED, 0);
        assert_eq!(gte.ncs(Vec3i16::new(0, 0, 4096)), [16, 16, 16]);
        assert_eq!(gte.flag & flag::IR1_SATURATED, flag::IR1_SATURATED);
    }

    #[test]
    fn nccs_modulates_vertex_color() {
        let mut gte = lit();
        gte.background_color = Vec3i32::default();
        assert_eq!(gte.nccs(Vec3i16::new(0, 0, -4096)), [128, 128, 64]);
        assert_eq!(gte.nccs(Vec3i16::new(0, 2896, -2896)), [90, 90, 45]);
        assert_eq!(gte.rgb_fifo[2], [90, 90, 45, 0x30]);
    }

    #[test]
    fn ncds_and_dpcs_fade_to_far_color() {
        let mut gte = lit();
        gte.background_color = Vec3i32::default();
        gte.far_color = Vec3i32::new(0, 64 << 4, 255 << 4);
        gte.ir[0] = 0;
        assert_eq!(gte.ncds(Vec3i16::new(0, 0, -4096)), [128, 128, 64]);
        gte.ir[0] = 0x1000;
        assert_eq!(gte.ncds(Vec3i16::new(0, 0, -4096)), [0, 64, 255]);
        gte.ir[0] = 0x800;
        assert_eq!(gte.dpcs([200, 100, 50]), [100, 82, 152]);
    }

    #[test]
    fn rtps_known_projection() {
        let mut gte = camera();
        let projected = gte.rtps(Vec3i16::new(100, 50, 1000));
        assert_eq!(
            projected,
            Projected {
                sx: 185,
                sy: 132,
                sz: 1000
            }
        );
        assert_eq!(gte.ir[1..], [100, 50, 1000]);
        assert_eq!(gte.flag, 0);
    }

    #[test]
    fn rtps_applies_translation() {
        let mut gte = camera();
        gte.translation = Vec3i32::new(-100, 0, 500);
        let projected = gte.rtps(Vec3i16::new(100, 0, 500));
        assert_eq!(
            projected,
            Projected {
                sx: 160,
                sy: 120,
                sz: 1000
            }
        );
    }

    #[test]
    fn screen_coordinates_saturate() {
        let mut gte = camera();
        let projected = gte.rtps(Vec3i16::new(30000, -30000, 10));
        assert_eq!((projected.sx, projected.sy), (0x3ff, -0x400));
        assert_ne!(gte.flag & flag::SX_SATURATED, 0);
        assert_ne!(gte.flag & flag::SY_SATURATED, 0);
        assert_ne!(gte.flag & flag::ERROR, 0);
    }

    #[test]
    fn behind_camera_saturates_sz_and_overflows_division() {
        let mut gte = camera();
        let projected = gte.rtps(Vec3i16::new(0, 0, -50));
        assert_eq!(projected.sz, 0);
        assert_ne!(gte.flag & flag::SZ_SATURATED, 0);
        assert_ne!(gte.flag & flag::DIVIDE_OVERFLOW, 0);
        assert_ne!(gte.flag & flag::ERROR, 0);
    }

    #[test]
    fn near_plane_division_overflow() {
        let mut gte = camera();
        gte.rtps(Vec3i16::new(0, 0, 128));
        assert_ne!(gte.flag & flag::DIVIDE_OVERFLOW, 0);
        gte.rtps(Vec3i16::new(0, 0, 129));
        assert_eq!(gte.flag & flag::DIVIDE_OVERFLOW, 0);
    }

    #[test]
    fn ir_saturation_respects_lm() {
        let mut gte = camera();
        gte.translation = Vec3i32::new(-40000, 40000, 0);
        assert_eq!(
            gte.rot_trans(Vec3i16::default(), true, false),
            [-0x8000, 0x7fff, 0]
        );
        assert_eq!(
            gte.flag & (flag::IR1_SATURATED | flag::IR2_SATURATED),
            flag::IR1_SATURATED | flag::IR2_SATURATED
        );
        assert_eq!(
            gte.rot_trans(Vec3i16::default(), true, true),
            [0, 0x7fff, 0]
        );
    }

    #[test]
    fn rtp_sf0_ir3_flag_uses_shifted_mac3() {
        let mut gte = camera();
        gte.begin();
        gte.rtp(Vec3i16::new(0, 0, 9), false, false, true);
        assert_eq!(gte.mac[3], 9 << 12);
        assert_eq!(gte.ir[3], 0x7fff);
        assert_eq!(gte.flag & flag::IR3_SATURATED, 0);
    }

    #[test]
    fn mac_overflow_flags() {
        let mut gte = camera();
        gte.rotation = Mat3Fx12([[0x7fff, 0x7fff, 0x7fff], [0; 3], [0; 3]]);
        gte.translation = Vec3i32::new(i32::MAX, i32::MIN, 0);
        gte.rot_trans(Vec3i16::new(0x7fff, 0x7fff, 0x7fff), true, false);
        assert_ne!(gte.flag & flag::MAC1_POSITIVE, 0);
        assert_eq!(gte.flag & flag::MAC2_NEGATIVE, 0);
        gte.rotation = Mat3Fx12([[0; 3], [-0x8000, -0x8000, 0], [0; 3]]);
        gte.rot_trans(Vec3i16::new(0x7fff, 0x7fff, 0), true, false);
        assert_ne!(gte.flag & flag::MAC2_NEGATIVE, 0);
    }

    #[test]
    fn rtpt_fills_fifos() {
        let mut gte = camera();
        let projected = gte.rtpt([
            Vec3i16::new(0, 0, 1000),
            Vec3i16::new(100, 0, 2000),
            Vec3i16::new(0, 100, 4000),
        ]);
        assert_eq!(projected.map(|p| p.sz), [1000, 2000, 4000]);
        assert_eq!(projected[0].sx, 160);
        assert_eq!(gte.sz[1..], [1000, 2000, 4000]);
    }

    #[test]
    fn nclip_winding() {
        let mut gte = camera();
        gte.sxy = [
            ScreenXy { x: 0, y: 0 },
            ScreenXy { x: 10, y: 0 },
            ScreenXy { x: 0, y: 10 },
        ];
        assert_eq!(gte.nclip(), 100);
        gte.sxy.swap(1, 2);
        assert_eq!(gte.nclip(), -100);
    }

    #[test]
    fn average_z() {
        let mut gte = camera();
        gte.sz = [0, 300, 600, 900];
        assert_eq!(gte.avsz3(), 599);
        gte.sz = [100, 300, 600, 900];
        assert_eq!(gte.avsz4(), 475);
        gte.zsf3 = -4096;
        assert_eq!(gte.avsz3(), 0);
        assert_ne!(gte.flag & flag::SZ_SATURATED, 0);
    }

    #[test]
    fn depth_cue_interpolates_and_saturates() {
        let mut gte = camera();
        gte.dqa = -100;
        gte.dqb = 0x1400000;
        assert_eq!(gte.depth_cue(65535), 0x1000);
        assert_eq!(gte.flag & flag::IR0_SATURATED, flag::IR0_SATURATED);
        gte.dqb = 0x0800000;
        let near = gte.depth_cue(200);
        let far = gte.depth_cue(20000);
        assert!(near < far, "{near} {far}");
        assert_eq!(gte.depth_cue(1), 0);
        assert_ne!(gte.flag & flag::IR0_SATURATED, 0);
    }

    proptest! {
        #[test]
        fn projection_always_in_range(
            rotation in prop::array::uniform3(prop::array::uniform3(any::<i16>())),
            translation in prop::array::uniform3(-0x10_0000i32..0x10_0000),
            vertex in prop::array::uniform3(any::<i16>()),
            h in any::<u16>(),
            ofx in any::<i32>(),
            ofy in any::<i32>(),
        ) {
            let mut gte = Gte {
                rotation: Mat3Fx12(rotation),
                translation: Vec3i32::new(translation[0], translation[1], translation[2]),
                h,
                ofx,
                ofy,
                ..Default::default()
            };
            let projected = gte.rtps(Vec3i16::new(vertex[0], vertex[1], vertex[2]));
            prop_assert!((SCREEN_MIN..=SCREEN_MAX).contains(&(projected.sx as i32)));
            prop_assert!((SCREEN_MIN..=SCREEN_MAX).contains(&(projected.sy as i32)));
            prop_assert!((0..=0x1000).contains(&gte.ir[0]));
            prop_assert_eq!(gte.flag & flag::ERROR != 0, gte.flag & flag::ERROR_MASK != 0);
        }
    }
}
