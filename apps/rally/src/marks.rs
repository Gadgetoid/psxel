use std::collections::VecDeque;

use psxel_math::{Mat3Fx12, Vec3i16};
use psxel_prim::BlendMode;

use crate::biome::{lerp_rgb, Rgb};
use crate::car::Car;
use crate::render::{fog_amount, world_to_psx, Camera, Point, Renderer, Surface};
use crate::track::Track;

const LIMIT: usize = 900;
const STEP: f32 = 0.6;
const BREAK: f32 = 3.0;
const WIDTH: f32 = 0.24;
const LIFT: f32 = 0.04;
const RADIUS: f32 = 90.0;
const MARK_COLOR: Rgb = [22, 18, 14];
const STALE_FADE: f32 = 0.35;
const BIAS: i32 = -200;

#[derive(Clone, Copy)]
struct Mark {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Default)]
pub struct TyreMarks {
    segments: VecDeque<(Mark, Mark)>,
    last: [Option<Mark>; 2],
}

impl TyreMarks {
    pub fn record(&mut self, car: &Car, track: &Track, marking: bool) {
        for (wheel, (x, z)) in car.rear_wheels().into_iter().enumerate() {
            if !marking {
                self.last[wheel] = None;
                continue;
            }
            let y = track.surface(x, z, car.y + 0.5).0 + LIFT;
            let mark = Mark { x, y, z };
            match self.last[wheel] {
                Some(last) => {
                    let distance = (x - last.x).hypot(z - last.z);
                    if distance > BREAK {
                        self.last[wheel] = Some(mark);
                    } else if distance > STEP {
                        self.segments.push_back((last, mark));
                        self.last[wheel] = Some(mark);
                    }
                }
                None => self.last[wheel] = Some(mark),
            }
        }
        while self.segments.len() > LIMIT {
            self.segments.pop_front();
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, camera: &Camera, fog: Rgb) {
        let fog_enabled = renderer.fog_enabled;
        renderer.fog_enabled = false;
        for (age, &(a, b)) in self.segments.iter().enumerate() {
            let distance = (a.x - camera.x).hypot(a.z - camera.z);
            if distance > RADIUS {
                continue;
            }
            let length = (b.x - a.x).hypot(b.z - a.z).max(0.01);
            let (px, pz) = (
                -(b.z - a.z) / length * WIDTH * 0.5,
                (b.x - a.x) / length * WIDTH * 0.5,
            );
            let staleness = 1.0 - age as f32 / self.segments.len() as f32;
            let fade = (fog_amount(distance) + staleness * STALE_FADE).min(1.0);
            let color = lerp_rgb(MARK_COLOR, fog, fade);
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
            let points = [
                Point::new(local(a.x - px, a.y, a.z - pz), color, (0, 0)),
                Point::new(local(a.x + px, a.y, a.z + pz), color, (0, 0)),
                Point::new(local(b.x - px, b.y, b.z - pz), color, (0, 0)),
                Point::new(local(b.x + px, b.y, b.z + pz), color, (0, 0)),
            ];
            renderer.polygon(&points, Surface::flat().blended(BlendMode::Half), BIAS, 0);
        }
        renderer.fog_enabled = fog_enabled;
    }
}
