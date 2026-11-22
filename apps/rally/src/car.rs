use psx_math::{Mat3Fx12, Vec3i16, Vec3i32};
use psx_prim::Texture;

use crate::biome::Rgb;
use crate::livery::{CarAtlas, BODY_FRONT, BODY_REAR};
use crate::render::{angle, world_to_psx, Point, Renderer, Surface};
use crate::textures::Image;
use crate::track::Track;

pub const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = 18.0;
const TOP_SPEED: f32 = 52.0;
const ACCELERATION: f32 = 14.0;
const BRAKING: f32 = 26.0;
const WHEELBASE: f32 = 2.6;
const MAX_STEER: f32 = 0.55;
const CAR_BIAS: i32 = -420;
const WHEEL_BIAS: i32 = 40;
const MAX_ROLL: f32 = 0.35;
const WHEEL_OFFSET_FORWARD: f32 = 1.2;
const WHEEL_OFFSET_SIDE: f32 = 0.8;

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub throttle: f32,
    pub steer: f32,
    pub handbrake: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Road,
    Shoulder,
    Rough,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Events {
    pub impact: f32,
    pub landed: f32,
    pub sliding: f32,
    pub splash: f32,
}

#[derive(Clone, Debug)]
pub struct Car {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub velocity: (f32, f32),
    pub vertical_speed: f32,
    previous_ground: f32,
    pub airborne: bool,
    pub steer: f32,
    pub yaw_rate: f32,
    pub index: usize,
    pub along: f32,
    pub lateral: f32,
    pub ground: Ground,
    pub pitch: f32,
    pub roll: f32,
    pub suspension: f32,
    pub suspension_pitch: f32,
    heave_velocity: f32,
    pub slope_pitch: f32,
    pub slope_roll: f32,
    pub wheel_spin: f32,
    pub slide: f32,
    flick: f32,
    last_throttle: f32,
    pub water_depth: f32,
    pub wet: bool,
}

impl Car {
    pub fn new(track: &Track) -> Self {
        let start = track.sample(6);
        Self {
            x: start.x,
            y: start.y,
            z: start.z,
            heading: start.heading,
            velocity: (0.0, 0.0),
            vertical_speed: 0.0,
            previous_ground: start.y,
            airborne: false,
            steer: 0.0,
            yaw_rate: 0.0,
            index: 6,
            along: 0.0,
            lateral: 0.0,
            ground: Ground::Road,
            pitch: 0.0,
            roll: 0.0,
            suspension: 0.0,
            suspension_pitch: 0.0,
            heave_velocity: 0.0,
            slope_pitch: 0.0,
            slope_roll: 0.0,
            wheel_spin: 0.0,
            slide: 0.0,
            flick: 0.0,
            last_throttle: 0.0,
            water_depth: 0.0,
            wet: false,
        }
    }

    pub fn reset_to_track(&mut self, track: &Track) {
        let road = track.terrain(self.x, self.z).road;
        let index = road.map(|road| road.index).unwrap_or(self.index).max(6);
        let sample = track.sample(index);
        self.x = sample.x;
        self.z = sample.z;
        self.y = sample.y + 0.5;
        self.heading = sample.heading;
        self.velocity = (0.0, 0.0);
        self.vertical_speed = 0.0;
        self.yaw_rate = 0.0;
        self.steer = 0.0;
        self.pitch = 0.0;
        self.roll = 0.0;
        self.airborne = true;
        self.index = index;
        self.previous_ground = sample.y;
    }

    pub fn forward(&self) -> (f32, f32) {
        (self.heading.sin(), self.heading.cos())
    }

    pub fn right(&self) -> (f32, f32) {
        (self.heading.cos(), -self.heading.sin())
    }

    pub fn speed(&self) -> f32 {
        let (fx, fz) = self.forward();
        self.velocity.0 * fx + self.velocity.1 * fz
    }

    pub fn distance(&self) -> f32 {
        (self.index as f32 + self.along) * crate::track::SAMPLE
    }

