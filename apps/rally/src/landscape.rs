use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use psxel_math::{Mat3Fx12, Vec3i16};
use psxel_vram::Vram;

use crate::art::{tiled, Art};
use crate::biome::{lerp_rgb, Prop, Rgb, BIOMES};
use crate::blend::{Choice, Kind};
use crate::render::{fog_amount, units, world_to_psx, Camera, Point, Renderer, Surface};
use crate::rng::{hash, hash_float, Rng};
use crate::sprites::band_image;
use crate::textures::{fog_band, Atlas, Image};
use crate::track::Track;
use crate::world::{fog_tint_from, Lighting};

type TextureKey = (u8, u8, u8, u8, u16);
pub type BoulderDraw<'a> = dyn Fn(&mut Renderer, &Image, [f32; 3], f32, u64) + 'a;

pub const LATTICE: f32 = 8.0;
const NEAR_RADIUS: f32 = 112.0;
const FAR_RADIUS: f32 = 256.0;
const PROP_RADIUS: f32 = 210.0;
const GROUND_BIAS: i32 = 200;
const UNDER_ROAD: f32 = 0.15;
const SLOT_DEPTH: f32 = 0.3;
const CACHE_LIMIT: usize = 120_000;
const DETAIL: i32 = 4;
const FINE: f32 = LATTICE / DETAIL as f32;
const DETAIL_RADIUS: f32 = 72.0;
const DETAIL_ERROR: f32 = 0.3;
const DETAIL_REACH: f32 = 24.0;
const STEEP: f32 = 0.75;
const DETAIL_BUDGET: usize = 24;

#[derive(Clone, Copy)]
struct Vertex {
    height: f32,
    road_index: u32,
    road_distance: f32,
    slot: bool,
}

#[derive(Default)]
pub struct Landscape {
    cache: RefCell<HashMap<(i32, i32), Vertex>>,
    detail: RefCell<HashMap<(i32, i32), i32>>,
    focus: Cell<(f32, f32)>,
    budget: Cell<usize>,
    generated: RefCell<usize>,
}

struct View {
    x: f32,
    z: f32,
    forward: (f32, f32),
}

impl View {
    fn new(camera: &Camera) -> Self {
        Self {
            x: camera.x,
            z: camera.z,
            forward: (camera.yaw.sin(), camera.yaw.cos()),
        }
    }

    fn visible(&self, x: f32, z: f32, margin: f32, radius: f32) -> bool {
        let (dx, dz) = (x - self.x, z - self.z);
        let distance = (dx * dx + dz * dz).sqrt();
        if distance > radius + margin {
            return false;
        }
        let ahead = dx * self.forward.0 + dz * self.forward.1;
        let side = dx * self.forward.1 - dz * self.forward.0;
        ahead > -margin && side.abs() < ahead.max(0.0) * 1.15 + margin * 1.5
    }
}

impl Landscape {
    fn coarse(&self, track: &Track, gx: i32, gz: i32) -> Vertex {
        self.vertex(track, gx * DETAIL, gz * DETAIL)
    }

    fn vertex(&self, track: &Track, fx: i32, fz: i32) -> Vertex {
        if let Some(vertex) = self.cache.borrow().get(&(fx, fz)) {
            return *vertex;
        }
        let (x, z) = (fx as f32 * FINE, fz as f32 * FINE);
        let terrain = track.drawn_terrain(x, z);
        let (road_index, road_distance) = terrain
            .road
            .map(|road| (road.index as u32, road.distance))
            .unwrap_or((u32::MAX, f32::MAX));
        let mut height = terrain.height;
        if let Some(road) = terrain.road.filter(|_| !track.tunnel_hill(x, z)) {
            let half = track.half_width(road.index);
            if road.distance < half + LATTICE {
                height -=
                    UNDER_ROAD * (1.0 - (road.distance - half).max(0.0) / LATTICE).clamp(0.0, 1.0);
            }
        }
        let slot = track.tunnel_slot(x, z);
        if let Some(road) = terrain.road.filter(|_| slot) {
            height = height.min(track.ground(road.index, road.t, 0.0) - SLOT_DEPTH);
        }
        let vertex = Vertex {
            height,
            road_index,
            road_distance,
            slot,
        };
        let mut cache = self.cache.borrow_mut();
        if cache.len() > CACHE_LIMIT {
            cache.clear();
            self.detail.borrow_mut().clear();
        }
        cache.insert((fx, fz), vertex);
        vertex
    }

