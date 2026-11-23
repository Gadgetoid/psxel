use psxel_prim::Texture;
use psxel_vram::{bgr555, Clut, TexPage, TextureDepth, Vram};

use crate::biome::{lerp_rgb, scale_rgb, BiomeSchedule, Rgb};
use crate::font;
use crate::rng::{hash_float, Rng};

pub const PAGE_X: u8 = 5;
const TYPE_PAGE_X: u8 = 7;
pub const LARGE_FONT_HEIGHT: u8 = 108;

#[derive(Clone, Copy, Debug)]
pub struct Image {
    pub u: u8,
    pub v: u8,
    pub width: u8,
    pub height: u8,
    pub texture: Texture,
}

pub struct Atlas {
    pub ground: Image,
    pub post: Image,
    pub spectators: [Image; 4],
    pub chevron: Image,
    pub puff: Image,
    pub font: Image,
    pub icons: Image,
    pub banner: Image,
    pub large_font: Image,
    pub dial: crate::dial::Dial,
    pub stone: Image,
    pub huge_font: Image,
}

const CLUT_Y: u16 = 480;

#[derive(Clone, Copy)]
enum Slot {
    Road,
    Shoulder,
    Ground,
    Pine,
    SnowPine,
    Broadleaf,
    Autumn,
    Cactus,
    Rock,
    Bush,
    Post,
    Spectator0,
    Spectator1,
    Spectator2,
    Spectator3,
    Chevron,
    Puff,
    Font,
    Icons,
    Banner,
    Car,
    Type,
    Stone,
}

const FOGGED_SLOTS: [Slot; 15] = [
    Slot::Pine,
    Slot::SnowPine,
    Slot::Broadleaf,
    Slot::Autumn,
    Slot::Cactus,
    Slot::Rock,
    Slot::Bush,
    Slot::Post,
    Slot::Spectator0,
    Slot::Spectator1,
    Slot::Spectator2,
    Slot::Spectator3,
    Slot::Chevron,
    Slot::Banner,
    Slot::Stone,
];

pub const FOG_BANDS: [f32; 5] = [0.0, 0.33, 0.58, 0.78, 0.93];
const FOG_BAND_LIMITS: [f32; 4] = [0.2, 0.45, 0.7, 0.85];

pub fn fog_band(fog_amount: f32) -> u8 {
    FOG_BAND_LIMITS
        .iter()
        .filter(|&&limit| fog_amount >= limit)
        .count() as u8
}

impl Image {
    pub fn in_fog_band(mut self, band: u8) -> Image {
        self.texture.clut.y = CLUT_Y + band as u16;
        self
    }
}

pub(crate) fn car_clut() -> Clut {
    clut(Slot::Car)
}

pub(crate) fn wheel_clut() -> Clut {
    Clut::new(Slot::Car as u16 * 16, CLUT_Y + 1)
}

fn clut(slot: Slot) -> Clut {
    Clut::new(slot as u16 * 16, CLUT_Y)
}

