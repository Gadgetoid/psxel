use psx_math::{Mat3Fx12, Vec3i16, Vec3i32};
use psx_prim::Texture;

use crate::biome::Rgb;
use crate::livery::{CarAtlas, SIDE_LENGTH, WHEEL_Z};
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
const MAX_ROLL: f32 = 0.35;
const WHEEL_OFFSET_FORWARD: f32 = 1.3;
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
        let (grip, turn_grip) = match self.ground {
            Ground::Road => (biome_grip, biome_grip),
            Ground::Shoulder => (
                (biome_grip + off_road_grip) * 0.5,
                (biome_grip + off_road_grip) * 0.5 + 0.1,
            ),
            Ground::Rough => (off_road_grip, off_road_grip + 0.15),
        };
        let rolling_drag: f32 = match self.ground {
            Ground::Road => 0.4 + (1.0 - biome_grip) * 1.2,
            Ground::Shoulder => 1.2 + off_road_drag * 0.3,
            Ground::Rough => off_road_drag,
        };
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

pub struct CarMesh {
    faces: Vec<Face>,
    wheel: Image,
}

fn v(x: i16, y: i16, z: i16) -> Vec3i16 {
    Vec3i16::new(x, y, z)
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

fn half_image(image: &Image, right: bool, v_from: u8, v_to: u8) -> Image {
    let half = image.width / 2;
    Image {
        u: if right { image.u + half } else { image.u },
        v: image.v + v_from,
        width: half,
        height: v_to - v_from + 1,
        texture: image.texture,
    }
}

fn beltline(z: i16) -> i16 {
    -42 - ((126 - z as i32) * 4 / 252) as i16
}

const SILL: i16 = -12;
const CHAMFER: i16 = 8;
const CREASE: f32 = 0.42;
const RUBBER: Rgb = [30, 30, 32];
const RIDE_HEIGHT: f32 = 0.08;
const SPRING: f32 = 90.0;
const DAMPING: f32 = 9.0;
const HALF_WIDTH: i16 = 52;
const FRONT: i16 = 126;
const REAR: i16 = -126;
const SCUTTLE: i16 = 28;
const ROOF_FRONT: i16 = 2;
const ROOF_REAR: i16 = -58;
const BOOT: i16 = -84;
const ROOF: i16 = -84;
const ROOF_HALF_WIDTH: i16 = 43;
const WHEEL_RADIUS: f32 = 20.0;
const WHEEL_Y: f32 = -20.0;

impl CarMesh {
    pub fn build(atlas: &CarAtlas) -> Self {
        let mut faces = Vec::new();
        let mut textured = |points: [Vec3i16; 4], image: &Image, mirror: bool, bias: i32| {
            faces.push(Face {
                normal: normal(&points),
                points: points.to_vec(),
                uvs: corners(image, mirror).to_vec(),
                texture: Some(image.texture),
                flat: [128, 128, 128],
                bias,
                wheel: false,
            });
        };
        let w = HALF_WIDTH;
        let rw = ROOF_HALF_WIDTH;
        let (nose, scuttle, boot, tail) = (
            beltline(FRONT),
            beltline(SCUTTLE),
            beltline(BOOT),
            beltline(REAR),
        );

        let nw = w - 8;
        let shoulder = beltline(FRONT - CHAMFER);
        let crease = nose - 2;
        textured(
            [
                v(-nw, nose, FRONT),
                v(0, crease, FRONT),
                v(-w, shoulder, FRONT - CHAMFER),
                v(0, crease - 1, FRONT - CHAMFER),
            ],
            &half_image(&atlas.bonnet, false, 0, 6),
            false,
            -24,
        );
        textured(
            [
                v(0, crease, FRONT),
                v(nw, nose, FRONT),
                v(0, crease - 1, FRONT - CHAMFER),
                v(w, shoulder, FRONT - CHAMFER),
            ],
            &half_image(&atlas.bonnet, true, 0, 6),
            false,
            -24,
        );
        textured(
            [
                v(-w, shoulder, FRONT - CHAMFER),
                v(0, crease - 1, FRONT - CHAMFER),
                v(-w, scuttle, SCUTTLE),
                v(0, scuttle - 2, SCUTTLE),
            ],
            &half_image(&atlas.bonnet, false, 6, 63),
            false,
            -24,
        );
        textured(
            [
                v(0, crease - 1, FRONT - CHAMFER),
                v(w, shoulder, FRONT - CHAMFER),
                v(0, scuttle - 2, SCUTTLE),
                v(w, scuttle, SCUTTLE),
            ],
            &half_image(&atlas.bonnet, true, 6, 63),
            false,
            -24,
        );
        textured(
            [
                v(nw, nose, FRONT),
                v(-nw, nose, FRONT),
                v(nw, SILL, FRONT + 2),
                v(-nw, SILL, FRONT + 2),
            ],
            &atlas.front,
            false,
            -26,
        );
        for side in [-1i16, 1] {
            let points = [
                v(side * nw, nose, FRONT),
                v(side * w, shoulder, FRONT - CHAMFER),
                v(side * nw, SILL, FRONT + 2),
                v(side * w, SILL, FRONT - CHAMFER),
            ];
            let points = if side < 0 {
                points
            } else {
                [points[1], points[0], points[3], points[2]]
            };
            textured(points, &atlas.paint, false, -26);
        }
        textured(
            [
                v(-w, boot, BOOT),
                v(w, boot, BOOT),
                v(-w, tail, REAR),
                v(w, tail, REAR),
            ],
            &atlas.paint,
            false,
            -24,
        );
        textured(
            [
                v(-w, tail, REAR),
                v(w, tail, REAR),
                v(-w, SILL, REAR - 2),
                v(w, SILL, REAR - 2),
            ],
            &atlas.rear,
            false,
            -26,
        );

        textured(
            [
                v(rw, ROOF, ROOF_FRONT),
                v(-rw, ROOF, ROOF_FRONT),
                v(w, scuttle, SCUTTLE),
                v(-w, scuttle, SCUTTLE),
            ],
            &atlas.windscreen,
            false,
            -28,
        );
        textured(
            [
                v(-rw, ROOF, ROOF_REAR),
                v(rw, ROOF, ROOF_REAR),
                v(-w, boot, BOOT),
                v(w, boot, BOOT),
            ],
            &atlas.rear_glass,
            false,
            -28,
        );
        textured(
            [
                v(rw, ROOF, ROOF_REAR),
                v(rw, ROOF, ROOF_FRONT),
                v(w, boot, BOOT),
                v(w, scuttle, SCUTTLE),
            ],
            &atlas.side_glass,
            false,
            -28,
        );
        textured(
            [
                v(-rw, ROOF, ROOF_FRONT),
                v(-rw, ROOF, ROOF_REAR),
                v(-w, scuttle, SCUTTLE),
                v(-w, boot, BOOT),
            ],
            &atlas.side_glass,
            true,
            -28,
        );

        let mut sculpted_faces: Vec<Face> = Vec::new();
        let mut sculpted = |points: [Vec3i16; 4], uvs: [(u8, u8); 4], image: &Image, bias: i32| {
            sculpted_faces.push(Face {
                normal: normal(&points),
                points: points.to_vec(),
                uvs: uvs.to_vec(),
                texture: Some(image.texture),
                flat: [128, 128, 128],
                bias,
                wheel: false,
            });
        };
        let stations: [(i16, i16); 6] = [
            (FRONT - CHAMFER, 0),
            (66, 1),
            (SCUTTLE, 1),
            (-40, 1),
            (-90, 3),
            (REAR, 1),
        ];
        for side in [-1i16, 1] {
            let image = if side > 0 {
                &atlas.side_right
            } else {
                &atlas.side_left
            };
            let u_at = |z: i16| {
                let u =
                    ((z as f32 + SIDE_LENGTH / 2.0) / SIDE_LENGTH * 255.0).clamp(0.0, 255.0) as u8;
                if side > 0 {
                    image.u.wrapping_add(u)
                } else {
                    image.u.wrapping_add(255 - u)
                }
            };
            let v_at = |fraction: f32| image.v + (fraction * 31.0) as u8;
            let profile = |z: i16, bulge: i16| {
                let top = if z >= FRONT - CHAMFER {
                    shoulder
                } else {
                    beltline(z)
                };
                let crease = top + ((SILL - top) as f32 * CREASE) as i16;
                [
                    (side * (w + bulge / 2), top, 0.0),
                    (side * (w + bulge), crease, CREASE),
                    (side * (w - 2), SILL, 1.0),
                ]
            };
            for pair in stations.windows(2) {
                let ((front_z, front_bulge), (rear_z, rear_bulge)) = (pair[0], pair[1]);
                let (front, rear) = (profile(front_z, front_bulge), profile(rear_z, rear_bulge));
                for band in 0..2 {
                    let corner = |row: &[(i16, i16, f32); 3], index: usize, z: i16| {
                        let (x, y, fraction) = row[index];
                        (v(x, y, z), (u_at(z), v_at(fraction)))
                    };
                    let (front_top, front_bottom) = (
                        corner(&front, band, front_z),
                        corner(&front, band + 1, front_z),
                    );
                    let (rear_top, rear_bottom) =
                        (corner(&rear, band, rear_z), corner(&rear, band + 1, rear_z));
                    let ordered = if side > 0 {
                        [rear_top, front_top, rear_bottom, front_bottom]
                    } else {
                        [front_top, rear_top, front_bottom, rear_bottom]
                    };
                    sculpted(
                        ordered.map(|(point, _)| point),
                        ordered.map(|(_, uv)| uv),
                        image,
                        -24,
                    );
                }
            }
        }
        let [roof_a, roof_b, roof_c, roof_d] = corners(&atlas.roof, false);
        let middle = ((roof_a.0 as u16 + roof_b.0 as u16) / 2) as u8;
        let crown = ROOF - 3;
        sculpted(
            [
                v(rw, ROOF, ROOF_REAR),
                v(0, crown, ROOF_REAR),
                v(rw, ROOF, ROOF_FRONT),
                v(0, crown, ROOF_FRONT),
            ],
            [roof_a, (middle, roof_a.1), roof_c, (middle, roof_c.1)],
            &atlas.roof,
            -26,
        );
        sculpted(
            [
                v(0, crown, ROOF_REAR),
                v(-rw, ROOF, ROOF_REAR),
                v(0, crown, ROOF_FRONT),
                v(-rw, ROOF, ROOF_FRONT),
            ],
            [(middle, roof_b.1), roof_b, (middle, roof_d.1), roof_d],
            &atlas.roof,
            -26,
        );

        for z in [FRONT + 4, REAR - 4] {
            let front = z > 0;
            let (a, b) = if front {
                (w + 2, -w - 2)
            } else {
                (-w - 2, w + 2)
            };
            textured(
                [v(a, -25, z), v(b, -25, z), v(a, -18, z), v(b, -18, z)],
                &atlas.chrome,
                false,
                -40,
            );
            let inner = if front { z - 4 } else { z + 4 };
            let (near, far) = if front { (z, inner) } else { (inner, z) };
            textured(
                [
                    v(-w - 2, -25, near),
                    v(w + 2, -25, near),
                    v(-w - 2, -25, far),
                    v(w + 2, -25, far),
                ],
                &atlas.chrome,
                false,
                -40,
            );
        }

        for (x, y, z, radius) in [
            (-17i16, -35i16, FRONT + 6, 8i16),
            (17, -35, FRONT + 6, 8),
            (-17, -20, FRONT + 9, 8),
            (17, -20, FRONT + 9, 8),
        ] {
            textured(
                [
                    v(x + radius, y - radius, z),
                    v(x - radius, y - radius, z),
                    v(x + radius, y + radius, z),
                    v(x - radius, y + radius, z),
                ],
                &atlas.lamp,
                false,
                -48,
            );
        }

        for side in [-1i16, 1] {
            let (outer, inner) = (side * 54, side * 40);
            let z = -(WHEEL_Z as i16) - 24;
            let (a, b) = if side < 0 {
                (outer, inner)
            } else {
                (inner, outer)
            };
            textured(
                [v(a, -26, z), v(b, -26, z), v(a, -3, z - 2), v(b, -3, z - 2)],
                &atlas.flap,
                false,
                -40,
            );
        }

        let mut flat = |points: [Vec3i16; 4], color: Rgb, bias: i32| {
            faces.push(Face {
                normal: normal(&points),
                points: points.to_vec(),
                uvs: vec![(0, 0); 4],
                texture: None,
                flat: color,
                bias,
                wheel: false,
            });
        };
        flat(
            [
                v(-w, SILL, FRONT),
                v(w, SILL, FRONT),
                v(-w, SILL, REAR),
                v(w, SILL, REAR),
            ],
            [20, 20, 22],
            0,
        );
        let mut oriented = |points: Vec<Vec3i16>,
                            uvs: Vec<(u8, u8)>,
                            texture: Option<Texture>,
                            color: Rgb,
                            bias: i32,
                            center: (f32, f32, f32)| {
            let count = points.len() as f32;
            let centroid = points.iter().fold((0.0, 0.0, 0.0), |acc, p| {
                (
                    acc.0 + p.x as f32 / count,
                    acc.1 + p.y as f32 / count,
                    acc.2 + p.z as f32 / count,
                )
            });
            let outward = (
                centroid.0 - center.0,
                centroid.1 - center.1,
                centroid.2 - center.2,
            );
            let mut points = points;
            let mut uvs = uvs;
            let n = normal(&points);
            if n.x as f32 * outward.0 + n.y as f32 * outward.1 + n.z as f32 * outward.2 < 0.0 {
                points.swap(0, 1);
                uvs.swap(0, 1);
                if points.len() == 4 {
                    points.swap(2, 3);
                    uvs.swap(2, 3);
                }
            }
            faces.push(Face {
                normal: normal(&points),
                points,
                uvs,
                texture,
                flat: color,
                bias,
                wheel: color == RUBBER,
            });
        };
        let rubber = RUBBER;
        for (x, z) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let center_z = z * WHEEL_Z;
            let (outer, inner) = (x * 55.0, x * 44.0);
            for segment in 0..8 {
                let a0 = segment as f32 / 8.0 * std::f32::consts::TAU;
                let a1 = (segment + 1) as f32 / 8.0 * std::f32::consts::TAU;
                let at = |x: f32, angle: f32| {
                    v(
                        x as i16,
                        (WHEEL_Y - angle.sin() * WHEEL_RADIUS) as i16,
                        (center_z + angle.cos() * WHEEL_RADIUS) as i16,
                    )
                };
                oriented(
                    vec![at(outer, a0), at(outer, a1), at(inner, a0), at(inner, a1)],
                    vec![(0, 0); 4],
                    None,
                    rubber,
                    -22,
                    ((outer + inner) / 2.0, WHEEL_Y, center_z),
                );
            }
            for segment in 0..4 {
                let a0 = 0.35 + segment as f32 / 4.0 * 2.45;
                let a1 = 0.35 + (segment + 1) as f32 / 4.0 * 2.45;
                let at = |x: f32, radius: f32, angle: f32| {
                    v(
                        x as i16,
                        (WHEEL_Y - angle.sin() * radius) as i16,
                        (center_z + angle.cos() * radius) as i16,
                    )
                };
                oriented(
                    vec![
                        at(x * 57.0, 25.0, a0),
                        at(x * 57.0, 25.0, a1),
                        at(x * 51.0, 24.0, a0),
                        at(x * 51.0, 24.0, a1),
                    ],
                    corners(&atlas.paint, false).to_vec(),
                    Some(atlas.paint.texture),
                    [128, 128, 128],
                    -27,
                    (x * 40.0, WHEEL_Y, center_z),
                );
            }
        }
        for x in [-1.0f32, 1.0] {
            let center = (x * 57.0, -50.0, 22.0);
            let box_point = |dx: f32, dy: f32, dz: f32| {
                v(
                    (center.0 + dx * x) as i16,
                    (center.1 + dy) as i16,
                    (center.2 + dz) as i16,
                )
            };
            let chrome = corners(&atlas.chrome, false).to_vec();
            let chrome_texture = Some(atlas.chrome.texture);
            oriented(
                vec![
                    box_point(4.0, -3.0, 2.0),
                    box_point(-3.0, -3.0, 2.0),
                    box_point(4.0, 3.0, 2.0),
                    box_point(-3.0, 3.0, 2.0),
                ],
                chrome.clone(),
                chrome_texture,
                [128; 3],
                -42,
                center,
            );
            oriented(
                vec![
                    box_point(4.0, -3.0, -2.0),
                    box_point(4.0, -3.0, 2.0),
                    box_point(4.0, 3.0, -2.0),
                    box_point(4.0, 3.0, 2.0),
                ],
                chrome.clone(),
                chrome_texture,
                [128; 3],
                -42,
                center,
            );
            oriented(
                vec![
                    box_point(-3.0, -3.0, -2.0),
                    box_point(4.0, -3.0, -2.0),
                    box_point(-3.0, 3.0, -2.0),
                    box_point(4.0, 3.0, -2.0),
                ],
                vec![(0, 0); 4],
                None,
                [20, 20, 24],
                -42,
                center,
            );
            oriented(
                vec![
                    box_point(-3.0, -3.0, 2.0),
                    box_point(4.0, -3.0, 2.0),
                    box_point(-3.0, -3.0, -2.0),
                    box_point(4.0, -3.0, -2.0),
                ],
                chrome.clone(),
                chrome_texture,
                [128; 3],
                -42,
                center,
            );
        }
        oriented(
            vec![
                v(-30, -17, FRONT + 4),
                v(30, -17, FRONT + 4),
                v(-30, -6, FRONT - 24),
                v(30, -6, FRONT - 24),
            ],
            vec![(0, 0); 4],
            None,
            [150, 150, 156],
            -30,
            (0.0, -60.0, 60.0),
        );
        for (px, py, pz) in [
            (-17.0f32, -35.0f32, (FRONT + 6) as f32),
            (17.0, -35.0, (FRONT + 6) as f32),
            (-17.0, -20.0, (FRONT + 9) as f32),
            (17.0, -20.0, (FRONT + 9) as f32),
        ] {
            for segment in 0..6 {
                let a0 = segment as f32 / 6.0 * std::f32::consts::TAU;
                let a1 = (segment + 1) as f32 / 6.0 * std::f32::consts::TAU;
                let at = |angle: f32, depth: f32| {
                    v(
                        (px + angle.cos() * 8.0) as i16,
                        (py - angle.sin() * 8.0) as i16,
                        (pz - depth) as i16,
                    )
                };
                oriented(
                    vec![at(a0, 0.0), at(a1, 0.0), at(a0, 6.0), at(a1, 6.0)],
                    vec![(0, 0); 4],
                    None,
                    [24, 24, 26],
                    -46,
                    (px, py, pz - 3.0),
                );
            }
        }
        faces.extend(sculpted_faces);
        Self {
            faces,
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
        let body = car.rotation();
        let wheels = car.wheel_rotation();
        for wheel_pass in [true, false] {
            let (rotation, origin) = if wheel_pass {
                (wheels, car.wheel_origin())
            } else {
                (body, car.origin())
            };
            renderer.set_object(origin, rotation);
            renderer.set_lighting(rotation, sun_direction, sun, ambient);
            for face in self.faces.iter().filter(|face| face.wheel == wheel_pass) {
                renderer.gte.rgbc = [face.flat[0], face.flat[1], face.flat[2], 0];
                let lit = renderer.gte.nccs(face.normal);
                let points: Vec<Point> = face
                    .points
                    .iter()
                    .zip(&face.uvs)
                    .map(|(&p, &uv)| Point::new(p, lit, uv))
                    .collect();
                if renderer.backface(&points) {
                    continue;
                }
                let surface = match face.texture {
                    Some(texture) => Surface::textured(texture),
                    None => Surface::flat(),
                };
                renderer.polygon(&points, surface, face.bias + CAR_BIAS, 0);
            }
        }
        renderer.set_object(car.wheel_origin(), wheels);
        renderer.set_lighting(wheels, sun_direction, sun, ambient);

        let (sin, cos) = (car.wheel_spin.sin(), car.wheel_spin.cos());
        for (x, z) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let center_z = z * WHEEL_Z;
            let outer = (x * 56.0) as i16;
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
            if renderer.backface(&points) {
                continue;
            }
            renderer.polygon(&points, Surface::textured(image.texture), -30 + CAR_BIAS, 0);
        }
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
