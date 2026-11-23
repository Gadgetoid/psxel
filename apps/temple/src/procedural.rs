use std::f32::consts::{PI, TAU};

use psxel_prim::Texture;
use psxel_vram::{bgr555, Clut, TexPage, TextureDepth, Vram};

use crate::biome::Rgb;
use crate::rng::{hash_float, Rng};
use crate::textures::Image;

const PAGE_X: u8 = 5;
const CLUT_X: u16 = 320;
const CLUT_Y: u16 = 300;
const SEMI: u16 = 0x8000;
const WATER_DARK: Rgb = [22, 66, 70];
const WATER_LIGHT: Rgb = [150, 210, 195];

#[derive(Clone, Copy)]
enum Slot {
    Fern,
    Flame,
    Glow,
    Mist,
    Mote,
    Water,
    Waterfall,
    Crystal,
    Cold,
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    fn set(&mut self, x: i32, y: i32, value: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = value;
        }
    }

    fn blit(&self, vram: &mut Vram, u: u8, v: u8) {
        vram.write_texture(page(), u as usize, v as usize, self.width, &self.pixels);
    }
}

fn page() -> TexPage {
    TexPage::new(PAGE_X, 0, TextureDepth::Clut4)
}

fn clut(slot: Slot) -> Clut {
    Clut::new(CLUT_X + slot as u16 * 16, CLUT_Y)
}

fn image(slot: Slot, u: u8, v: u8, width: u8, height: u8) -> Image {
    Image {
        u,
        v,
        width,
        height,
        texture: Texture {
            page: page(),
            clut: clut(slot),
            raw: false,
            window: None,
        },
    }
}

fn lerp_rgb(from: Rgb, to: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|c| (from[c] as f32 + (to[c] as f32 - from[c] as f32) * t).round() as u8)
}

fn color(rgb: Rgb, flags: u16) -> u16 {
    let value = bgr555(rgb[0], rgb[1], rgb[2]) | flags;
    if value == 0 {
        0x8000
    } else {
        value
    }
}

fn palette(stops: &[(usize, Rgb, u16)]) -> [u16; 16] {
    let mut colors = [0u16; 16];
    for pair in stops.windows(2) {
        let ((first, from, flags), (last, to, _)) = (pair[0], pair[1]);
        for (index, entry) in colors.iter_mut().enumerate().take(last + 1).skip(first) {
            let t = (index - first) as f32 / (last - first).max(1) as f32;
            *entry = color(lerp_rgb(from, to, t), flags);
        }
    }
    colors
}

fn fern(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(64, 48);
    let mut rng = Rng::new(seed);
    let fronds = 9;
    for frond in 0..fronds {
        let t = frond as f32 / (fronds - 1) as f32;
        let angle = -PI + 0.25 + t * (PI - 0.5) + rng.range(-0.12, 0.12);
        let (mut x, mut y) = (32.0 + rng.range(-2.0, 2.0), 47.0);
        let (mut dx, mut dy) = (angle.cos(), angle.sin());
        let length = rng.range(20.0, 30.0) * (0.75 + 0.25 * (1.0 - (t - 0.5).abs() * 2.0));
        let shade = rng.range(0.35, 0.85);
        let steps = length as i32;
        for step in 0..steps {
            let along = step as f32 / steps as f32;
            canvas.set(x as i32, y as i32, 2);
            if step % 2 == 0 && step > 1 {
                let leaflet = (1.0 - along) * 5.0 + 1.0;
                for sign in [-1.0f32, 1.0] {
                    let (px, py) = (-dy * sign, dx * sign);
                    for k in 1..=leaflet as i32 {
                        let f = k as f32;
                        let light = shade + along * 0.2 - f / leaflet * 0.15;
                        canvas.set(
                            (x + px * f + dx * f * 0.4) as i32,
                            (y + py * f + dy * f * 0.4 + f * 0.15) as i32,
                            3 + (light.clamp(0.0, 0.99) * 13.0) as u8,
                        );
                    }
                }
            }
            x += dx;
            y += dy;
            dy += 0.045;
            let norm = dx.hypot(dy);
            dx /= norm;
            dy /= norm;
        }
    }
    canvas
}