    pub fn update(&mut self, controls: Controls, track: &mut Track) -> Events {
        let mut events = Events::default();
        track.ensure(self.index + 120);
        let (index, along, _) = track.locate(self.x, self.z, self.index);
        let surface_road = track.terrain(self.x, self.z).road;
        let (road_index, lateral) = surface_road
            .map(|road| (road.index, road.lateral))
            .unwrap_or((index, f32::MAX));
        self.index = index;
        self.along = along;
        self.lateral = lateral;
        self.ground = if lateral.abs() < track.half_width(road_index) {
            Ground::Road
        } else if lateral.abs() < track.shoulder(road_index) {
            Ground::Shoulder
        } else {
            Ground::Rough
        };

        let biome_grip = track.biomes.scalar(self.distance(), |biome| biome.grip);
        let off_road_grip = track.biomes.scalar(self.distance(), |biome| {
            crate::biome::ground_traits(biome.ground).0
        });
        let off_road_drag = track.biomes.scalar(self.distance(), |biome| {
            crate::biome::ground_traits(biome.ground).1
        });
        self.water_depth = track
            .water_at(self.x, self.z)
            .map(|level| (level - self.y).max(0.0))
            .unwrap_or(0.0);
        let puddle = self.ground == Ground::Road && track.puddle_at(road_index, self.x, self.z);
        let wet = self.water_depth > 0.05 || (puddle && !self.airborne);
        if wet && !self.wet {
            let speed = self.velocity.0.hypot(self.velocity.1);
            events.splash = speed
                * if self.water_depth > 0.05 {
                    1.0 + self.water_depth * 2.0
                } else {
                    0.4
                };
        }
        self.wet = wet;
        let wet_grip = if wet { 0.75 } else { 1.0 };
        let (grip, turn_grip) = match self.ground {
            Ground::Road => (biome_grip, biome_grip),
            Ground::Shoulder => (
                (biome_grip + off_road_grip) * 0.5,
                (biome_grip + off_road_grip) * 0.5 + 0.1,
            ),
            Ground::Rough => (off_road_grip, off_road_grip + 0.15),
        };
        let (grip, turn_grip) = (grip * wet_grip, turn_grip * wet_grip);
        let rolling_drag: f32 = match self.ground {
            Ground::Road => 0.4 + (1.0 - biome_grip) * 1.2,
            Ground::Shoulder => 1.2 + off_road_drag * 0.3,
            Ground::Rough => off_road_drag,
        } + self.water_depth.min(0.4)
            * (4.0 + (self.velocity.0.hypot(self.velocity.1) - 10.0).max(0.0) * 2.0)
            + if puddle { 0.6 } else { 0.0 };
        let traction = 0.35 + 0.65 * (grip / 0.8).min(1.0);

        let steer_rate = if controls.steer.abs() > self.steer.abs() {
            3.0
        } else {
            6.0
        };
        self.steer += (controls.steer - self.steer).clamp(-steer_rate * DT, steer_rate * DT);

        let (fx, fz) = self.forward();
        let (rx, rz) = self.right();
        let mut forward_speed = self.velocity.0 * fx + self.velocity.1 * fz;
        let mut lateral_speed = self.velocity.0 * rx + self.velocity.1 * rz;

        if !self.airborne {
            if controls.throttle > 0.0 {
                if forward_speed < -0.5 {
                    forward_speed += BRAKING * traction * controls.throttle * DT;
                } else {
                    let headroom = (1.0 - forward_speed / TOP_SPEED).max(0.0);
                    forward_speed +=
                        ACCELERATION * traction * controls.throttle * (0.35 + 0.65 * headroom) * DT;
                }
            } else if controls.throttle < 0.0 {
                if forward_speed > 0.5 {
                    forward_speed += BRAKING * traction * controls.throttle * DT;
                } else {
                    forward_speed =
                        (forward_speed + ACCELERATION * 0.5 * controls.throttle * DT).max(-12.0);
                }
            }
            if controls.handbrake {
                forward_speed -= forward_speed.signum() * 3.5 * DT;
            }
            forward_speed -= forward_speed * forward_speed.abs() * 0.0016 * DT;
            forward_speed -=
                forward_speed.signum() * rolling_drag.min(forward_speed.abs() / DT) * DT;

            let slip = lateral_speed.atan2(forward_speed.abs().max(1.0)).abs();
            let power_slide = controls.throttle > 0.3 && slip > 0.1;
            let lift_off = self.last_throttle > 0.5
                && controls.throttle < 0.2
                && self.steer.abs() > 0.3
                && forward_speed > 12.0;
            if lift_off {
                self.flick = 0.35;
            }
            self.flick = (self.flick - DT).max(0.0);
            let rear_grip = if controls.handbrake {
                grip * 0.12
            } else if power_slide {
                grip * 0.42
            } else if self.flick > 0.0 {
                grip * 0.6
            } else {
                grip
            };
            self.last_throttle = controls.throttle;
            let lateral_decay = (rear_grip * 9.0 * DT).min(1.0);
            events.sliding =
                lateral_speed.abs() + (1.0 - traction) * controls.throttle.max(0.0) * 4.0;
            self.slide = lateral_speed.abs();
            lateral_speed -= lateral_speed * lateral_decay;

            let speed_factor = (forward_speed.abs() / 8.0).min(1.0);
            let steer_angle =
                self.steer * MAX_STEER * (1.0 - (forward_speed.abs() / TOP_SPEED) * 0.5);
            let geometric_yaw = forward_speed / WHEELBASE * steer_angle.tan() * speed_factor;
            let slide_boost = 1.0 + slip * 2.0;
            let grip_limit = (turn_grip * 28.0 + 3.0) / forward_speed.abs().max(6.0) * slide_boost;
            let mut target_yaw = geometric_yaw.clamp(-grip_limit, grip_limit);
            let drifting = slip > 0.15;
            let countering =
                drifting && self.steer.signum() != self.yaw_rate.signum() && self.steer.abs() > 0.1;
            let mut yaw_response =
                ((3.0 + turn_grip * 6.0 + if countering { 5.0 } else { 0.0 }) * DT).min(1.0);
            if controls.handbrake && forward_speed.abs() > 4.0 {
                let direction = if self.steer.abs() > 0.05 {
                    self.steer.signum()
                } else {
                    self.yaw_rate.signum()
                };
                let amount = self.steer.abs().max(0.6);
                target_yaw =
                    direction * amount * 2.9 * forward_speed.signum() + self.yaw_rate * 0.3;
                yaw_response = (8.0 * DT).min(1.0);
            }
            self.yaw_rate += (target_yaw - self.yaw_rate) * yaw_response;
        } else {
            self.yaw_rate += (self.steer * 0.6 - self.yaw_rate) * 0.03;
        }

        let carry = if self.airborne {
            1.0
        } else {
            0.92 + 0.08 * grip
        };
        self.velocity = (
            fx * forward_speed + rx * lateral_speed * carry,
            fz * forward_speed + rz * lateral_speed * carry,
        );
        self.heading += self.yaw_rate * DT;
        self.x += self.velocity.0 * DT;
        self.z += self.velocity.1 * DT;

        let (fx, fz) = self.forward();
        let (rx, rz) = self.right();
        let wheel = |forward: f32, side: f32| {
            track
                .terrain(
                    self.x + fx * forward + rx * side,
                    self.z + fz * forward + rz * side,
                )
                .height
        };
        let (front_left, front_right) = (
            wheel(WHEEL_OFFSET_FORWARD, -WHEEL_OFFSET_SIDE),
            wheel(WHEEL_OFFSET_FORWARD, WHEEL_OFFSET_SIDE),
        );
        let (rear_left, rear_right) = (
            wheel(-WHEEL_OFFSET_FORWARD, -WHEEL_OFFSET_SIDE),
            wheel(-WHEEL_OFFSET_FORWARD, WHEEL_OFFSET_SIDE),
        );
        let ground = (front_left + front_right + rear_left + rear_right) * 0.25;
        let slope_pitch = (((front_left + front_right) - (rear_left + rear_right)) * 0.5
            / (WHEEL_OFFSET_FORWARD * 2.0))
            .atan();
        let slope_roll = -(((front_right + rear_right) - (front_left + rear_left)) * 0.5
            / (WHEEL_OFFSET_SIDE * 2.0))
            .atan();
        if self.airborne {
            self.vertical_speed -= GRAVITY * DT;
            self.y += self.vertical_speed * DT;
            if self.y <= ground {
                events.landed = -self.vertical_speed;
                self.heave_velocity += (-self.vertical_speed * 0.18).min(2.5);
                self.y = ground;
                self.airborne = false;
                self.vertical_speed = (ground - self.previous_ground) / DT;
            }
        } else {
            let following = (ground - self.y) / DT;
            if following < self.vertical_speed - GRAVITY * DT * 2.0 && forward_speed > 12.0 {
                self.airborne = true;
                self.vertical_speed -= GRAVITY * DT;
                self.y += self.vertical_speed * DT;
            } else {
                let following = following.clamp(-30.0, 30.0);
                let jolt = following - self.vertical_speed;
                self.heave_velocity += (-jolt * 0.05).clamp(-0.6, 0.6);
                self.vertical_speed = following;
                self.y = ground;
            }
        }
        self.previous_ground = ground;

        let target_pitch = if self.airborne {
            self.pitch * 0.98 - 0.002
        } else {
            slope_pitch + self.suspension_pitch
        };
        self.pitch += (target_pitch - self.pitch) * 0.25;
        let lean = (-self.yaw_rate * forward_speed.abs() * 0.004 - lateral_speed * 0.006)
            .clamp(-0.12, 0.12);
        let target_roll = if self.airborne {
            self.roll * 0.99
        } else {
            (slope_roll + lean).clamp(-MAX_ROLL, MAX_ROLL)
        };
        self.roll += (target_roll - self.roll) * 0.2;
        if !self.airborne {
            self.slope_pitch += (slope_pitch - self.slope_pitch) * 0.3;
            self.slope_roll += (slope_roll.clamp(-MAX_ROLL, MAX_ROLL) - self.slope_roll) * 0.3;
        }
        self.suspension_pitch += ((controls.throttle.max(0.0) * 0.03
            - controls.throttle.min(0.0) * -0.05)
            - self.suspension_pitch)
            * 0.1;
        self.heave_velocity += (-SPRING * self.suspension - DAMPING * self.heave_velocity) * DT;
        self.suspension = (self.suspension + self.heave_velocity * DT).clamp(-0.05, 0.14);
        self.wheel_spin += forward_speed * DT * 2.0;
        events
    }

