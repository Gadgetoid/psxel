use psxel_prim::{exceeds_gpu_limits, BlendMode, Primitive, Texture, Vertex};
use psxel_vram::{TextureWindow, Vram, MASK_BIT, VRAM_HEIGHT, VRAM_WIDTH};

pub const DITHER: [[i32; 4]; 4] = [
    [-4, 0, -3, 1],
    [2, -2, 3, -1],
    [-3, 1, -4, 0],
    [3, -1, 2, -2],
];

const FRACTION: u32 = 12;
const HALF: i64 = 1 << (FRACTION - 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawEnv {
    pub clip_left: i32,
    pub clip_top: i32,
    pub clip_right: i32,
    pub clip_bottom: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub dither: bool,
    pub set_mask: bool,
    pub check_mask: bool,
    pub window: TextureWindow,
}

impl DrawEnv {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            clip_left: x,
            clip_top: y,
            clip_right: x + width - 1,
            clip_bottom: y + height - 1,
            offset_x: x,
            offset_y: y,
            dither: true,
            set_mask: false,
            check_mask: false,
            window: TextureWindow::default(),
        }
    }
}

#[derive(Clone, Copy)]
struct Shading {
    gouraud: bool,
    texture: Option<Texture>,
    blend: Option<BlendMode>,
    dither: bool,
}

pub fn fill_rect(vram: &mut Vram, x: usize, y: usize, width: usize, height: usize, color: u16) {
    vram.fill_rect(x & !0xf, y, (width + 0xf) & !0xf, height, color & 0x7fff);
}

fn blend(mode: BlendMode, back: i32, front: i32) -> i32 {
    match mode {
        BlendMode::Half => (back + front) >> 1,
        BlendMode::Add => back + front,
        BlendMode::Subtract => back - front,
        BlendMode::AddQuarter => back + (front >> 2),
    }
    .clamp(0, 31)
}

#[inline(always)]
fn channel(pixel: u16, shift: u32) -> i32 {
    ((pixel >> shift) & 0x1f) as i32
}

struct Raster<'a> {
    vram: &'a mut Vram,
    env: &'a DrawEnv,
}

