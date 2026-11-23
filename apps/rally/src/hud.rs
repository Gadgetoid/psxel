use psx_prim::{BlendMode, Primitive, Vertex};

use crate::biome::Rgb;
use crate::font::{self, cell, COLUMNS, GLYPH_SIZE};
use crate::render::{screen_quad, SCREEN_WIDTH};
use crate::textures::{Atlas, Image};
use crate::track::{NoteKind, PaceNote};

const CALL_WIDTH: i16 = 118;

pub const WHITE: Rgb = [128, 128, 128];
pub const YELLOW: Rgb = [132, 112, 10];
pub const RED: Rgb = [135, 28, 18];
pub const GREEN: Rgb = [60, 132, 50];
pub const CYAN: Rgb = [70, 118, 132];

pub struct Hud<'a> {
    pub overlay: &'a mut Vec<Primitive>,
    pub atlas: &'a Atlas,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Small,
    Large,
    Huge,
}

impl Size {
    fn advance(self) -> i16 {
        match self {
            Self::Small => GLYPH_SIZE as i16 - 1,
            Self::Large => 12,
            Self::Huge => 25,
        }
    }

    fn cell(self) -> (u16, u16) {
        match self {
            Self::Small => (GLYPH_SIZE as u16, GLYPH_SIZE as u16),
            Self::Large => (
                font::LARGE.cell_width as u16,
                font::LARGE.cell_height as u16,
            ),
            Self::Huge => (font::HUGE.cell_width as u16, font::HUGE.cell_height as u16),
        }
    }
}

pub fn text_width(text: &str, size: Size) -> i16 {
    text.chars().count() as i16 * size.advance() + 2
}