    pub fn rotation(&self) -> Mat3Fx12 {
        Mat3Fx12::rotation_y(angle(self.heading))
            * Mat3Fx12::rotation_x(angle(self.pitch))
            * Mat3Fx12::rotation_z(angle(self.roll))
    }

    pub fn origin(&self) -> Vec3i32 {
        world_to_psx(self.x, self.y + RIDE_HEIGHT - self.suspension, self.z)
    }

    pub fn wheel_rotation(&self) -> Mat3Fx12 {
        Mat3Fx12::rotation_y(angle(self.heading))
            * Mat3Fx12::rotation_x(angle(self.slope_pitch))
            * Mat3Fx12::rotation_z(angle(self.slope_roll))
    }

    pub fn wheel_origin(&self) -> Vec3i32 {
        world_to_psx(self.x, self.y, self.z)
    }
}

struct Face {
    points: Vec<Vec3i16>,
    uvs: Vec<(u8, u8)>,
    normal: Vec3i16,
    texture: Option<Texture>,
    flat: Rgb,
    bias: i32,
    wheel: bool,
}

impl Face {
    fn textured(points: Vec<Vec3i16>, uvs: Vec<(u8, u8)>, texture: Texture, bias: i32) -> Self {
        Self {
            points,
            uvs,
            normal: Vec3i16::new(0, 0, 0),
            texture: Some(texture),
            flat: [128, 128, 128],
            bias,
            wheel: false,
        }
    }

    fn flat(points: Vec<Vec3i16>, color: Rgb, bias: i32) -> Self {
        let uvs = vec![(0, 0); points.len()];
        Self {
            points,
            uvs,
            normal: Vec3i16::new(0, 0, 0),
            texture: None,
            flat: color,
            bias,
            wheel: false,
        }
    }
}

pub struct CarMesh {
    faces: Vec<Face>,
    wheel: Image,
}

fn v(x: i16, y: i16, z: i16) -> Vec3i16 {
    Vec3i16::new(x, y, z)
}

fn point(x: f32, y: f32, z: f32) -> Vec3i16 {
    v(x.round() as i16, y.round() as i16, z.round() as i16)
}

pub fn normal(points: &[Vec3i16]) -> Vec3i16 {
    let p = |i: usize| [points[i].x as f32, points[i].y as f32, points[i].z as f32];
    let (a, b, c) = (p(0), p(1), p(2));
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    Vec3i16::new(
        (-n[0] / length * 4096.0) as i16,
        (-n[1] / length * 4096.0) as i16,
        (-n[2] / length * 4096.0) as i16,
    )
}

fn corners(image: &Image, mirror: bool) -> [(u8, u8); 4] {
    let u1 = image.u.wrapping_add(image.width).wrapping_sub(1);
    let v1 = image.v.wrapping_add(image.height).wrapping_sub(1);
    let (left, right) = if mirror { (u1, image.u) } else { (image.u, u1) };
    [(left, image.v), (right, image.v), (left, v1), (right, v1)]
}

fn half_image(image: &Image, right: bool) -> Image {
    let half = image.width / 2;
    Image {
        u: if right { image.u + half } else { image.u },
        v: image.v,
        width: half,
        height: image.height,
        texture: image.texture,
    }
}

fn image_uv(image: &Image, across: f32, down: f32) -> (u8, u8) {
    let u = across.clamp(0.0, 1.0) * (image.width - 1) as f32;
    let v = down.clamp(0.0, 1.0) * (image.height - 1) as f32;
    (
        image.u.wrapping_add(u.round() as u8),
        image.v.wrapping_add(v.round() as u8),
    )
}

fn interpolate(stations: &[(f32, f32)], z: f32) -> f32 {
    for pair in stations.windows(2) {
        let ((front_z, front_y), (rear_z, rear_y)) = (pair[0], pair[1]);
        if z >= rear_z {
            let along = ((front_z - z) / (front_z - rear_z)).clamp(0.0, 1.0);
            return front_y + (rear_y - front_y) * along;
        }
    }
    stations[stations.len() - 1].1
}

fn belt(z: f32) -> f32 {
    interpolate(&BELT, z)
}

fn crease(z: f32) -> f32 {
    interpolate(&CREASE, z)
}

fn crown(z: f32) -> f32 {
    belt(z) - CROWN
}

fn body_x(z: f32, y: f32) -> f32 {
    let (top, middle) = (belt(z), crease(z));
    if y <= middle {
        let along = ((y - top) / (middle - top)).clamp(0.0, 1.0);
        BELT_X + (HALF_WIDTH - BELT_X) * along
    } else {
        let along = ((y - middle) / (SILL - middle)).clamp(0.0, 1.0);
        HALF_WIDTH + (SILL_X - HALF_WIDTH) * along
    }
}

fn body_point(side: f32, z: f32, y: f32) -> Vec3i16 {
    point(side * body_x(z, y), y, z)
}

