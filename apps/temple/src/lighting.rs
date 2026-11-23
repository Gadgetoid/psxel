use crate::biome::Rgb;
use crate::mesh::{add, dot, length, scale, sub, to_rgb, Vec3, Zone};
use crate::terrain::{ground_height, smoothstep, value_noise, TEMPLE_Z};

const SUN_DIRECTION: Vec3 = [0.42, 0.82, -0.39];
const SUN: Vec3 = [150.0, 136.0, 100.0];
const AMBIENT: Vec3 = [56.0, 66.0, 54.0];
const SKY: Vec3 = [20.0, 28.0, 26.0];
const INTERIOR_AMBIENT: Vec3 = [13.0, 11.0, 10.0];
const DOOR_SPILL: Vec3 = [78.0, 84.0, 70.0];
const SHAFT: Vec3 = [135.0, 130.0, 105.0];
const TORCH: Vec3 = [255.0, 135.0, 50.0];
const TORCH_RANGE: f32 = 8.0;

pub fn sun_direction() -> Vec3 {
    SUN_DIRECTION
}

pub struct Torch {
    pub position: Vec3,
    pub zone: Zone,
}

#[derive(Clone, Copy)]
pub struct Shade {
    pub base: Vec3,
    pub glow: Vec3,
    pub torch: usize,
}

pub struct Lighting {
    pub torches: Vec<Torch>,
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
        (0..self.torches.len())
            .map(|index| {
                let offset = index as f32;
                0.82 + 0.1 * (time * 9.1 + offset * 1.7).sin()
                    + 0.08 * (time * 23.3 + offset * 4.1).sin()
            })
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
                let door = (1.0 - (14.0 - x.abs()) / 6.0).max(0.0).powf(1.5);
                let radius = x.hypot(z - TEMPLE_Z);
                let facing = normal.map_or(0.6, |normal| 0.35 + 0.65 * normal[1].max(0.0));
                let shaft = (1.0 - radius / 4.5).max(0.0).powi(2) * facing;
                add(
                    INTERIOR_AMBIENT,
                    add(scale(DOOR_SPILL, door), scale(SHAFT, shaft)),
                )
            }
        };
        let mut glow = [0.0; 3];
        let (mut torch, mut strongest) = (usize::MAX, 0.0);
        for (index, light) in self.torches.iter().enumerate() {
            if light.zone == Zone::Interior && zone == Zone::Exterior {
                continue;
            }
            let offset = sub(light.position, position);
            let distance = length(offset);
            if distance >= TORCH_RANGE {
                continue;
            }
            let lambert = match normal {
                Some(normal) => dot(normal, scale(offset, 1.0 / distance.max(0.01))),
                None => 0.8,
            };
            if lambert <= 0.0 {
                continue;
            }
            let amount = (1.0 - distance / TORCH_RANGE).powi(2) * (0.3 + 0.7 * lambert) * 1.4;
            glow = add(glow, scale(TORCH, amount));
            if amount > strongest {
                strongest = amount;
                torch = index;
            }
        }
        Shade { base, glow, torch }
    }
}
