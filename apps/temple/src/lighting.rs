use crate::biome::Rgb;
use crate::grotto;
use crate::mesh::{add, dot, length, scale, sub, to_rgb, Vec3, Zone};
use crate::terrain::{ground_height, smoothstep, value_noise, CLIFF_X, FALLS_Z, TEMPLE_Z};

const SUN_DIRECTION: Vec3 = [0.42, 0.82, -0.39];
const SUN: Vec3 = [150.0, 136.0, 100.0];
const AMBIENT: Vec3 = [56.0, 66.0, 54.0];
const SKY: Vec3 = [20.0, 28.0, 26.0];
const INTERIOR_AMBIENT: Vec3 = [13.0, 11.0, 10.0];
const DOOR_SPILL: Vec3 = [78.0, 84.0, 70.0];
const SHAFT: Vec3 = [135.0, 130.0, 105.0];
const CAVE_AMBIENT: Vec3 = [18.0, 24.0, 28.0];
const MOUTH_SPILL: Vec3 = [70.0, 90.0, 95.0];
const MOUTH: Vec3 = [CLIFF_X, 2.5, FALLS_Z];
const GROTTO_AMBIENT: Vec3 = [30.0, 36.0, 34.0];
const GROTTO_SKY: Vec3 = [150.0, 150.0, 120.0];
const GROTTO_MOUTH: Vec3 = [90.0, 105.0, 95.0];

pub fn sun_direction() -> Vec3 {
    SUN_DIRECTION
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    Torch,
    Crystal,
    Shimmer,
}

impl LightKind {
    fn color(self) -> Vec3 {
        match self {
            Self::Torch => [255.0, 135.0, 50.0],
            Self::Crystal => [70.0, 210.0, 230.0],
            Self::Shimmer => [80.0, 110.0, 120.0],
        }
    }

    fn range(self) -> f32 {
        match self {
            Self::Torch => 8.0,
            Self::Crystal => 10.0,
            Self::Shimmer => 9.0,
        }
    }

    fn flicker(self, time: f32, offset: f32) -> f32 {
        match self {
            Self::Torch => {
                0.82 + 0.1 * (time * 9.1 + offset * 1.7).sin()
                    + 0.08 * (time * 23.3 + offset * 4.1).sin()
            }
            Self::Crystal => 0.85 + 0.15 * (time * 1.3 + offset * 2.3).sin(),
            Self::Shimmer => {
                0.7 + 0.2 * (time * 7.3 + offset).sin() + 0.1 * (time * 13.1 + offset * 2.0).sin()
            }
        }
    }
}

pub struct Light {
    pub position: Vec3,
    pub zone: Zone,
    pub kind: LightKind,
}

#[derive(Clone, Copy)]
pub struct Shade {
    pub base: Vec3,
    pub glow: Vec3,
    pub torch: usize,
}

pub struct Lighting {
    pub lights: Vec<Light>,
}

impl Lighting {
    pub fn shade(&self, position: Vec3, normal: Vec3, zone: Zone) -> Shade {
        self.light(position, Some(normal), zone)
    }

    pub fn billboard(&self, position: Vec3, zone: Zone) -> Rgb {
        let shade = self.light(position, None, zone);
        to_rgb(add(shade.base, shade.glow))
    }

    pub fn flicker(&self, time: f32) -> Vec<f32> {
        self.lights
            .iter()
            .enumerate()
            .map(|(index, light)| light.kind.flicker(time, index as f32))
            .collect()
    }

    fn light(&self, position: Vec3, normal: Option<Vec3>, zone: Zone) -> Shade {
        let [x, y, z] = position;
        let base = match zone {
            Zone::Exterior => {
                let dapple = 0.45 + 0.55 * value_noise(x / 3.5 + y * 0.15, z / 3.5, 31);
                let (sun, sky, occlusion) = match normal {
                    Some(normal) => {
                        let occlusion = if normal[1].abs() < 0.7 {
                            0.6 + 0.4 * smoothstep(0.0, 2.5, y - ground_height(x, z))
                        } else {
                            1.0
                        };
                        (
                            dot(normal, SUN_DIRECTION).max(0.0),
                            normal[1].max(0.0),
                            occlusion,
                        )
                    }
                    None => (0.6, 0.6, 1.0),
                };
                scale(
                    add(AMBIENT, add(scale(SUN, sun * dapple), scale(SKY, sky))),
                    occlusion,
                )
            }
            Zone::Interior => {
                let corridor = (1.0 - (14.0 - x.abs()) / 6.0).max(0.0).powf(1.5);
                let shrine = if y > 13.5 {
                    (1.0 - (z - (TEMPLE_Z - 4.0)) / 5.0).max(0.0).powf(1.5)
                } else {
                    0.0
                };
                let door = corridor.max(shrine);
                let radius = x.hypot(z - TEMPLE_Z);
                let facing = normal.map_or(0.6, |normal| 0.35 + 0.65 * normal[1].max(0.0));
                let shaft = (1.0 - radius / 4.5).max(0.0).powi(2) * facing;
                add(
                    INTERIOR_AMBIENT,
                    add(scale(DOOR_SPILL, door), scale(SHAFT, shaft)),
                )
            }
            Zone::Cave => {
                let offset = sub(MOUTH, position);
                let distance = length(offset);
                let facing = normal.map_or(0.7, |normal| {
                    0.3 + 0.7 * dot(normal, scale(offset, 1.0 / distance.max(0.01))).max(0.0)
                });
                let spill = (1.0 - distance / 20.0).max(0.0).powi(2) * facing;
                add(CAVE_AMBIENT, scale(MOUTH_SPILL, spill))
            }
            Zone::Grotto => {
                let towards = |point: Vec3, reach: f32| {
                    let offset = sub(point, position);
                    let distance = length(offset);
                    let facing = normal.map_or(0.7, |normal| {
                        0.3 + 0.7 * dot(normal, scale(offset, 1.0 / distance.max(0.01))).max(0.0)
                    });
                    (1.0 - distance / reach).max(0.0).powi(2) * facing
                };
                let sky = grotto::skylights()
                    .iter()
                    .map(|&point| towards(point, 9.0))
                    .sum::<f32>();
                add(
                    GROTTO_AMBIENT,
                    add(
                        scale(GROTTO_SKY, sky),
                        scale(GROTTO_MOUTH, towards(grotto::mouth(), 12.0)),
                    ),
                )
            }
        };
        let mut glow = [0.0; 3];
        let (mut torch, mut strongest) = (usize::MAX, 0.0);
        for (index, light) in self.lights.iter().enumerate() {
            let reaches =
                light.zone == zone || (light.zone == Zone::Exterior && zone == Zone::Interior);
            if !reaches {
                continue;
            }
            let offset = sub(light.position, position);
            let distance = length(offset);
            let range = light.kind.range();
            if distance >= range {
                continue;
            }
            let lambert = match normal {
                Some(normal) => dot(normal, scale(offset, 1.0 / distance.max(0.01))),
                None => 0.8,
            };
            if lambert <= 0.0 {
                continue;
            }
            let amount = (1.0 - distance / range).powi(2) * (0.3 + 0.7 * lambert) * 1.4;
            glow = add(glow, scale(light.kind.color(), amount));
            if amount > strongest {
                strongest = amount;
                torch = index;
            }
        }
        Shade { base, glow, torch }
    }
}