const RUBBER: Rgb = [30, 30, 32];
const UNDERBODY: Rgb = [20, 20, 22];
const WHEEL_WELL: Rgb = [14, 14, 16];
const LAMP_HOUSING: Rgb = [24, 24, 26];
const SUMP_GUARD: Rgb = [150, 150, 156];
const RIDE_HEIGHT: f32 = 0.08;
const SPRING: f32 = 90.0;
const DAMPING: f32 = 9.0;
const FRONT: f32 = BODY_FRONT;
const REAR: f32 = BODY_REAR;
const NOSE_Z: f32 = 112.0;
const CHIN_Z: f32 = 110.0;
const TAIL_TOP_Z: f32 = -136.0;
const TAIL_BOTTOM_Z: f32 = -135.0;
const TAIL_BOTTOM_Y: f32 = -10.0;
const SIDE_FRONT: f32 = 107.0;
const SIDE_REAR: f32 = -131.0;
const SCUTTLE: f32 = 38.0;
const BONNET_MIDDLE: f32 = 75.0;
const ROOF_FRONT: f32 = 6.0;
const B_PILLAR: f32 = -30.0;
const ROOF_REAR: f32 = -76.0;
const BOOT: f32 = -100.0;
const ROOF: f32 = -77.0;
const CROWN: f32 = 3.0;
const SILL: f32 = -8.0;
const SILL_X: f32 = 47.0;
const BELT_X: f32 = 46.0;
const HALF_WIDTH: f32 = 50.0;
const NOSE_HALF_WIDTH: f32 = 40.0;
const ROOF_HALF_WIDTH: f32 = 38.0;
const BELT: [(f32, f32); 4] = [
    (NOSE_Z, -42.0),
    (SCUTTLE, -50.0),
    (BOOT, -53.0),
    (TAIL_TOP_Z, -47.0),
];
const CREASE: [(f32, f32); 5] = [
    (NOSE_Z, -36.0),
    (SCUTTLE, -35.0),
    (-47.0, -35.0),
    (-80.0, -38.0),
    (TAIL_TOP_Z, -37.0),
];
const MID_STATIONS: [f32; 4] = [SCUTTLE, 4.0, B_PILLAR, -47.0];
const TAIL_STATIONS: [f32; 2] = [-110.0, SIDE_REAR];
const FRONT_ARCH: [f32; 4] = [SIDE_FRONT, 87.0, 62.0, SCUTTLE];
const REAR_ARCH: [f32; 4] = [-47.0, -70.0, BOOT, -110.0];
const ARCH_Y: f32 = -16.0;
const ARCH_RADIUS: f32 = 28.0;
const LIP_RADIUS: f32 = 23.0;
const FLARE_X: f32 = 56.0;
const WELL_X: f32 = 40.0;
const WHEEL_Z: f32 = 77.0;
const WHEEL_RADIUS: f32 = 19.0;
const WHEEL_Y: f32 = -19.0;
const TYRE_OUTER: f32 = 54.0;
const TYRE_INNER: f32 = 42.0;
const TYRE_SEGMENTS: usize = 12;
const MUDFLAP_OFFSET: f32 = 24.0;
const FRONT_BUMPER: (f32, f32, f32, f32) = (117.0, 104.0, -23.0, -18.0);
const REAR_BUMPER: (f32, f32, f32, f32) = (-141.0, -127.0, -26.0, -21.0);
const BUMPER_HALF_WIDTH: f32 = 36.0;
const BUMPER_WRAP_X: f32 = 51.0;
const BUMPER_DEPTH: f32 = 5.0;
const SPOT_LAMPS: [(f32, f32, f32); 4] = [
    (-17.0, -31.0, 119.0),
    (17.0, -31.0, 119.0),
    (-20.0, -14.0, 122.0),
    (20.0, -14.0, 122.0),
];
const SPOT_RADIUS: f32 = 7.0;
const SPOT_DEPTH: f32 = 5.0;
const SPOT_SEGMENTS: usize = 6;

struct MeshBuilder {
    faces: Vec<Face>,
}

impl MeshBuilder {
    fn add(&mut self, mut face: Face, outward: [f32; 3]) {
        let facing = normal(&face.points);
        let alignment = facing.x as f32 * outward[0]
            + facing.y as f32 * outward[1]
            + facing.z as f32 * outward[2];
        if alignment < 0.0 {
            face.points.swap(0, 1);
            face.uvs.swap(0, 1);
            if face.points.len() == 4 {
                face.points.swap(2, 3);
                face.uvs.swap(2, 3);
            }
        }
        face.normal = normal(&face.points);
        self.faces.push(face);
    }

    fn planar(
        &mut self,
        image: &Image,
        points: Vec<Vec3i16>,
        mapping: impl Fn(f32, f32, f32) -> (f32, f32),
        bias: i32,
        outward: [f32; 3],
    ) {
        let uvs = points
            .iter()
            .map(|p| {
                let (across, down) = mapping(p.x as f32, p.y as f32, p.z as f32);
                image_uv(image, across, down)
            })
            .collect();
        self.add(Face::textured(points, uvs, image.texture, bias), outward);
    }

    fn spot_lamps(&mut self, atlas: &CarAtlas) {
        for (x, y, z) in SPOT_LAMPS {
            let radius = SPOT_RADIUS;
            let lens = vec![
                point(x - radius, y - radius, z),
                point(x + radius, y - radius, z),
                point(x - radius, y + radius, z),
                point(x + radius, y + radius, z),
            ];
            let uvs = corners(&atlas.lamp, false).to_vec();
            self.add(
                Face::textured(lens, uvs, atlas.lamp.texture, -48),
                [0.0, 0.0, 1.0],
            );
            for segment in 0..SPOT_SEGMENTS {
                let step = std::f32::consts::TAU / SPOT_SEGMENTS as f32;
                let (start, end) = (segment as f32 * step, (segment + 1) as f32 * step);
                let middle = (start + end) / 2.0;
                let rim = |angle: f32, depth: f32| {
                    point(
                        x + angle.cos() * radius,
                        y - angle.sin() * radius,
                        z - depth,
                    )
                };
                self.add(
                    Face::flat(
                        vec![
                            rim(start, 0.0),
                            rim(end, 0.0),
                            rim(start, SPOT_DEPTH),
                            rim(end, SPOT_DEPTH),
                        ],
                        LAMP_HOUSING,
                        -46,
                    ),
                    [middle.cos(), -middle.sin(), 0.0],
                );
            }
        }
    }

    fn mirrors(&mut self, atlas: &CarAtlas) {
        let chrome = corners(&atlas.chrome, false).to_vec();
        for side in [-1.0f32, 1.0] {
            let corner = |x: f32, y: f32, z: f32| point(side * x, y, z);
            let (inner, outer, top, bottom, front, back) = (48.0, 55.0, -58.0, -52.0, 30.0, 26.0);
            self.add(
                Face::textured(
                    vec![
                        corner(inner, top, front),
                        corner(outer, top, front),
                        corner(inner, bottom, front),
                        corner(outer, bottom, front),
                    ],
                    chrome.clone(),
                    atlas.chrome.texture,
                    -42,
                ),
                [0.0, 0.0, 1.0],
            );
            self.add(
                Face::textured(
                    vec![
                        corner(outer, top, front),
                        corner(outer, top, back),
                        corner(outer, bottom, front),
                        corner(outer, bottom, back),
                    ],
                    chrome.clone(),
                    atlas.chrome.texture,
                    -42,
                ),
                [side, 0.0, 0.0],
            );
            self.add(
                Face::textured(
                    vec![
                        corner(inner, top, back),
                        corner(outer, top, back),
                        corner(inner, top, front),
                        corner(outer, top, front),
                    ],
                    chrome.clone(),
                    atlas.chrome.texture,
                    -42,
                ),
                [0.0, -1.0, 0.0],
            );
            self.add(
                Face::flat(
                    vec![
                        corner(inner, top, back),
                        corner(outer, top, back),
                        corner(inner, bottom, back),
                        corner(outer, bottom, back),
                    ],
                    [20, 20, 24],
                    -42,
                ),
                [0.0, 0.0, -1.0],
            );
        }
    }

