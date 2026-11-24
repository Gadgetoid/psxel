use std::f32::consts::TAU;

use psxel_prim::Texture;
use psxel_vram::{bgr555, Clut, TexPage, TextureDepth, Vram};

use crate::biome::Rgb;
use crate::rng::{hash_float, Rng};
use crate::textures::Image;

const PAGE_X: u8 = 5;
const CLUT_X: u16 = 320;
const CLUT_Y: u16 = 300;
const SEMI: u16 = 0x8000;

#[derive(Clone, Copy)]
enum Slot {
    Kelp,
    FishYellow,
    FishBlue,
    FishSilver,
    Fan,
    Sponge,
    Caustic,
    Glow,
    Mote,
    Bubble,
    Water,
    Anemone,
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

fn kelp(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(16, 64);
    let mut rng = Rng::new(seed);
    for y in 0..64 {
        let sway = (y as f32 / 64.0 * TAU * 1.5).sin() * 1.6;
        let x = 8.0 + sway;
        canvas.set(x as i32, y, 3);
        canvas.set(x as i32 + 1, y, 4);
        if y % 5 == 0 && y > 2 {
            let side = if (y / 5) % 2 == 0 { 1.0 } else { -1.0 };
            let length = rng.range(3.0, 6.5);
            for step in 0..(length * 2.0) as i32 {
                let t = step as f32 / 2.0;
                let leaf_x = x + side * t;
                let leaf_y = y as f32 + t * 0.6;
                let width = (1.0 - t / length) * 2.0 + 0.5;
                for w in 0..=(width as i32) {
                    let shade = 5 + ((1.0 - t / length) * 8.0) as u8 + (w as u8 % 2);
                    canvas.set(leaf_x as i32, leaf_y as i32 + w, shade.min(15));
                }
            }
        }
    }
    canvas
}

fn fish(kind: u64) -> Canvas {
    let mut canvas = Canvas::new(16, 8);
    for y in 0..8 {
        for x in 0..16 {
            let (dx, dy) = (
                (x as f32 - 7.0) / 6.0,
                (y as f32 - 3.5) / (3.2 - (x as f32 - 7.0).abs() * 0.12),
            );
            if dx * dx + dy * dy <= 1.0 && x >= 2 {
                let stripe = kind == 2 && (x / 2) % 2 == 0;
                let belly = y >= 5;
                canvas.set(
                    x,
                    y,
                    if stripe {
                        6
                    } else if belly {
                        12
                    } else {
                        9
                    },
                );
            }
        }
    }
    for y in 1..7 {
        let spread = (y - 3i32).abs();
        if spread <= 2 {
            canvas.set(0, y, 7);
            canvas.set(1, y, 8);
        }
    }
    canvas.set(12, 3, 1);
    canvas
}

fn fan(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(32, 32);
    let mut rng = Rng::new(seed);
    let mut branches = vec![(16.0f32, 31.0f32, -std::f32::consts::FRAC_PI_2, 9.0f32, 0u32)];
    while let Some((x, y, angle, length, depth)) = branches.pop() {
        let (mut px, mut py) = (x, y);
        for step in 0..length as i32 {
            px += angle.cos();
            py += angle.sin();
            let shade = 4 + (depth * 2 + step as u32 / 3).min(10) as u8;
            canvas.set(px as i32, py as i32, shade);
        }
        if depth < 4 {
            for turn in [-0.5f32, 0.45] {
                branches.push((
                    px,
                    py,
                    angle + turn + rng.range(-0.15, 0.15),
                    length * 0.72,
                    depth + 1,
                ));
            }
        }
    }
    canvas
}

fn sponge() -> Canvas {
    let mut canvas = Canvas::new(16, 32);
    for (cx, height, width) in [
        (5.0f32, 26.0f32, 2.6f32),
        (10.0, 20.0, 2.2),
        (12.5, 13.0, 1.8),
    ] {
        for y in (32.0 - height) as i32..32 {
            for x in (cx - width) as i32..=(cx + width) as i32 {
                let side = (x as f32 - cx) / width;
                let shade = (8.0 + side * -4.0) as u8;
                canvas.set(x, y, shade.clamp(2, 14));
            }
        }
        for x in (cx - width + 1.0) as i32..(cx + width) as i32 {
            canvas.set(x, (32.0 - height) as i32, 1);
        }
    }
    canvas
}

fn anemone(seed: u64) -> Canvas {
    let mut canvas = Canvas::new(16, 16);
    let mut rng = Rng::new(seed);
    for _ in 0..22 {
        let angle = rng.range(-2.8, -0.35);
        let length = rng.range(5.0, 9.0);
        for step in 0..(length * 1.5) as i32 {
            let t = step as f32 / 1.5;
            let x = 8.0 + angle.cos() * t + (t * 0.8).sin() * 0.6;
            let y = 15.0 + angle.sin() * t;
            canvas.set(x as i32, y as i32, 6 + (t / length * 8.0) as u8);
        }
    }
    canvas
}

fn caustic() -> Canvas {
    let mut canvas = Canvas::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let (fx, fy) = (x as f32 / 64.0 * TAU, y as f32 / 64.0 * TAU);
            let warp_x = fx + 0.9 * (fy * 2.0).sin();
            let warp_y = fy + 0.9 * (fx * 2.0 + 1.3).sin();
            let a = (warp_x * 3.0).sin();
            let b = (warp_y * 3.0).sin();
            let c = ((warp_x + warp_y) * 2.0).sin();
            let line = (1.0 - ((a + b + c) / 3.0).abs()).powf(9.0);
            canvas.set(x, y, (line.min(0.999) * 15.99) as u8);
        }
    }
    canvas
}