impl Hud<'_> {
    fn glyph(&self, character: char, size: Size) -> Option<(Image, u8, u8)> {
        match size {
            Size::Small => {
                let glyph = cell(character)?;
                let font = self.atlas.font;
                Some((
                    font,
                    font.u + ((glyph % COLUMNS) * GLYPH_SIZE) as u8,
                    font.v + ((glyph / COLUMNS) * GLYPH_SIZE) as u8,
                ))
            }
            Size::Large => {
                let glyph = cell(character)?;
                let font = self.atlas.large_font;
                let (width, height) = size.cell();
                Some((
                    font,
                    ((glyph % 16) as u16 * width) as u8,
                    ((glyph / 16) as u16 * height) as u8,
                ))
            }
            Size::Huge => {
                let slot = font::HUGE_CHARACTERS.find(character.to_ascii_uppercase())?;
                let font = self.atlas.huge_font;
                let (width, height) = size.cell();
                Some((
                    font,
                    ((slot % 8) as u16 * width) as u8,
                    font.v + ((slot / 8) as u16 * height) as u8,
                ))
            }
        }
    }

    pub fn write(&mut self, x: i16, y: i16, text: &str, color: Rgb, size: Size) {
        let (width, height) = size.cell();
        for (index, character) in text.chars().enumerate() {
            let Some((font, u, v)) = self.glyph(character, size) else {
                continue;
            };
            self.overlay.push(Primitive::Sprite {
                x: x + index as i16 * size.advance(),
                y,
                width,
                height,
                u,
                v,
                color,
                texture: Some(font.texture),
                blend: if size == Size::Small {
                    None
                } else {
                    Some(BlendMode::Half)
                },
            });
        }
    }

    pub fn text(&mut self, x: i16, y: i16, text: &str, color: Rgb) {
        self.write(x, y, text, color, Size::Small);
    }

    pub fn large(&mut self, x: i16, y: i16, text: &str, color: Rgb) {
        self.write(x, y, text, color, Size::Large);
    }

    pub fn centered(&mut self, y: i16, text: &str, color: Rgb, size: Size) {
        let x = (SCREEN_WIDTH as i16 - text_width(text, size)) / 2;
        self.write(x, y, text, color, size);
    }

    pub fn right_aligned(&mut self, right: i16, y: i16, text: &str, color: Rgb, size: Size) {
        self.write(right - text_width(text, size), y, text, color, size);
    }

    fn arc_point(center: (i16, i16), radius: f32, angle: f32) -> (i16, i16) {
        (
            center.0 + (angle.cos() * radius).round() as i16,
            center.1 - (angle.sin() * radius).round() as i16,
        )
    }

    pub fn tachometer(
        &mut self,
        center: (i16, i16),
        rpm: f32,
        gear: &str,
        speed_kmh: i32,
        handbrake: bool,
    ) {
        const RADIUS: f32 = 34.0;
        let half = (crate::dial::SIZE / 2) as i16;
        let face = self.atlas.dial.face;
        let (left, top) = (center.0 - half, center.1 - half);
        self.overlay.push(Primitive::Sprite {
            x: left,
            y: top,
            width: face.width as u16,
            height: face.height as u16,
            u: face.u,
            v: face.v,
            color: [128, 128, 128],
            texture: Some(face.texture),
            blend: Some(BlendMode::Half),
        });
        let lit = self.atlas.dial.lit;
        let end = crate::dial::angle_for(rpm.clamp(0.0, crate::dial::MAX_RPM));
        let steps = 24;
        let span = crate::dial::START - end;
        let uv = |point: (i16, i16)| {
            (
                lit.u + (point.0 - left) as u8,
                lit.v + (point.1 - top) as u8,
            )
        };
        for step in 0..steps {
            let a0 = crate::dial::START - span * step as f32 / steps as f32;
            let a1 = crate::dial::START - span * (step + 1) as f32 / steps as f32;
            let p0 = Self::arc_point(center, half as f32, a0);
            let p1 = Self::arc_point(center, half as f32, a1);
            let corners = [center, p0, p1];
            self.overlay.push(Primitive::Triangle {
                vertices: corners.map(|point| {
                    let (u, v) = uv(point);
                    Vertex::new(point.0, point.1)
                        .with_uv(u, v)
                        .with_color([128, 128, 128])
                }),
                gouraud: false,
                texture: Some(lit.texture),
                blend: None,
            });
        }
        let needle_angle = crate::dial::angle_for(rpm.clamp(0.0, crate::dial::MAX_RPM));
        let tip = Self::arc_point(center, RADIUS - 2.0, needle_angle);
        let left = Self::arc_point(center, 3.0, needle_angle + 1.57);
        let right = Self::arc_point(center, 3.0, needle_angle - 1.57);
        self.overlay.push(Primitive::Triangle {
            vertices: [
                Vertex::new(tip.0, tip.1).with_color([255, 80, 60]),
                Vertex::new(left.0, left.1).with_color([150, 20, 10]),
                Vertex::new(right.0, right.1).with_color([150, 20, 10]),
            ],
            gouraud: true,
            texture: None,
            blend: None,
        });
        self.overlay.push(screen_quad(
            center.0 - 2,
            center.1 - 2,
            4,
            4,
            [[200, 200, 200]; 4],
            None,
        ));

        let bubble = (center.0 + 30, center.1 - 30);
        for segment in 0..12 {
            let a0 = std::f32::consts::TAU * segment as f32 / 12.0;
            let a1 = std::f32::consts::TAU * (segment + 1) as f32 / 12.0;
            let p0 = Self::arc_point(bubble, 9.5, a0);
            let p1 = Self::arc_point(bubble, 9.5, a1);
            self.overlay.push(Primitive::Triangle {
                vertices: [
                    Vertex::new(bubble.0, bubble.1).with_color([210, 40, 25]),
                    Vertex::new(p0.0, p0.1).with_color([90, 10, 5]),
                    Vertex::new(p1.0, p1.1).with_color([90, 10, 5]),
                ],
                gouraud: true,
                texture: None,
                blend: None,
            });
        }
        self.write(bubble.0 - 7, bubble.1 - 9, gear, WHITE, Size::Large);

        let readout = format!("{speed_kmh:3}");
        let led_x = center.0 - 19;
        let led_y = center.1 + 8;
        self.write(led_x, led_y, &readout, [150, 30, 20], Size::Large);
        self.text(center.0 - 10, led_y + 18, "KMH", [150, 30, 20]);
        if handbrake {
            self.write(center.0 - 66, center.1 + 14, "HB", RED, Size::Large);
        }
    }

    pub fn icon(&mut self, x: i16, y: i16, index: usize, mirror: bool, color: Rgb, size: i16) {
        let icons = self.atlas.icons;
        let u = icons.u + ((index % 8) * 32) as u8;
        let v = icons.v + ((index / 8) * 32) as u8;
        let (u0, u1) = if mirror { (u + 31, u) } else { (u, u + 31) };
        self.overlay.push(Primitive::Quad {
            vertices: [
                Vertex::new(x, y).with_uv(u0, v).with_color(color),
                Vertex::new(x + size, y).with_uv(u1, v).with_color(color),
                Vertex::new(x, y + size)
                    .with_uv(u0, v + 31)
                    .with_color(color),
                Vertex::new(x + size, y + size)
                    .with_uv(u1, v + 31)
                    .with_color(color),
            ],
            gouraud: false,
            texture: Some(icons.texture),
            blend: None,
        });
    }

    pub fn note(
        &mut self,
        x: i16,
        y: i16,
        note: &PaceNote,
        distance: f32,
        fresh: bool,
        small: bool,
    ) {
        let (icon, mirror) = match note.kind {
            NoteKind::Turn {
                direction,
                severity,
            } => (severity as usize, direction.sign() < 0.0),
            NoteKind::Crest | NoteKind::Bridge => (7, false),
            NoteKind::Jump | NoteKind::Ford => (8, false),
        };
        let color = if note.caution() {
            RED
        } else if fresh {
            YELLOW
        } else {
            WHITE
        };
        let size = if small { 20 } else { 32 };
        self.icon(x, y, icon, mirror, color, size);
        let call = note.call();
        if small {
            self.text(x + size + 2, y + 6, &call, WHITE);
        } else {
            let left = x + size + 3;
            let metres = format!("{}M", ((distance / 10.0).round() * 10.0).max(0.0) as i32);
            let mut wrapped: Vec<String> = Vec::new();
            for word in call.split(' ') {
                match wrapped.last_mut() {
                    Some(line)
                        if text_width(&format!("{line} {word}"), Size::Large) <= CALL_WIDTH =>
                    {
                        line.push(' ');
                        line.push_str(word);
                    }
                    _ => wrapped.push(word.to_string()),
                }
            }
            let mut line_y = y - 1;
            for line in &wrapped {
                self.large(left, line_y, line, color);
                line_y += 15;
            }
            let last_width = wrapped
                .last()
                .map(|line| text_width(line, Size::Large))
                .unwrap_or(0);
            self.text(left + 2 + last_width, line_y - 9, &metres, CYAN);
            self.text(left + 2, line_y + 3, &note.modifiers(), YELLOW);
        }
    }

    pub fn drag_indicator(&mut self, start: (i32, i32), current: (i32, i32)) {
        let (sx, sy) = (start.0 as i16, start.1 as i16);
        let (cx, cy) = (current.0 as i16, current.1 as i16);
        let ring = [[-8, -8], [8, -8], [8, 8], [-8, 8], [-8, -8]];
        for pair in ring.windows(2) {
            self.overlay.push(Primitive::Line {
                vertices: [
                    Vertex::new(sx + pair[0][0], sy + pair[0][1]).with_color([200, 200, 200]),
                    Vertex::new(sx + pair[1][0], sy + pair[1][1]).with_color([200, 200, 200]),
                ],
                gouraud: false,
                blend: Some(BlendMode::Half),
            });
        }
        self.overlay.push(Primitive::Line {
            vertices: [
                Vertex::new(sx, sy).with_color([90, 90, 90]),
                Vertex::new(cx, cy).with_color([255, 230, 120]),
            ],
            gouraud: true,
            blend: None,
        });
        self.overlay.push(screen_quad(
            cx - 3,
            cy - 3,
            6,
            6,
            [[255, 230, 120]; 4],
            None,
        ));
    }
}