    fn bumper(&mut self, atlas: &CarAtlas, face_z: f32, wrap_z: f32, top: f32, bottom: f32) {
        let direction = face_z.signum();
        let chrome = corners(&atlas.chrome, false).to_vec();
        let outline = [
            (-BUMPER_WRAP_X, wrap_z),
            (-BUMPER_HALF_WIDTH, face_z),
            (BUMPER_HALF_WIDTH, face_z),
            (BUMPER_WRAP_X, wrap_z),
        ];
        let outward_of = |index: usize| {
            let ((x0, z0), (x1, z1)) = (outline[index], outline[index + 1]);
            let (dx, dz) = (x1 - x0, z1 - z0);
            let length = dx.hypot(dz);
            (-dz * direction / length, dx * direction / length)
        };
        for index in 0..outline.len() - 1 {
            let ((x0, z0), (x1, z1)) = (outline[index], outline[index + 1]);
            let (out_x, out_z) = outward_of(index);
            self.add(
                Face::textured(
                    vec![
                        point(x0, top, z0),
                        point(x1, top, z1),
                        point(x0, bottom, z0),
                        point(x1, bottom, z1),
                    ],
                    chrome.clone(),
                    atlas.chrome.texture,
                    -40,
                ),
                [out_x, 0.0, out_z],
            );
            let inset =
                |x: f32, z: f32| point(x - out_x * BUMPER_DEPTH, top, z - out_z * BUMPER_DEPTH);
            self.add(
                Face::textured(
                    vec![
                        inset(x0, z0),
                        inset(x1, z1),
                        point(x0, top, z0),
                        point(x1, top, z1),
                    ],
                    chrome.clone(),
                    atlas.chrome.texture,
                    -40,
                ),
                [0.0, -1.0, 0.0],
            );
        }
    }

    fn mudflaps(&mut self, atlas: &CarAtlas) {
        for wheel_z in [WHEEL_Z, -WHEEL_Z] {
            for side in [-1.0f32, 1.0] {
                let z = wheel_z - MUDFLAP_OFFSET;
                let flap = vec![
                    point(side * TYRE_INNER, -12.0, z),
                    point(side * TYRE_OUTER, -12.0, z),
                    point(side * TYRE_INNER, -1.0, z - 2.0),
                    point(side * TYRE_OUTER, -1.0, z - 2.0),
                ];
                let uvs = corners(&atlas.flap, false).to_vec();
                self.add(
                    Face::textured(flap.clone(), uvs, atlas.flap.texture, -40),
                    [0.0, 0.0, -1.0],
                );
                self.add(Face::flat(flap, UNDERBODY, -40), [0.0, 0.0, 1.0]);
            }
        }
    }

    fn sump_guard(&mut self) {
        self.add(
            Face::flat(
                vec![
                    point(-26.0, -9.0, 108.0),
                    point(26.0, -9.0, 108.0),
                    point(-26.0, -4.0, 82.0),
                    point(26.0, -4.0, 82.0),
                ],
                SUMP_GUARD,
                -30,
            ),
            [0.0, 1.0, 0.3],
        );
    }

    fn wheel_arch(&mut self, atlas: &CarAtlas, side: f32, wheel_z: f32, stations: [f32; 4]) {
        let image = side_image(atlas, side);
        let contour = [
            (stations[0], SILL),
            (stations[0], crease(stations[0])),
            (stations[0], belt(stations[0])),
            (stations[1], belt(stations[1])),
            (stations[2], belt(stations[2])),
            (stations[3], belt(stations[3])),
            (stations[3], crease(stations[3])),
            (stations[3], SILL),
        ];
        let angles = contour.map(|(z, y)| {
            let angle = (ARCH_Y - y).atan2(z - wheel_z);
            if angle < -std::f32::consts::FRAC_PI_2 {
                angle + std::f32::consts::TAU
            } else {
                angle
            }
        });
        let ring = |radius: f32, angle: f32| {
            (
                wheel_z + angle.cos() * radius,
                ARCH_Y - angle.sin() * radius,
            )
        };
        for index in 0..contour.len() - 1 {
            let (base_a, base_b) = (
                ring(ARCH_RADIUS, angles[index]),
                ring(ARCH_RADIUS, angles[index + 1]),
            );
            let (outer_a, outer_b) = (contour[index], contour[index + 1]);
            self.planar(
                image,
                vec![
                    body_point(side, base_a.0, base_a.1),
                    body_point(side, base_b.0, base_b.1),
                    body_point(side, outer_a.0, outer_a.1),
                    body_point(side, outer_b.0, outer_b.1),
                ],
                |_, y, z| side_uv(side, z, y),
                -24,
                [side, 0.0, 0.0],
            );
            let (lip_a, lip_b) = (
                ring(LIP_RADIUS, angles[index]),
                ring(LIP_RADIUS, angles[index + 1]),
            );
            self.add(
                Face::textured(
                    vec![
                        point(side * FLARE_X, lip_a.1, lip_a.0),
                        point(side * FLARE_X, lip_b.1, lip_b.0),
                        body_point(side, base_a.0, base_a.1),
                        body_point(side, base_b.0, base_b.1),
                    ],
                    corners(&atlas.paint, false).to_vec(),
                    atlas.paint.texture,
                    -30,
                ),
                [side, 0.0, 0.0],
            );
        }
        self.add(
            Face::flat(
                vec![
                    point(side * WELL_X, -40.0, wheel_z + 24.0),
                    point(side * WELL_X, -40.0, wheel_z - 24.0),
                    point(side * WELL_X, -10.0, wheel_z + 24.0),
                    point(side * WELL_X, -10.0, wheel_z - 24.0),
                ],
                WHEEL_WELL,
                60,
            ),
            [side, 0.0, 0.0],
        );
    }

    fn flank(&mut self, atlas: &CarAtlas, side: f32) {
        let image = side_image(atlas, side);
        let column = |z: f32| {
            [
                body_point(side, z, belt(z)),
                body_point(side, z, crease(z)),
                body_point(side, z, SILL),
            ]
        };
        let nose = [
            point(side * NOSE_HALF_WIDTH, belt(NOSE_Z), NOSE_Z),
            point(side * NOSE_HALF_WIDTH, crease(NOSE_Z), FRONT),
            point(side * NOSE_HALF_WIDTH, SILL, CHIN_Z),
        ];
        let tail = [
            point(side * NOSE_HALF_WIDTH, belt(TAIL_TOP_Z), TAIL_TOP_Z),
            point(side * NOSE_HALF_WIDTH, crease(TAIL_TOP_Z), REAR),
            point(side * NOSE_HALF_WIDTH, TAIL_BOTTOM_Y, TAIL_BOTTOM_Z),
        ];
        self.strip(image, side, nose, column(SIDE_FRONT), [side, 0.0, 1.0], -26);
        for pair in MID_STATIONS.windows(2) {
            self.strip(
                image,
                side,
                column(pair[0]),
                column(pair[1]),
                [side, 0.0, 0.0],
                -24,
            );
        }
        self.strip(
            image,
            side,
            column(TAIL_STATIONS[0]),
            column(TAIL_STATIONS[1]),
            [side, 0.0, 0.0],
            -24,
        );
        self.strip(image, side, column(SIDE_REAR), tail, [side, 0.0, -1.0], -26);
    }