impl Raster<'_> {
    #[inline(always)]
    fn plot(&mut self, x: i32, y: i32, color: [i32; 3], uv: (u8, u8), shading: &Shading) {
        let index = (y as usize & (VRAM_HEIGHT - 1)) * VRAM_WIDTH + (x as usize & (VRAM_WIDTH - 1));
        let background = self.vram.pixels[index];
        if self.env.check_mask && background & MASK_BIT != 0 {
            return;
        }
        let (mut rgb, semi_transparent, mask) = match shading.texture {
            Some(texture) => {
                let (u, v) = texture.window.unwrap_or(self.env.window).apply(uv.0, uv.1);
                let texel = self.vram.texel(texture.page, texture.clut, u, v);
                if texel == 0 {
                    return;
                }
                let texel_rgb = [channel(texel, 0), channel(texel, 5), channel(texel, 10)];
                let rgb = if texture.raw {
                    texel_rgb.map(|c| c << 3)
                } else {
                    [0, 1, 2].map(|c| ((texel_rgb[c] * color[c]) >> 4).min(255))
                };
                (rgb, texel & MASK_BIT != 0, texel & MASK_BIT)
            }
            None => (color, true, 0),
        };
        if shading.dither {
            let offset = DITHER[y as usize & 3][x as usize & 3];
            rgb = rgb.map(|c| (c + offset).clamp(0, 255));
        }
        let mut rgb5 = rgb.map(|c| c >> 3);
        if let (Some(mode), true) = (shading.blend, semi_transparent) {
            rgb5 = [0, 1, 2].map(|c| blend(mode, channel(background, c as u32 * 5), rgb5[c]));
        }
        let mask = if self.env.set_mask { MASK_BIT } else { mask };
        self.vram.pixels[index] =
            mask | (rgb5[2] << 10) as u16 | (rgb5[1] << 5) as u16 | rgb5[0] as u16;
    }

    fn triangle(&mut self, vertices: [Vertex; 3], shading: Shading) {
        if exceeds_gpu_limits(&vertices) {
            return;
        }
        let points: [(i64, i64); 3] = vertices.map(|v| {
            (
                v.x as i64 + self.env.offset_x as i64,
                v.y as i64 + self.env.offset_y as i64,
            )
        });
        let attributes: [[i64; 5]; 3] = vertices.map(|v| {
            [
                v.color[0] as i64,
                v.color[1] as i64,
                v.color[2] as i64,
                v.u as i64,
                v.v as i64,
            ]
        });
        let (x0, y0) = points[0];
        let (x1, y1) = points[1];
        let (x2, y2) = points[2];
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area == 0 {
            return;
        }
        let flat_color = attributes[0];
        let mut dx = [0i64; 5];
        let mut dy = [0i64; 5];
        let mut base = [0i64; 5];
        for k in 0..5 {
            let gradient_active =
                (k < 3 && shading.gouraud) || (k >= 3 && shading.texture.is_some());
            let a0 = if k < 3 && !shading.gouraud {
                flat_color[k]
            } else {
                attributes[0][k]
            };
            base[k] = (a0 << FRACTION) + if k < 3 { HALF } else { 0 };
            if gradient_active {
                let d1 = attributes[1][k] - attributes[0][k];
                let d2 = attributes[2][k] - attributes[0][k];
                dx[k] = ((d1 * (y2 - y0) - d2 * (y1 - y0)) << FRACTION) / area;
                dy[k] = ((d2 * (x1 - x0) - d1 * (x2 - x0)) << FRACTION) / area;
            }
        }

        let mut order = [0usize, 1, 2];
        order.sort_by_key(|&i| points[i].1);
        let [top, middle, bottom] = order.map(|i| points[i]);
        if top.1 == bottom.1 {
            return;
        }

        let y_start = top.1.max(self.env.clip_top as i64);
        let y_end = bottom.1.min(self.env.clip_bottom as i64 + 1);
        let long_on_left = {
            let cross =
                (bottom.0 - top.0) * (middle.1 - top.1) - (middle.0 - top.0) * (bottom.1 - top.1);
            cross < 0
        };
        let edge_x = |a: (i64, i64), b: (i64, i64), y: i64| -> i64 {
            let numerator = a.0 * (b.1 - a.1) + (b.0 - a.0) * (y - a.1);
            let denominator = b.1 - a.1;
            numerator.div_euclid(denominator) + i64::from(numerator.rem_euclid(denominator) != 0)
        };

        for y in y_start..y_end {
            let long_x = edge_x(top, bottom, y);
            let short_x = if y < middle.1 {
                edge_x(top, middle, y)
            } else {
                edge_x(middle, bottom, y)
            };
            let (left, right) = if long_on_left {
                (long_x, short_x)
            } else {
                (short_x, long_x)
            };
            let x_start = left.max(self.env.clip_left as i64);
            let x_end = right.min(self.env.clip_right as i64 + 1);
            if x_start >= x_end {
                continue;
            }
            let mut values = [0i64; 5];
            for k in 0..5 {
                values[k] = base[k] + dx[k] * (x_start - x0) + dy[k] * (y - y0);
            }
            for x in x_start..x_end {
                let color = [
                    (values[0] >> FRACTION) as i32,
                    (values[1] >> FRACTION) as i32,
                    (values[2] >> FRACTION) as i32,
                ];
                let uv = ((values[3] >> FRACTION) as u8, (values[4] >> FRACTION) as u8);
                self.plot(x as i32, y as i32, color, uv, &shading);
                for k in 0..5 {
                    values[k] += dx[k];
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sprite(
        &mut self,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        uv: (u8, u8),
        color: [u8; 3],
        shading: Shading,
    ) {
        let x0 = x as i32 + self.env.offset_x;
        let y0 = y as i32 + self.env.offset_y;
        let color = color.map(|c| c as i32);
        for row in 0..height as i32 {
            let py = y0 + row;
            if py < self.env.clip_top || py > self.env.clip_bottom {
                continue;
            }
            for column in 0..width as i32 {
                let px = x0 + column;
                if px < self.env.clip_left || px > self.env.clip_right {
                    continue;
                }
                let texel_uv = (
                    uv.0.wrapping_add(column as u8),
                    uv.1.wrapping_add(row as u8),
                );
                self.plot(px, py, color, texel_uv, &shading);
            }
        }
    }

    fn line(&mut self, vertices: [Vertex; 2], shading: Shading) {
        let [a, b] = vertices;
        let (ax, ay) = (
            a.x as i32 + self.env.offset_x,
            a.y as i32 + self.env.offset_y,
        );
        let (bx, by) = (
            b.x as i32 + self.env.offset_x,
            b.y as i32 + self.env.offset_y,
        );
        let (dx, dy) = (bx - ax, by - ay);
        if dx.abs() > 1023 || dy.abs() > 511 {
            return;
        }
        let steps = dx.abs().max(dy.abs()).max(1);
        for step in 0..=steps {
            let x = ax + (dx * step + steps / 2 * dx.signum()) / steps;
            let y = ay + (dy * step + steps / 2 * dy.signum()) / steps;
            if x < self.env.clip_left
                || x > self.env.clip_right
                || y < self.env.clip_top
                || y > self.env.clip_bottom
            {
                continue;
            }
            let color = if shading.gouraud {
                [0, 1, 2].map(|c| {
                    a.color[c] as i32 + (b.color[c] as i32 - a.color[c] as i32) * step / steps
                })
            } else {
                a.color.map(|c| c as i32)
            };
            self.plot(x, y, color, (0, 0), &shading);
        }
    }
}

pub fn draw(vram: &mut Vram, env: &DrawEnv, primitive: &Primitive) {
    let mut raster = Raster { vram, env };
    let dither_for = |gouraud: bool, texture: Option<Texture>| {
        env.dither && (gouraud || texture.is_some_and(|texture| !texture.raw))
    };
    match *primitive {
        Primitive::Triangle {
            vertices,
            gouraud,
            texture,
            blend,
        } => raster.triangle(
            vertices,
            Shading {
                gouraud,
                texture,
                blend,
                dither: dither_for(gouraud, texture),
            },
        ),
        Primitive::Quad {
            vertices,
            gouraud,
            texture,
            blend,
        } => {
            let shading = Shading {
                gouraud,
                texture,
                blend,
                dither: dither_for(gouraud, texture),
            };
            let flat = vertices[0].color;
            let recolor = |mut v: Vertex| {
                if !gouraud {
                    v.color = flat;
                }
                v
            };
            raster.triangle([vertices[0], vertices[1], vertices[2]], shading);
            raster.triangle(
                [
                    recolor(vertices[1]),
                    recolor(vertices[2]),
                    recolor(vertices[3]),
                ],
                shading,
            );
        }
        Primitive::Sprite {
            x,
            y,
            width,
            height,
            u,
            v,
            color,
            texture,
            blend,
        } => raster.sprite(
            x,
            y,
            width,
            height,
            (u, v),
            color,
            Shading {
                gouraud: false,
                texture,
                blend,
                dither: false,
            },
        ),
        Primitive::Line {
            vertices,
            gouraud,
            blend,
        } => raster.line(
            vertices,
            Shading {
                gouraud,
                texture: None,
                blend,
                dither: env.dither && gouraud,
            },
        ),
    }
}

pub fn draw_all<'a>(
    vram: &mut Vram,
    env: &DrawEnv,
    primitives: impl IntoIterator<Item = &'a Primitive>,
) {
    for primitive in primitives {
        draw(vram, env, primitive);
    }
}

#[cfg(test)]
mod tests;
