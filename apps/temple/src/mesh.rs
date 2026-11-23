use std::collections::BTreeMap;

use psxel_math::{Mat3Fx12, Vec3i16};
use psxel_vram::TextureWindow;

use crate::biome::Rgb;
use crate::lighting::Lighting;
use crate::render::{units, world_to_psx, Point, Renderer, Surface};
use crate::textures::Image;

pub type Vec3 = [f32; 3];

const CHUNK: f32 = 16.0;
const NEAR_DETAIL: f32 = 12.0;

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, factor: f32) -> Vec3 {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

pub fn multiply(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

pub fn normalize(a: Vec3) -> Vec3 {
    scale(a, 1.0 / length(a).max(1e-6))
}

pub fn to_rgb(color: Vec3) -> Rgb {
    color.map(|channel| channel.clamp(0.0, 255.0) as u8)
}

pub fn local(position: Vec3, origin: Vec3) -> Vec3i16 {
    Vec3i16::new(
        units(position[0] - origin[0]) as i16,
        -units(position[1] - origin[1]) as i16,
        units(position[2] - origin[2]) as i16,
    )
}

pub fn tiled(image: &Image) -> Image {
    let mut tiled = *image;
    let mask = 0x18;
    tiled.texture.window = Some(TextureWindow {
        mask_x: mask,
        mask_y: mask,
        offset_x: (image.u / 8) & mask,
        offset_y: (image.v / 8) & mask,
    });
    tiled
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zone {
    Exterior,
    Interior,
    Cave,
    Grotto,
}

#[derive(Clone, Copy)]
pub struct Material {
    pub surface: Surface,
    pub tint: Vec3,
    pub double_sided: bool,
    pub bias: i32,
}

impl Material {
    pub fn textured(image: &Image) -> Self {
        Self {
            surface: Surface::textured(tiled(image).texture),
            tint: [1.0; 3],
            double_sided: false,
            bias: 0,
        }
    }

    pub fn flat(color: Rgb) -> Self {
        Self {
            surface: Surface::gouraud(),
            tint: color.map(|channel| channel as f32 / 128.0),
            double_sided: false,
            bias: 0,
        }
    }

    pub fn tinted(mut self, tint: Vec3) -> Self {
        self.tint = multiply(self.tint, tint);
        self
    }

    pub fn biased(mut self, bias: i32) -> Self {
        self.bias = bias;
        self
    }

    pub fn double_sided(mut self) -> Self {
        self.double_sided = true;
        self
    }
}

#[derive(Clone, Copy)]
pub struct Cell {
    pub material: Material,
    pub panel: Option<Image>,
}

impl Cell {
    pub fn new(material: Material) -> Self {
        Self {
            material,
            panel: None,
        }
    }

    pub fn panel(image: &Image, tint: Vec3) -> Self {
        let mut material = Material::textured(image).tinted(tint);
        material.surface = Surface::textured(image.texture);
        Self {
            material,
            panel: Some(*image),
        }
    }

    pub fn biased(mut self, bias: i32) -> Self {
        self.material = self.material.biased(bias);
        self
    }

    fn uvs(&self, tiled: [(f32, f32); 4]) -> [(f32, f32); 4] {
        match self.panel {
            Some(image) => {
                let (u0, v0) = (image.u as f32, image.v as f32);
                let (u1, v1) = (
                    u0 + image.width as f32 - 1.0,
                    v0 + image.height as f32 - 1.0,
                );
                [(u0, v0), (u1, v0), (u0, v1), (u1, v1)]
            }
            None => tiled,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Corner {
    position: Vec3i16,
    uv: (u8, u8),
    base: Vec3,
    glow: Vec3,
    torch: usize,
}

struct Face {
    corners: [Corner; 4],
    count: usize,
    surface: Surface,
    double_sided: bool,
    bias: i32,
}

struct Chunk {
    origin: Vec3,
    radius: f32,
    zone: Zone,
    faces: Vec<Face>,
}

pub struct View {
    pub position: Vec3,
    pub forward: Vec3,
    pub right: (f32, f32),
    pub fog: Rgb,
    pub near: f32,
    pub far: f32,
    pub interior_bias: i32,
    pub time: f32,
}

impl View {
    pub fn visible(&self, center: Vec3, radius: f32) -> bool {
        let offset = sub(center, self.position);
        length(offset) - radius < self.far && dot(offset, self.forward) > -radius
    }

    pub fn fog_amount(&self, distance: f32) -> f32 {
        let near = 1.0 / self.near;
        let far = 1.0 / self.far;
        ((near - 1.0 / distance.max(0.01)) / (near - far)).clamp(0.0, 1.0)
    }
}

pub struct Mesh {
    chunks: Vec<Chunk>,
}

impl Mesh {
    pub fn draw(&self, renderer: &mut Renderer, view: &View, flicker: &[f32]) {
        for chunk in &self.chunks {
            if !view.visible(chunk.origin, chunk.radius) {
                continue;
            }
            renderer.set_object(
                world_to_psx(chunk.origin[0], chunk.origin[1], chunk.origin[2]),
                Mat3Fx12::IDENTITY,
            );
            let close = length(sub(chunk.origin, view.position)) - chunk.radius < NEAR_DETAIL;
            let detail = if chunk.zone == Zone::Exterior && !close {
                2
            } else {
                3
            };
            let bias = if chunk.zone != Zone::Exterior {
                view.interior_bias
            } else {
                0
            };
            for face in &chunk.faces {
                let mut points = [Point::default(); 4];
                for (point, corner) in points.iter_mut().zip(&face.corners[..face.count]) {
                    let strength = flicker.get(corner.torch).copied().unwrap_or(1.0);
                    let color = to_rgb(add(corner.base, scale(corner.glow, strength)));
                    *point = Point::new(corner.position, color, corner.uv);
                }
                let points = &points[..face.count];
                if !face.double_sided && renderer.backface(points) {
                    continue;
                }
                let surface = if face.surface.texture.is_some() {
                    face.surface.tinted(view.fog)
                } else {
                    face.surface
                };
                renderer.polygon(points, surface, bias + face.bias, detail);
            }
        }
    }
}

struct Pending {
    zone: Zone,
    positions: [Vec3; 4],
    uvs: [(f32, f32); 4],
    count: usize,
    material: Material,
}

pub struct Builder<'a> {
    lighting: &'a Lighting,
    pending: Vec<Pending>,
}

pub fn steps(from: f32, to: f32, step: f32) -> Vec<f32> {
    let count = ((to - from) / step).round().max(1.0) as usize;
    (0..=count)
        .map(|index| from + (to - from) * index as f32 / count as f32)
        .collect()
}

impl<'a> Builder<'a> {
    pub fn new(lighting: &'a Lighting) -> Self {
        Self {
            lighting,
            pending: Vec::new(),
        }
    }

    pub fn quad(
        &mut self,
        zone: Zone,
        positions: [Vec3; 4],
        uvs: [(f32, f32); 4],
        material: Material,
    ) {
        self.pending.push(Pending {
            zone,
            positions,
            uvs,
            count: 4,
            material,
        });
    }

    pub fn quad_facing(
        &mut self,
        zone: Zone,
        positions: [Vec3; 4],
        uvs: [(f32, f32); 4],
        material: Material,
        towards: Vec3,
    ) {
        let [a, b, c, d] = positions;
        let normal = cross(sub(b, a), sub(c, a));
        let centroid = scale(add(add(a, b), add(c, d)), 0.25);
        if dot(normal, sub(towards, centroid)) >= 0.0 {
            self.quad(zone, positions, uvs, material);
        } else {
            self.quad(
                zone,
                [b, a, d, c],
                [uvs[1], uvs[0], uvs[3], uvs[2]],
                material,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn wall(
        &mut self,
        zone: Zone,
        a: (f32, f32),
        b: (f32, f32),
        rows: &[f32],
        step: f32,
        density: f32,
        select: impl Fn(Vec3) -> Option<Cell>,
    ) {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let span = dx.hypot(dz);
        let count = (span / step).ceil().max(1.0) as usize;
        let at = |s: f32, y: f32| [a.0 + dx * s, y, a.1 + dz * s];
        for column in 0..count {
            let (s0, s1) = (
                column as f32 / count as f32,
                (column + 1) as f32 / count as f32,
            );
            for row in rows.windows(2) {
                let (y0, y1) = (row[0], row[1]);
                let Some(cell) = select(at((s0 + s1) * 0.5, (y0 + y1) * 0.5)) else {
                    continue;
                };
                let u0 = (s0 * span * density).rem_euclid(64.0);
                let v0 = (-y1 * density).rem_euclid(64.0);
                let (u1, v1) = (u0 + (s1 - s0) * span * density, v0 + (y1 - y0) * density);
                self.quad(
                    zone,
                    [at(s0, y1), at(s1, y1), at(s0, y0), at(s1, y0)],
                    cell.uvs([(u0, v0), (u1, v0), (u0, v1), (u1, v1)]),
                    cell.material,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn floor(
        &mut self,
        zone: Zone,
        xs: &[f32],
        zs: &[f32],
        height: impl Fn(f32, f32) -> f32,
        up: bool,
        density: f32,
        select: impl Fn(Vec3) -> Option<Cell>,
    ) {
        for zw in zs.windows(2) {
            for xw in xs.windows(2) {
                let (x0, x1, z0, z1) = (xw[0], xw[1], zw[0], zw[1]);
                let corner = |x: f32, z: f32| [x, height(x, z), z];
                let heights = [
                    corner(x0, z0),
                    corner(x1, z0),
                    corner(x0, z1),
                    corner(x1, z1),
                ];
                let middle = [
                    (x0 + x1) * 0.5,
                    heights.iter().map(|p| p[1]).sum::<f32>() * 0.25,
                    (z0 + z1) * 0.5,
                ];
                let Some(cell) = select(middle) else {
                    continue;
                };
                let u0 = (x0 * density).rem_euclid(64.0);
                let u1 = u0 + (x1 - x0) * density;
                if up {
                    let v0 = (-z1 * density).rem_euclid(64.0);
                    let v1 = v0 + (z1 - z0) * density;
                    self.quad(
                        zone,
                        [
                            corner(x0, z1),
                            corner(x1, z1),
                            corner(x0, z0),
                            corner(x1, z0),
                        ],
                        cell.uvs([(u0, v0), (u1, v0), (u0, v1), (u1, v1)]),
                        cell.material,
                    );
                } else {
                    let v0 = (z0 * density).rem_euclid(64.0);
                    let v1 = v0 + (z1 - z0) * density;
                    self.quad(
                        zone,
                        [
                            corner(x0, z0),
                            corner(x1, z0),
                            corner(x0, z1),
                            corner(x1, z1),
                        ],
                        cell.uvs([(u0, v0), (u1, v0), (u0, v1), (u1, v1)]),
                        cell.material,
                    );
                }
            }
        }
    }

    pub fn block(&mut self, zone: Zone, min: Vec3, max: Vec3, density: f32, material: Material) {
        let rows = steps(min[1], max[1], 2.5);
        let corners = [
            (min[0], min[2]),
            (max[0], min[2]),
            (max[0], max[2]),
            (min[0], max[2]),
        ];
        for side in 0..4 {
            self.wall(
                zone,
                corners[side],
                corners[(side + 1) % 4],
                &rows,
                2.0,
                density,
                |_| Some(Cell::new(material)),
            );
        }
        self.floor(
            zone,
            &steps(min[0], max[0], 2.0),
            &steps(min[2], max[2], 2.0),
            |_, _| max[1],
            true,
            density,
            |_| Some(Cell::new(material)),
        );
    }

    pub fn dressed_block(
        &mut self,
        zone: Zone,
        min: Vec3,
        max: Vec3,
        density: f32,
        sides: [Cell; 4],
        top: Cell,
    ) {
        let rows = steps(min[1], max[1], 2.5);
        let corners = [
            (min[0], min[2]),
            (max[0], min[2]),
            (max[0], max[2]),
            (min[0], max[2]),
        ];
        for (side, cell) in sides.into_iter().enumerate() {
            self.wall(
                zone,
                corners[side],
                corners[(side + 1) % 4],
                &rows,
                4.0,
                density,
                |_| Some(cell),
            );
        }
        self.floor(
            zone,
            &[min[0], max[0]],
            &[min[2], max[2]],
            |_, _| max[1],
            true,
            density,
            |_| Some(top),
        );
    }

    pub fn build(self) -> Mesh {
        let lighting = self.lighting;
        let mut groups: BTreeMap<(Zone, i32, i32, i32), Vec<Pending>> = BTreeMap::new();
        for pending in self.pending {
            let centroid = scale(
                pending.positions[..pending.count]
                    .iter()
                    .fold([0.0; 3], |sum, &position| add(sum, position)),
                1.0 / pending.count as f32,
            );
            let key = (
                pending.zone,
                (centroid[0] / CHUNK).floor() as i32,
                (centroid[1] / CHUNK).floor() as i32,
                (centroid[2] / CHUNK).floor() as i32,
            );
            groups.entry(key).or_default().push(pending);
        }
        let chunks = groups
            .into_iter()
            .map(|((zone, ..), list)| {
                let positions: Vec<Vec3> = list
                    .iter()
                    .flat_map(|pending| pending.positions[..pending.count].to_vec())
                    .collect();
                let origin = scale(
                    positions
                        .iter()
                        .fold([0.0; 3], |sum, &position| add(sum, position)),
                    1.0 / positions.len() as f32,
                );
                let radius = positions
                    .iter()
                    .map(|&position| length(sub(position, origin)))
                    .fold(0.0, f32::max);
                let faces = list
                    .iter()
                    .map(|pending| lit_face(lighting, pending, origin))
                    .collect();
                Chunk {
                    origin,
                    radius,
                    zone,
                    faces,
                }
            })
            .collect();
        Mesh { chunks }
    }
}

fn lit_face(lighting: &Lighting, pending: &Pending, origin: Vec3) -> Face {
    let positions = &pending.positions;
    let normal = normalize(cross(
        sub(positions[1], positions[0]),
        sub(positions[2], positions[0]),
    ));
    let tint = pending.material.tint;
    let mut corners = [Corner::default(); 4];
    for index in 0..pending.count {
        let shade = lighting.shade(positions[index], normal, pending.zone);
        let (u, v) = pending.uvs[index];
        corners[index] = Corner {
            position: local(positions[index], origin),
            uv: (
                u.round().clamp(0.0, 255.0) as u8,
                v.round().clamp(0.0, 255.0) as u8,
            ),
            base: multiply(shade.base, tint),
            glow: multiply(shade.glow, tint),
            torch: shade.torch,
        };
    }
    Face {
        corners,
        count: pending.count,
        surface: pending.material.surface,
        double_sided: pending.material.double_sided,
        bias: pending.material.bias,
    }
}