fn page() -> TexPage {
    TexPage::new(PAGE_X, 0, TextureDepth::Clut4)
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

fn type_image(u: u8, v: u8) -> Image {
    Image {
        u,
        v,
        width: 255,
        height: 0,
        texture: Texture {
            page: TexPage::new(TYPE_PAGE_X, 0, TextureDepth::Clut4),
            clut: clut(Slot::Type),
            raw: false,
            window: None,
        },
    }
}

pub(crate) fn color(rgb: Rgb) -> u16 {
    let value = bgr555(rgb[0], rgb[1], rgb[2]);
    if value == 0 {
        0x8000
    } else {
        value
    }
}

fn ramp(dark: Rgb, light: Rgb) -> [u16; 16] {
    let mut colors = [0u16; 16];
    for (index, entry) in colors.iter_mut().enumerate().skip(1) {
        *entry = color(lerp_rgb(dark, light, (index - 1) as f32 / 14.0));
    }
    colors
}

pub(crate) struct Canvas {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, value: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.pixels[y as usize * self.width + x as usize] = value;
        }
    }

    pub(crate) fn disc(&mut self, cx: f32, cy: f32, radius: f32, value: impl Fn(f32, f32) -> u8) {
        for y in (cy - radius) as i32..=(cy + radius) as i32 + 1 {
            for x in (cx - radius) as i32..=(cx + radius) as i32 + 1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy <= radius * radius {
                    self.set(x, y, value(dx / radius, dy / radius));
                }
            }
        }
    }

    pub(crate) fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, value: u8) {
        for py in y..y + height {
            for px in x..x + width {
                self.set(px, py, value);
            }
        }
    }

    pub(crate) fn blit(&self, vram: &mut Vram, u: u8, v: u8) {
        vram.write_texture(page(), u as usize, v as usize, self.width, &self.pixels);
    }

    pub(crate) fn blit_to(&self, vram: &mut Vram, page: TexPage, u: u8, v: u8) {
        vram.write_texture(page, u as usize, v as usize, self.width, &self.pixels);
    }

    pub(crate) fn mirrored(&self) -> Canvas {
        let mut mirrored = Canvas::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                mirrored.pixels[y * self.width + x] =
                    self.pixels[y * self.width + self.width - 1 - x];
            }
        }
        mirrored
    }

    pub(crate) fn text(&mut self, x: i32, y: i32, text: &str, value: u8, scale: i32) {
        let glyphs = font::bitmap(128, 48);
        for (index, character) in text.chars().enumerate() {
            let Some(cell) = font::cell(character) else {
                continue;
            };
            for gy in 0..8 {
                for gx in 0..8 {
                    let texel = glyphs
                        [((cell / font::COLUMNS) * 8 + gy) * 128 + (cell % font::COLUMNS) * 8 + gx];
                    if texel == 2 {
                        self.rect(
                            x + (index as i32 * 6 + gx as i32 - 1) * scale,
                            y + gy as i32 * scale,
                            scale,
                            scale,
                            value,
                        );
                    }
                }
            }
        }
    }
}

fn noise_texture(seed: u64, size: usize, smooth: usize, tracks: bool) -> Canvas {
    let mut canvas = Canvas::new(size, size);
    let mut rng = Rng::new(seed);
    let mut field: Vec<f32> = (0..size * size).map(|_| rng.float()).collect();
    for _ in 0..smooth {
        let previous = field.clone();
        for y in 0..size {
            for x in 0..size {
                let at = |dx: i32, dy: i32| {
                    previous[((y as i32 + dy).rem_euclid(size as i32) as usize) * size
                        + (x as i32 + dx).rem_euclid(size as i32) as usize]
                };
                field[y * size + x] =
                    (at(0, 0) * 4.0 + at(1, 0) + at(-1, 0) + at(0, 1) + at(0, -1)) / 8.0;
            }
        }
    }
    for y in 0..size {
        for x in 0..size {
            let speck = rng.float();
            let mut value = field[y * size + x] * 0.7 + speck * 0.3;
            if tracks {
                let lane = |center: f32| (-((x as f32 - center) / 5.0).powi(2)).exp();
                value -= (lane(15.0) + lane(48.0)) * 0.25;
            }
            canvas.pixels[y * size + x] = (1.0 + value.clamp(0.0, 0.999) * 14.0) as u8;
        }
    }
    if tracks {
        for y in 0..size {
            if (y / 16) % 2 == 0 {
                canvas.pixels[y * size + size / 2 - 1] = 15;
                canvas.pixels[y * size + size / 2] = 15;
            }
        }
    }
    canvas
}

