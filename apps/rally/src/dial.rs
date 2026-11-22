use psx_prim::Texture;
use psx_vram::{bgr555, Clut, TexPage, TextureDepth, Vram, MASK_BIT};

use crate::font;
use crate::textures::Image;

pub const SIZE: usize = 80;
pub const START: f32 = 3.93;
pub const SWEEP: f32 = 4.71;
pub const MAX_RPM: f32 = 8.0;
pub const REDLINE: f32 = 6.5;
const SUPERSAMPLE: usize = 4;
const PAGE: (u8, u8) = (5, 1);
const CENTER: f32 = SIZE as f32 / 2.0;
const RIM_OUTER: f32 = 39.5;
const RIM_INNER: f32 = 36.5;

pub fn angle_for(rpm: f32) -> f32 {
    START - rpm / MAX_RPM * SWEEP
}

#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Outside,
    Face,
    Detail,
}

type Rgb = [f32; 3];

fn digit_ink(character: char, x: f32, y: f32, center: (f32, f32), height: f32) -> bool {
    let Some(rows) = font::rows(character) else {
        return false;
    };
    let scale = height / 7.0;
    let gy = (y - (center.1 - height / 2.0)) / scale;
    let shear = (7.0 - gy) * 0.22;
    let gx = (x - (center.0 - 2.5 * scale)) / scale - shear;
    if !(0.0..5.0).contains(&gx) || !(0.0..7.0).contains(&gy) {
        return false;
    }
    rows[gy as usize] & (0x10 >> gx as usize) != 0
}

fn sample(x: f32, y: f32, lit: bool) -> (Layer, Rgb) {
    let (dx, dy) = (x - CENTER, CENTER - y);
    let radius = (dx * dx + dy * dy).sqrt();
    if radius > RIM_OUTER {
        return (Layer::Outside, [0.0; 3]);
    }
    if radius > RIM_INNER {
        if lit {
            return (Layer::Outside, [0.0; 3]);
        }
        let shine = 0.55 + 0.45 * ((dx - dy) / radius * 0.7).clamp(-1.0, 1.0);
        let edge = 1.0 - ((radius - (RIM_INNER + RIM_OUTER) / 2.0).abs() / 1.5) * 0.35;
        let value = 200.0 * shine * edge;
        return (Layer::Detail, [value, value, value * 1.05]);
    }
    let angle = dy.atan2(dx);
    let mut sweep_angle = START - angle;
    if sweep_angle < -0.5 {
        sweep_angle += std::f32::consts::TAU;
    }
    let rpm = sweep_angle / SWEEP * MAX_RPM;
    if (0.0..=MAX_RPM).contains(&rpm) {
        let ticks = 32.0;
        let position = rpm / MAX_RPM * ticks;
        let within = position.fract();
        let major = (position.round() as i32) % 4 == 0;
        let inner = if major { 25.0 } else { 28.5 };
        let gap = 0.18;
        if radius > inner && radius < 34.5 && within > gap / 2.0 && within < 1.0 - gap / 2.0 {
            let red = rpm >= REDLINE;
            let base = if red {
                [235.0, 35.0, 25.0]
            } else {
                [255.0, 150.0, 25.0]
            };
            let color = if lit { base } else { base.map(|c| c * 0.28) };
            return (Layer::Detail, color);
        }
    }
    if lit {
        return (Layer::Outside, [0.0; 3]);
    }
    for value in 0..=8 {
        let theta = angle_for(value as f32);
        let center = (CENTER + theta.cos() * 17.0, CENTER - theta.sin() * 17.0);
        let character = char::from_digit(value, 10).unwrap();
        if digit_ink(character, x, y, center, 8.5) {
            let color = if value as f32 >= REDLINE {
                [235.0, 50.0, 40.0]
            } else {
                [240.0, 240.0, 240.0]
            };
            return (Layer::Detail, color);
        }
    }
    if radius < 4.0 {
        return (Layer::Detail, [170.0, 170.0, 176.0]);
    }
    (Layer::Face, [14.0, 14.0, 18.0])
}

fn render(lit: bool) -> Vec<u16> {
    let mut pixels = vec![0u16; SIZE * SIZE];
    let samples = SUPERSAMPLE * SUPERSAMPLE;
    for py in 0..SIZE {
        for px in 0..SIZE {
            let (mut outside, mut face) = (0, 0);
            let mut sum = [0.0f32; 3];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = px as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32;
                    let (layer, color) = sample(x, y, lit);
                    match layer {
                        Layer::Outside => outside += 1,
                        Layer::Face => face += 1,
                        Layer::Detail => {}
                    }
                    if layer != Layer::Outside {
                        for c in 0..3 {
                            sum[c] += color[c];
                        }
                    }
                }
            }
            let covered = samples - outside;
            if covered * 2 < samples {
                continue;
            }
            let average = sum.map(|c| (c / covered as f32).round().clamp(0.0, 255.0) as u8);
            let value = bgr555(average[0], average[1], average[2]);
            let value = if value == 0 { 0x0421 } else { value };
            let semi = face * 4 >= covered * 3;
            pixels[py * SIZE + px] = if semi { value | MASK_BIT } else { value };
        }
    }
    pixels
}

pub struct Dial {
    pub face: Image,
    pub lit: Image,
}

pub fn build(vram: &mut Vram) -> Dial {
    let page = TexPage::new(PAGE.0, PAGE.1, TextureDepth::Direct15);
    let (base_x, base_y) = page.base();
    vram.write_rect(base_x, base_y, SIZE, &render(false));
    vram.write_rect(base_x + SIZE, base_y, SIZE, &render(true));
    let image = |u: u8| Image {
        u,
        v: 0,
        width: SIZE as u8,
        height: SIZE as u8,
        texture: Texture {
            page,
            clut: Clut::default(),
            raw: true,
            window: None,
        },
    };
    Dial {
        face: image(0),
        lit: image(SIZE as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_has_transparent_corners_and_semi_face() {
        let face = render(false);
        assert_eq!(face[0], 0);
        let center_ring = face[(SIZE / 2 + 10) * SIZE + SIZE / 2];
        assert_ne!(center_ring & MASK_BIT, 0);
        let lit = render(true);
        assert!(lit.iter().filter(|&&pixel| pixel != 0).count() > 200);
        assert!(lit.iter().all(|&pixel| pixel & MASK_BIT == 0));
    }
}
