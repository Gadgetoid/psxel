use std::sync::atomic::{AtomicU32, Ordering};

use psxel_math::{Gte, Mat3Fx12, Projected, Vec3i16, Vec3i32, FULL_TURN};
use psxel_prim::{exceeds_gpu_limits, BlendMode, OrderingTable, Primitive, Texture, Vertex};

use crate::biome::Rgb;

pub const UNITS_PER_METER: f32 = 64.0;
pub const SCREEN_WIDTH: i32 = 320;
pub const SCREEN_HEIGHT: i32 = 240;
pub const PROJECTION_H: u16 = 210;
pub const NEAR_Z: u16 = 40;
pub const SUBDIVIDE_Z: u16 = 1400;
pub const OT_SIZE: usize = 1024;
pub const OT_SHIFT: u32 = 4;
pub const FOG_NEAR: f32 = 25.0;
pub const FOG_FAR: f32 = 260.0;

static FOG_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);

pub fn set_fog_scale(scale: f32) {
    FOG_SCALE.store(scale.to_bits(), Ordering::Relaxed);
}

pub fn fog_scale() -> f32 {
    f32::from_bits(FOG_SCALE.load(Ordering::Relaxed))
}

pub fn fog_amount(distance: f32) -> f32 {
    let scale = fog_scale();
    let near = 1.0 / (FOG_NEAR * scale);
    let far = 1.0 / (FOG_FAR * scale);
    ((near - 1.0 / distance.max(0.01)) / (near - far)).clamp(0.0, 1.0)
}

pub fn angle(radians: f32) -> i32 {
    (radians * FULL_TURN as f32 / std::f32::consts::TAU).round() as i32
}

pub fn units(meters: f32) -> i32 {
    (meters * UNITS_PER_METER).round() as i32
}

