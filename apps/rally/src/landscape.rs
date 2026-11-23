use std::cell::RefCell;
use std::collections::HashMap;

use psx_math::{Mat3Fx12, Vec3i16};
use psx_vram::Vram;

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
const UNDER_ROAD: f32 = 0.6;
const NEAR_ROAD: f32 = 12.0;
const NEAR_ROAD_RAMP: f32 = 0.7;
const CACHE_LIMIT: usize = 60_000;

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
    fn vertex(&self, track: &Track, gx: i32, gz: i32) -> Vertex {
        if let Some(vertex) = self.cache.borrow().get(&(gx, gz)) {
            return *vertex;
        }
        let (x, z) = (gx as f32 * LATTICE, gz as f32 * LATTICE);
        let terrain = track.terrain(x, z);
        let (road_index, road_distance) = terrain
            .road
            .map(|road| (road.index as u32, road.distance))
            .unwrap_or((u32::MAX, f32::MAX));
        let mut height = terrain.height;
        if let Some(road) = terrain.road.filter(|_| !track.tunnel_hill(x, z)) {
            let half = track.half_width(road.index);
            let shoulder = track.shoulder(road.index);
            let road_y = track.ground(road.index, road.t, 0.0);
            if road.distance < shoulder + NEAR_ROAD {
                let ramp = road_y + 0.3 + (road.distance - shoulder).max(0.0) * NEAR_ROAD_RAMP;
                height = height.min(ramp);
            }
            if road.distance < half + LATTICE {
                height -=
                    UNDER_ROAD * (1.0 - (road.distance - half).max(0.0) / LATTICE).clamp(0.0, 1.0);
            }
        }
        let vertex = Vertex {
            height,
            road_index,
            road_distance,
            slot: track.tunnel_slot(x, z),
        };
        let mut cache = self.cache.borrow_mut();
        if cache.len() > CACHE_LIMIT {
            cache.clear();
        }
        cache.insert((gx, gz), vertex);
        vertex
    }

    fn refresh(&self, track: &Track) {
        let processed = *self.generated.borrow();
        let regions = &track.dirty[processed.min(track.dirty.len())..];
        if regions.is_empty() {
            return;
        }
        *self.generated.borrow_mut() = track.dirty.len();
        self.cache.borrow_mut().retain(|&(gx, gz), _| {
            let (x, z) = (gx as f32 * LATTICE, gz as f32 * LATTICE);
            !regions.iter().any(|&(min_x, min_z, max_x, max_z)| {
                x >= min_x && x <= max_x && z >= min_z && z <= max_z
            })
        });
    }

    pub fn height_at(&self, track: &Track, x: f32, z: f32) -> f32 {
        let (fx, fz) = (x / LATTICE, z / LATTICE);
        let (gx, gz) = (fx.floor() as i32, fz.floor() as i32);
        let (tx, tz) = (fx - gx as f32, fz - gz as f32);
        let h = |dx: i32, dz: i32| self.vertex(track, gx + dx, gz + dz).height;
        let top = h(0, 0) + (h(1, 0) - h(0, 0)) * tx;
        let bottom = h(0, 1) + (h(1, 1) - h(0, 1)) * tx;
        top + (bottom - top) * tz
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
                                self.vertex(track, gx + dx, gz + dz).road_distance < size + 8.0
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
        {
            for &(gx, gz, step, centre_distance) in &cells {
                {
                    let size = LATTICE * step as f32;
                    let (ix, iz) = (gx / step, gz / step);
                    let (x0, z0) = (gx as f32 * LATTICE, gz as f32 * LATTICE);
                    let corners = [(0, 0), (step, 0), (0, step), (step, step)]
                        .map(|(dx, dz)| self.vertex(track, gx + dx, gz + dz));
                    if corners.iter().any(|vertex| vertex.slot) {
                        continue;
                    }
                    let heights = corners.map(|vertex| vertex.height);
                    let low = heights.iter().copied().fold(f32::MAX, f32::min);
                    let high = heights.iter().copied().fold(f32::MIN, f32::max);
                    let road_index = corners
                        .iter()
                        .map(|vertex| vertex.road_index)
                        .find(|&index| index != u32::MAX)
                        .unwrap_or(0) as usize;
                    let distance = Track::distance(road_index);
                    let (a, b, t) = track.biomes.at(distance);
                    let textured = centre_distance < NEAR_RADIUS;
                    let steep = (high - low) / size > 0.75;
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
                    let key = hash_float(track.seed ^ 0xb1e7, ix as u64, iz as u64);
                    let kind = if steep { Kind::Cliff } else { Kind::Ground };
                    let riverbed = !steep && track.riverbed(x0 + size * 0.5, z0 + size * 0.5);
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
                        ((heights[1] + heights[3]) - (heights[0] + heights[2])) * 0.5 / size;
                    let slope_z =
                        ((heights[2] + heights[3]) - (heights[0] + heights[1])) * 0.5 / size;
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
                    let origin = world_to_psx(x0, low, z0);
                    renderer.set_object(origin, Mat3Fx12::IDENTITY);
                    let span = (size * 8.0).min(255.0) as u8;
                    let (u0, u1, v0, v1) = if windowed {
                        (0u8, span.saturating_sub(1), 0u8, span.saturating_sub(1))
                    } else {
                        (
                            image.u,
                            image.u.wrapping_add(image.width).wrapping_sub(1),
                            image.v,
                            image.v.wrapping_add(image.height).wrapping_sub(1),
                        )
                    };
                    let corner = |index: usize, dx: f32, dz: f32, u: u8, v: u8| {
                        let local = Vec3i16::new(
                            units(dx) as i16,
                            -units(heights[index] - low) as i16,
                            units(dz) as i16,
                        );
                        Point::new(local, lighting.lit_by_beam(color, x0 + dx, z0 + dz), (u, v))
                    };
                    let points = if heights[0] + heights[3] <= heights[1] + heights[2] {
                        [
                            corner(2, 0.0, size, u0, v0),
                            corner(3, size, size, u1, v0),
                            corner(0, 0.0, 0.0, u0, v1),
                            corner(1, size, 0.0, u1, v1),
                        ]
                    } else {
                        [
                            corner(0, 0.0, 0.0, u0, v1),
                            corner(2, 0.0, size, u0, v0),
                            corner(1, size, 0.0, u1, v1),
                            corner(3, size, size, u1, v0),
                        ]
                    };
                    let surface = if textured {
                        Surface::textured(image.texture)
                            .tinted(fog_tint_from(average, lighting.fog))
                    } else {
                        Surface::gouraud()
                    };
                    let subdivide = if centre_distance < 20.0 {
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
                let vertex = self.vertex(track, ix, iz);
                if vertex.road_index == u32::MAX || vertex.road_distance < 3.0 * LATTICE {
                    let Some(road) = track.terrain(x, z).road else {
                        continue;
                    };
                    if road.distance < track.shoulder(road.index) + 3.0 {
                        continue;
                    }
                }
                if track.structure_at(x, z, 2.0).is_some() {
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
                let image = if image.texture.page.depth == psx_vram::TextureDepth::Clut8 {
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