fn tree(seed: u64, width: usize, height: usize, conifer: bool) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    let center = width as f32 / 2.0;
    let trunk = if conifer { 14 } else { height as i32 * 5 / 8 };
    canvas.rect(center as i32 - 2, height as i32 - trunk, 4, trunk, 2);
    canvas.rect(center as i32 - 1, height as i32 - trunk, 1, trunk, 3);
    let mut rng = Rng::new(seed);
    if conifer {
        let top = 2.0;
        let bottom = height as f32 - 9.0;
        for y in top as i32..bottom as i32 {
            let t = (y as f32 - top) / (bottom - top);
            let tier = (t * 4.0).fract();
            let half = (2.0 + t * (center - 3.0)) * (0.65 + tier * 0.35);
            for x in (center - half) as i32..=(center + half) as i32 {
                let shade = 0.25
                    + 0.6 * (1.0 - (x as f32 - center + half) / (2.0 * half + 1.0))
                    + rng.range(-0.15, 0.15);
                canvas.set(x, y, 4 + (shade.clamp(0.0, 0.99) * 11.0) as u8);
            }
        }
    } else {
        for _ in 0..14 {
            let cx = center + rng.range(-8.0, 8.0);
            let cy = height as f32 * 0.42 + rng.range(-12.0, 10.0);
            let radius = rng.range(6.0, 10.0);
            let jitter = rng.next_u32() as u64;
            canvas.disc(cx, cy, radius, |dx, dy| {
                let light = 0.55 - dx * 0.25 - dy * 0.3
                    + (hash_float(jitter, (dx * 9.0) as u64, (dy * 9.0) as u64) - 0.5) * 0.4;
                4 + (light.clamp(0.0, 0.99) * 11.0) as u8
            });
        }
    }
    canvas
}

fn icons() -> Canvas {
    let mut canvas = Canvas::new(256, 64);
    for icon in 0..10 {
        let ox = (icon % 8) as f32 * 32.0;
        let oy = (icon / 8) as f32 * 32.0;
        let mut points: Vec<(f32, f32)> = Vec::new();
        match icon {
            0..=6 => {
                let sweep = [3.1, 2.4, 1.9, 1.5, 1.15, 0.8, 0.45][icon];
                let radius = [6.0, 7.0, 8.5, 10.0, 13.0, 18.0, 30.0][icon];
                let straight = 7.0;
                let (mut x, mut y, mut heading) =
                    (ox + 16.0 - radius * 0.35 * (sweep / 3.1), oy + 29.0, 0.0f32);
                for _ in 0..(straight as i32) {
                    points.push((x, y));
                    y -= 1.0;
                }
                let steps = (radius * sweep) as i32;
                for _ in 0..steps {
                    points.push((x, y));
                    heading += 1.0 / radius;
                    x += heading.sin();
                    y -= heading.cos();
                }
                points.push((x, y));
                let tip = (x + heading.sin() * 6.0, y - heading.cos() * 6.0);
                let side = (heading.cos() * 5.0, heading.sin() * 5.0);
                fill_triangle(
                    &mut canvas,
                    tip,
                    (x + side.0, y + side.1),
                    (x - side.0, y - side.1),
                );
            }
            7 => {
                for step in 0..=24 {
                    let t = step as f32 / 24.0;
                    points.push((
                        ox + 4.0 + t * 24.0,
                        oy + 22.0 - (t * std::f32::consts::PI).sin() * 12.0,
                    ));
                }
            }
            8 => {
                for step in 0..=14 {
                    points.push((ox + 16.0, oy + 4.0 + step as f32));
                }
                canvas.disc(ox + 16.0, oy + 26.0, 2.5, |_, _| 2);
            }
            _ => {
                for row in 0..4 {
                    for column in 0..5 {
                        let value = if (row + column) % 2 == 0 { 2 } else { 3 };
                        canvas.rect(
                            ox as i32 + 6 + column * 4,
                            oy as i32 + 6 + row * 4,
                            4,
                            4,
                            value,
                        );
                    }
                }
                canvas.rect(ox as i32 + 5, oy as i32 + 6, 1, 22, 2);
            }
        }
        for &(x, y) in &points {
            canvas.disc(x, y, 2.2, |_, _| 2);
        }
    }
    outline(&mut canvas);
    canvas
}