pub fn world_to_psx(x: f32, y: f32, z: f32) -> Vec3i32 {
    Vec3i32::new(units(x), -units(y), units(z))
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

#[derive(Clone, Copy)]
pub struct Surface {
    pub texture: Option<Texture>,
    pub blend: Option<BlendMode>,
    pub gouraud: bool,
    pub fog_tint: Rgb,
}

impl Surface {
    pub fn flat() -> Self {
        Self {
            texture: None,
            blend: None,
            gouraud: false,
            fog_tint: [128; 3],
        }
    }

    pub fn gouraud() -> Self {
        Self {
            texture: None,
            blend: None,
            gouraud: true,
            fog_tint: [128; 3],
        }
    }

    pub fn textured(texture: Texture) -> Self {
        Self {
            texture: Some(texture),
            blend: None,
            gouraud: true,
            fog_tint: [128; 3],
        }
    }

    pub fn tinted(mut self, fog_tint: Rgb) -> Self {
        self.fog_tint = fog_tint;
        self
    }

    pub fn blended(mut self, mode: BlendMode) -> Self {
        self.blend = Some(mode);
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub position: Vec3i16,
    pub color: Rgb,
    pub uv: (u8, u8),
}

impl Point {
    pub fn new(position: Vec3i16, color: Rgb, uv: (u8, u8)) -> Self {
        Self {
            position,
            color,
            uv,
        }
    }

    fn midpoint(a: &Point, b: &Point) -> Point {
        let mid = |p: i16, q: i16| ((p as i32 + q as i32) >> 1) as i16;
        let mid8 = |p: u8, q: u8| ((p as u16 + q as u16) >> 1) as u8;
        Point {
            position: Vec3i16::new(
                mid(a.position.x, b.position.x),
                mid(a.position.y, b.position.y),
                mid(a.position.z, b.position.z),
            ),
            color: [0, 1, 2].map(|c| mid8(a.color[c], b.color[c])),
            uv: (mid8(a.uv.0, b.uv.0), mid8(a.uv.1, b.uv.1)),
        }
    }
}

pub struct Renderer {
    pub gte: Gte,
    pub ot: OrderingTable,
    pub overlay: Vec<Primitive>,
    pub camera_rotation: Mat3Fx12,
    camera_position: Vec3i32,
    pub fog: Rgb,
    pub fog_enabled: bool,
    fog_scale: f32,
    pub force_bucket: Option<usize>,
    pub force_otz: Option<u32>,
    pub polygons: usize,
}

impl Renderer {
    pub fn new() -> Self {
        let mut gte = Gte {
            ofx: (SCREEN_WIDTH / 2) << 16,
            ofy: (SCREEN_HEIGHT / 2) << 16,
            h: PROJECTION_H,
            zsf3: (4096 / 3) as i16,
            zsf4: (4096 / 4) as i16,
            ..Default::default()
        };
        set_depth_cue(&mut gte, 1.0);
        Self {
            gte,
            ot: OrderingTable::new(OT_SIZE, OT_SHIFT),
            overlay: Vec::new(),
            camera_rotation: Mat3Fx12::IDENTITY,
            camera_position: Vec3i32::default(),
            fog: [128, 128, 128],
            fog_enabled: true,
            fog_scale: 1.0,
            force_bucket: None,
            force_otz: None,
            polygons: 0,
        }
    }

    pub fn begin(&mut self, camera: &Camera, fog: Rgb) {
        self.ot.clear();
        self.overlay.clear();
        self.polygons = 0;
        self.fog = fog;
        let scale = fog_scale();
        if scale != self.fog_scale {
            self.fog_scale = scale;
            set_depth_cue(&mut self.gte, scale);
        }
        self.camera_rotation = Mat3Fx12::rotation_z(angle(camera.roll))
            * Mat3Fx12::rotation_x(angle(camera.pitch))
            * Mat3Fx12::rotation_y(angle(-camera.yaw));
        self.camera_position = world_to_psx(camera.x, camera.y, camera.z);
        self.gte.far_color = Vec3i32::new(
            (fog[0] as i32) << 4,
            (fog[1] as i32) << 4,
            (fog[2] as i32) << 4,
        );
    }

    pub fn set_object(&mut self, origin: Vec3i32, rotation: Mat3Fx12) {
        let relative = Vec3i32::new(
            origin.x - self.camera_position.x,
            origin.y - self.camera_position.y,
            origin.z - self.camera_position.z,
        );
        self.gte.translation = self.camera_rotation.apply(relative);
        self.gte.rotation = self.camera_rotation * rotation;
    }

    pub fn set_lighting(
        &mut self,
        rotation: Mat3Fx12,
        sun_direction: [f32; 3],
        sun: Rgb,
        ambient: Rgb,
    ) {
        let to_sun = rotation.transpose().apply(Vec3i32::new(
            (sun_direction[0] * 4096.0) as i32,
            (-sun_direction[1] * 4096.0) as i32,
            (sun_direction[2] * 4096.0) as i32,
        ));
        let channel = |value: u8| (value as i32 * 4096 / 255) as i16;
        self.gte.light = Mat3Fx12([
            [to_sun.x as i16, to_sun.y as i16, to_sun.z as i16],
            [0; 3],
            [0; 3],
        ]);
        self.gte.light_color = Mat3Fx12([
            [channel(sun[0]), 0, 0],
            [channel(sun[1]), 0, 0],
            [channel(sun[2]), 0, 0],
        ]);
        self.gte.background_color = Vec3i32::new(
            channel(ambient[0]) as i32,
            channel(ambient[1]) as i32,
            channel(ambient[2]) as i32,
        );
    }

    pub fn set_sky_object(&mut self, rotation: Mat3Fx12) {
        self.gte.translation = Vec3i32::default();
        self.gte.rotation = self.camera_rotation * rotation;
    }

    fn fogged(&mut self, color: Rgb, sz: u16, surface: &Surface) -> [u8; 3] {
        if !self.fog_enabled {
            return color;
        }
        self.gte.depth_cue(sz);
        if surface.texture.is_some() {
            let ir0 = self.gte.ir[0] as i32;
            return [0, 1, 2].map(|c| {
                let from = color[c] as i32;
                (from + (((surface.fog_tint[c] as i32 - from) * ir0) >> 12)).clamp(0, 255) as u8
            });
        }
        self.gte.dpcs(color)
    }

    fn project(&mut self, point: &Point) -> Projected {
        self.gte.rtps(point.position)
    }

    pub fn polygon(&mut self, points: &[Point], surface: Surface, depth_bias: i32, subdivide: u8) {
        let projected: Vec<Projected> = points.iter().map(|point| self.project(point)).collect();
        let min_z = projected.iter().map(|p| p.sz).min().unwrap_or(0);
        let max_z = projected.iter().map(|p| p.sz).max().unwrap_or(0);
        if max_z < NEAR_Z {
            return;
        }
        if subdivide > 0 && min_z < SUBDIVIDE_Z && points.len() == 4 {
            let [a, b, c, d] = [&points[0], &points[1], &points[2], &points[3]];
            let ab = Point::midpoint(a, b);
            let cd = Point::midpoint(c, d);
            let ac = Point::midpoint(a, c);
            let bd = Point::midpoint(b, d);
            let center = Point::midpoint(&ab, &cd);
            for quad in [
                [*a, ab, ac, center],
                [ab, *b, center, bd],
                [ac, center, *c, cd],
                [center, bd, cd, *d],
            ] {
                self.polygon(&quad, surface, depth_bias, subdivide - 1);
            }
            return;
        }
        if min_z < NEAR_Z {
            return;
        }
        let off_screen = projected.iter().all(|p| p.sx < 0)
            || projected.iter().all(|p| p.sx >= SCREEN_WIDTH as i16)
            || projected.iter().all(|p| p.sy < 0)
            || projected.iter().all(|p| p.sy >= SCREEN_HEIGHT as i16);
        if off_screen {
            return;
        }
        let mut vertices = [Vertex::default(); 4];
        for (index, (point, screen)) in points.iter().zip(&projected).enumerate() {
            let color = if surface.gouraud || index == 0 {
                self.fogged(point.color, screen.sz, &surface)
            } else {
                vertices[0].color
            };
            vertices[index] = Vertex {
                x: screen.sx,
                y: screen.sy,
                color,
                u: point.uv.0,
                v: point.uv.1,
            };
        }
        let vertices = &vertices[..points.len()];
        if exceeds_gpu_limits(vertices) {
            return;
        }
        let average: u32 =
            projected.iter().map(|p| p.sz as u32).sum::<u32>() / projected.len() as u32;
        let otz = match self.force_otz {
            Some(otz) => (otz as i32 + depth_bias).max(0) as u32,
            None => (average as i32 + depth_bias).max(0) as u32,
        };
        let primitive = if points.len() == 4 {
            Primitive::Quad {
                vertices: [vertices[0], vertices[1], vertices[2], vertices[3]],
                gouraud: surface.gouraud,
                texture: surface.texture,
                blend: surface.blend,
            }
        } else {
            Primitive::Triangle {
                vertices: [vertices[0], vertices[1], vertices[2]],
                gouraud: surface.gouraud,
                texture: surface.texture,
                blend: surface.blend,
            }
        };
        self.polygons += 1;
        match self.force_bucket {
            Some(bucket) => self.ot.add_at(bucket, primitive),
            None => self.ot.add(otz, primitive),
        }
    }

    pub fn depth_of(&mut self, local: Vec3i16) -> u16 {
        self.gte.rtps(local).sz
    }

    pub fn backface(&mut self, points: &[Point]) -> bool {
        let [a, b, c] = [&points[0], &points[1], &points[2]].map(|p| self.project(p));
        let area = (b.sx as i32 - a.sx as i32) * (c.sy as i32 - a.sy as i32)
            - (c.sx as i32 - a.sx as i32) * (b.sy as i32 - a.sy as i32);
        area <= 0
    }
}

pub fn screen_quad(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    colors: [Rgb; 4],
    blend: Option<BlendMode>,
) -> Primitive {
    Primitive::Quad {
        vertices: [
            Vertex::new(x, y).with_color(colors[0]),
            Vertex::new(x + width, y).with_color(colors[1]),
            Vertex::new(x, y + height).with_color(colors[2]),
            Vertex::new(x + width, y + height).with_color(colors[3]),
        ],
        gouraud: true,
        texture: None,
        blend,
    }
}

fn set_depth_cue(gte: &mut Gte, scale: f32) {
    let divisor = |meters: f32| {
        PROJECTION_H as f64 * 65536.0 / (meters as f64 * scale as f64 * UNITS_PER_METER as f64)
    };
    let (near, far) = (divisor(FOG_NEAR), divisor(FOG_FAR));
    let dqa = (4096.0 * 4096.0 / (far - near)).round();
    gte.dqa = dqa as i16;
    gte.dqb = (-dqa * near) as i32;
}