fn radial(size: usize, power: f32) -> Canvas {
    let mut canvas = Canvas::new(size, size);
    let half = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let radius = (x as f32 + 0.5 - half).hypot(y as f32 + 0.5 - half) / half;
            let intensity = (1.0 - radius).max(0.0).powf(power);
            if intensity > 0.03 {
                canvas.set(x as i32, y as i32, 1 + (intensity.min(0.999) * 14.99) as u8);
            }
        }
    }
    canvas
}

fn bubble() -> Canvas {
    let mut canvas = Canvas::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            let radius = (x as f32 - 7.5).hypot(y as f32 - 7.5) / 7.5;
            if radius <= 1.0 {
                let rim = ((radius - 0.75) / 0.25).clamp(0.0, 1.0);
                let highlight = if x < 7 && y < 7 && radius < 0.55 && radius > 0.3 {
                    0.9
                } else {
                    0.0
                };
                let value = (rim * 0.8 + highlight).min(0.999);
                if value > 0.05 {
                    canvas.set(x, y, 1 + (value * 14.99) as u8);
                }
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
            let ridge = (1.0 - ((a + b) / 2.0).abs()).powf(3.0);
            let noise = hash_float(5, x as u64, y as u64) * 0.08;
            canvas.set(x, y, 1 + ((ridge + noise).min(0.999) * 14.99) as u8);
        }
    }
    canvas
}

pub struct Procedural {
    pub fan: Image,
    pub sponge: Image,
    pub anemone: Image,
    pub caustic: Image,
    pub glow: Image,
    pub mote: Image,
    pub bubble: Image,
    pub water: Image,
}

impl Procedural {
    pub fn build(vram: &mut Vram) -> Self {
        kelp(1).blit(vram, 0, 0);
        kelp(2).blit(vram, 16, 0);
        for kind in 0..3u8 {
            fish(kind as u64).blit(vram, 32, kind * 8);
        }
        fan(4).blit(vram, 64, 0);
        sponge().blit(vram, 96, 0);
        anemone(6).blit(vram, 112, 0);
        caustic().blit(vram, 128, 0);
        radial(32, 2.0).blit(vram, 192, 0);
        radial(16, 2.5).blit(vram, 224, 0);
        bubble().blit(vram, 240, 0);
        water().blit(vram, 0, 64);

        vram.write_clut(
            clut(Slot::Kelp),
            &palette(&[
                (1, [30, 24, 10], 0),
                (3, [60, 58, 20], 0),
                (5, [50, 72, 22], 0),
                (15, [150, 160, 60], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::FishYellow),
            &palette(&[
                (1, [10, 10, 10], 0),
                (6, [200, 120, 20], 0),
                (9, [250, 210, 40], 0),
                (15, [255, 250, 200], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::FishBlue),
            &palette(&[
                (1, [10, 10, 10], 0),
                (6, [20, 60, 140], 0),
                (9, [50, 120, 220], 0),
                (15, [200, 230, 255], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::FishSilver),
            &palette(&[
                (1, [10, 10, 10], 0),
                (6, [40, 50, 60], 0),
                (9, [160, 175, 185], 0),
                (15, [240, 245, 250], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::Fan),
            &palette(&[
                (1, [60, 10, 40], 0),
                (4, [140, 30, 90], 0),
                (15, [250, 140, 190], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::Sponge),
            &palette(&[
                (1, [40, 10, 0], 0),
                (2, [120, 40, 10], 0),
                (14, [250, 150, 40], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::Anemone),
            &palette(&[
                (1, [40, 20, 60], 0),
                (6, [120, 60, 170], 0),
                (15, [240, 200, 255], 0),
            ]),
        );
        vram.write_clut(
            clut(Slot::Caustic),
            &palette(&[(0, [0, 0, 0], SEMI), (15, [170, 230, 230], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Glow),
            &palette(&[(1, [8, 8, 6], SEMI), (15, [255, 245, 210], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Mote),
            &palette(&[(1, [10, 12, 12], SEMI), (15, [200, 220, 220], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Bubble),
            &palette(&[(1, [10, 14, 16], SEMI), (15, [220, 245, 255], SEMI)]),
        );
        vram.write_clut(
            clut(Slot::Water),
            &palette(&[(1, [40, 110, 120], SEMI), (15, [210, 250, 245], SEMI)]),
        );
        Self {
            fan: image(Slot::Fan, 64, 0, 32, 32),
            sponge: image(Slot::Sponge, 96, 0, 16, 32),
            anemone: image(Slot::Anemone, 112, 0, 16, 16),
            caustic: image(Slot::Caustic, 128, 0, 64, 64),
            glow: image(Slot::Glow, 192, 0, 32, 32),
            mote: image(Slot::Mote, 224, 0, 16, 16),
            bubble: image(Slot::Bubble, 240, 0, 16, 16),
            water: image(Slot::Water, 0, 64, 64, 64),
        }
    }
}