fn flame(frame: usize) -> Canvas {
    let mut canvas = Canvas::new(16, 32);
    for y in 0..32 {
        for x in 0..16 {
            let t = (31 - y) as f32 / 31.0;
            let width = 6.5 * (1.0 - t).powf(0.65) * (0.45 + 0.55 * (t * 4.0).min(1.0));
            let sway = (t * 5.0 + frame as f32 * 1.6).sin() * t * 2.2;
            let distance = ((x as f32 + 0.5 - 8.0 - sway) / width.max(0.01)).abs();
            let noise = hash_float(frame as u64 + 40, x as u64, (y / 2) as u64);
            let heat = (1.0 - distance) * (1.1 - t * 0.7) + (noise - 0.5) * 0.35;
            if heat > 0.08 {
                canvas.set(x, y, 1 + (heat.min(0.999) * 14.99) as u8);
            }
        }
    }
    canvas
}

fn radial(size: usize, power: f32, grain: f32, seed: u64) -> Canvas {
    let mut canvas = Canvas::new(size, size);
    let half = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let radius = (x as f32 + 0.5 - half).hypot(y as f32 + 0.5 - half) / half;
            let noise = hash_float(seed, (x / 2) as u64, (y / 2) as u64);
            let intensity = (1.0 - radius).max(0.0).powf(power) * (1.0 - grain + grain * noise);
            if intensity > 0.03 {
                canvas.set(x as i32, y as i32, 1 + (intensity.min(0.999) * 14.99) as u8);
            }
        }
    }
    canvas
}

fn crystal(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(32, 32);
    let mut rng = Rng::new(seed);
    for _ in 0..6 {
        let base = 16.0 + rng.range(-8.0, 8.0);
        let lean = rng.range(-0.35, 0.35);
        let height = rng.range(14.0, 30.0);
        let width = rng.range(2.0, 4.0);
        for step in 0..height as i32 {
            let t = step as f32 / height;
            let centre = base + lean * step as f32;
            let half = width * (1.0 - t);
            let y = 31 - step;
            for x in (centre - half).floor() as i32..=(centre + half).ceil() as i32 {
                let side = (x as f32 + 0.5 - centre) / half.max(0.5);
                let light = 0.55 - side * 0.35 + t * 0.35;
                canvas.set(x, y, 1 + (light.clamp(0.0, 0.999) * 14.99) as u8);
            }
        }
    }
    canvas
}

fn water() -> Canvas {
    let mut canvas = Canvas::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let (fx, fy) = (x as f32 / 64.0 * TAU, y as f32 / 64.0 * TAU);
            let a = (fx * 2.0 + fy.sin() * 1.3).sin();
            let b = (fy * 3.0 + (fx * 2.0).cos() * 1.1).sin();
            let c = (fx + fy * 2.0 + (fy * 2.0 - fx).sin()).sin();
            let ridge = (1.0 - ((a + b + c) / 3.0).abs()).powf(3.0);
            canvas.set(x, y, 1 + (ridge.min(0.999) * 14.99) as u8);
        }
    }
    canvas
}

fn waterfall(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(64, 64);
    let mut rng = Rng::new(seed);
    let phases: Vec<f32> = (0..64).map(|_| rng.range(0.0, TAU)).collect();
    let waves: Vec<(f32, f32)> = [3.0, 5.0, 9.0, 14.0]
        .iter()
        .map(|&frequency| (frequency, rng.range(0.0, TAU)))
        .collect();
    for x in 0..64 {
        let column = waves
            .iter()
            .map(|&(frequency, phase)| (TAU * frequency * x as f32 / 64.0 + phase).sin())
            .sum::<f32>()
            / waves.len() as f32;
        for y in 0..64 {
            let along = y as f32 / 64.0 * TAU;
            let value = 0.5
                + 0.45 * column
                + 0.22 * (along * 2.0 + phases[x]).sin()
                + 0.12 * (along * 5.0 + phases[(x * 7) % 64]).sin()
                + (hash_float(seed, x as u64, y as u64) - 0.5) * 0.15;
            canvas.set(x as i32, y, 1 + (value.clamp(0.0, 0.999) * 14.99) as u8);
        }
    }
    canvas
}