fn fill_triangle(canvas: &mut Canvas, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
    let min_x = a.0.min(b.0).min(c.0) as i32;
    let max_x = a.0.max(b.0).max(c.0) as i32 + 1;
    let min_y = a.1.min(b.1).min(c.1) as i32;
    let max_y = a.1.max(b.1).max(c.1) as i32 + 1;
    let edge = |p: (f32, f32), q: (f32, f32), x: f32, y: f32| {
        (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0)
    };
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let e = [edge(a, b, px, py), edge(b, c, px, py), edge(c, a, px, py)];
            if e.iter().all(|&v| v >= 0.0) || e.iter().all(|&v| v <= 0.0) {
                canvas.set(x, y, 2);
            }
        }
    }
}

fn outline(canvas: &mut Canvas) {
    let source = canvas.pixels.clone();
    for y in 0..canvas.height as i32 {
        for x in 0..canvas.width as i32 {
            if source[y as usize * canvas.width + x as usize] != 0 {
                continue;
            }
            let near = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && (nx as usize) < canvas.width
                        && (ny as usize) < canvas.height
                        && source[ny as usize * canvas.width + nx as usize] >= 2
                })
            });
            if near {
                canvas.set(x, y, 1);
            }
        }
    }
}

impl Atlas {
    pub fn build(vram: &mut Vram, seed: u64) -> Self {
        noise_texture(seed ^ 1, 64, 1, true).blit(vram, 0, 0);
        noise_texture(seed ^ 2, 64, 3, false).blit(vram, 64, 0);
        tree(seed ^ 3, 32, 64, true).blit(vram, 128, 0);
        tree(seed ^ 4, 32, 64, false).blit(vram, 160, 0);

        let mut cactus = Canvas::new(32, 64);
        cactus.rect(13, 10, 6, 54, 8);
        cactus.rect(14, 10, 2, 54, 12);
        cactus.rect(5, 26, 4, 18, 8);
        cactus.rect(5, 40, 10, 4, 8);
        cactus.rect(23, 18, 4, 16, 8);
        cactus.rect(17, 32, 10, 4, 8);
        cactus.disc(16.0, 10.0, 3.0, |_, _| 10);
        cactus.blit(vram, 192, 0);

        let mut rock = Canvas::new(32, 32);
        let mut rng = Rng::new(seed ^ 5);
        for _ in 0..6 {
            let (cx, cy, radius) = (
                rng.range(9.0, 23.0),
                rng.range(16.0, 24.0),
                rng.range(6.0, 9.0),
            );
            rock.disc(cx, cy, radius, |dx, dy| {
                (3.0 + (0.6 - dx * 0.3 - dy * 0.4).clamp(0.0, 0.99) * 12.0) as u8
            });
        }
        rock.blit(vram, 224, 0);

        let mut bush = Canvas::new(32, 32);
        for _ in 0..9 {
            let (cx, cy, radius) = (
                rng.range(7.0, 25.0),
                rng.range(20.0, 26.0),
                rng.range(4.0, 7.0),
            );
            bush.disc(cx, cy, radius, |dx, dy| {
                (4.0 + (0.55 - dx * 0.2 - dy * 0.4).clamp(0.0, 0.99) * 11.0) as u8
            });
        }
        bush.blit(vram, 224, 32);

        let mut spectator = Canvas::new(16, 32);
        spectator.disc(8.0, 5.0, 3.2, |_, _| 3);
        spectator.rect(4, 9, 8, 11, 5);
        spectator.rect(2, 10, 2, 8, 5);
        spectator.rect(12, 6, 2, 6, 5);
        spectator.rect(4, 20, 3, 12, 8);
        spectator.rect(9, 20, 3, 12, 8);
        spectator.blit(vram, 0, 64);

        let mut chevron = Canvas::new(32, 32);
        chevron.rect(2, 4, 28, 20, 2);
        for y in 4..24i32 {
            for x in 2..30 {
                let stripe = ((x - 2) - (y - 14).abs() / 2).rem_euclid(9) < 4;
                if stripe {
                    chevron.set(x, y, 3);
                }
            }
        }
        chevron.rect(7, 24, 2, 8, 4);
        chevron.rect(23, 24, 2, 8, 4);
        chevron.blit(vram, 16, 64);

        let mut puff = Canvas::new(16, 16);
        puff.disc(8.0, 8.0, 7.5, |dx, dy| {
            (1.0 + (1.0 - (dx * dx + dy * dy)).clamp(0.0, 0.99) * 15.0) as u8
        });
        puff.blit(vram, 48, 64);

        let mut post = Canvas::new(16, 32);
        post.rect(6, 4, 4, 28, 6);
        post.rect(6, 4, 1, 28, 9);
        post.rect(6, 7, 4, 3, 14);
        post.blit(vram, 96, 64);

        let mut banner = Canvas::new(128, 16);
        banner.rect(0, 0, 128, 16, 3);
        banner.rect(0, 0, 128, 2, 2);
        banner.rect(0, 14, 128, 2, 2);
        let glyphs = font::bitmap(128, 48);
        let text = "CHECKPOINT";
        let start = (128 - text.len() * 10) / 2;
        for (index, character) in text.chars().enumerate() {
            if let Some(cell) = font::cell(character) {
                for y in 0..8 {
                    for x in 0..8 {
                        let value = glyphs[((cell / font::COLUMNS) * 8 + y) * 128
                            + (cell % font::COLUMNS) * 8
                            + x];
                        if value == 2 {
                            banner.rect((start + index * 10 + x) as i32, 4 + y as i32, 1, 1, 1);
                        }
                    }
                }
            }
        }
        banner.blit(vram, 128, 64);

        let mut stone = noise_texture(seed ^ 6, 64, 1, false);
        let mut crack_rng = Rng::new(seed ^ 7);
        for _ in 0..7 {
            let (mut x, mut y) = (crack_rng.range(0.0, 64.0), crack_rng.range(0.0, 64.0));
            let angle = crack_rng.range(0.0, std::f32::consts::TAU);
            for _ in 0..crack_rng.range(10.0, 30.0) as i32 {
                stone.set(x as i32, y as i32, 1);
                x += angle.cos() + crack_rng.range(-0.6, 0.6);
                y += angle.sin() + crack_rng.range(-0.6, 0.6);
            }
        }
        for y in 0..64 {
            for x in 0..64 {
                let value = stone.pixels[y * 64 + x];
                stone.pixels[y * 64 + x] = if value == 1 { 1 } else { (value + 2).min(15) };
            }
        }
        stone.blit(vram, 128, 80);

        let mut text_canvas = Canvas::new(128, 48);
        text_canvas.pixels = glyphs;
        text_canvas.blit(vram, 0, 128);
        icons().blit(vram, 0, 176);

        let type_page = TexPage::new(TYPE_PAGE_X, 0, TextureDepth::Clut4);
        let (large, large_width, large_height) =
            font::styled_atlas(&font::LARGE, &font::ascii_characters(), 16);
        vram.write_texture(type_page, 0, 0, large_width, &large);
        let huge_characters: Vec<char> = font::HUGE_CHARACTERS.chars().collect();
        let (huge, huge_width, _) = font::styled_atlas(&font::HUGE, &huge_characters, 8);
        vram.write_texture(type_page, 0, large_height, huge_width, &huge);
        vram.write_clut(clut(Slot::Type), &{
            let mut colors = [0u16; 16];
            colors[font::OUTLINE as usize] = 0x0421;
            colors[font::SHADOW as usize] = 0x8000 | 0x0421;
            colors[font::FACE_TOP as usize] = color([255, 255, 255]);
            colors[font::FACE_TOP as usize + 1] = color([242, 242, 242]);
            colors[font::FACE_TOP as usize + 2] = color([222, 222, 222]);
            colors[font::FACE_TOP as usize + 3] = color([196, 196, 200]);
            colors
        });
        vram.write_clut(clut(Slot::Font), &{
            let mut colors = [0u16; 16];
            colors[1] = 0x8000;
            colors[2] = color([255, 255, 255]);
            colors
        });
        vram.write_clut(clut(Slot::Icons), &{
            let mut colors = [0u16; 16];
            colors[1] = 0x8000;
            colors[2] = color([255, 255, 255]);
            colors[3] = color([30, 30, 30]);
            colors
        });
        vram.write_clut(clut(Slot::Chevron), &{
            let mut colors = [0u16; 16];
            colors[2] = color([230, 230, 230]);
            colors[3] = color([210, 30, 20]);
            colors[4] = color([90, 90, 90]);
            colors
        });
        vram.write_clut(clut(Slot::Banner), &{
            let mut colors = [0u16; 16];
            colors[1] = color([255, 255, 255]);
            colors[2] = color([250, 200, 0]);
            colors[3] = color([20, 40, 150]);
            colors
        });
        vram.write_clut(
            clut(Slot::Puff),
            &ramp([255, 255, 255], [255, 255, 255]).map(|c| if c == 0 { 0 } else { c | 0x8000 }),
        );
        vram.write_clut(clut(Slot::Post), &ramp([60, 40, 25], [200, 200, 200]));
        let shirts = [[200, 30, 30], [30, 60, 200], [230, 200, 40], [40, 160, 60]];
        for (index, shirt) in shirts.iter().enumerate() {
            let mut colors = [0u16; 16];
            colors[3] = color([230, 180, 140]);
            colors[5] = color(*shirt);
            colors[8] = color([40, 40, 60]);
            let slot = [
                Slot::Spectator0,
                Slot::Spectator1,
                Slot::Spectator2,
                Slot::Spectator3,
            ][index];
            vram.write_clut(clut(slot), &colors);
        }

        Self {
            ground: image(Slot::Ground, 64, 0, 64, 64),
            post: image(Slot::Post, 96, 64, 16, 32),
            spectators: [
                image(Slot::Spectator0, 0, 64, 16, 32),
                image(Slot::Spectator1, 0, 64, 16, 32),
                image(Slot::Spectator2, 0, 64, 16, 32),
                image(Slot::Spectator3, 0, 64, 16, 32),
            ],
            chevron: image(Slot::Chevron, 16, 64, 32, 32),
            puff: image(Slot::Puff, 48, 64, 16, 16),
            font: image(Slot::Font, 0, 128, 128, 48),
            icons: image(Slot::Icons, 0, 176, 255, 64),
            banner: image(Slot::Banner, 128, 64, 128, 16),
            large_font: type_image(0, 0),
            dial: crate::dial::build(vram),
            stone: image(Slot::Stone, 128, 80, 63, 63),
            huge_font: type_image(0, LARGE_FONT_HEIGHT),
        }
    }

