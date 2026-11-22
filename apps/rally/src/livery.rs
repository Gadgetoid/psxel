use psx_prim::Texture;
use psx_vram::{TexPage, TextureDepth, Vram};

use crate::biome::{scale_rgb, Rgb};
use crate::textures::{car_clut, color, Canvas, Image};

pub const PAGE_X: u8 = 6;

const BLACK: u8 = 1;
const RUBBER: u8 = 2;
const PAINT: u8 = 3;
const SHADE: u8 = 4;
const STRIPE: u8 = 5;
const PIN: u8 = 6;
const WHITE: u8 = 7;
const CHROME: u8 = 8;
const CHROME_DARK: u8 = 9;
const GLASS_DARK: u8 = 10;
const GLASS: u8 = 11;
const LENS: u8 = 12;
const AMBER: u8 = 13;
const RED: u8 = 14;
const BONNET: u8 = 15;

#[derive(Clone, Copy, Debug)]
pub struct Livery {
    pub paint: Rgb,
    pub stripe: Rgb,
    pub pinstripe: Rgb,
    pub bonnet: Rgb,
}

pub const LIVERIES: [Livery; 6] = [
    Livery {
        paint: [235, 235, 230],
        stripe: [30, 70, 190],
        pinstripe: [20, 30, 90],
        bonnet: [20, 20, 22],
    },
    Livery {
        paint: [235, 232, 220],
        stripe: [200, 30, 30],
        pinstripe: [110, 15, 15],
        bonnet: [235, 232, 220],
    },
    Livery {
        paint: [25, 45, 120],
        stripe: [235, 235, 235],
        pinstripe: [120, 170, 230],
        bonnet: [25, 45, 120],
    },
    Livery {
        paint: [190, 25, 25],
        stripe: [240, 240, 240],
        pinstripe: [60, 60, 60],
        bonnet: [20, 20, 22],
    },
    Livery {
        paint: [240, 200, 30],
        stripe: [25, 25, 25],
        pinstripe: [25, 25, 25],
        bonnet: [240, 200, 30],
    },
    Livery {
        paint: [30, 80, 50],
        stripe: [235, 230, 210],
        pinstripe: [230, 190, 40],
        bonnet: [30, 80, 50],
    },
];

pub struct CarAtlas {
    pub side_right: Image,
    pub side_left: Image,
    pub front: Image,
    pub rear: Image,
    pub windscreen: Image,
    pub rear_glass: Image,
    pub bonnet: Image,
    pub roof: Image,
    pub side_glass: Image,
    pub wheel: Image,
    pub lamp: Image,
    pub flap: Image,
    pub chrome: Image,
    pub paint: Image,
}

fn page() -> TexPage {
    TexPage::new(PAGE_X, 0, TextureDepth::Clut4)
}

fn image(u: u8, v: u8, width: u8, height: u8) -> Image {
    Image {
        u,
        v,
        width,
        height,
        texture: Texture {
            page: page(),
            clut: car_clut(),
            raw: false,
            window: None,
        },
    }
}

pub const SIDE_LENGTH: f32 = 252.0;
pub const WHEEL_Z: f32 = 86.0;

fn side_u(z: f32) -> f32 {
    (z + SIDE_LENGTH / 2.0) / SIDE_LENGTH * 255.0
}

fn ellipse(canvas: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32, value: u8) {
    for y in (cy - ry) as i32 - 1..=(cy + ry) as i32 + 1 {
        for x in (cx - rx) as i32 - 1..=(cx + rx) as i32 + 1 {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                canvas.set(x, y, value);
            }
        }
    }
}

fn side_base() -> Canvas {
    let mut side = Canvas::new(256, 32);
    side.rect(0, 0, 256, 32, PAINT);
    side.rect(0, 0, 256, 1, SHADE);
    side.rect(0, 29, 256, 3, SHADE);
    side.rect(6, 2, 240, 1, PIN);
    side.rect(4, 3, 246, 8, STRIPE);
    side.rect(4, 11, 246, 1, PIN);
    for x in 246..252 {
        let taper = x - 246;
        side.rect(x, 3 + taper, 1, (8 - taper * 2).max(0), STRIPE);
    }
    let (door_front, door_rear) = (side_u(28.0) as i32, side_u(-40.0) as i32);
    side.rect(door_front, 1, 1, 28, SHADE);
    side.rect(door_rear, 1, 1, 28, SHADE);
    side.rect(door_rear + 6, 13, 5, 2, CHROME);
    for wheel_z in [WHEEL_Z, -WHEEL_Z] {
        let u = side_u(wheel_z);
        ellipse(&mut side, u, 26.0, 23.0, 22.0, BLACK);
        ellipse(&mut side, u, 26.0, 20.0, 19.0, 0);
    }
    side.rect(side_u(118.0) as i32, 14, 3, 2, AMBER);
    side
}