    fn strip(
        &mut self,
        image: &Image,
        side: f32,
        front: [Vec3i16; 3],
        rear: [Vec3i16; 3],
        outward: [f32; 3],
        bias: i32,
    ) {
        for band in 0..2 {
            self.planar(
                image,
                vec![front[band], rear[band], front[band + 1], rear[band + 1]],
                |_, y, z| side_uv(side, z, y),
                bias,
                outward,
            );
        }
    }

    fn nose_and_tail(&mut self, atlas: &CarAtlas) {
        for side in [-1.0f32, 1.0] {
            let front_uv = |x: f32, y: f32, _: f32| {
                (
                    0.5 - x / (2.0 * NOSE_HALF_WIDTH),
                    (y - crown(NOSE_Z)) / (SILL - crown(NOSE_Z)),
                )
            };
            let nose_rows = [
                (
                    point(side * NOSE_HALF_WIDTH, belt(NOSE_Z), NOSE_Z),
                    point(0.0, crown(NOSE_Z), NOSE_Z),
                ),
                (
                    point(side * NOSE_HALF_WIDTH, crease(NOSE_Z), FRONT),
                    point(0.0, crease(NOSE_Z), FRONT),
                ),
                (
                    point(side * NOSE_HALF_WIDTH, SILL, CHIN_Z),
                    point(0.0, SILL, CHIN_Z),
                ),
            ];
            for pair in nose_rows.windows(2) {
                self.planar(
                    &atlas.front,
                    vec![pair[0].0, pair[0].1, pair[1].0, pair[1].1],
                    front_uv,
                    -24,
                    [0.0, 0.0, 1.0],
                );
            }

            let rear_uv = |x: f32, y: f32, _: f32| {
                (
                    0.5 + x / (2.0 * NOSE_HALF_WIDTH),
                    (y - crown(TAIL_TOP_Z)) / (TAIL_BOTTOM_Y - crown(TAIL_TOP_Z)),
                )
            };
            let tail_rows = [
                (
                    point(side * NOSE_HALF_WIDTH, belt(TAIL_TOP_Z), TAIL_TOP_Z),
                    point(0.0, crown(TAIL_TOP_Z), TAIL_TOP_Z),
                ),
                (
                    point(side * NOSE_HALF_WIDTH, crease(TAIL_TOP_Z), REAR),
                    point(0.0, crease(TAIL_TOP_Z), REAR),
                ),
                (
                    point(side * NOSE_HALF_WIDTH, TAIL_BOTTOM_Y, TAIL_BOTTOM_Z),
                    point(0.0, TAIL_BOTTOM_Y, TAIL_BOTTOM_Z),
                ),
            ];
            for pair in tail_rows.windows(2) {
                self.planar(
                    &atlas.rear,
                    vec![pair[0].0, pair[0].1, pair[1].0, pair[1].1],
                    rear_uv,
                    -24,
                    [0.0, 0.0, -1.0],
                );
            }

            let bonnet_uv = |x: f32, _: f32, z: f32| {
                (0.5 + x / (2.0 * BELT_X), (NOSE_Z - z) / (NOSE_Z - SCUTTLE))
            };
            let bonnet_rows = [
                (NOSE_Z, NOSE_HALF_WIDTH),
                (BONNET_MIDDLE, BELT_X),
                (SCUTTLE, BELT_X),
            ];
            for pair in bonnet_rows.windows(2) {
                let ((front_z, front_width), (rear_z, rear_width)) = (pair[0], pair[1]);
                self.planar(
                    &atlas.bonnet,
                    vec![
                        point(0.0, crown(front_z), front_z),
                        point(side * front_width, belt(front_z), front_z),
                        point(0.0, crown(rear_z), rear_z),
                        point(side * rear_width, belt(rear_z), rear_z),
                    ],
                    bonnet_uv,
                    -24,
                    [0.0, -1.0, 0.0],
                );
            }
            self.planar(
                &atlas.bonnet,
                vec![
                    point(side * NOSE_HALF_WIDTH, belt(NOSE_Z), NOSE_Z),
                    point(side * BELT_X, belt(SIDE_FRONT), SIDE_FRONT),
                    point(side * BELT_X, belt(BONNET_MIDDLE), BONNET_MIDDLE),
                ],
                bonnet_uv,
                -24,
                [0.0, -1.0, 0.0],
            );

            let boot_uv = |x: f32, _: f32, z: f32| {
                (0.5 + x / (2.0 * BELT_X), (BOOT - z) / (BOOT - TAIL_TOP_Z))
            };
            self.planar(
                &atlas.paint,
                vec![
                    point(0.0, crown(BOOT), BOOT),
                    point(side * BELT_X, belt(BOOT), BOOT),
                    point(0.0, crown(TAIL_TOP_Z), TAIL_TOP_Z),
                    point(side * NOSE_HALF_WIDTH, belt(TAIL_TOP_Z), TAIL_TOP_Z),
                ],
                boot_uv,
                -24,
                [0.0, -1.0, 0.0],
            );
            self.planar(
                &atlas.paint,
                vec![
                    point(side * BELT_X, belt(BOOT), BOOT),
                    point(side * BELT_X, belt(SIDE_REAR), SIDE_REAR),
                    point(side * NOSE_HALF_WIDTH, belt(TAIL_TOP_Z), TAIL_TOP_Z),
                ],
                boot_uv,
                -24,
                [0.0, -1.0, 0.0],
            );
        }
    }