    pub fn update_fog_cluts(&self, vram: &mut Vram, fog: Rgb) {
        let fog = fog.map(|c| (c >> 3) as i32);
        for slot in FOGGED_SLOTS {
            let base = clut(slot);
            let source: Vec<u16> = (0..16)
                .map(|index| vram.get(base.x as usize + index, base.y as usize))
                .collect();
            for (band, amount) in FOG_BANDS.iter().enumerate().skip(1) {
                let banded: Vec<u16> = source
                    .iter()
                    .map(|&entry| {
                        if entry == 0 {
                            return 0;
                        }
                        let mix = |shift: u32, target: i32| {
                            let channel = ((entry >> shift) & 0x1f) as i32;
                            ((channel as f32 + (target - channel) as f32 * amount).round() as u16)
                                << shift
                        };
                        (entry & 0x8000) | mix(0, fog[0]) | mix(5, fog[1]) | mix(10, fog[2])
                    })
                    .collect();
                vram.write_clut(Clut::new(base.x, CLUT_Y + band as u16), &banded);
            }
        }
    }

    pub fn update_biome_cluts(&self, vram: &mut Vram, biomes: &BiomeSchedule, distance: f32) {
        let mix = |field: fn(&crate::biome::Biome) -> Rgb| biomes.color(distance, field);
        let road_dark = mix(|b| b.road_dark);
        let road_light = mix(|b| b.road_light);
        let mut road = ramp(road_dark, road_light);
        road[15] = color(mix(|b| b.road_paint));
        vram.write_clut(clut(Slot::Road), &road);
        let ground_dark = mix(|b| b.ground_dark);
        let ground_light = mix(|b| b.ground_light);
        vram.write_clut(clut(Slot::Ground), &ramp(ground_dark, ground_light));
        vram.write_clut(
            clut(Slot::Shoulder),
            &ramp(
                lerp_rgb(road_dark, ground_dark, 0.5),
                lerp_rgb(road_light, ground_light, 0.5),
            ),
        );
        let foliage_dark = mix(|b| b.foliage_dark);
        let foliage_light = mix(|b| b.foliage_light);
        let trunk = [[0u8; 3], [60, 40, 25], [95, 65, 40]];
        let mut foliage = ramp(foliage_dark, foliage_light);
        foliage[1..4].copy_from_slice(&[color(trunk[0]), color(trunk[1]), color(trunk[2])]);
        foliage[1] = 0;
        vram.write_clut(clut(Slot::Pine), &foliage);
        vram.write_clut(clut(Slot::Broadleaf), &foliage);
        let mut snowy = ramp([40, 70, 70], [245, 250, 255]);
        snowy[1..4].copy_from_slice(&[0, color(trunk[1]), color(trunk[2])]);
        vram.write_clut(clut(Slot::SnowPine), &snowy);
        let mut autumn = ramp([120, 30, 10], [250, 170, 40]);
        autumn[1..4].copy_from_slice(&[0, color(trunk[1]), color(trunk[2])]);
        vram.write_clut(clut(Slot::Autumn), &autumn);
        let mut cactus = ramp([30, 70, 35], [120, 170, 80]);
        cactus[10] = color([240, 120, 160]);
        vram.write_clut(clut(Slot::Cactus), &cactus);
        let rock_base = lerp_rgb(ground_dark, [110, 105, 100], 0.6);
        vram.write_clut(
            clut(Slot::Rock),
            &ramp(scale_rgb(rock_base, 0.5), scale_rgb(rock_base, 1.6)),
        );
        vram.write_clut(
            clut(Slot::Bush),
            &ramp(scale_rgb(foliage_dark, 0.8), foliage_light),
        );
    }

    pub fn average(&self, vram: &Vram, image: &Image) -> Rgb {
        let mut sum = [0u32; 3];
        let mut count = 0u32;
        for v in (0..image.height).step_by(4) {
            for u in (0..image.width).step_by(4) {
                let texel = vram.texel(
                    image.texture.page,
                    image.texture.clut,
                    image.u + u,
                    image.v + v,
                );
                if texel != 0 {
                    sum[0] += ((texel & 0x1f) << 3) as u32;
                    sum[1] += (((texel >> 5) & 0x1f) << 3) as u32;
                    sum[2] += (((texel >> 10) & 0x1f) << 3) as u32;
                    count += 1;
                }
            }
        }
        sum.map(|c| (c / count.max(1)) as u8)
    }
}