fn roundel(side: &mut Canvas, center_u: f32, number: u32) {
    side.disc(center_u, 17.0, 12.5, |_, _| WHITE);
    let digits = number.to_string();
    let width = digits.len() as i32 * 12 - 2;
    side.text(center_u as i32 - width / 2, 10, &digits, BLACK, 2);
}

fn front() -> Canvas {
    let mut front = Canvas::new(64, 32);
    front.rect(0, 0, 64, 32, PAINT);
    front.rect(0, 0, 64, 2, SHADE);
    front.rect(17, 8, 30, 11, CHROME);
    front.rect(18, 9, 28, 9, BLACK);
    for y in (10..17).step_by(2) {
        front.rect(19, y, 26, 1, CHROME_DARK);
    }
    front.rect(31, 9, 2, 9, CHROME);
    for cx in [9.5f32, 54.5] {
        front.disc(cx, 13.0, 8.5, |_, _| CHROME);
        front.disc(cx, 13.0, 7.0, |dx, dy| {
            if dx * dx + dy * dy < 0.12 && dx < 0.0 && dy < 0.0 {
                WHITE
            } else {
                LENS
            }
        });
        front.rect(cx as i32 - 8, 13, 17, 1, CHROME_DARK);
    }
    front.rect(4, 22, 6, 3, AMBER);
    front.rect(54, 22, 6, 3, AMBER);
    front.rect(24, 21, 16, 3, BLACK);
    front.rect(0, 25, 64, 1, CHROME);
    front.rect(0, 26, 64, 2, CHROME_DARK);
    front.rect(0, 28, 64, 4, SHADE);
    front
}

fn rear(number: u32) -> Canvas {
    let mut rear = Canvas::new(64, 32);
    rear.rect(0, 0, 64, 32, PAINT);
    rear.rect(0, 0, 64, 1, SHADE);
    rear.rect(6, 4, 52, 1, SHADE);
    for x in [3, 55] {
        rear.rect(x, 4, 6, 14, CHROME);
        rear.rect(x + 1, 5, 4, 4, AMBER);
        rear.rect(x + 1, 9, 4, 8, RED);
    }
    rear.rect(18, 13, 28, 10, BLACK);
    rear.rect(19, 14, 26, 8, LENS);
    rear.text(20, 14, &format!("X{number:02}"), BLACK, 1);
    rear.rect(0, 24, 64, 1, CHROME);
    rear.rect(0, 25, 64, 2, CHROME_DARK);
    rear.rect(0, 27, 64, 5, SHADE);
    rear.rect(10, 28, 4, 2, BLACK);
    rear
}

fn glass(sun_strip: bool) -> Canvas {
    let mut glass = Canvas::new(64, 32);
    for y in 0..32 {
        let value = if (y / 3) % 4 == 0 && y > 10 {
            GLASS
        } else {
            GLASS_DARK
        };
        glass.rect(0, y, 64, 1, value);
    }
    glass.rect(0, 0, 64, 1, CHROME);
    glass.rect(0, 31, 64, 1, CHROME);
    glass.rect(0, 0, 1, 32, CHROME);
    glass.rect(63, 0, 1, 32, CHROME);
    if sun_strip {
        glass.rect(1, 1, 62, 8, STRIPE);
        glass.text(9, 1, "WORKS", WHITE, 1);
        glass.rect(12, 28, 18, 1, BLACK);
        glass.rect(36, 28, 18, 1, BLACK);
    }
    glass
}

fn bonnet() -> Canvas {
    let mut bonnet = Canvas::new(64, 64);
    bonnet.rect(0, 0, 64, 64, BONNET);
    bonnet.rect(31, 2, 2, 60, SHADE);
    for x in [8, 52] {
        bonnet.rect(x, 0, 4, 64, STRIPE);
    }
    for x in [16, 47] {
        bonnet.disc(x as f32 + 0.5, 6.5, 1.6, |_, _| CHROME);
    }
    bonnet
}

fn roof(number: u32) -> Canvas {
    let mut roof = Canvas::new(64, 64);
    roof.rect(0, 0, 64, 64, PAINT);
    roof.rect(0, 0, 64, 2, SHADE);
    roof.disc(32.0, 32.0, 14.0, |_, _| WHITE);
    let digits = number.to_string();
    let width = digits.len() as i32 * 12 - 2;
    roof.text(32 - width / 2, 25, &digits, BLACK, 2);
    roof
}

fn side_glass() -> Canvas {
    let mut glass = Canvas::new(64, 32);
    glass.rect(0, 0, 64, 32, PAINT);
    glass.rect(3, 3, 26, 27, GLASS_DARK);
    glass.rect(34, 3, 28, 27, GLASS_DARK);
    for y in (6..30).step_by(5) {
        glass.rect(3, y, 26, 1, GLASS);
        glass.rect(34, y, 28, 1, GLASS);
    }
    glass.rect(29, 0, 5, 32, BLACK);
    glass.rect(0, 30, 64, 2, CHROME);
    glass
}