    fn detail(&self, track: &Track, gx: i32, gz: i32) -> i32 {
        if let Some(&detail) = self.detail.borrow().get(&(gx, gz)) {
            return detail;
        }
        let (fx, fz) = self.focus.get();
        let centre = ((gx as f32 + 0.5) * LATTICE, (gz as f32 + 0.5) * LATTICE);
        if (centre.0 - fx).hypot(centre.1 - fz) > DETAIL_RADIUS + LATTICE {
            return 1;
        }
        let at_portal = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .any(|&(dx, dz)| self.coarse(track, gx + dx, gz + dz).slot);
        if !at_portal {
            if self.budget.get() == 0 {
                return 1;
            }
            self.budget.set(self.budget.get() - 1);
        }
        let detail = self.measure_detail(track, gx, gz);
        self.detail.borrow_mut().insert((gx, gz), detail);
        detail
    }

    fn measure_detail(&self, track: &Track, gx: i32, gz: i32) -> i32 {
        let near_road = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .any(|&(dx, dz)| self.coarse(track, gx + dx, gz + dz).road_distance < DETAIL_REACH);
        let centre = ((gx as f32 + 0.5) * LATTICE, (gz as f32 + 0.5) * LATTICE);
        if !near_road && !track.tunnel_hill(centre.0, centre.1) {
            return 1;
        }
        let fine = |i: i32, j: i32| self.vertex(track, gx * DETAIL + i, gz * DETAIL + j).height;
        let error = |cells: i32| {
            let step = DETAIL / cells;
            let mut worst = 0.0f32;
            for j in 0..=DETAIL {
                for i in 0..=DETAIL {
                    let (ci, cj) = ((i / step).min(cells - 1), (j / step).min(cells - 1));
                    let tx = (i - ci * step) as f32 / step as f32;
                    let tz = (j - cj * step) as f32 / step as f32;
                    let corner = |a: i32, b: i32| fine((ci + a) * step, (cj + b) * step);
                    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
                    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
                    worst = worst.max((fine(i, j) - (top + (bottom - top) * tz)).abs());
                }
            }
            worst
        };
        if error(1) <= DETAIL_ERROR {
            1
        } else if error(2) <= DETAIL_ERROR {
            2
        } else {
            DETAIL
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stitched(
        &self,
        track: &Track,
        gx: i32,
        gz: i32,
        cells: i32,
        neighbours: [i32; 4],
        i: i32,
        j: i32,
    ) -> f32 {
        let step = DETAIL / cells;
        let (fi, fj) = (i * step, j * step);
        let fine = |a: i32, b: i32| self.vertex(track, gx * DETAIL + a, gz * DETAIL + b).height;
        let along = |across_x: bool, fixed: i32, moving: i32, coarser: i32| {
            let span = DETAIL / coarser;
            let first = moving / span * span;
            if first == moving {
                return None;
            }
            let t = (moving - first) as f32 / span as f32;
            let (a, b) = if across_x {
                (fine(fixed, first), fine(fixed, first + span))
            } else {
                (fine(first, fixed), fine(first + span, fixed))
            };
            Some(a + (b - a) * t)
        };
        let [left, right, down, up] = neighbours;
        let snapped = [
            (fi == 0 && left < cells).then(|| along(true, 0, fj, left)),
            (fi == DETAIL && right < cells).then(|| along(true, DETAIL, fj, right)),
            (fj == 0 && down < cells).then(|| along(false, 0, fi, down)),
            (fj == DETAIL && up < cells).then(|| along(false, DETAIL, fi, up)),
        ];
        snapped
            .into_iter()
            .flatten()
            .flatten()
            .next()
            .unwrap_or_else(|| fine(fi, fj))
    }

    fn refresh(&self, track: &Track) {
        let processed = *self.generated.borrow();
        let regions = &track.dirty[processed.min(track.dirty.len())..];
        if regions.is_empty() {
            return;
        }
        *self.generated.borrow_mut() = track.dirty.len();
        let inside = |x: f32, z: f32| {
            regions.iter().any(|&(min_x, min_z, max_x, max_z)| {
                x >= min_x - LATTICE
                    && x <= max_x + LATTICE
                    && z >= min_z - LATTICE
                    && z <= max_z + LATTICE
            })
        };
        self.cache
            .borrow_mut()
            .retain(|&(fx, fz), _| !inside(fx as f32 * FINE, fz as f32 * FINE));
        self.detail
            .borrow_mut()
            .retain(|&(gx, gz), _| !inside(gx as f32 * LATTICE, gz as f32 * LATTICE));
    }

    pub fn height_at(&self, track: &Track, x: f32, z: f32) -> f32 {
        let (fx, fz) = (x / LATTICE, z / LATTICE);
        let (gx, gz) = (fx.floor() as i32, fz.floor() as i32);
        let cells = self.detail(track, gx, gz);
        let neighbours = if cells > 1 {
            [(-1, 0), (1, 0), (0, -1), (0, 1)].map(|(dx, dz)| self.detail(track, gx + dx, gz + dz))
        } else {
            [1; 4]
        };
        let (fx, fz) = (
            (fx - gx as f32) * cells as f32,
            (fz - gz as f32) * cells as f32,
        );
        let (ci, cj) = (
            (fx.floor() as i32).clamp(0, cells - 1),
            (fz.floor() as i32).clamp(0, cells - 1),
        );
        let (tx, tz) = (fx - ci as f32, fz - cj as f32);
        let h =
            |dx: i32, dz: i32| self.stitched(track, gx, gz, cells, neighbours, ci + dx, cj + dz);
        let top = h(0, 0) + (h(1, 0) - h(0, 0)) * tx;
        let bottom = h(0, 1) + (h(1, 1) - h(0, 1)) * tx;
        top + (bottom - top) * tz
    }

    pub fn slope_at(&self, track: &Track, x: f32, z: f32) -> f32 {
        let dx = self.height_at(track, x + 1.0, z) - self.height_at(track, x - 1.0, z);
        let dz = self.height_at(track, x, z + 1.0) - self.height_at(track, x, z - 1.0);
        dx.hypot(dz) * 0.5
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        vram: &Vram,
        atlas: &Atlas,
        art: &Art,
        track: &Track,
        camera: &Camera,
        lighting: Lighting,
    ) {
        self.refresh(track);
        self.focus.set((camera.x, camera.z));
        self.budget.set(DETAIL_BUDGET);
        let view = View::new(camera);
        let mut averages: Vec<(TextureKey, Rgb)> = Vec::new();
        let mut average_of = |image: &Image| -> Rgb {
            let key = (
                image.texture.page.x,
                image.texture.page.y,
                image.u,
                image.v,
                image.texture.clut.x,
            );
            if let Some((_, average)) = averages.iter().find(|(entry, _)| *entry == key) {
                return *average;
            }
            let average = atlas.average(vram, image);
            averages.push((key, average));
            average
        };
        let sun = lighting.sun_direction;
        let mut cells: Vec<(i32, i32, i32, f32)> = Vec::new();
        for (step, radius, inner) in [(1i32, NEAR_RADIUS, 0.0f32), (2, FAR_RADIUS, NEAR_RADIUS)] {
            let size = LATTICE * step as f32;
            let reach = (radius / size).ceil() as i32 + 1;
            let (cx, cz) = (
                (camera.x / size).floor() as i32,
                (camera.z / size).floor() as i32,
            );
            for iz in cz - reach..=cz + reach {
                for ix in cx - reach..=cx + reach {
                    let (mx, mz) = ((ix as f32 + 0.5) * size, (iz as f32 + 0.5) * size);
                    let centre_distance =
                        ((mx - camera.x).powi(2) + (mz - camera.z).powi(2)).sqrt();
                    if centre_distance < inner || !view.visible(mx, mz, size, radius) {
                        continue;
                    }
                    let (gx, gz) = (ix * step, iz * step);
                    let near_road = step > 1
                        && [(0, 0), (step, 0), (0, step), (step, step)]
                            .iter()
                            .any(|&(dx, dz)| {
                                self.coarse(track, gx + dx, gz + dz).road_distance < size + 8.0
                            });
                    if near_road {
                        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            cells.push((gx + dx, gz + dz, 1, centre_distance));
                        }
                    } else {
                        cells.push((gx, gz, step, centre_distance));
                    }
                }
            }
        }
        let drawn: HashMap<(i32, i32), i32> = cells
            .iter()
            .filter(|&&(_, _, step, centre_distance)| step == 1 && centre_distance < DETAIL_RADIUS)
            .map(|&(gx, gz, _, _)| ((gx, gz), self.detail(track, gx, gz)))
            .collect();
        let drawn_detail = |gx: i32, gz: i32| drawn.get(&(gx, gz)).copied().unwrap_or(1);
        for &(gx, gz, step, centre_distance) in &cells {
            let size = LATTICE * step as f32;
            let (ix, iz) = (gx / step, gz / step);
            let (x0, z0) = (gx as f32 * LATTICE, gz as f32 * LATTICE);
            let corners = [(0, 0), (step, 0), (0, step), (step, step)]
                .map(|(dx, dz)| self.coarse(track, gx + dx, gz + dz));
            let road_index = corners
                .iter()
                .map(|vertex| vertex.road_index)
                .find(|&index| index != u32::MAX)
                .unwrap_or(0) as usize;
            let distance = Track::distance(road_index);
            let (a, b, t) = track.biomes.at(distance);
            let textured = centre_distance < NEAR_RADIUS;
            let key = hash_float(track.seed ^ 0xb1e7, ix as u64, iz as u64);
            let cells_across = if step == 1 { drawn_detail(gx, gz) } else { 1 };
            let neighbours =
                [(-1, 0), (1, 0), (0, -1), (0, 1)].map(|(dx, dz)| drawn_detail(gx + dx, gz + dz));
            let sub = size / cells_across as f32;
            for j in 0..cells_across {
                for i in 0..cells_across {
                    let heights = if cells_across == 1 {
                        corners.map(|vertex| vertex.height)
                    } else {
                        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(di, dj)| {
                            self.stitched(track, gx, gz, cells_across, neighbours, i + di, j + dj)
                        })
                    };
                    let buried = if cells_across == 1 {
                        corners.iter().any(|vertex| vertex.slot)
                    } else {
                        let step = DETAIL / cells_across;
                        [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(di, dj)| {
                            self.vertex(
                                track,
                                gx * DETAIL + (i + di) * step,
                                gz * DETAIL + (j + dj) * step,
                            )
                            .slot
                        })
                    };
                    if buried {
                        continue;
                    }
                    let (qx, qz) = (x0 + i as f32 * sub, z0 + j as f32 * sub);
                    let low = heights.iter().copied().fold(f32::MAX, f32::min);
                    let high = heights.iter().copied().fold(f32::MIN, f32::max);
                    let steep = (high - low) / sub > STEEP;
                    let image_of = |biome: &crate::biome::Biome| {
                        if steep {
                            (
                                art.cliff(biome.ground, hash(track.seed, ix as u64, iz as u64)),
                                true,
                            )
                        } else if let Some(ground) = art.ground(biome.ground) {
                            (tiled(ground), true)
                        } else {
                            (atlas.ground, false)
                        }
                    };
                    let (image_a, windowed_a) = image_of(&BIOMES[a]);
                    let (image_b, windowed_b) = image_of(&BIOMES[b]);
                    let average_a = average_of(&image_a);
                    let average_b = average_of(&image_b);
                    let target = lerp_rgb(average_a, average_b, t);
                    let kind = if steep { Kind::Cliff } else { Kind::Ground };
                    let riverbed = !steep && track.riverbed(qx + sub * 0.5, qz + sub * 0.5);
                    let (image, windowed, average) = match art.blends.choose(kind, (a, b), t, key) {
                        Choice::First => (image_a, windowed_a, average_a),
                        Choice::Second => (image_b, windowed_b, average_b),
                        Choice::Mix(mix) => {
                            let mix = tiled(&mix);
                            (mix, true, average_of(&mix))
                        }
                    };
                    let (image, windowed, average) = if riverbed {
                        let bed = tiled(&art.riverbed);
                        (bed, true, average_of(&bed))
                    } else {
                        (image, windowed, average)
                    };
                    let slope_x =
                        ((heights[1] + heights[3]) - (heights[0] + heights[2])) * 0.5 / sub;
                    let slope_z =
                        ((heights[2] + heights[3]) - (heights[0] + heights[1])) * 0.5 / sub;
                    let length = (slope_x * slope_x + slope_z * slope_z + 1.0).sqrt();
                    let facing = (-slope_x * sun[0] + sun[1] - slope_z * sun[2]) / length;
                    let shade = (0.45 + facing.max(0.0) * 0.75).clamp(0.3, 1.15);
                    let lit = [0, 1, 2].map(|c| {
                        let ambient = lighting.ambient[c] as f32 / 255.0;
                        let direct = lighting.sun[c] as f32 / 255.0 * shade;
                        ((ambient + direct) * 128.0).clamp(0.0, 255.0) as u8
                    });
                    let color = if textured {
                        [0, 1, 2].map(|c| {
                            let pull = (target[c] as f32 + 4.0) / (average[c] as f32 + 4.0);
                            (lit[c] as f32 * pull.clamp(0.5, 2.0)).min(255.0) as u8
                        })
                    } else {
                        [0, 1, 2].map(|c| ((target[c] as u32 * lit[c] as u32) / 128).min(255) as u8)
                    };
                    let origin = world_to_psx(qx, low, qz);
                    renderer.set_object(origin, Mat3Fx12::IDENTITY);
                    let (u_base, u_span, v_base, v_span) = if windowed {
                        (
                            0u32,
                            (size * 8.0).min(256.0) as u32,
                            0u32,
                            (size * 8.0).min(256.0) as u32,
                        )
                    } else {
                        (
                            image.u as u32,
                            image.width as u32,
                            image.v as u32,
                            image.height as u32,
                        )
                    };
                    let across = cells_across as u32;
                    let (i, j) = (i as u32, j as u32);
                    let u0 = (u_base + u_span * i / across) as u8;
                    let u1 = (u_base + u_span * (i + 1) / across - 1) as u8;
                    let v0 = (v_base + v_span * (across - j - 1) / across) as u8;
                    let v1 = (v_base + v_span * (across - j) / across - 1) as u8;
                    let corner = |index: usize, dx: f32, dz: f32, u: u8, v: u8| {
                        let local = Vec3i16::new(
                            units(dx) as i16,
                            -units(heights[index] - low) as i16,
                            units(dz) as i16,
                        );
                        Point::new(local, lighting.lit_by_beam(color, qx + dx, qz + dz), (u, v))
                    };
                    let points = if heights[0] + heights[3] <= heights[1] + heights[2] {
                        [
                            corner(2, 0.0, sub, u0, v0),
                            corner(3, sub, sub, u1, v0),
                            corner(0, 0.0, 0.0, u0, v1),
                            corner(1, sub, 0.0, u1, v1),
                        ]
                    } else {
                        [
                            corner(0, 0.0, 0.0, u0, v1),
                            corner(2, 0.0, sub, u0, v0),
                            corner(1, sub, 0.0, u1, v1),
                            corner(3, sub, sub, u1, v0),
                        ]
                    };
                    let surface = if textured {
                        Surface::textured(image.texture)
                            .tinted(fog_tint_from(average, lighting.fog))
                    } else {
                        Surface::gouraud()
                    };
                    let subdivide = if sub < LATTICE {
                        u8::from(centre_distance < 20.0 && sub > FINE)
                    } else if centre_distance < 20.0 {
                        2
                    } else if centre_distance < 48.0 {
                        1
                    } else {
                        0
                    };
                    renderer.polygon(&points, surface, GROUND_BIAS, subdivide);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_props(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        atlas: &Atlas,
        track: &Track,
        camera: &Camera,
        lighting: Lighting,
        boulder: &BoulderDraw,
    ) {
        let view = View::new(camera);
        let right = (camera.yaw.cos(), -camera.yaw.sin());
        let lit = crate::biome::lerp_rgb(lighting.ambient, lighting.sun, 0.8)
            .map(|c| ((c as u32 * 128) / 200).min(255) as u8);
        let reach = (PROP_RADIUS / LATTICE).ceil() as i32;
        let (cx, cz) = (
            (camera.x / LATTICE).floor() as i32,
            (camera.z / LATTICE).floor() as i32,
        );
        for iz in cz - reach..=cz + reach {
            for ix in cx - reach..=cx + reach {
                let key = hash(track.seed ^ 0x9a0b, ix as u64, iz as u64);
                if hash_float(key, 0, 0) > 0.6 {
                    continue;
                }
                let x = (ix as f32 + hash_float(key, 1, 0)) * LATTICE;
                let z = (iz as f32 + hash_float(key, 2, 0)) * LATTICE;
                if !view.visible(x, z, 6.0, PROP_RADIUS) {
                    continue;
                }
                let vertex = self.coarse(track, ix, iz);
                if vertex.road_index == u32::MAX || vertex.road_distance < 5.0 * LATTICE {
                    let Some(road) = track.terrain(x, z).road else {
                        continue;
                    };
                    if road.distance < track.shoulder(road.index) + 3.0
                        || track.in_forest(road.index, road.lateral)
                    {
                        continue;
                    }
                }
                if track.structure_at(x, z, 2.0).is_some() || track.riverbed(x, z) {
                    continue;
                }
                if vertex.road_index == u32::MAX {
                    continue;
                }
                let index = vertex.road_index as usize;
                let character = track.character(index);
                let (a, b, t) = track.biomes.at(Track::distance(index));
                let biome = if hash_float(key, 3, 0) < t {
                    &BIOMES[b]
                } else {
                    &BIOMES[a]
                };
                let remote = (vertex.road_distance / 150.0).clamp(0.0, 0.6);
                let density = biome.density
                    * 0.55
                    * (1.0 + character.forest * 2.2)
                    * (1.0 - character.open * 0.7)
                    * (1.0 - remote);
                if hash_float(key, 4, 0) > density {
                    continue;
                }
                let options: Vec<(Prop, f32)> = biome
                    .props
                    .iter()
                    .copied()
                    .filter(|(prop, _)| {
                        !matches!(prop, Prop::Spectator | Prop::FencePost | Prop::HayBale)
                            || vertex.road_distance < 25.0
                    })
                    .filter(|(prop, _)| character.forest < 0.5 || prop.is_tree())
                    .collect();
                if options.is_empty() {
                    continue;
                }
                let weights: Vec<f32> = options.iter().map(|(_, weight)| *weight).collect();
                let prop = options[Rng::new(key).pick_weighted(&weights)].0;
                let distance = ((x - camera.x).powi(2) + (z - camera.z).powi(2)).sqrt();
                if distance < 3.0
                    || (distance > 130.0
                        && matches!(prop, Prop::SmallRock | Prop::Bush | Prop::Scrub))
                {
                    continue;
                }
                if self.slope_at(track, x, z) > prop.max_slope() {
                    continue;
                }
                let y = self.height_at(track, x, z);
                let band = fog_band(fog_amount(distance));
                let scale = 0.8 + hash_float(key, 5, 0) * 0.5;
                if prop == Prop::Boulder {
                    boulder(
                        renderer,
                        &atlas.stone.in_fog_band(band),
                        [x, y, z],
                        scale * 1.4,
                        key,
                    );
                    continue;
                }
                let Some((image, width, height)) = prop_sprite(art, atlas, prop, key) else {
                    continue;
                };
                let image = if image.texture.page.depth == psxel_vram::TextureDepth::Clut8 {
                    band_image(&image, band)
                } else {
                    image.in_fog_band(band)
                };
                billboard(
                    renderer,
                    &image,
                    [x, y, z],
                    width * scale,
                    height * scale,
                    right,
                    lighting.lit_by_beam(lit, x, z),
                );
            }
        }
    }
}

pub fn prop_sprite(art: &Art, atlas: &Atlas, prop: Prop, key: u64) -> Option<(Image, f32, f32)> {
    let pick = |family: &Vec<Image>| family[(key % family.len() as u64) as usize];
    Some(match prop {
        Prop::Fir => (pick(&art.fir), 4.5, 9.0),
        Prop::Pine => (pick(&art.pine), 5.5, 11.0),
        Prop::SnowFir => (pick(&art.snow_fir), 4.5, 9.0),
        Prop::Broadleaf => (pick(&art.broadleaf), 7.0, 14.0),
        Prop::Kapok => (pick(&art.kapok), 9.0, 18.0),
        Prop::AutumnTree => (pick(&art.autumn), 7.0, 14.0),
        Prop::BareTree => (pick(&art.bare), 7.0, 14.0),
        Prop::Joshua => (pick(&art.joshua), 4.0, 8.0),
        Prop::Cactus => (pick(&art.cactus), 1.8, 3.6),
        Prop::Rock => (pick(&art.rock), 2.6, 2.6),
        Prop::SnowRock => (pick(&art.snow_rock), 2.6, 2.6),
        Prop::SmallRock => (pick(&art.small_rock), 0.9, 0.9),
        Prop::Bush => (pick(&art.bush), 3.0, 2.25),
        Prop::Scrub => (pick(&art.scrub), 2.6, 1.95),
        Prop::HayBale => (pick(&art.hay), 1.9, 1.4),
        Prop::FencePost => (atlas.post, 0.5, 1.4),
        Prop::Spectator => (atlas.spectators[(key % 4) as usize], 0.8, 1.75),
        Prop::Boulder => return None,
    })
}

pub fn billboard(
    renderer: &mut Renderer,
    image: &Image,
    base: [f32; 3],
    width: f32,
    height: f32,
    right: (f32, f32),
    color: Rgb,
) {
    renderer.set_object(world_to_psx(base[0], base[1], base[2]), Mat3Fx12::IDENTITY);
    let half = width * 0.5;
    let (dx, dz) = (units(right.0 * half) as i16, units(right.1 * half) as i16);
    let top = -units(height) as i16;
    let (u0, u1) = (image.u, image.u.wrapping_add(image.width.saturating_sub(1)));
    let (v0, v1) = (image.v, image.v.wrapping_add(image.height).wrapping_sub(1));
    let points = [
        Point::new(Vec3i16::new(-dx, top, -dz), color, (u0, v0)),
        Point::new(Vec3i16::new(dx, top, dz), color, (u1, v0)),
        Point::new(Vec3i16::new(-dx, 0, -dz), color, (u0, v1)),
        Point::new(Vec3i16::new(dx, 0, dz), color, (u1, v1)),
    ];
    renderer.polygon(
        &points,
        Surface::textured(image.texture).tinted(color),
        0,
        1,
    );
}
