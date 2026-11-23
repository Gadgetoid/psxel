use psxel_math::{Mat3Fx12, Vec3i16};
use psxel_prim::{BlendMode, Primitive, Vertex};
use psxel_vram::Vram;

use crate::art::{tiled, Art, Ground, Road};
use crate::biome::{lerp_rgb, scale_rgb, Horizon, Prop, Rgb, Weather, BIOMES};
use crate::blend::{Choice, Kind};
use crate::render::{
    fog_amount, screen_quad, units, world_to_psx, Camera, Point, Renderer, Surface, OT_SIZE,
    PROJECTION_H, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use crate::rng::{hash, hash_float, value_noise, Rng};
use crate::signs::road_sign;
use crate::sprites::band_image;
use crate::textures::{fog_band, Atlas, Image};
use crate::track::{
    NoteKind, Track, BOUNDARY, ROAD_HALF, SAMPLE, STRUCTURE_KERB, TUNNEL_COVER_HEIGHT,
    TUNNEL_HEIGHT, TUNNEL_ROOF_HALF,
};

pub const VIEW_SAMPLES: usize = 68;
const BEHIND_SAMPLES: usize = 3;
const TEXTURED_SAMPLES: usize = 24;
const CHUNK: usize = 8;
const SIGN_WIDTH: f32 = 1.3;
const SIGN_TEXELS: f32 = 48.0;
const SIGN_MOUNT: f32 = 1.5;
const SIGN_POST: f32 = 0.16;
const SIGN_SETBACK: f32 = 0.8;
const ROOF_CLEARANCE: f32 = 0.3;
const TREELINE_SETBACK: f32 = 4.0;
const TREELINE_SINK: f32 = 0.6;
const TREELINE_LOW: f32 = 7.0;
const TREELINE_HIGH: f32 = 12.0;
const TREELINE_TEXELS: usize = 16;
const TREELINE_REPEAT: usize = 4;
const TREELINE_FLOOR: f32 = 0.35;
const TREELINE_GAP: f32 = 0.18;
const TREELINE_VINES: f32 = 0.45;
const VINE_HEIGHT: f32 = 4.0;
const PRECIPITATION_DROPS: usize = 240;
const PRECIPITATION_RADIUS: f32 = 24.0;
const GROUND_BIAS: i32 = 160;
const WATER_BIAS: i32 = -250;
const BEAM_STRENGTH: f32 = 0.5;
const SINGLE_TRACK_HALF: f32 = 3.7;
const PUDDLE_TEXELS: f32 = 16.0;
const ICE_TEXELS: f32 = 12.0;
const ICE_SHADE: [f32; 3] = [0.62, 0.78, 1.0];
const TUNNEL_DARK: f32 = 0.22;
const PORTAL_HALF: f32 = TUNNEL_ROOF_HALF;
const PORTAL_TOP: f32 = TUNNEL_COVER_HEIGHT + 1.0;
const FACADE_TEXELS: f32 = 8.0;
const ROOF_TEXELS: f32 = 8.0;
const PARAPET: f32 = 0.9;
const FASCIA: f32 = 1.1;
const PIER_SPACING: usize = 4;
const PIER_HALF: f32 = 0.45;
const CONCRETE: Rgb = [150, 146, 138];
const STONE_TEXELS: f32 = 24.0;
const PORTAL_TEXELS: f32 = 12.0;
const PIER_TEXELS: f32 = 8.0;
const STREAM_BIAS: i32 = 60;
const WATER_DRAW_RANGE: f32 = 260.0;
const ROAD_BIAS: i32 = -180;

pub struct Particle {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub life: f32,
    pub size: f32,
    pub color: Rgb,
}

pub struct Flake {
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
}

pub struct World {
    pub particles: Vec<Particle>,
    pub landscape: crate::landscape::Landscape,
    flakes: Vec<Flake>,
    drops: Vec<[f32; 4]>,
    rng: Rng,
}

#[derive(Clone, Copy)]
pub struct Lighting {
    pub sun: Rgb,
    pub ambient: Rgb,
    pub fog: Rgb,
    pub sun_direction: [f32; 3],
    pub sky_top: Rgb,
    pub sky_horizon: Rgb,
    pub daylight: f32,
    pub headlight: Option<Headlight>,
}

#[derive(Clone, Copy)]
pub struct Headlight {
    pub x: f32,
    pub z: f32,
    pub forward: (f32, f32),
    pub strength: f32,
}

const BEAM_COLOR: [f32; 3] = [235.0, 220.0, 175.0];
const BEAM_RANGE: f32 = 60.0;

impl Lighting {
    pub fn beam(&self, x: f32, z: f32) -> f32 {
        let Some(light) = self.headlight else {
            return 0.0;
        };
        let (dx, dz) = (x - light.x, z - light.z);
        let along = dx * light.forward.0 + dz * light.forward.1;
        if along <= 0.5 || along > BEAM_RANGE {
            return 0.0;
        }
        let lateral = (dx * light.forward.1 - dz * light.forward.0).abs();
        let width = 3.2 + along * 0.42;
        let spread = (1.0 - (lateral / width).powi(2)).max(0.0);
        let falloff = (1.0 - along / BEAM_RANGE).powi(2);
        let onset = (along / 3.0).min(1.0);
        light.strength * spread * falloff * onset
    }

    pub fn lit_by_beam(&self, color: Rgb, x: f32, z: f32) -> Rgb {
        let beam = self.beam(x, z);
        if beam <= 0.0 {
            return color;
        }
        [0, 1, 2].map(|c| (color[c] as f32 + BEAM_COLOR[c] * beam * BEAM_STRENGTH).min(255.0) as u8)
    }

    pub fn shade(&self) -> f32 {
        0.22 + 0.78 * self.daylight
    }
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0xf1a4e);
        let flakes = (0..90)
            .map(|_| Flake {
                x: rng.range(0.0, SCREEN_WIDTH as f32),
                y: rng.range(0.0, SCREEN_HEIGHT as f32),
                speed: rng.range(0.6, 1.4),
                phase: rng.range(0.0, std::f32::consts::TAU),
            })
            .collect();
        Self {
            particles: Vec::new(),
            landscape: crate::landscape::Landscape::default(),
            flakes,
            drops: Vec::new(),
            rng,
        }
    }

    pub fn spawn_dust(
        &mut self,
        position: [f32; 3],
        heading_velocity: (f32, f32),
        color: Rgb,
        amount: f32,
    ) {
        let count = amount.min(3.0) as usize + usize::from(self.rng.chance(amount.fract()));
        for _ in 0..count {
            if self.particles.len() > 120 {
                break;
            }
            let spread = 1.2;
            self.particles.push(Particle {
                position: [
                    position[0] + self.rng.range(-spread, spread),
                    position[1] + 0.2,
                    position[2] + self.rng.range(-spread, spread),
                ],
                velocity: [
                    heading_velocity.0 * 0.15 + self.rng.range(-1.5, 1.5),
                    self.rng.range(0.8, 2.5),
                    heading_velocity.1 * 0.15 + self.rng.range(-1.5, 1.5),
                ],
                life: self.rng.range(0.5, 0.9),
                size: self.rng.range(0.5, 0.9),
                color,
            });
        }
    }

    pub fn update(&mut self, dt: f32, weather: Weather, camera_turn: f32, speed: f32) {
        for particle in &mut self.particles {
            for axis in 0..3 {
                particle.position[axis] += particle.velocity[axis] * dt;
            }
            particle.velocity[1] -= 1.5 * dt;
            particle.size += dt * 1.1;
            particle.life -= dt;
        }
        self.particles.retain(|particle| particle.life > 0.0);
        let (fall, drift) = match weather {
            Weather::Snow => (45.0, 14.0),
            _ => (420.0, 70.0),
        };
        for flake in &mut self.flakes {
            flake.phase += dt * 2.0;
            flake.y += flake.speed * (fall + speed * 2.0) * dt;
            flake.x += (flake.phase.sin() * drift * 0.3 - camera_turn * 900.0) * dt * flake.speed;
            if flake.y > SCREEN_HEIGHT as f32 {
                flake.y -= SCREEN_HEIGHT as f32 + 10.0;
                flake.x = self.rng.range(0.0, SCREEN_WIDTH as f32);
            }
            flake.x = flake.x.rem_euclid(SCREEN_WIDTH as f32);
        }
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
        from_index: usize,
        lighting: Lighting,
    ) {
        let distance = Track::distance(from_index);
        let window = view_window(track, camera, from_index);
        self.draw_sky(renderer, art, track, camera, distance, lighting);
        self.landscape
            .draw(renderer, vram, atlas, art, track, camera, lighting);
        self.draw_ground(
            renderer, vram, atlas, art, track, from_index, window, lighting,
        );
        self.draw_bridges(renderer, art, track, window, lighting);
        self.draw_walls(renderer, art, track, camera, window, lighting);
        self.draw_treeline(renderer, vram, atlas, art, track, camera, window, lighting);
        self.draw_tunnels(renderer, art, track, window, lighting);
        self.draw_water(renderer, art, track, camera, window, lighting);
        self.draw_props(renderer, atlas, art, track, camera, window, lighting);
        let boulder =
            |renderer: &mut Renderer, image: &Image, base: [f32; 3], scale: f32, key: u64| {
                self.boulder(renderer, image, base, scale, key, lighting);
            };
        self.landscape
            .draw_props(renderer, art, atlas, track, camera, lighting, &boulder);
        self.draw_particles(renderer, atlas, camera, lighting);
    }

    fn draw_walls(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        track: &Track,
        camera: &Camera,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;
        let sun = lighting.sun_direction;
        for index in start..end {
            for side in [-1.0f32, 1.0] {
                let (Some(first), Some(second)) =
                    (track.obstacle(index, side), track.obstacle(index + 1, side))
                else {
                    continue;
                };
                if first.barrier != second.barrier {
                    continue;
                }
                let (lateral_a, lateral_b) = (first.lateral, second.lateral);
                let (wall_height, wall_thickness) = (first.height, first.thickness);
                let material = if first.barrier {
                    art.barrier
                } else {
                    tiled(&if track.ground_at(index) == Ground::Earth {
                        art.desert_wall
                    } else {
                        art.stone_wall
                    })
                };
                let (a, b) = (track.sample(index), track.sample(index + 1));
                let origin = world_to_psx(a.x, a.y, a.z);
                renderer.set_object(origin, Mat3Fx12::IDENTITY);
                let corner = |sample: &crate::track::Sample, lateral: f32, offset: f32, up: f32| {
                    let (rx, rz) = sample.right();
                    let reach = lateral + side * offset;
                    let (x, z) = (sample.x + rx * reach, sample.z + rz * reach);
                    let base = self.landscape.height_at(track, x, z) - 0.2;
                    let position = world_to_psx(x, base + up, z);
                    (
                        Vec3i16::new(
                            (position.x - origin.x) as i16,
                            (position.y - origin.y) as i16,
                            (position.z - origin.z) as i16,
                        ),
                        x,
                        base + up,
                        z,
                    )
                };
                let lit = |facing: f32, x: f32, z: f32| {
                    let shade = (0.55 + facing * 0.45).clamp(0.3, 1.1);
                    let base = [0, 1, 2].map(|c| {
                        let light =
                            (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * shade) / 255.0;
                        (128.0 * light).clamp(0.0, 255.0) as u8
                    });
                    lighting.lit_by_beam(base, x, z)
                };
                let (rx, rz) = a.right();
                let facing = (rx * sun[0] + rz * sun[2]) * side;
                let (u0, length, v0, height, top) = if first.barrier {
                    (
                        material.u,
                        material.width - 1,
                        material.v,
                        material.height - 1,
                        (1, 2),
                    )
                } else {
                    (
                        0,
                        ((SAMPLE * STONE_TEXELS) as u8).max(1),
                        0,
                        (wall_height * STONE_TEXELS) as u8,
                        (0, (wall_thickness * STONE_TEXELS) as u8),
                    )
                };
                let faces = [
                    (0.0, 0.0, wall_height, 0.0, -facing, (0, height), -1.0),
                    (
                        wall_thickness,
                        wall_thickness,
                        wall_height,
                        0.0,
                        facing,
                        (0, height),
                        1.0,
                    ),
                    (0.0, wall_thickness, wall_height, wall_height, 0.9, top, 0.0),
                ];
                for (
                    offset_top,
                    offset_bottom,
                    top,
                    bottom,
                    face_light,
                    (v_start, v_span),
                    outward,
                ) in faces
                {
                    let points = [
                        corner(&a, lateral_a, offset_top, top),
                        corner(&b, lateral_b, offset_top, top),
                        corner(&a, lateral_a, offset_bottom, bottom),
                        corner(&b, lateral_b, offset_bottom, bottom),
                    ];
                    let (_, x, y, z) = points[0];
                    let visible = if outward == 0.0 {
                        let along = [points[1].1 - x, points[1].2 - y, points[1].3 - z];
                        let across = [points[2].1 - x, points[2].2 - y, points[2].3 - z];
                        let normal = [
                            along[1] * across[2] - along[2] * across[1],
                            along[2] * across[0] - along[0] * across[2],
                            along[0] * across[1] - along[1] * across[0],
                        ];
                        let facing = (camera.x - x) * normal[0]
                            + (camera.y - y) * normal[1]
                            + (camera.z - z) * normal[2];
                        facing * normal[1] > 0.0
                    } else {
                        ((camera.x - x) * rx + (camera.z - z) * rz) * side * outward > 0.0
                    };
                    if !visible {
                        continue;
                    }
                    let v_top = v0.wrapping_add(v_start);
                    let uvs = [
                        (u0, v_top),
                        (u0.wrapping_add(length), v_top),
                        (u0, v_top.wrapping_add(v_span)),
                        (u0.wrapping_add(length), v_top.wrapping_add(v_span)),
                    ];
                    let points = points
                        .iter()
                        .zip(uvs)
                        .map(|(&(position, x, _, z), uv)| {
                            Point::new(position, lit(face_light, x, z), uv)
                        })
                        .collect::<Vec<_>>();
                    renderer.polygon(
                        &points,
                        Surface::textured(material.texture),
                        ROAD_BIAS + 10,
                        1,
                    );
                }
                let continues = |other: Option<usize>| {
                    other
                        .and_then(|other| track.obstacle(other, side))
                        .is_some_and(|obstacle| obstacle.barrier == first.barrier)
                };
                let caps = [
                    (&a, lateral_a, -1.0, continues(index.checked_sub(1))),
                    (&b, lateral_b, 1.0, continues(Some(index + 2))),
                ];
                for (sample, lateral, outward, joined) in caps {
                    if joined {
                        continue;
                    }
                    let points = [
                        corner(sample, lateral, 0.0, wall_height),
                        corner(sample, lateral, wall_thickness, wall_height),
                        corner(sample, lateral, 0.0, 0.0),
                        corner(sample, lateral, wall_thickness, 0.0),
                    ];
                    let (_, x, _, z) = points[0];
                    let (fx, fz) = sample.forward();
                    if ((camera.x - x) * fx + (camera.z - z) * fz) * outward <= 0.0 {
                        continue;
                    }
                    let end_facing = (fx * sun[0] + fz * sun[2]) * outward;
                    let uvs = [
                        (u0, v0),
                        (u0.wrapping_add(top.1), v0),
                        (u0, v0.wrapping_add(height)),
                        (u0.wrapping_add(top.1), v0.wrapping_add(height)),
                    ];
                    let points = points
                        .iter()
                        .zip(uvs)
                        .map(|(&(position, x, _, z), uv)| {
                            Point::new(position, lit(end_facing, x, z), uv)
                        })
                        .collect::<Vec<_>>();
                    renderer.polygon(
                        &points,
                        Surface::textured(material.texture),
                        ROAD_BIAS + 10,
                        1,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_treeline(
        &self,
        renderer: &mut Renderer,
        vram: &Vram,
        atlas: &Atlas,
        art: &Art,
        track: &Track,
        camera: &Camera,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;
        let tint = fog_tint_from(atlas.average(vram, &art.treeline[0]), lighting.fog);
        let right = (camera.yaw.cos(), -camera.yaw.sin());
        let lit = |shade: f32, x: f32, z: f32| {
            let base = [0, 1, 2].map(|c| {
                let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * 0.6) / 255.0;
                (128.0 * light * shade).clamp(0.0, 255.0) as u8
            });
            lighting.lit_by_beam(base, x, z)
        };
        let jungle = |index: usize| {
            let (a, b, t) = track.biomes.at(Track::distance(index));
            let weight = |biome: usize| {
                if BIOMES[biome].ground == Ground::Jungle {
                    1.0
                } else {
                    0.0
                }
            };
            weight(a) * (1.0 - t) + weight(b) * t
        };
        for index in start..end {
            if track.is_structure(index) || track.is_structure(index + 1) {
                continue;
            }
            let presence = jungle(index).min(jungle(index + 1));
            if presence <= 0.0 {
                continue;
            }
            for side in [-1.0f32, 1.0] {
                let key = hash(track.seed ^ 0x7ee5, index as u64, side as i64 as u64);
                let gap = value_noise(track.seed ^ 0x6a95 ^ side as i64 as u64, index as f32 / 9.0);
                if hash_float(key, 1, 0) > presence || gap < TREELINE_GAP {
                    continue;
                }
                let blocked = |index: usize| {
                    let character = track.character(index);
                    character.cutting > 0.3 || character.ledge * side > 0.3
                };
                if blocked(index) || blocked(index + 1) {
                    continue;
                }
                let edge = |index: usize| {
                    let sample = track.sample(index);
                    let setback = track.shoulder(index) + TREELINE_SETBACK;
                    let lateral = side * setback.min(sample.clearance(side) - 1.0);
                    let (x, z) = sample.offset(lateral);
                    let ground = self.landscape.height_at(track, x, z) - TREELINE_SINK;
                    let height = TREELINE_LOW
                        + (TREELINE_HIGH - TREELINE_LOW)
                            * value_noise(
                                track.seed ^ 0x7e11 ^ side as i64 as u64,
                                index as f32 / 2.3,
                            );
                    (x, ground, z, height)
                };
                let (a, b) = (edge(index), edge(index + 1));
                if track.riverbed(a.0, a.2) || track.riverbed(b.0, b.2) {
                    continue;
                }
                let origin = world_to_psx(a.0, a.1, a.2);
                renderer.set_object(origin, Mat3Fx12::IDENTITY);
                let local = |x: f32, y: f32, z: f32| {
                    let position = world_to_psx(x, y, z);
                    Vec3i16::new(
                        (position.x - origin.x) as i16,
                        (position.y - origin.y) as i16,
                        (position.z - origin.z) as i16,
                    )
                };
                let wall =
                    tiled(&art.treeline[(hash(key, 5, 0) % art.treeline.len() as u64) as usize]);
                let u0 = ((index % TREELINE_REPEAT) * TREELINE_TEXELS) as u8;
                let u1 = u0 + TREELINE_TEXELS as u8 - 1;
                let v = |_: f32| 63u8;
                let points = [
                    Point::new(local(a.0, a.1 + a.3, a.2), lit(1.0, a.0, a.2), (u0, 0)),
                    Point::new(local(b.0, b.1 + b.3, b.2), lit(1.0, b.0, b.2), (u1, 0)),
                    Point::new(
                        local(a.0, a.1, a.2),
                        lit(TREELINE_FLOOR, a.0, a.2),
                        (u0, v(a.3)),
                    ),
                    Point::new(
                        local(b.0, b.1, b.2),
                        lit(TREELINE_FLOOR, b.0, b.2),
                        (u1, v(b.3)),
                    ),
                ];
                renderer.polygon(&points, Surface::textured(wall.texture).tinted(tint), 0, 1);
                if hash_float(key, 2, 0) < TREELINE_VINES {
                    let vine = art.vines[(hash(key, 3, 0) % art.vines.len() as u64) as usize];
                    let t = hash_float(key, 4, 0);
                    let (x, z) = (a.0 + (b.0 - a.0) * t, a.2 + (b.2 - a.2) * t);
                    let top = a.1 + a.3 + (b.1 + b.3 - a.1 - a.3) * t + 0.6;
                    let (rx, rz) = track.sample(index).right();
                    let (x, z) = (x - rx * side * 0.4, z - rz * side * 0.4);
                    let distance = (x - camera.x).hypot(z - camera.z);
                    let image = band_image(&vine, fog_band(fog_amount(distance)));
                    let color = lit(1.0, x, z);
                    self.billboard(
                        renderer,
                        &image,
                        [x, top - VINE_HEIGHT, z],
                        VINE_HEIGHT * 0.8,
                        VINE_HEIGHT,
                        right,
                        color,
                        color,
                    );
                }
            }
        }
    }

    fn draw_tunnels(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        track: &Track,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;

        let wall_color = |x: f32, z: f32, shade: f32| {
            let base = [0, 1, 2].map(|c| {
                let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * 0.4) / 255.0;
                (128.0 * light * shade * TUNNEL_DARK * 2.0).clamp(0.0, 255.0) as u8
            });
            lighting.lit_by_beam(base, x, z)
        };
        let edge = |index: usize, side: f32, up: f32| {
            let sample = track.sample(index);
            let half = track.half_width(index);
            let (rx, rz) = sample.right();
            let lateral = (half + STRUCTURE_KERB) * side;
            let y = sample.y + track.side_height(index, half * side) + up;
            (sample.x + rx * lateral, y, sample.z + rz * lateral)
        };
        for index in start..end {
            if !track.is_tunnel(index) {
                continue;
            }
            let rock = tiled(&art.tunnel_lining(track.ground_at(index)));
            let anchor = track.sample(index);
            let origin = world_to_psx(anchor.x, anchor.y, anchor.z);
            renderer.set_object(origin, Mat3Fx12::IDENTITY);
            let local = |(x, y, z): (f32, f32, f32)| {
                let position = world_to_psx(x, y, z);
                Vec3i16::new(
                    (position.x - origin.x) as i16,
                    (position.y - origin.y) as i16,
                    (position.z - origin.z) as i16,
                )
            };
            let point = |position: (f32, f32, f32), shade: f32, uv: (u8, u8)| {
                Point::new(
                    local(position),
                    wall_color(position.0, position.2, shade),
                    uv,
                )
            };
            let panels = [
                (
                    edge(index, -1.0, TUNNEL_HEIGHT),
                    edge(index, -1.0, 0.0),
                    edge(index + 1, -1.0, TUNNEL_HEIGHT),
                    edge(index + 1, -1.0, 0.0),
                    0.8,
                ),
                (
                    edge(index, 1.0, 0.0),
                    edge(index, 1.0, TUNNEL_HEIGHT),
                    edge(index + 1, 1.0, 0.0),
                    edge(index + 1, 1.0, TUNNEL_HEIGHT),
                    0.8,
                ),
                (
                    edge(index, 1.0, TUNNEL_HEIGHT),
                    edge(index, -1.0, TUNNEL_HEIGHT),
                    edge(index + 1, 1.0, TUNNEL_HEIGHT),
                    edge(index + 1, -1.0, TUNNEL_HEIGHT),
                    0.5,
                ),
            ];
            for (a, b, c, d, shade) in panels {
                let (across, along) = (
                    (TUNNEL_HEIGHT * PORTAL_TEXELS) as u8,
                    (SAMPLE * PORTAL_TEXELS) as u8,
                );
                let points = [
                    point(a, shade, (0, 0)),
                    point(b, shade, (across, 0)),
                    point(c, shade, (0, along)),
                    point(d, shade, (across, along)),
                ];
                renderer.polygon(&points, Surface::textured(rock.texture), ROAD_BIAS + 40, 1);
            }
        }
        for tunnel in &track.tunnels {
            if tunnel.end + 2 < start || tunnel.start > end + 2 {
                continue;
            }
            let roof_light = [0, 1, 2].map(|c| {
                let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * 0.85) / 255.0;
                (128.0 * light).clamp(0.0, 255.0) as u8
            });
            for index in tunnel.start..tunnel.end - 1 {
                let ground = art
                    .ground(track.ground_at(index))
                    .map(tiled)
                    .unwrap_or(tiled(&art.rock_texture));
                let anchor = track.sample(index);
                let origin = world_to_psx(anchor.x, anchor.y, anchor.z);
                renderer.set_object(origin, Mat3Fx12::IDENTITY);
                let roof = |sample_index: usize, lateral: f32| {
                    let sample = track.sample(sample_index);
                    let (rx, rz) = sample.right();
                    let (x, z) = (sample.x + rx * lateral, sample.z + rz * lateral);
                    let y = (track.terrain(x, z).height + 0.05)
                        .max(sample.y + TUNNEL_HEIGHT + ROOF_CLEARANCE);
                    let position = world_to_psx(x, y, z);
                    let along = if sample_index == index {
                        0
                    } else {
                        (SAMPLE * ROOF_TEXELS) as u8
                    };
                    Point::new(
                        Vec3i16::new(
                            (position.x - origin.x) as i16,
                            (position.y - origin.y) as i16,
                            (position.z - origin.z) as i16,
                        ),
                        lighting.lit_by_beam(roof_light, x, z),
                        (((lateral + TUNNEL_ROOF_HALF) * ROOF_TEXELS) as u8, along),
                    )
                };
                for (left, right) in [(-TUNNEL_ROOF_HALF, 0.0), (0.0, TUNNEL_ROOF_HALF)] {
                    let points = [
                        roof(index, left),
                        roof(index, right),
                        roof(index + 1, left),
                        roof(index + 1, right),
                    ];
                    renderer.polygon(&points, Surface::textured(ground.texture), GROUND_BIAS, 1);
                }
            }
            for (index, facing) in [(tunnel.start, -1.0f32), (tunnel.end - 1, 1.0)] {
                if index < start || index > end {
                    continue;
                }
                let rock = tiled(&art.tunnel_lining(track.ground_at(index)));
                let sample = track.sample(index);
                let (fx, fz) = sample.forward();
                let (rx, rz) = sample.right();
                let half = track.half_width(index) + STRUCTURE_KERB;
                let face = |lateral: f32, up: f32| {
                    (
                        sample.x + rx * lateral + fx * facing * 0.2,
                        sample.y + up,
                        sample.z + rz * lateral + fz * facing * 0.2,
                    )
                };
                let origin = world_to_psx(sample.x, sample.y, sample.z);
                renderer.set_object(origin, Mat3Fx12::IDENTITY);
                let local = |(x, y, z): (f32, f32, f32)| {
                    let position = world_to_psx(x, y, z);
                    Vec3i16::new(
                        (position.x - origin.x) as i16,
                        (position.y - origin.y) as i16,
                        (position.z - origin.z) as i16,
                    )
                };
                let concrete = [0, 1, 2].map(|c| {
                    let light =
                        (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * 0.85) / 255.0;
                    (128.0 * light).clamp(0.0, 255.0) as u8
                });
                let uv = |lateral: f32, up: f32| {
                    (
                        ((lateral + PORTAL_HALF) * FACADE_TEXELS).clamp(0.0, 255.0) as u8,
                        ((PORTAL_TOP - up) * FACADE_TEXELS).clamp(0.0, 255.0) as u8,
                    )
                };
                let quads = [
                    (-PORTAL_HALF, -half, -1.0, PORTAL_TOP),
                    (half, PORTAL_HALF, -1.0, PORTAL_TOP),
                    (-half, half, TUNNEL_HEIGHT, PORTAL_TOP),
                ];
                for (left, right, bottom, top) in quads {
                    let (left, right) = if facing < 0.0 {
                        (left, right)
                    } else {
                        (right, left)
                    };
                    let points = [
                        Point::new(local(face(left, top)), concrete, uv(left, top)),
                        Point::new(local(face(right, top)), concrete, uv(right, top)),
                        Point::new(local(face(left, bottom)), concrete, uv(left, bottom)),
                        Point::new(local(face(right, bottom)), concrete, uv(right, bottom)),
                    ];
                    renderer.polygon(&points, Surface::textured(rock.texture), ROAD_BIAS + 60, 1);
                }
            }
        }
    }

    fn draw_bridges(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        track: &Track,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;
        let lit = |shade: f32| {
            [0, 1, 2].map(|c| {
                let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * shade) / 255.0;
                (CONCRETE[c] as f32 * light).clamp(0.0, 255.0) as u8
            })
        };
        let stone_lit = |shade: f32, x: f32, z: f32| {
            let base = [0, 1, 2].map(|c| {
                let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * shade) / 255.0;
                (128.0 * light).clamp(0.0, 255.0) as u8
            });
            lighting.lit_by_beam(base, x, z)
        };
        let texel = |metres: f32| (metres * STONE_TEXELS).clamp(0.0, 255.0) as u8;
        for index in start..end {
            if !track.is_deck(index) {
                continue;
            }
            let ravine = track.spans_ravine(index) || track.spans_ravine(index + 1);
            let stone = tiled(&if ravine {
                art.bridge_wall(track.ground_at(index))
            } else if hash(track.seed, index as u64 / 32, 5) & 1 == 0 {
                art.metal_blue
            } else {
                art.metal_red
            });
            let pier = if ravine { stone } else { tiled(&art.concrete) };
            let (a, b) = (track.sample(index), track.sample(index + 1));
            let origin = world_to_psx(a.x, a.y, a.z);
            renderer.set_object(origin, Mat3Fx12::IDENTITY);
            let local = |x: f32, y: f32, z: f32| {
                let position = world_to_psx(x, y, z);
                Vec3i16::new(
                    (position.x - origin.x) as i16,
                    (position.y - origin.y) as i16,
                    (position.z - origin.z) as i16,
                )
            };
            let edge = |sample: &crate::track::Sample, sample_index: usize, side: f32| {
                let lateral = (track.half_width(sample_index) + STRUCTURE_KERB) * side;
                let (rx, rz) = sample.right();
                let y = sample.y
                    + track.side_height(sample_index, track.half_width(sample_index) * side);
                (sample.x + rx * lateral, y, sample.z + rz * lateral)
            };
            for side in [-1.0f32, 1.0] {
                let (ax, ay, az) = edge(&a, index, side);
                let (bx, by, bz) = edge(&b, index + 1, side);
                let span = texel(SAMPLE);
                for (top, bottom, shade) in [(PARAPET, 0.0, 0.9), (0.0, -FASCIA, 0.6)] {
                    let height = texel(top - bottom);
                    let points = [
                        Point::new(local(ax, ay + top, az), stone_lit(shade, ax, az), (0, 0)),
                        Point::new(local(bx, by + top, bz), stone_lit(shade, bx, bz), (span, 0)),
                        Point::new(
                            local(ax, ay + bottom, az),
                            stone_lit(shade, ax, az),
                            (0, height),
                        ),
                        Point::new(
                            local(bx, by + bottom, bz),
                            stone_lit(shade, bx, bz),
                            (span, height),
                        ),
                    ];
                    renderer.polygon(&points, Surface::textured(stone.texture), ROAD_BIAS + 20, 0);
                }
            }
            let under = lit(0.25);
            let ((lax, lay, laz), (rax, ray, raz)) = (edge(&a, index, -1.0), edge(&a, index, 1.0));
            let ((lbx, lby, lbz), (rbx, rby, rbz)) =
                (edge(&b, index + 1, -1.0), edge(&b, index + 1, 1.0));
            let points = [
                Point::new(local(lax, lay - FASCIA, laz), under, (0, 0)),
                Point::new(local(rax, ray - FASCIA, raz), under, (0, 0)),
                Point::new(local(lbx, lby - FASCIA, lbz), under, (0, 0)),
                Point::new(local(rbx, rby - FASCIA, rbz), under, (0, 0)),
            ];
            renderer.polygon(&points, Surface::gouraud(), 0, 0);
            if !index.is_multiple_of(PIER_SPACING) {
                continue;
            }
            let (fx, fz) = a.forward();
            let (rx, rz) = a.right();
            let top = a.y - FASCIA;
            for side in [-0.5f32, 0.5] {
                let lateral = track.half_width(index) * side;
                let (cx, cz) = (a.x + rx * lateral, a.z + rz * lateral);
                let foot = self.landscape.height_at(track, cx, cz) - 0.3;
                if foot >= top {
                    continue;
                }
                let corner = |along: f32, across: f32, y: f32| {
                    local(
                        cx + fx * along + rx * across,
                        y,
                        cz + fz * along + rz * across,
                    )
                };
                let faces = [
                    ((-1.0, -1.0), (-1.0, 1.0), 0.5),
                    ((-1.0, 1.0), (1.0, 1.0), 0.8),
                    ((1.0, 1.0), (1.0, -1.0), 0.5),
                    ((1.0, -1.0), (-1.0, -1.0), 0.8),
                ];
                let width = texel(PIER_HALF * 2.0);
                let height = ((top - foot) * PIER_TEXELS).clamp(1.0, 255.0) as u8;
                for ((a0, c0), (a1, c1), shade) in faces {
                    let color = stone_lit(shade, cx, cz);
                    let (a0, c0, a1, c1) = (
                        a0 * PIER_HALF,
                        c0 * PIER_HALF,
                        a1 * PIER_HALF,
                        c1 * PIER_HALF,
                    );
                    let points = [
                        Point::new(corner(a0, c0, top), color, (0, 0)),
                        Point::new(corner(a1, c1, top), color, (width, 0)),
                        Point::new(corner(a0, c0, foot), color, (0, height)),
                        Point::new(corner(a1, c1, foot), color, (width, height)),
                    ];
                    renderer.polygon(&points, Surface::textured(pier.texture), 0, 0);
                }
            }
        }
    }

    fn draw_water(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        track: &Track,
        camera: &Camera,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let surface = Surface::gouraud().blended(BlendMode::Half);
        let shine = lerp_rgb(lighting.sky_horizon, lighting.sky_top, 0.3);
        let mud = tiled(&art.puddle_mud);
        let mud_surface = Surface::textured(mud.texture).blended(BlendMode::Half);
        let ice = tiled(&art.ice);
        let ice_surface = Surface::textured(ice.texture);
        let ice_tint = [0, 1, 2].map(|c| {
            let light = (lighting.ambient[c] as f32 + lighting.sun[c] as f32 * 0.9) / 255.0;
            (128.0 * light * ICE_SHADE[c]).clamp(0.0, 255.0) as u8
        });
        let mud_tint = scale_rgb(shine, 0.8);
        let puddle_uv = |x: f32, z: f32| {
            (
                (32.0 + x * PUDDLE_TEXELS).clamp(0.0, 255.0) as u8,
                (32.0 + z * PUDDLE_TEXELS).clamp(0.0, 255.0) as u8,
            )
        };
        let (start, end) = window;
        for index in start..end {
            let Some(puddle) = track.puddle(index) else {
                continue;
            };
            renderer.set_object(
                world_to_psx(puddle.x, puddle.y + 0.03, puddle.z),
                Mat3Fx12::IDENTITY,
            );
            let (look, tint) = if puddle.frozen {
                (ice_surface, ice_tint)
            } else {
                (mud_surface, mud_tint)
            };
            let (sin, cos) = puddle.heading.sin_cos();
            let rim = |step: usize| {
                let angle = step as f32 / 8.0 * std::f32::consts::TAU;
                let along = angle.cos() * puddle.radius * 1.4;
                let across = angle.sin() * puddle.radius;
                let x = sin * along + cos * across;
                let z = cos * along - sin * across;
                Point::new(
                    Vec3i16::new(units(x) as i16, 0, units(z) as i16),
                    lighting.lit_by_beam(tint, puddle.x + x, puddle.z + z),
                    puddle_uv(x, z),
                )
            };
            let centre = Point::new(
                Vec3i16::new(0, 0, 0),
                lighting.lit_by_beam(tint, puddle.x, puddle.z),
                puddle_uv(0.0, 0.0),
            );
            for step in 0..8 {
                renderer.polygon(&[centre, rim(step + 1), rim(step)], look, WATER_BIAS, 0);
            }
        }

        let stream = lerp_rgb(shine, [40, 70, 80], 0.35);
        for ford in track.fords.iter().chain(&track.rivers) {
            if (ford.x - camera.x).hypot(ford.z - camera.z) > WATER_DRAW_RANGE {
                continue;
            }
            renderer.set_object(world_to_psx(ford.x, ford.level, ford.z), Mat3Fx12::IDENTITY);
            let path = &ford.path;
            let edges = |index: usize| {
                let point = path[index];
                let before = path[index.saturating_sub(1)];
                let after = path[(index + 1).min(path.len() - 1)];
                let (dx, dz) = (after.x - before.x, after.z - before.z);
                let length = dx.hypot(dz).max(1e-3);
                let (nx, nz) = (dz / length, -dx / length);
                let width = point.width + 1.5;
                let level = track.settled_level(point.surface, point.x, point.z);
                [-1.0f32, 1.0].map(|side| {
                    let (x, z) = (point.x + nx * width * side, point.z + nz * width * side);
                    Point::new(
                        Vec3i16::new(
                            units(x - ford.x) as i16,
                            -units(level - ford.level) as i16,
                            units(z - ford.z) as i16,
                        ),
                        lighting.lit_by_beam(if ford.frozen { ice_tint } else { stream }, x, z),
                        (0, 0),
                    )
                })
            };
            for index in 1..path.len() {
                let (a, b) = (edges(index - 1), edges(index));
                let crossing = index.abs_diff(ford.crossing) <= 1;
                let mut quad = [a[0], a[1], b[0], b[1]];
                let look = if ford.frozen {
                    let (from, to) = (path[index - 1], path[index]);
                    let along =
                        ((to.x - from.x).hypot(to.z - from.z) * ICE_TEXELS).min(255.0) as u8;
                    let across = ((from.width + 1.5) * 2.0 * ICE_TEXELS).min(255.0) as u8;
                    quad[0].uv = (0, 0);
                    quad[1].uv = (across, 0);
                    quad[2].uv = (0, along);
                    quad[3].uv = (across, along);
                    ice_surface
                } else {
                    surface
                };
                renderer.polygon(
                    &quad,
                    look,
                    if crossing { WATER_BIAS } else { STREAM_BIAS },
                    0,
                );
            }
        }
    }

    fn draw_sky(
        &self,
        renderer: &mut Renderer,
        art: &Art,
        track: &Track,
        camera: &Camera,
        distance: f32,
        lighting: Lighting,
    ) {
        let biomes = &track.biomes;
        let top = lighting.sky_top;
        let horizon_color = lighting.sky_horizon;
        let fog = lighting.fog;
        let horizon =
            (SCREEN_HEIGHT as f32 / 2.0 - camera.pitch.tan() * PROJECTION_H as f32) as i16;
        let horizon = horizon.clamp(-200, 400);
        let back = OT_SIZE - 1;
        if lighting.daylight < 0.6 {
            draw_stars(
                renderer,
                track.seed,
                camera,
                horizon,
                1.0 - lighting.daylight / 0.6,
            );
        }
        let ground = lerp_rgb(
            biomes.color(distance, |b| b.ground_dark),
            biomes.color(distance, |b| b.ground_light),
            0.5,
        );
        let ground = scale_rgb(ground, lighting.shade());
        let far_ground = lerp_rgb(fog, ground, 0.45);
        let near_ground = scale_rgb(ground, 0.8);
        let band = 20;
        renderer.ot.add_at(
            back,
            screen_quad(
                0,
                horizon,
                SCREEN_WIDTH as i16,
                band,
                [fog, fog, far_ground, far_ground],
                None,
            ),
        );
        let lower = horizon + band;
        if lower < SCREEN_HEIGHT as i16 {
            renderer.ot.add_at(
                back,
                screen_quad(
                    0,
                    lower,
                    SCREEN_WIDTH as i16,
                    SCREEN_HEIGHT as i16 - lower + 2,
                    [far_ground, far_ground, near_ground, near_ground],
                    None,
                ),
            );
        }
        if horizon > 0 {
            renderer.ot.add_at(
                back,
                screen_quad(
                    0,
                    -2,
                    SCREEN_WIDTH as i16,
                    horizon + 2,
                    [top, top, horizon_color, horizon_color],
                    None,
                ),
            );
        }

        let (biome_a, biome_b, blend) = biomes.at(distance);
        let biome_index = if blend < 0.5 { biome_a } else { biome_b };
        if let Some(backdrop) = art.backdrop(biome_index) {
            draw_backdrop(renderer, &backdrop, lighting.shade());
            return;
        }
        let (style_a, style_b, t) = biomes.mix(distance, |b| b.horizon);
        let silhouette_color = biomes.color(distance, |b| b.horizon_color);
        let far = lerp_rgb(silhouette_color, fog, 0.35);
        let base = lerp_rgb(silhouette_color, fog, 0.75);
        renderer.set_sky_object(Mat3Fx12::IDENTITY);
        renderer.force_bucket = Some(OT_SIZE - 2);
        let fog_enabled = renderer.fog_enabled;
        renderer.fog_enabled = false;
        let segments = 64;
        let radius = 28000.0;
        let height_at = |style: Horizon, segment: usize| -> f32 {
            let position = segment as f32 / segments as f32 * 16.0;
            let seed = track.seed ^ 0x40a1;
            match style {
                Horizon::Hills => {
                    700.0
                        + value_noise(seed, position) * 1600.0
                        + value_noise(seed ^ 9, position * 3.0) * 500.0
                }
                Horizon::Peaks => {
                    let ridge = value_noise(seed, position * 1.5);
                    600.0 + ridge * ridge * 4200.0 + hash_float(seed, segment as u64, 1) * 700.0
                }
                Horizon::Mesas => {
                    let plateau = value_noise(seed, position * 0.8);
                    if plateau > 0.5 {
                        2200.0 + hash_float(seed, segment as u64 / 3, 2) * 300.0
                    } else {
                        400.0 + plateau * 400.0
                    }
                }
                Horizon::Flat => 250.0 + value_noise(seed, position * 4.0) * 350.0,
            }
        };
        let ground_level = 1400i16;
        for segment in 0..segments {
            let corner = |index: usize| {
                let theta = index as f32 / segments as f32 * std::f32::consts::TAU;
                let height = height_at(style_a, index % segments) * (1.0 - t)
                    + height_at(style_b, index % segments) * t;
                (
                    (theta.sin() * radius) as i16,
                    (theta.cos() * radius) as i16,
                    -(height as i16) + ground_level / 4,
                )
            };
            let (ax, az, ay) = corner(segment);
            let (bx, bz, by) = corner(segment + 1);
            let snowy = |height: i16| {
                if height < -3000 {
                    lerp_rgb(far, [235, 240, 250], 0.7)
                } else {
                    far
                }
            };
            let points = [
                Point::new(Vec3i16::new(bx, by, bz), snowy(by), (0, 0)),
                Point::new(Vec3i16::new(ax, ay, az), snowy(ay), (0, 0)),
                Point::new(Vec3i16::new(bx, ground_level, bz), base, (0, 0)),
                Point::new(Vec3i16::new(ax, ground_level, az), base, (0, 0)),
            ];
            renderer.polygon(&points, Surface::gouraud(), 0, 0);
        }
        renderer.fog_enabled = fog_enabled;
        renderer.force_bucket = None;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_ground(
        &self,
        renderer: &mut Renderer,
        vram: &Vram,
        atlas: &Atlas,
        art: &Art,
        track: &Track,
        from_index: usize,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;
        let light = |base: f32| -> Rgb {
            let sun = lighting.sun.map(|c| c as f32 / 255.0);
            let ambient = lighting.ambient.map(|c| c as f32 / 255.0);
            [0, 1, 2].map(|c| ((ambient[c] + sun[c] * base) * 128.0).clamp(0.0, 255.0) as u8)
        };
        let mut averages: Vec<((u8, u8, u8, u8), Rgb)> = Vec::new();
        let mut average_of = |image: &Image| -> Rgb {
            let key = (image.texture.page.x, image.texture.page.y, image.u, image.v);
            if let Some((_, average)) = averages.iter().find(|(entry, _)| *entry == key) {
                return *average;
            }
            let average = atlas.average(vram, image);
            averages.push((key, average));
            average
        };

        let mut chunk_start = start;
        while chunk_start < end {
            let chunk_end = (chunk_start + CHUNK).min(end);
            let anchor = track.sample(chunk_start);
            let origin = world_to_psx(anchor.x, anchor.y, anchor.z);
            renderer.set_object(origin, Mat3Fx12::IDENTITY);
            let local = |x: f32, y: f32, z: f32| {
                let position = world_to_psx(x, y, z);
                Vec3i16::new(
                    (position.x - origin.x) as i16,
                    (position.y - origin.y) as i16,
                    (position.z - origin.z) as i16,
                )
            };
            let mut sections: Vec<Vec<RibbonVertex>> = Vec::with_capacity(CHUNK + 1);
            let mut laterals: Vec<Vec<f32>> = Vec::with_capacity(CHUNK + 1);
            let mut centres: Vec<u16> = Vec::with_capacity(CHUNK + 1);
            for index in chunk_start..=chunk_end {
                let sample = track.sample(index);
                let (rx, rz) = sample.right();
                let half = track.half_width(index);
                let deck = track.is_structure(index);
                let outer = [if deck {
                    half + STRUCTURE_KERB
                } else {
                    track.shoulder(index)
                }];
                let mut offsets: Vec<f32> = outer.iter().rev().map(|offset| -offset).collect();
                offsets.push(-half);
                offsets.push(half);
                offsets.extend(outer);
                let slope_light = 0.75 + sample.curvature.abs().min(0.05) * 2.0;
                let mut row = Vec::with_capacity(offsets.len());
                let mut row_laterals = Vec::with_capacity(offsets.len());
                for &offset in &offsets {
                    let lateral = track.drawn_lateral(index, offset);
                    let x = sample.x + rx * lateral;
                    let z = sample.z + rz * lateral;
                    let height = if deck && offset.abs() > half + 0.01 {
                        track.side_height(index, half * lateral.signum())
                    } else if offset.abs() > half + 0.01 {
                        self.landscape.height_at(track, x, z) - sample.y + 0.05
                    } else {
                        track.side_height(index, lateral)
                    };
                    let outward = lateral + lateral.signum();
                    let bank = track.side_height(index, outward) + sample.sink(outward) - height;
                    let shade = (slope_light - bank * 0.35).clamp(0.35, 1.1);
                    row.push((local(x, sample.y + height, z), shade, height, x, z));
                    row_laterals.push(lateral);
                }
                centres.push(renderer.depth_of(local(sample.x, sample.y, sample.z)));
                sections.push(row);
                laterals.push(row_laterals);
            }
            for row in 0..chunk_end - chunk_start {
                let index = chunk_start + row;
                let textured = index.abs_diff(from_index) < TEXTURED_SAMPLES;
                let near_row = index.abs_diff(from_index) <= 3;
                let row_depth = (centres[row] as u32 + centres[row + 1] as u32) / 2;
                let darkness = if track.is_tunnel(index) {
                    TUNNEL_DARK
                } else {
                    1.0
                };
                let (a, b, t) = track.biomes.at(Track::distance(index));
                let key = hash_float(track.seed ^ 0x90ad, index as u64 / 2, 0);
                let pick = |kind: Kind, first: Image, second: Image, tile: bool| match art
                    .blends
                    .choose(kind, (a, b), t, key)
                {
                    Choice::First => first,
                    Choice::Second => second,
                    Choice::Mix(mix) if tile => tiled(&mix),
                    Choice::Mix(mix) => mix,
                };
                let (first, second) = (&BIOMES[a], &BIOMES[b]);
                let variant = hash(track.seed, index as u64 / 2, 77);
                let biome = if t > 0.5 { second } else { first };
                let narrow = track.half_width(index) < SINGLE_TRACK_HALF;
                let single_track = match biome.road {
                    Road::Gravel | Road::Mud if narrow => Some(art.road_single),
                    Road::Snow if narrow => Some(art.road_snow_single),
                    _ => None,
                };
                let road_image = if let Some(single) = single_track {
                    single
                } else {
                    pick(
                        Kind::Road,
                        *art.road(first.road, variant),
                        *art.road(second.road, variant),
                        false,
                    )
                };
                let grassy_verge = matches!(biome.ground, Ground::Grass | Ground::Leaves);
                let ground_image = match (art.ground(first.ground), art.ground(second.ground)) {
                    (Some(from), Some(to)) => {
                        Some(pick(Kind::Ground, tiled(from), tiled(to), true))
                    }
                    _ => None,
                };
                let cliff_image = pick(
                    Kind::Cliff,
                    art.cliff(first.ground, index as u64),
                    art.cliff(second.ground, index as u64),
                    true,
                );
                let strips = sections[row].len() - 1;
                for strip in 0..strips {
                    let middle = strips / 2;
                    let is_road = strip == middle;
                    let is_verge = strip + 1 == middle || strip == middle + 1;
                    let width = (laterals[row][strip + 1] - laterals[row][strip])
                        .abs()
                        .max(0.01);
                    let rise = (sections[row][strip + 1].2 - sections[row][strip].2).abs();
                    let steep = !is_road && rise / width > 0.8;
                    let (image, windowed, bias, shade_scale) = if is_road {
                        (road_image, false, ROAD_BIAS, 1.0)
                    } else if steep {
                        (cliff_image, true, GROUND_BIAS, 0.95)
                    } else if is_verge && grassy_verge {
                        (tiled(&art.verge_dirt), true, ROAD_BIAS, 0.9)
                    } else if let Some(ground) = ground_image {
                        (
                            ground,
                            true,
                            if is_verge { ROAD_BIAS } else { GROUND_BIAS },
                            if is_verge { 0.8 } else { 1.0 },
                        )
                    } else {
                        (
                            atlas.ground,
                            false,
                            if is_verge { ROAD_BIAS } else { GROUND_BIAS },
                            if is_verge { 0.85 } else { 1.0 },
                        )
                    };
                    let average = average_of(&image);
                    let fog_tint = fog_tint_from(average, lighting.fog);
                    let (u0, u1, v0) = if windowed {
                        let span = ((width * 8.0) as u32).clamp(4, 255) as u8;
                        (
                            0u8,
                            span - 1,
                            if index.is_multiple_of(2) { 0u8 } else { 32 },
                        )
                    } else {
                        (
                            image.u,
                            image.u.wrapping_add(image.width).wrapping_sub(1),
                            image.v + if index.is_multiple_of(2) { 0 } else { 32 },
                        )
                    };
                    let v1 = v0 + 31;
                    let corner = |r: usize, s: usize, u: u8, v: u8| {
                        let (position, shade, _, x, z) = sections[r][s];
                        let lit = lighting.lit_by_beam(
                            scale_rgb(light(shade * shade_scale), darkness),
                            x,
                            z,
                        );
                        let color = if textured {
                            lit
                        } else {
                            [0, 1, 2]
                                .map(|k| ((average[k] as u32 * lit[k] as u32) / 128).min(255) as u8)
                        };
                        Point::new(position, color, (u, v))
                    };
                    let points = [
                        corner(row + 1, strip, u0, v0),
                        corner(row + 1, strip + 1, u1, v0),
                        corner(row, strip, u0, v1),
                        corner(row, strip + 1, u1, v1),
                    ];
                    let surface = if textured {
                        Surface::textured(image.texture).tinted(fog_tint)
                    } else {
                        Surface::gouraud()
                    };
                    let subdivide = if near_row {
                        3
                    } else if textured {
                        1
                    } else {
                        0
                    };
                    let lateral_push = (strip as i32 - middle as i32).unsigned_abs() as i32 * 6;
                    renderer.force_otz = Some(row_depth);
                    renderer.polygon(&points, surface, bias + lateral_push, subdivide);
                    renderer.force_otz = None;
                }
            }
            chunk_start = chunk_end;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_props(
        &self,
        renderer: &mut Renderer,
        atlas: &Atlas,
        art: &Art,
        track: &Track,
        camera: &Camera,
        window: (usize, usize),
        lighting: Lighting,
    ) {
        let (start, end) = window;
        let right = (camera.yaw.cos(), -camera.yaw.sin());
        let lit = lerp_rgb(lighting.ambient, lighting.sun, 0.8)
            .map(|c| ((c as u32 * 128) / 200).min(255) as u8);
        let notes: Vec<_> = track
            .upcoming_notes(start, 24)
            .filter(|note| {
                matches!(
                    note.kind,
                    NoteKind::Turn {
                        severity: 0..=3,
                        ..
                    }
                )
            })
            .copied()
            .collect();
        let band_at = |x: f32, z: f32| {
            let distance = ((x - camera.x).powi(2) + (z - camera.z).powi(2)).sqrt();
            fog_band(fog_amount(distance))
        };

        for index in start..end {
            if track.is_structure(index) {
                continue;
            }
            let sample = track.sample(index);
            let distance = Track::distance(index);
            let (a, b, t) = track.biomes.at(distance);
            let character = track.character(index);
            renderer.set_object(
                world_to_psx(sample.x, sample.y, sample.z),
                Mat3Fx12::IDENTITY,
            );
            let row_depth = renderer.depth_of(Vec3i16::default()) as u32;
            for side in [-1.0f32, 1.0] {
                let side_key = hash(track.seed, index as u64, side as i64 as u64);
                let biome = if hash_float(side_key, 1, 0) < t {
                    &BIOMES[b]
                } else {
                    &BIOMES[a]
                };
                let boundary = track.boundary(index, side);
                let drop_side = character.ledge != 0.0 && character.ledge.signum() != side;
                let density = (biome.density
                    * (1.0 + character.forest * 1.3)
                    * (1.0 - character.open * 0.65))
                    .min(0.97);
                let count = 1 + (character.forest * 2.6 * hash_float(side_key, 5, 0)) as usize;
                for slot in 0..count {
                    let key = hash(side_key, slot as u64, 11);
                    if hash_float(key, 2, 0) >= density {
                        continue;
                    }
                    let crowded = slot > 0 || character.forest > 0.5;
                    let options: Vec<(Prop, f32)> = biome
                        .props
                        .iter()
                        .copied()
                        .filter(|(prop, _)| {
                            matches!(prop, Prop::Spectator | Prop::FencePost | Prop::HayBale)
                        })
                        .filter(|_| !crowded)
                        .collect();
                    if options.is_empty() {
                        continue;
                    }
                    let weights: Vec<f32> = options.iter().map(|(_, weight)| *weight).collect();
                    let mut chooser = Rng::new(key);
                    let prop = options[chooser.pick_weighted(&weights)].0;
                    if drop_side && matches!(prop, Prop::Spectator | Prop::FencePost) {
                        continue;
                    }
                    if prop == Prop::FencePost && track.obstacle(index, side).is_some() {
                        continue;
                    }
                    let spread = if crowded { 12.0 } else { 30.0 };
                    let lateral = match prop {
                        Prop::FencePost => side * (boundary + 0.5),
                        Prop::Spectator => side * (boundary + 1.0 + hash_float(key, 3, 0) * 3.0),
                        _ => {
                            side * (boundary
                                + 1.0
                                + hash_float(key, 3, 0).powi(2) * spread
                                + slot as f32 * 3.0)
                        }
                    };
                    if lateral.abs() > sample.clearance(lateral).min(sample.fold(lateral)) - 1.5 {
                        continue;
                    }
                    let along =
                        (hash_float(key, 6, 0) - 0.5) * SAMPLE * if crowded { 1.0 } else { 0.0 };
                    let scale = 0.8 + hash_float(key, 4, 0) * 0.5;
                    let (fx, fz) = sample.forward();
                    let (x, z) = sample.offset(lateral);
                    let (x, z) = (x + fx * along, z + fz * along);
                    if track.riverbed(x, z)
                        || self.landscape.slope_at(track, x, z) > prop.max_slope()
                    {
                        continue;
                    }
                    let y = self.landscape.height_at(track, x, z);
                    let band = band_at(x, z);
                    renderer.set_object(world_to_psx(x, y, z), Mat3Fx12::IDENTITY);
                    renderer.force_otz = Some(renderer.depth_of(Vec3i16::default()) as u32);
                    if prop == Prop::Boulder {
                        self.boulder(
                            renderer,
                            &atlas.stone.in_fog_band(band),
                            [x, y, z],
                            scale * 1.4,
                            key,
                            lighting,
                        );
                        continue;
                    }
                    let (image, width, height, photo) = self.prop_image(atlas, art, prop, key);
                    let image = if photo {
                        band_image(&image, band)
                    } else {
                        image.in_fog_band(band)
                    };
                    self.billboard(
                        renderer,
                        &image,
                        [x, y, z],
                        width * scale,
                        height * scale,
                        right,
                        lighting.lit_by_beam(lit, x, z),
                        lighting.lit_by_beam(lit, x, z),
                    );
                }
            }
            renderer.force_otz = Some(row_depth + 20);
            if let Some(note) = notes.iter().find(|note| {
                index >= note.start && index < note.end && (index - note.start) % 3 == 0
            }) {
                if let NoteKind::Turn { direction, .. } = note.kind {
                    let lateral = -direction.sign() * (BOUNDARY + 0.5);
                    let (x, z) = sample.offset(lateral);
                    if !track.riverbed(x, z) {
                        let y = self.landscape.height_at(track, x, z);
                        let mirror = direction.sign() > 0.0;
                        self.chevron(
                            renderer,
                            &atlas.chevron.in_fog_band(band_at(x, z)),
                            track,
                            index,
                            mirror,
                            [x, y, z],
                            lit,
                            lit,
                        );
                    }
                }
            }
            if let Some(sign) = road_sign(track, index) {
                let lateral = -(track.shoulder(index) + SIGN_SETBACK)
                    .min(sample.clearance(-1.0) - SIGN_SETBACK);
                let (x, z) = sample.offset(lateral);
                if !track.riverbed(x, z) {
                    let y = self.landscape.height_at(track, x, z);
                    let image = band_image(&art.sign(sign.kind), band_at(x, z));
                    self.sign(
                        renderer,
                        &image,
                        &sample,
                        sign.mirror,
                        [x, y, z],
                        right,
                        lighting.lit_by_beam(lit, x, z),
                    );
                }
            }
            renderer.force_otz = Some(row_depth);
            if Track::is_checkpoint(index) || index == 8 {
                let sample = track.sample(index);
                self.gate(
                    renderer,
                    atlas,
                    track,
                    index,
                    lit,
                    band_at(sample.x, sample.z),
                );
            }
        }
        renderer.force_otz = None;
    }

    fn boulder(
        &self,
        renderer: &mut Renderer,
        image: &Image,
        base: [f32; 3],
        scale: f32,
        key: u64,
        lighting: Lighting,
    ) {
        const SIDES: usize = 7;
        let mut rng = Rng::new(key ^ 0xb0u64);
        let radius = units(rng.range(0.9, 1.7) * scale) as f32;
        let height = units(rng.range(0.7, 1.5) * scale) as f32;
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let ring = |rng: &mut Rng, level: f32, spread: f32, twist: f32| -> Vec<Vec3i16> {
            (0..SIDES)
                .map(|side| {
                    let theta = (side as f32 + twist) / SIDES as f32 * std::f32::consts::TAU;
                    let r = radius * spread * rng.range(0.75, 1.1);
                    Vec3i16::new(
                        (theta.sin() * r) as i16,
                        (-level * rng.range(0.85, 1.1)) as i16,
                        (theta.cos() * r) as i16,
                    )
                })
                .collect()
        };
        let low = ring(&mut rng, -height * 0.15, 1.0, 0.0);
        let middle = ring(&mut rng, height * 0.6, 0.85, 0.5);
        let top = Vec3i16::new(
            (rng.range(-0.2, 0.2) * radius) as i16,
            -height as i16,
            (rng.range(-0.2, 0.2) * radius) as i16,
        );
        let rotation = Mat3Fx12::rotation_y(crate::render::angle(yaw));
        renderer.set_object(world_to_psx(base[0], base[1], base[2]), rotation);
        renderer.set_lighting(
            rotation,
            lighting.sun_direction,
            lighting.sun,
            lighting.ambient,
        );
        let fog_enabled = renderer.fog_enabled;
        renderer.fog_enabled = false;
        let step = (image.width / SIDES as u8).max(1);
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            let (u0, u1) = (
                image.u + side as u8 * step,
                image.u + side as u8 * step + step,
            );
            let (v0, v1, v2) = (
                image.v,
                image.v + image.height / 2,
                image.v.wrapping_add(image.height).wrapping_sub(1),
            );
            let mut faces: Vec<BoulderFace> = vec![
                (
                    vec![middle[side], middle[next], low[side], low[next]],
                    vec![(u0, v1), (u1, v1), (u0, v2), (u1, v2)],
                ),
                (
                    vec![top, middle[side], middle[next]],
                    vec![
                        (((u0 as u16 + u1 as u16) / 2) as u8, v0),
                        (u0, v1),
                        (u1, v1),
                    ],
                ),
            ];
            for (points, uvs) in &mut faces {
                let outward = {
                    let count = points.len() as i32;
                    let sum = points.iter().fold((0, 0, 0), |acc, p| {
                        (
                            acc.0 + p.x as i32,
                            acc.1 + p.y as i32 + (height as i32) / 2,
                            acc.2 + p.z as i32,
                        )
                    });
                    (sum.0 / count, sum.1 / count, sum.2 / count)
                };
                let normal = crate::car::normal(points);
                let facing = normal.x as i32 * outward.0
                    + normal.y as i32 * outward.1
                    + normal.z as i32 * outward.2;
                if facing < 0 {
                    points.swap(0, 1);
                    uvs.swap(0, 1);
                    if points.len() == 4 {
                        points.swap(2, 3);
                        uvs.swap(2, 3);
                    }
                }
                let normal = crate::car::normal(points);
                renderer.gte.rgbc = [128, 128, 128, 0];
                let lit = renderer.gte.nccs(normal);
                let corners: Vec<Point> = points
                    .iter()
                    .zip(uvs.iter())
                    .map(|(&p, &uv)| Point::new(p, lit, uv))
                    .collect();
                if renderer.backface(&corners) {
                    continue;
                }
                renderer.polygon(&corners, Surface::textured(image.texture), 0, 1);
            }
        }
        renderer.fog_enabled = fog_enabled;
    }

    fn prop_image(
        &self,
        atlas: &Atlas,
        art: &Art,
        prop: Prop,
        key: u64,
    ) -> (Image, f32, f32, bool) {
        let pick = |family: &Vec<Image>| family[(key % family.len() as u64) as usize];
        match prop {
            Prop::Fir => (pick(&art.fir), 4.5, 9.0, true),
            Prop::Pine => (pick(&art.pine), 5.5, 11.0, true),
            Prop::SnowFir => (pick(&art.snow_fir), 4.5, 9.0, true),
            Prop::Broadleaf => (pick(&art.broadleaf), 7.0, 14.0, true),
            Prop::Kapok => (pick(&art.kapok), 9.0, 18.0, true),
            Prop::AutumnTree => (pick(&art.autumn), 7.0, 14.0, true),
            Prop::BareTree => (pick(&art.bare), 7.0, 14.0, true),
            Prop::Joshua => (pick(&art.joshua), 4.0, 8.0, true),
            Prop::Cactus => (pick(&art.cactus), 1.8, 3.6, true),
            Prop::Rock => (pick(&art.rock), 2.6, 2.6, true),
            Prop::SnowRock => (pick(&art.snow_rock), 2.6, 2.6, true),
            Prop::SmallRock => (pick(&art.small_rock), 0.9, 0.9, true),
            Prop::Bush => (pick(&art.bush), 3.0, 2.25, true),
            Prop::Scrub => (pick(&art.scrub), 2.6, 1.95, true),
            Prop::Boulder => (atlas.stone, 2.0, 2.0, false),
            Prop::HayBale => (pick(&art.hay), 1.9, 1.4, true),
            Prop::FencePost => (atlas.post, 0.5, 1.4, false),
            Prop::Spectator => (atlas.spectators[(key % 4) as usize], 0.8, 1.75, false),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn billboard(
        &self,
        renderer: &mut Renderer,
        image: &Image,
        base: [f32; 3],
        width: f32,
        height: f32,
        right: (f32, f32),
        color: Rgb,
        tint: Rgb,
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
        renderer.polygon(&points, Surface::textured(image.texture).tinted(tint), 0, 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn chevron(
        &self,
        renderer: &mut Renderer,
        image: &Image,
        track: &Track,
        index: usize,
        mirror: bool,
        base: [f32; 3],
        color: Rgb,
        tint: Rgb,
    ) {
        let sample = track.sample(index);
        let (rx, rz) = sample.right();
        renderer.set_object(world_to_psx(base[0], base[1], base[2]), Mat3Fx12::IDENTITY);
        let half = 1.1;
        let (dx, dz) = (units(rx * half) as i16, units(rz * half) as i16);
        let top = -units(2.2) as i16;
        let (u0, u1) = if mirror {
            (image.u + 31, image.u)
        } else {
            (image.u, image.u + 31)
        };
        let points = [
            Point::new(Vec3i16::new(-dx, top, -dz), color, (u0, image.v)),
            Point::new(Vec3i16::new(dx, top, dz), color, (u1, image.v)),
            Point::new(Vec3i16::new(-dx, 0, -dz), color, (u0, image.v + 31)),
            Point::new(Vec3i16::new(dx, 0, dz), color, (u1, image.v + 31)),
        ];
        renderer.polygon(&points, Surface::textured(image.texture).tinted(tint), 0, 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn sign(
        &self,
        renderer: &mut Renderer,
        image: &Image,
        sample: &crate::track::Sample,
        mirror: bool,
        base: [f32; 3],
        camera_right: (f32, f32),
        color: Rgb,
    ) {
        renderer.set_object(world_to_psx(base[0], base[1], base[2]), Mat3Fx12::IDENTITY);
        renderer.force_otz = Some(renderer.depth_of(Vec3i16::default()) as u32);
        let (rx, rz) = sample.right();
        let width = SIGN_WIDTH * image.width as f32 / SIGN_TEXELS;
        let height = width * image.height as f32 / image.width as f32;
        let (dx, dz) = (
            units(rx * width * 0.5) as i16,
            units(rz * width * 0.5) as i16,
        );
        let (bottom, top) = (
            -units(SIGN_MOUNT) as i16,
            -units(SIGN_MOUNT + height) as i16,
        );
        let (u_last, v_last) = (image.u + image.width - 1, image.v + image.height - 1);
        let (u0, u1) = if mirror {
            (u_last, image.u)
        } else {
            (image.u, u_last)
        };
        let points = [
            Point::new(Vec3i16::new(-dx, top, -dz), color, (u0, image.v)),
            Point::new(Vec3i16::new(dx, top, dz), color, (u1, image.v)),
            Point::new(Vec3i16::new(-dx, bottom, -dz), color, (u0, v_last)),
            Point::new(Vec3i16::new(dx, bottom, dz), color, (u1, v_last)),
        ];
        renderer.polygon(&points, Surface::textured(image.texture), 0, 0);
        let post = units(SIGN_POST * 0.5);
        let (px, pz) = (
            (camera_right.0 * post as f32) as i16,
            (camera_right.1 * post as f32) as i16,
        );
        let post_top = -units(SIGN_MOUNT + height * 0.5) as i16;
        let grey = scale_rgb(color, 0.55);
        let points = [
            Point::new(Vec3i16::new(-px, post_top, -pz), grey, (0, 0)),
            Point::new(Vec3i16::new(px, post_top, pz), grey, (0, 0)),
            Point::new(Vec3i16::new(-px, 0, -pz), grey, (0, 0)),
            Point::new(Vec3i16::new(px, 0, pz), grey, (0, 0)),
        ];
        renderer.polygon(&points, Surface::flat(), 1, 0);
        renderer.force_otz = None;
    }

    #[allow(clippy::too_many_arguments)]
    fn gate(
        &self,
        renderer: &mut Renderer,
        atlas: &Atlas,
        track: &Track,
        index: usize,
        color: Rgb,
        band: u8,
    ) {
        let sample = track.sample(index);
        let origin = world_to_psx(sample.x, sample.y, sample.z);
        renderer.set_object(origin, Mat3Fx12::IDENTITY);
        let (rx, rz) = sample.right();
        let span = ROAD_HALF + 1.5;
        let at = |lateral: f32, height: f32| {
            Vec3i16::new(
                units(rx * lateral) as i16,
                -units(height) as i16,
                units(rz * lateral) as i16,
            )
        };
        let banner = atlas.banner.in_fog_band(band);
        let tint = color;
        let points = [
            Point::new(at(-span, 6.4), color, (banner.u, banner.v)),
            Point::new(at(span, 6.4), color, (banner.u + 127, banner.v)),
            Point::new(at(-span, 4.8), color, (banner.u, banner.v + 15)),
            Point::new(at(span, 4.8), color, (banner.u + 127, banner.v + 15)),
        ];
        renderer.polygon(
            &points,
            Surface::textured(banner.texture).tinted(tint),
            -40,
            2,
        );
        for side in [-1.0f32, 1.0] {
            let lateral = side * span;
            let (fx, fz) = sample.forward();
            let post = |dl: f32, df: f32, height: f32| {
                Vec3i16::new(
                    units(rx * (lateral + dl) + fx * df) as i16,
                    -units(height) as i16,
                    units(rz * (lateral + dl) + fz * df) as i16,
                )
            };
            let dark = [60, 60, 70];
            for (a, b) in [
                ((-0.2, -0.2), (0.2, -0.2)),
                ((0.2, -0.2), (0.2, 0.2)),
                ((0.2, 0.2), (-0.2, 0.2)),
                ((-0.2, 0.2), (-0.2, -0.2)),
            ] {
                let points = [
                    Point::new(post(a.0, a.1, 6.5), dark, (0, 0)),
                    Point::new(post(b.0, b.1, 6.5), dark, (0, 0)),
                    Point::new(post(a.0, a.1, 0.0), color, (0, 0)),
                    Point::new(post(b.0, b.1, 0.0), color, (0, 0)),
                ];
                if !renderer.backface(&points) {
                    renderer.polygon(&points, Surface::gouraud(), -20, 1);
                }
            }
        }
    }

    fn draw_particles(
        &self,
        renderer: &mut Renderer,
        atlas: &Atlas,
        camera: &Camera,
        lighting: Lighting,
    ) {
        let right = (camera.yaw.cos(), -camera.yaw.sin());
        for particle in &self.particles {
            let color = scale_rgb(
                lerp_rgb(particle.color, lighting.fog, 0.2),
                0.55 * lighting.shade(),
            );
            let color = lighting.lit_by_beam(color, particle.position[0], particle.position[2]);
            let image = atlas.puff;
            renderer.set_object(
                world_to_psx(
                    particle.position[0],
                    particle.position[1],
                    particle.position[2],
                ),
                Mat3Fx12::IDENTITY,
            );
            let half = particle.size * 0.5;
            let (dx, dz) = (units(right.0 * half) as i16, units(right.1 * half) as i16);
            let (top, bottom) = (-units(half) as i16, units(half) as i16);
            let points = [
                Point::new(Vec3i16::new(-dx, top, -dz), color, (image.u, image.v)),
                Point::new(Vec3i16::new(dx, top, dz), color, (image.u + 15, image.v)),
                Point::new(
                    Vec3i16::new(-dx, bottom, -dz),
                    color,
                    (image.u, image.v + 15),
                ),
                Point::new(
                    Vec3i16::new(dx, bottom, dz),
                    color,
                    (image.u + 15, image.v + 15),
                ),
            ];
            let fog_enabled = renderer.fog_enabled;
            renderer.fog_enabled = false;
            renderer.polygon(
                &points,
                Surface::textured(image.texture).blended(BlendMode::Half),
                -380,
                0,
            );
            renderer.fog_enabled = fog_enabled;
        }
    }

    pub fn update_precipitation(
        &mut self,
        dt: f32,
        weather: Weather,
        intensity: f32,
        camera: &Camera,
    ) {
        let (target, fall) = match weather {
            Weather::Rain => ((PRECIPITATION_DROPS as f32 * intensity) as usize, 9.0),
            Weather::Snow => ((PRECIPITATION_DROPS as f32 * 0.8 * intensity) as usize, 1.3),
            _ => (0, 0.0),
        };
        let forward = (camera.yaw.sin(), camera.yaw.cos());
        let center = (camera.x + forward.0 * 12.0, camera.z + forward.1 * 12.0);
        let rng = &mut self.rng;
        let spawn = |rng: &mut Rng, top: bool| {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let radius = PRECIPITATION_RADIUS * rng.float().sqrt();
            let height = if top {
                rng.range(6.0, 12.0)
            } else {
                rng.range(-3.0, 12.0)
            };
            [
                center.0 + angle.sin() * radius,
                camera.y + height,
                center.1 + angle.cos() * radius,
                rng.range(0.0, std::f32::consts::TAU),
            ]
        };
        self.drops.truncate(target);
        while self.drops.len() < target {
            let drop = spawn(rng, false);
            self.drops.push(drop);
        }
        for drop in &mut self.drops {
            drop[1] -= fall * dt;
            if weather == Weather::Snow {
                drop[3] += dt * 1.7;
                drop[0] += drop[3].sin() * 0.6 * dt;
                drop[2] += drop[3].cos() * 0.6 * dt;
            }
            let (dx, dz) = (drop[0] - center.0, drop[2] - center.1);
            let outside = dx * dx + dz * dz > PRECIPITATION_RADIUS * PRECIPITATION_RADIUS;
            if drop[1] < camera.y - 4.0 || outside {
                *drop = spawn(rng, !outside);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_precipitation(
        &self,
        renderer: &mut Renderer,
        weather: Weather,
        velocity: (f32, f32),
        fog: Rgb,
    ) {
        let fall = if weather == Weather::Snow { 1.3 } else { 9.0 };
        let streak = 0.018;
        let tail = Vec3i16::new(
            units(-velocity.0 * streak) as i16,
            units(fall * streak) as i16,
            units(-velocity.1 * streak) as i16,
        );
        for drop in &self.drops {
            renderer.set_object(world_to_psx(drop[0], drop[1], drop[2]), Mat3Fx12::IDENTITY);
            let head = renderer.gte.rtps(Vec3i16::default());
            if head.sz < crate::render::NEAR_Z
                || head.sx < -8
                || head.sx > SCREEN_WIDTH as i16 + 8
                || head.sy < -16
                || head.sy > SCREEN_HEIGHT as i16 + 8
            {
                continue;
            }
            let distance = head.sz as f32 / crate::render::UNITS_PER_METER;
            let haze = fog_amount(distance);
            match weather {
                Weather::Snow => {
                    let size =
                        ((PROJECTION_H as f32 * 4.0) / head.sz as f32).clamp(1.0, 6.0) as u16;
                    let color = lerp_rgb([245, 245, 250], fog, haze);
                    renderer.ot.add(
                        (head.sz as u32).saturating_sub(350),
                        Primitive::Sprite {
                            x: head.sx - size as i16 / 2,
                            y: head.sy - size as i16 / 2,
                            width: size,
                            height: size,
                            u: 0,
                            v: 0,
                            color,
                            texture: None,
                            blend: None,
                        },
                    );
                }
                _ => {
                    let mut end = renderer.gte.rtps(tail);
                    let (dx, dy) = (
                        end.sx as i32 - head.sx as i32,
                        end.sy as i32 - head.sy as i32,
                    );
                    let length = ((dx * dx + dy * dy) as f32).sqrt();
                    if length > 14.0 {
                        let scale = 14.0 / length;
                        end.sx = head.sx + (dx as f32 * scale) as i16;
                        end.sy = head.sy + (dy as f32 * scale) as i16;
                    }
                    let brightness = 1.0 - haze * 0.85;
                    let bright = scale_rgb([120, 130, 150], brightness);
                    let dim = scale_rgb([40, 45, 55], brightness);
                    let mut push = |offset: i16| {
                        renderer.ot.add(
                            (head.sz as u32).saturating_sub(350),
                            Primitive::Line {
                                vertices: [
                                    Vertex::new(head.sx + offset, head.sy).with_color(dim),
                                    Vertex::new(end.sx + offset, end.sy).with_color(bright),
                                ],
                                gouraud: true,
                                blend: Some(BlendMode::Add),
                            },
                        );
                    };
                    push(0);
                    if distance < 5.0 {
                        push(1);
                    }
                }
            }
        }
    }

    pub fn draw_weather(&self, overlay: &mut Vec<Primitive>, weather: Weather, intensity: f32) {
        if intensity <= 0.05 {
            return;
        }
        let count = (self.flakes.len() as f32 * intensity) as usize;
        for flake in self.flakes.iter().take(count) {
            let (x, y) = (flake.x as i16, flake.y as i16);
            match weather {
                Weather::Snow | Weather::Rain => {}
                Weather::Dust => {
                    if flake.speed > 1.2 {
                        overlay.push(Primitive::Sprite {
                            x,
                            y,
                            width: 1,
                            height: 1,
                            u: 0,
                            v: 0,
                            color: [120, 80, 40],
                            texture: None,
                            blend: Some(BlendMode::Add),
                        });
                    }
                }
                Weather::Clear => {}
            }
        }
        if weather == Weather::Dust {
            let haze = (intensity * 40.0) as u8;
            overlay.insert(
                0,
                screen_quad(
                    0,
                    0,
                    SCREEN_WIDTH as i16,
                    SCREEN_HEIGHT as i16,
                    [[haze, haze / 2, haze / 4]; 4],
                    Some(BlendMode::AddQuarter),
                ),
            );
        }
    }
}

pub fn fog_tint_from(average: Rgb, fog: Rgb) -> Rgb {
    [0, 1, 2].map(|c| ((fog[c] as u32 * 128) / (average[c] as u32).max(16)).min(255) as u8)
}

fn view_window(track: &Track, camera: &Camera, from_index: usize) -> (usize, usize) {
    let sample = track.sample(from_index);
    let facing = (camera.yaw - sample.heading).cos();
    let behind = if facing < -0.3 {
        VIEW_SAMPLES
    } else if facing < 0.4 {
        VIEW_SAMPLES / 2
    } else {
        BEHIND_SAMPLES
    };
    let ahead = if facing < -0.3 {
        BEHIND_SAMPLES + 3
    } else if facing < 0.4 {
        VIEW_SAMPLES / 2
    } else {
        VIEW_SAMPLES
    };
    (
        from_index.saturating_sub(behind),
        (from_index + ahead).min(track.samples.len() - 2),
    )
}

fn draw_backdrop(renderer: &mut Renderer, image: &Image, shade: f32) {
    const COPIES: usize = 8;
    const PANELS: usize = 8;
    const RADIUS: f32 = 28000.0;
    let ground_level = 1400i16;
    let top = ground_level
        - (RADIUS * std::f32::consts::TAU / COPIES as f32 * image.height as f32 / 256.0) as i16;
    let texture = band_image(image, 2).texture;
    renderer.set_sky_object(Mat3Fx12::IDENTITY);
    renderer.force_bucket = Some(OT_SIZE - 2);
    let fog_enabled = renderer.fog_enabled;
    renderer.fog_enabled = false;
    let segments = COPIES * PANELS;
    let step = (256 / PANELS) as u8;
    for segment in 0..segments {
        let at = |index: usize| {
            let theta = index as f32 / segments as f32 * std::f32::consts::TAU;
            ((theta.sin() * RADIUS) as i16, (theta.cos() * RADIUS) as i16)
        };
        let ((ax, az), (bx, bz)) = (at(segment), at(segment + 1));
        let panel = (segment % PANELS) as u8;
        let mirrored = (segment / PANELS) % 2 == 1;
        let start = panel as u16 * step as u16;
        let end = (start + step as u16 - 1) as u8;
        let start = start as u8;
        let (ua, ub) = if mirrored {
            (255 - start, 255 - end)
        } else {
            (start, end)
        };
        let (v0, v1) = (image.v, image.v + image.height - 1);
        let color = scale_rgb([128, 128, 128], shade);
        let points = [
            Point::new(Vec3i16::new(bx, top, bz), color, (ub, v0)),
            Point::new(Vec3i16::new(ax, top, az), color, (ua, v0)),
            Point::new(Vec3i16::new(bx, ground_level, bz), color, (ub, v1)),
            Point::new(Vec3i16::new(ax, ground_level, az), color, (ua, v1)),
        ];
        renderer.polygon(&points, Surface::textured(texture), 0, 0);
    }
    renderer.fog_enabled = fog_enabled;
    renderer.force_bucket = None;
}

fn draw_stars(renderer: &mut Renderer, seed: u64, camera: &Camera, horizon: i16, amount: f32) {
    let field = std::f32::consts::TAU;
    let pixels_per_radian = PROJECTION_H as f32;
    for star in 0..120u64 {
        let azimuth = hash_float(seed ^ 0x57a5, star, 0) * field;
        let altitude = 0.05 + hash_float(seed ^ 0x57a5, star, 1).powi(2) * 0.9;
        let mut relative = azimuth - camera.yaw;
        while relative > std::f32::consts::PI {
            relative -= field;
        }
        while relative < -std::f32::consts::PI {
            relative += field;
        }
        if relative.abs() > 0.8 {
            continue;
        }
        let x = SCREEN_WIDTH as f32 / 2.0 + relative.tan() * pixels_per_radian;
        let y = horizon as f32 - altitude.tan() * pixels_per_radian;
        if y < 0.0 || y >= horizon as f32 {
            continue;
        }
        let brightness = (hash_float(seed ^ 0x57a5, star, 2) * 0.7 + 0.3) * amount;
        let value = (brightness * 230.0) as u8;
        renderer.ot.add_at(
            OT_SIZE - 1,
            Primitive::Sprite {
                x: x as i16,
                y: y as i16,
                width: 1,
                height: 1,
                u: 0,
                v: 0,
                color: [value, value, value.saturating_add(20)],
                texture: None,
                blend: Some(BlendMode::Add),
            },
        );
    }
}

type RibbonVertex = (Vec3i16, f32, f32, f32, f32);

type BoulderFace = (Vec<Vec3i16>, Vec<(u8, u8)>);

pub fn sun_direction_at(heading_offset: f32, elevation: f32) -> [f32; 3] {
    [
        heading_offset.sin() * elevation.cos(),
        elevation.sin(),
        heading_offset.cos() * elevation.cos(),
    ]
}