pub struct Procedural {
    pub fern: Image,
    pub flames: [Image; 4],
    pub glow: Image,
    pub cold_glow: Image,
    pub crystal: Image,
    pub mist: Image,
    pub mote: Image,
    pub water: Image,
    pub waterfall: Image,
}

impl Procedural {
    pub fn build(vram: &mut Vram) -> Self {
        fern(3).blit(vram, 0, 0);
        for frame in 0..4 {
            flame(frame).blit(vram, 192 + frame as u8 * 16, 0);
        }
        radial(32, 2.0, 0.0, 0).blit(vram, 192, 32);
        radial(32, 1.3, 0.35, 50).blit(vram, 224, 32);
        radial(16, 2.5, 0.0, 0).blit(vram, 192, 64);
        water().blit(vram, 0, 128);
        waterfall(9).blit(vram, 64, 128);
        crystal(21).blit(vram, 224, 64);

        let green = |dark: Rgb, light: Rgb| {
            palette(&[
                (1, [18, 30, 12], 0),
                (2, [60, 48, 26], 0),
                (3, dark, 0),
                (15, light, 0),
            ])
        };
        vram.write_clut(clut(Slot::Fern), &green([24, 56, 20], [140, 195, 82]));
        vram.write_clut(
            clut(Slot::Flame),
            &palette(&[
                (1, [60, 8, 0], SEMI),
                (5, [190, 55, 6], SEMI),
                (10, [250, 150, 30], SEMI),
                (15, [255, 245, 190], SEMI),
            ]),
        );
        vram.write_clut(
            clut(Slot::Glow),
            &palette(&[(1, [10, 5, 2], SEMI), (15, [255, 190, 120], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Mist),
            &palette(&[(1, [8, 10, 11], SEMI), (15, [210, 222, 230], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Crystal),
            &palette(&[
                (1, [10, 40, 60], 0),
                (8, [60, 170, 200], 0),
                (15, [225, 255, 255], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::Cold),
            &palette(&[(1, [2, 6, 8], SEMI), (15, [120, 230, 255], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Mote),
            &palette(&[(1, [20, 18, 10], SEMI), (15, [255, 240, 190], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Waterfall),
            &palette(&[
                (1, [24, 52, 60], SEMI),
                (9, [150, 185, 195], SEMI),
                (10, [185, 210, 215], 0),
                (15, [250, 255, 255], 0),
            ]),
        );
        let procedural = Self {
            fern: image(Slot::Fern, 0, 0, 64, 48),
            flames: [0, 1, 2, 3].map(|frame| image(Slot::Flame, 192 + frame * 16, 0, 16, 32)),
            glow: image(Slot::Glow, 192, 32, 32, 32),
            cold_glow: image(Slot::Cold, 192, 32, 32, 32),
            crystal: image(Slot::Crystal, 224, 64, 32, 32),
            mist: image(Slot::Mist, 224, 32, 32, 32),
            mote: image(Slot::Mote, 192, 64, 16, 16),
            water: image(Slot::Water, 0, 128, 64, 64),
            waterfall: image(Slot::Waterfall, 64, 128, 64, 64),
        };
        procedural.animate(vram, 0.0);
        procedural
    }

    pub fn animate(&self, vram: &mut Vram, time: f32) {
        let phase = (time * 0.6).fract();
        let mut colors = [0u16; 16];
        for (index, entry) in colors.iter_mut().enumerate().skip(1) {
            let level = (index - 1) as f32 / 14.0;
            let cycle = 0.5 - 0.5 * (TAU * (level + phase)).cos();
            let brightness = level * 0.6 + cycle * 0.4;
            *entry = color(lerp_rgb(WATER_DARK, WATER_LIGHT, brightness), SEMI);
        }
        vram.write_clut(clut(Slot::Water), &colors);
    }
}