    fn glasshouse(&mut self, atlas: &CarAtlas) {
        let roof_edge = |side: f32, z: f32| point(side * ROOF_HALF_WIDTH, ROOF, z);
        let roof_middle = |z: f32| point(0.0, ROOF - CROWN, z);
        let belt_edge = |side: f32, z: f32| point(side * BELT_X, belt(z), z);
        let belt_middle = |z: f32| point(0.0, crown(z), z);
        for side in [-1.0f32, 1.0] {
            let right = side > 0.0;
            let windscreen = if right {
                [
                    roof_edge(side, ROOF_FRONT),
                    roof_middle(ROOF_FRONT),
                    belt_edge(side, SCUTTLE),
                    belt_middle(SCUTTLE),
                ]
            } else {
                [
                    roof_middle(ROOF_FRONT),
                    roof_edge(side, ROOF_FRONT),
                    belt_middle(SCUTTLE),
                    belt_edge(side, SCUTTLE),
                ]
            };
            self.add(
                Face::textured(
                    windscreen.to_vec(),
                    corners(&half_image(&atlas.windscreen, !right), false).to_vec(),
                    atlas.windscreen.texture,
                    -28,
                ),
                [0.0, -0.6, 1.0],
            );
            let rear_screen = if right {
                [
                    roof_middle(ROOF_REAR),
                    roof_edge(side, ROOF_REAR),
                    belt_middle(BOOT),
                    belt_edge(side, BOOT),
                ]
            } else {
                [
                    roof_edge(side, ROOF_REAR),
                    roof_middle(ROOF_REAR),
                    belt_edge(side, BOOT),
                    belt_middle(BOOT),
                ]
            };
            self.add(
                Face::textured(
                    rear_screen.to_vec(),
                    corners(&half_image(&atlas.rear_glass, right), false).to_vec(),
                    atlas.rear_glass.texture,
                    -28,
                ),
                [0.0, -0.6, -1.0],
            );
            self.add(
                Face::textured(
                    vec![
                        roof_edge(side, B_PILLAR),
                        roof_edge(side, ROOF_FRONT),
                        belt_edge(side, B_PILLAR),
                        belt_edge(side, SCUTTLE),
                    ],
                    corners(&atlas.door_glass, false).to_vec(),
                    atlas.door_glass.texture,
                    -28,
                ),
                [side, 0.0, 0.0],
            );
            self.add(
                Face::textured(
                    vec![
                        roof_edge(side, ROOF_REAR),
                        roof_edge(side, B_PILLAR),
                        belt_edge(side, BOOT),
                        belt_edge(side, B_PILLAR),
                    ],
                    corners(&atlas.quarter_glass, false).to_vec(),
                    atlas.quarter_glass.texture,
                    -28,
                ),
                [side, 0.0, 0.0],
            );
            let roof_uv = |x: f32, _: f32, z: f32| {
                (
                    0.5 + x / (2.0 * ROOF_HALF_WIDTH),
                    (ROOF_FRONT - z) / (ROOF_FRONT - ROOF_REAR),
                )
            };
            for pair in [ROOF_FRONT, B_PILLAR, ROOF_REAR].windows(2) {
                self.planar(
                    &atlas.roof,
                    vec![
                        roof_middle(pair[0]),
                        roof_edge(side, pair[0]),
                        roof_middle(pair[1]),
                        roof_edge(side, pair[1]),
                    ],
                    roof_uv,
                    -26,
                    [0.0, -1.0, 0.0],
                );
            }
        }
    }

    fn underside(&mut self) {
        self.add(
            Face::flat(
                vec![
                    point(-SILL_X, SILL, SIDE_FRONT),
                    point(SILL_X, SILL, SIDE_FRONT),
                    point(-SILL_X, SILL, SIDE_REAR),
                    point(SILL_X, SILL, SIDE_REAR),
                ],
                UNDERBODY,
                0,
            ),
            [0.0, 1.0, 0.0],
        );
    }

    fn tyres(&mut self) {
        for wheel_z in [WHEEL_Z, -WHEEL_Z] {
            for side in [-1.0f32, 1.0] {
                for segment in 0..TYRE_SEGMENTS {
                    let step = std::f32::consts::TAU / TYRE_SEGMENTS as f32;
                    let (start, end) = (segment as f32 * step, (segment + 1) as f32 * step);
                    let middle = (start + end) / 2.0;
                    let tread = |x: f32, angle: f32| {
                        point(
                            side * x,
                            WHEEL_Y - angle.sin() * WHEEL_RADIUS,
                            wheel_z + angle.cos() * WHEEL_RADIUS,
                        )
                    };
                    let mut face = Face::flat(
                        vec![
                            tread(TYRE_OUTER, start),
                            tread(TYRE_OUTER, end),
                            tread(TYRE_INNER, start),
                            tread(TYRE_INNER, end),
                        ],
                        RUBBER,
                        -22,
                    );
                    face.wheel = true;
                    self.add(face, [0.0, -middle.sin(), middle.cos()]);
                }
            }
        }
    }
}

fn side_image(atlas: &CarAtlas, side: f32) -> &Image {
    if side > 0.0 {
        &atlas.side_right
    } else {
        &atlas.side_left
    }
}

fn side_uv(side: f32, z: f32, y: f32) -> (f32, f32) {
    let along = (z - REAR) / (FRONT - REAR);
    let top = belt(z);
    let down = (y - top) / (SILL - top);
    if side > 0.0 {
        (along, down)
    } else {
        (1.0 - along, down)
    }
}

