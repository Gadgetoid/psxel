use crate::biome::Rgb;
use crate::mesh::{add, dot, multiply, normalize, scale, to_rgb, Vec3, Zone};
use crate::wreck;

const SUN: Vec3 = [150.0, 160.0, 152.0];
const AMBIENT: Vec3 = [50.0, 62.0, 66.0];
const DEEP_AMBIENT: Vec3 = [22.0, 34.0, 44.0];
const INTERIOR: Vec3 = [12.0, 16.0, 20.0];
const OPENING: Vec3 = [40.0, 60.0, 70.0];

pub fn sun_direction() -> Vec3 {
    normalize([0.25, 1.0, 0.18])
}

pub fn water_filter(depth: f32) -> Vec3 {
    let depth = depth.max(0.0);
    [
        (-depth / 18.0).exp(),
        (-depth / 55.0).exp(),
        (-depth / 85.0).exp(),
    ]
}

#[derive(Clone, Copy)]
pub struct Shade {
    pub base: Vec3,
    pub glow: Vec3,
    pub torch: usize,
}

pub struct Lighting;

impl Lighting {
    pub fn shade(&self, position: Vec3, normal: Vec3, zone: Zone) -> Shade {
        Shade {
            base: self.light(position, Some(normal), zone),
            glow: [0.0; 3],
            torch: usize::MAX,
        }
    }

    pub fn billboard(&self, position: Vec3, zone: Zone) -> Rgb {
        to_rgb(self.light(position, None, zone))
    }

    fn light(&self, position: Vec3, normal: Option<Vec3>, zone: Zone) -> Vec3 {
        let filter = water_filter(-position[1]);
        match zone {
            Zone::Interior => {
                let opening = wreck::opening_light(position);
                add(INTERIOR, scale(multiply(OPENING, filter), opening))
            }
            _ => {
                let facing = normal.map_or(0.65, |normal| dot(normal, sun_direction()).max(0.0));
                let skylight = normal.map_or(0.5, |normal| normal[1].max(0.0) * 0.5 + 0.25);
                add(
                    DEEP_AMBIENT,
                    multiply(
                        add(scale(AMBIENT, skylight), scale(SUN, facing * 0.8)),
                        filter,
                    ),
                )
            }
        }
    }
}