fn wheel() -> Canvas {
    let mut wheel = Canvas::new(32, 32);
    wheel.disc(16.0, 16.0, 15.8, |_, _| BLACK);
    wheel.disc(16.0, 16.0, 13.5, |_, _| RUBBER);
    wheel.disc(16.0, 16.0, 10.0, |dx, dy| {
        let angle = dy.atan2(dx);
        let spoke = ((angle / std::f32::consts::TAU * 8.0).rem_euclid(1.0) - 0.5).abs() < 0.22;
        let radius = (dx * dx + dy * dy).sqrt();
        if radius > 0.85 {
            CHROME
        } else if radius < 0.3 {
            CHROME_DARK
        } else if spoke {
            CHROME
        } else {
            BLACK
        }
    });
    wheel.disc(16.0, 16.0, 1.5, |_, _| BLACK);
    wheel
}

fn lamp() -> Canvas {
    let mut lamp = Canvas::new(32, 32);
    lamp.disc(16.0, 16.0, 15.8, |_, _| CHROME);
    lamp.disc(16.0, 16.0, 14.0, |_, _| BLACK);
    lamp.disc(16.0, 16.0, 10.5, |_, _| WHITE);
    lamp.disc(16.0, 16.0, 9.0, |_, _| BLACK);
    lamp.rect(9, 15, 14, 3, WHITE);
    lamp.rect(15, 9, 3, 14, WHITE);
    lamp
}

fn flap() -> Canvas {
    let mut flap = Canvas::new(16, 32);
    flap.rect(0, 0, 16, 32, BLACK);
    flap.rect(0, 20, 16, 5, STRIPE);
    flap
}

pub fn build(vram: &mut Vram, number: u32) -> CarAtlas {
    let page = page();
    let mut right = side_base();
    let mut left = right.mirrored();
    roundel(&mut right, side_u(-6.0), number);
    roundel(&mut left, 255.0 - side_u(-6.0), number);
    right.text(side_u(-112.0) as i32, 4, "WORKS", WHITE, 1);
    left.text(255 - side_u(-112.0) as i32 - 30, 4, "WORKS", WHITE, 1);
    right.blit_to(vram, page, 0, 0);
    left.blit_to(vram, page, 0, 32);
    front().blit_to(vram, page, 0, 64);
    rear(number).blit_to(vram, page, 64, 64);
    glass(true).blit_to(vram, page, 128, 64);
    glass(false).blit_to(vram, page, 192, 64);
    bonnet().blit_to(vram, page, 0, 96);
    roof(number).blit_to(vram, page, 64, 96);
    side_glass().blit_to(vram, page, 128, 96);
    wheel().blit_to(vram, page, 192, 96);
    lamp().blit_to(vram, page, 224, 96);
    flap().blit_to(vram, page, 128, 128);
    let mut chrome = Canvas::new(16, 16);
    chrome.rect(0, 0, 16, 8, CHROME);
    chrome.rect(0, 8, 16, 8, CHROME_DARK);
    chrome.blit_to(vram, page, 144, 128);
    let mut paint = Canvas::new(16, 16);
    paint.rect(0, 0, 16, 16, PAINT);
    paint.rect(0, 0, 16, 1, SHADE);
    paint.blit_to(vram, page, 160, 128);
    CarAtlas {
        side_right: image(0, 0, 255, 32),
        side_left: image(0, 32, 255, 32),
        front: image(0, 64, 64, 32),
        rear: image(64, 64, 64, 32),
        windscreen: image(128, 64, 64, 32),
        rear_glass: image(192, 64, 64, 32),
        bonnet: image(0, 96, 64, 64),
        roof: image(64, 96, 64, 64),
        side_glass: image(128, 96, 64, 32),
        wheel: image(192, 96, 32, 32),
        lamp: image(224, 96, 32, 32),
        flap: image(128, 128, 16, 32),
        chrome: image(144, 128, 16, 16),
        paint: image(160, 128, 16, 16),
    }
}

pub fn write_clut(vram: &mut Vram, livery: &Livery, braking: bool) {
    let mut colors = [0u16; 16];
    colors[BLACK as usize] = color([12, 12, 14]);
    colors[RUBBER as usize] = color([40, 40, 42]);
    colors[PAINT as usize] = color(livery.paint);
    colors[SHADE as usize] = color(scale_rgb(livery.paint, 0.72));
    colors[STRIPE as usize] = color(livery.stripe);
    colors[PIN as usize] = color(livery.pinstripe);
    colors[WHITE as usize] = color([245, 245, 240]);
    colors[CHROME as usize] = color([225, 228, 235]);
    colors[CHROME_DARK as usize] = color([110, 112, 120]);
    colors[GLASS_DARK as usize] = color([30, 40, 55]);
    colors[GLASS as usize] = color([90, 110, 130]);
    colors[LENS as usize] = color([245, 230, 160]);
    colors[AMBER as usize] = color([250, 140, 20]);
    colors[RED as usize] = color(if braking {
        [255, 40, 30]
    } else {
        [120, 15, 15]
    });
    colors[BONNET as usize] = color(livery.bonnet);
    vram.write_clut(car_clut(), &colors);
}