impl CarMesh {
    pub fn build(atlas: &CarAtlas) -> Self {
        let mut mesh = MeshBuilder { faces: Vec::new() };
        mesh.spot_lamps(atlas);
        mesh.mirrors(atlas);
        let (face_z, wrap_z, top, bottom) = FRONT_BUMPER;
        mesh.bumper(atlas, face_z, wrap_z, top, bottom);
        let (face_z, wrap_z, top, bottom) = REAR_BUMPER;
        mesh.bumper(atlas, face_z, wrap_z, top, bottom);
        mesh.mudflaps(atlas);
        mesh.sump_guard();
        for side in [-1.0f32, 1.0] {
            mesh.wheel_arch(atlas, side, WHEEL_Z, FRONT_ARCH);
            mesh.wheel_arch(atlas, side, -WHEEL_Z, REAR_ARCH);
        }
        mesh.glasshouse(atlas);
        mesh.nose_and_tail(atlas);
        for side in [-1.0f32, 1.0] {
            mesh.flank(atlas, side);
        }
        mesh.underside();
        mesh.tyres();
        Self {
            faces: mesh.faces,
            wheel: atlas.wheel,
        }
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        car: &Car,
        sun_direction: [f32; 3],
        sun: Rgb,
        ambient: Rgb,
    ) {
        let placement = |wheel: bool| {
            if wheel {
                (car.wheel_rotation(), car.wheel_origin())
            } else {
                (car.rotation(), car.origin())
            }
        };
        let mut queue: Vec<Queued> = Vec::new();
        for wheel_pass in [false, true] {
            let (rotation, origin) = placement(wheel_pass);
            renderer.set_object(origin, rotation);
            renderer.set_lighting(rotation, sun_direction, sun, ambient);
            let wheel_bias = if wheel_pass { WHEEL_BIAS } else { 0 };
            for face in self.faces.iter().filter(|face| face.wheel == wheel_pass) {
                renderer.gte.rgbc = [face.flat[0], face.flat[1], face.flat[2], 0];
                let lit = renderer.gte.nccs(face.normal);
                let points: Vec<Point> = face
                    .points
                    .iter()
                    .zip(&face.uvs)
                    .map(|(&p, &uv)| Point::new(p, lit, uv))
                    .collect();
                let surface = match face.texture {
                    Some(texture) => Surface::textured(texture),
                    None => Surface::flat(),
                };
                queue.extend(Queued::new(
                    renderer,
                    points,
                    surface,
                    face.bias + wheel_bias,
                    wheel_pass,
                ));
            }
        }

        let (sin, cos) = (car.wheel_spin.sin(), car.wheel_spin.cos());
        for (x, z) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let center_z = z * WHEEL_Z;
            let outer = (x * TYRE_OUTER) as i16;
            let corner = |dy: f32, dz: f32| {
                let ry = dy * cos - dz * sin;
                let rz = dy * sin + dz * cos;
                v(
                    outer,
                    (WHEEL_Y + ry * WHEEL_RADIUS) as i16,
                    (center_z + rz * WHEEL_RADIUS) as i16,
                )
            };
            let image = &self.wheel;
            let uvs = corners(image, false);
            let order = if x < 0.0 {
                [(-1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (1.0, -1.0)]
            } else {
                [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
            };
            let normal = Vec3i16::new((x * 4096.0) as i16, 0, 0);
            renderer.gte.rgbc = [128, 128, 128, 0];
            let lit = renderer.gte.nccs(normal);
            let points: Vec<Point> = order
                .iter()
                .zip(uvs)
                .map(|(&(dy, dz), uv)| Point::new(corner(dy, dz), lit, uv))
                .collect();
            queue.extend(Queued::new(
                renderer,
                points,
                Surface::textured(image.texture),
                -30 + WHEEL_BIAS,
                true,
            ));
        }

        queue.sort_by_key(|queued| queued.depth);
        let mut placed = None;
        for queued in queue {
            if placed != Some(queued.wheel) {
                let (rotation, origin) = placement(queued.wheel);
                renderer.set_object(origin, rotation);
                placed = Some(queued.wheel);
            }
            renderer.polygon(&queued.points, queued.surface, queued.bias + CAR_BIAS, 0);
        }
    }
}

struct Queued {
    points: Vec<Point>,
    surface: Surface,
    bias: i32,
    wheel: bool,
    depth: i32,
}

impl Queued {
    fn new(
        renderer: &mut Renderer,
        points: Vec<Point>,
        surface: Surface,
        bias: i32,
        wheel: bool,
    ) -> Option<Self> {
        if renderer.backface(&points) {
            return None;
        }
        let total: i32 = points
            .iter()
            .map(|point| renderer.depth_of(point.position) as i32)
            .sum();
        let depth = total / points.len() as i32 + bias;
        Some(Self {
            points,
            surface,
            bias,
            wheel,
            depth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_leave_the_road_and_keep_driving() {
        for seed in 0..6u64 {
            let mut track = Track::new(seed);
            let mut car = Car::new(&track);
            let mut furthest = 0.0f32;
            for tick in 0..600 {
                let steer = if (210..260).contains(&tick) {
                    -1.0
                } else {
                    0.0
                };
                car.update(
                    Controls {
                        throttle: 1.0,
                        steer,
                        handbrake: false,
                    },
                    &mut track,
                );
                if let Some(road) = track.terrain(car.x, car.z).road {
                    furthest = furthest.max(road.distance);
                }
            }
            assert!(
                furthest > 15.0,
                "seed {seed} only reached {furthest} m from the road"
            );
            assert!(car.speed() > 8.0, "seed {seed} stuck at {}", car.speed());
            car.reset_to_track(&track);
            let road = track.terrain(car.x, car.z).road.unwrap();
            assert!(road.distance < 0.5);
        }
    }

    #[test]
    fn rough_ground_does_not_launch_the_car() {
        let mut track = Track::new(9);
        let mut car = Car::new(&track);
        let mut airborne_ticks = 0;
        for tick in 0..600 {
            let steer = if (200..215).contains(&tick) {
                -0.6
            } else {
                0.0
            };
            car.update(
                Controls {
                    throttle: 0.6,
                    steer,
                    handbrake: false,
                },
                &mut track,
            );
            if car.ground == Ground::Rough && car.airborne {
                airborne_ticks += 1;
            }
        }
        assert!(
            airborne_ticks < 150,
            "{airborne_ticks} airborne ticks off-road"
        );
    }

    #[test]
    fn mostly_grounded_at_speed() {
        for seed in 0..4u64 {
            let mut track = Track::new(seed);
            let mut car = Car::new(&track);
            let (mut air, mut total) = (0, 0);
            for _ in 0..3600 {
                let target = track.sample(car.index + 4);
                let mut error = (target.x - car.x).atan2(target.z - car.z) - car.heading;
                while error > std::f32::consts::PI {
                    error -= std::f32::consts::TAU;
                }
                while error < -std::f32::consts::PI {
                    error += std::f32::consts::TAU;
                }
                car.update(
                    Controls {
                        throttle: if car.speed() < 30.0 { 1.0 } else { 0.0 },
                        steer: (error * 3.0).clamp(-1.0, 1.0),
                        handbrake: false,
                    },
                    &mut track,
                );
                total += 1;
                if car.airborne {
                    air += 1;
                }
            }
            let fraction = air as f32 / total as f32;
            assert!(
                fraction < 0.12,
                "seed {seed} airborne {:.1}%",
                fraction * 100.0
            );
        }
    }

    #[test]
    fn steering_and_handbrake_rates() {
        for handbrake in [false, true] {
            for (label, lateral) in [("road", 0.0f32), ("rough", 9.5)] {
                for speed in [10.0f32, 20.0, 30.0] {
                    let mut track = Track::new(3);
                    track.ensure(400);
                    let mut car = Car::new(&track);
                    let start = track.sample(30);
                    let (x, z) = start.offset(lateral);
                    car.x = x;
                    car.z = z;
                    car.index = 30;
                    car.y = track.ground(30, 0.0, lateral);
                    car.heading = start.heading;
                    car.velocity = (start.forward().0 * speed, start.forward().1 * speed);
                    let course_start = car.velocity.1.atan2(car.velocity.0);
                    let heading_start = car.heading;
                    for _ in 0..30 {
                        car.update(
                            Controls {
                                throttle: 0.3,
                                steer: 1.0,
                                handbrake,
                            },
                            &mut track,
                        );
                    }
                    let course_end = car.velocity.1.atan2(car.velocity.0);
                    let yaw = (car.heading - heading_start) / 0.5;
                    let course = -(course_end - course_start) / 0.5;
                    if handbrake {
                        assert!(
                            yaw > 1.2 && course < yaw * 0.5,
                            "{label} {speed}: handbrake yaw {yaw} course {course}"
                        );
                    } else {
                        assert!(
                            (0.45..1.2).contains(&yaw) && course > 0.25,
                            "{label} {speed}: yaw {yaw} course {course}"
                        );
                    }
                }
            }
        }
    }
}
