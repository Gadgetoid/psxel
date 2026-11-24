use std::f32::consts::TAU;

use psxel_math::Mat3Fx12;

use crate::biome::Rgb;
use crate::lighting::{sun_direction, water_filter};
use crate::mesh::{add, cross, dot, length, local, multiply, normalize, scale, sub, to_rgb, Vec3};
use crate::render::{world_to_psx, Point, Renderer, Surface};
use crate::terrain::{seabed, BOUNDS, SURFACE};
use crate::textures::Image;
use crate::wreck;

const YELLOW: Rgb = [240, 196, 34];
const TRIM: Rgb = [214, 120, 26];
const GLASS: Rgb = [40, 90, 120];
const LIT_GLASS: Rgb = [255, 226, 140];
const BRASS: Rgb = [190, 150, 70];
const STEEL: Rgb = [110, 115, 120];
const LAMP_OFF: Rgb = [90, 90, 80];
const LAMP_ON: Rgb = [255, 252, 225];
const DUCT: Rgb = [34, 34, 38];
const NEUTRAL: Rgb = [128, 128, 128];
pub const PANEL_FRAME: usize = 2;
pub const PANEL_HATCH: usize = 3;
const RADIUS: f32 = 1.3;
const THRUST: f32 = 4.2;
const LIFT: f32 = 2.6;
const DRAG: f32 = 1.1;
const TURN: f32 = 1.15;
const TETHER_POINTS: usize = 48;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hull,
    Glass,
    Lamp,
    Propeller,
}

struct Face {
    points: Vec<Vec3>,
    color: Rgb,
    kind: Kind,
    double_sided: bool,
    pivot: Vec3,
    panel: Option<usize>,
}

#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub thrust: f32,
    pub turn: f32,
    pub lift: f32,
}

pub struct Basis {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl Basis {
    pub fn apply(&self, point: Vec3) -> Vec3 {
        add(
            add(scale(self.right, point[0]), scale(self.up, point[1])),
            scale(self.forward, point[2]),
        )
    }
}

pub struct Sub {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub yaw_rate: f32,
    pub pitch: f32,
    pub roll: f32,
    pub propeller: f32,
    pub thrust: f32,
    pub lights: bool,
    pub trail: Vec<Vec3>,
    pub panels: Vec<Image>,
    faces: Vec<Face>,
}

fn oriented(mut points: Vec<Vec3>, away_from: Vec3) -> Vec<Vec3> {
    let normal = cross(sub(points[1], points[0]), sub(points[2], points[0]));
    let centre = scale(
        points.iter().fold([0.0; 3], |sum, &p| add(sum, p)),
        1.0 / points.len() as f32,
    );
    if dot(normal, sub(centre, away_from)) < 0.0 {
        points.swap(1, 2);
    }
    points
}

fn quad_points(a: Vec3, b: Vec3, c: Vec3, d: Vec3, away_from: Vec3) -> Vec<Vec3> {
    let normal = cross(sub(b, a), sub(c, a));
    let centre = scale(add(add(a, b), add(c, d)), 0.25);
    if dot(normal, sub(centre, away_from)) >= 0.0 {
        vec![a, b, c, d]
    } else {
        vec![b, a, d, c]
    }
}

fn boxed(faces: &mut Vec<Face>, min: Vec3, max: Vec3, color: Rgb) {
    let centre = scale(add(min, max), 0.5);
    let corner = |x: usize, y: usize, z: usize| {
        [
            if x == 0 { min[0] } else { max[0] },
            if y == 0 { min[1] } else { max[1] },
            if z == 0 { min[2] } else { max[2] },
        ]
    };
    let sides = [
        [
            corner(0, 1, 1),
            corner(1, 1, 1),
            corner(0, 0, 1),
            corner(1, 0, 1),
        ],
        [
            corner(1, 1, 0),
            corner(0, 1, 0),
            corner(1, 0, 0),
            corner(0, 0, 0),
        ],
        [
            corner(1, 1, 1),
            corner(1, 1, 0),
            corner(1, 0, 1),
            corner(1, 0, 0),
        ],
        [
            corner(0, 1, 0),
            corner(0, 1, 1),
            corner(0, 0, 0),
            corner(0, 0, 1),
        ],
        [
            corner(0, 1, 0),
            corner(1, 1, 0),
            corner(0, 1, 1),
            corner(1, 1, 1),
        ],
        [
            corner(0, 0, 1),
            corner(1, 0, 1),
            corner(0, 0, 0),
            corner(1, 0, 0),
        ],
    ];
    for [a, b, c, d] in sides {
        faces.push(Face {
            points: quad_points(a, b, c, d, centre),
            color,
            kind: Kind::Hull,
            double_sided: false,
            pivot: [0.0; 3],
            panel: None,
        });
    }
}

fn cylinder(
    faces: &mut Vec<Face>,
    centre: Vec3,
    radius: f32,
    z0: f32,
    z1: f32,
    color: Rgb,
    inside: Rgb,
) {
    let sides = 12;
    let point = |side: usize, z: f32| {
        let angle = side as f32 / sides as f32 * TAU;
        [
            centre[0] + angle.cos() * radius,
            centre[1] + angle.sin() * radius,
            z,
        ]
    };
    for side in 0..sides {
        let next = (side + 1) % sides;
        let (a, b, c, d) = (
            point(side, z1),
            point(next, z1),
            point(side, z0),
            point(next, z0),
        );
        let axis = [centre[0], centre[1], (z0 + z1) * 0.5];
        faces.push(Face {
            points: quad_points(a, b, c, d, axis),
            color,
            kind: Kind::Hull,
            double_sided: false,
            pivot: [0.0; 3],
            panel: None,
        });
        let shrink = |p: Vec3| {
            [
                centre[0] + (p[0] - centre[0]) * 0.94,
                centre[1] + (p[1] - centre[1]) * 0.94,
                p[2],
            ]
        };
        faces.push(Face {
            points: quad_points(
                shrink(b),
                shrink(a),
                shrink(d),
                shrink(c),
                [centre[0] + 9.0, centre[1] + 9.0, (z0 + z1) * 0.5],
            ),
            color: inside,
            kind: Kind::Hull,
            double_sided: true,
            pivot: [0.0; 3],
            panel: None,
        });
    }
}

fn profile() -> Vec<((f32, f32), bool)> {
    let (half, bottom, top) = (0.75f32, -0.35f32, 0.5f32);
    let corners = [
        (
            (half - 0.14, bottom + 0.14),
            0.14,
            -std::f32::consts::FRAC_PI_2,
        ),
        ((half - 0.3, top - 0.3), 0.3, 0.0),
        ((-half + 0.3, top - 0.3), 0.3, std::f32::consts::FRAC_PI_2),
        ((-half + 0.14, bottom + 0.14), 0.14, std::f32::consts::PI),
    ];
    let mut points = Vec::new();
    for ((cx, cy), radius, start) in corners {
        for step in 0..=3 {
            let angle = start + step as f32 / 3.0 * std::f32::consts::FRAC_PI_2;
            points.push((
                (cx + angle.cos() * radius, cy + angle.sin() * radius),
                step == 3,
            ));
        }
    }
    points
}
const BODY: (f32, f32) = (-1.1, 0.85);
const DOME: (Vec3, f32) = ([0.0, 0.05, 0.85], 0.36);
const THRUSTERS: [Vec3; 2] = [[-1.05, 0.0, -0.75], [1.05, 0.0, -0.75]];

fn build_faces() -> Vec<Face> {
    let mut faces = Vec::new();
    let axis = |z: f32| [0.0, 0.08, z];
    let outline = profile();
    let count = outline.len();
    let slices = [
        (BODY.0, 0.84),
        (BODY.0 + 0.16, 1.0),
        (BODY.1 - 0.12, 1.0),
        (BODY.1, 0.9),
    ];
    let at = |index: usize, slice: (f32, f32)| {
        let ((x, y), _) = outline[index % count];
        [x * slice.1, 0.08 + (y - 0.08) * slice.1, slice.0]
    };
    for pair in slices.windows(2) {
        let middle = pair[0].1 == 1.0 && pair[1].1 == 1.0;
        for index in 0..count {
            let flat = outline[index].1;
            let (a, b) = (outline[index].0, outline[(index + 1) % count].0);
            let points = quad_points(
                at(index + 1, pair[1]),
                at(index, pair[1]),
                at(index + 1, pair[0]),
                at(index, pair[0]),
                axis((pair[0].0 + pair[1].0) * 0.5),
            );
            let horizontal = (a.1 - b.1).abs() < 0.01;
            let vertical = (a.0 - b.0).abs() < 0.01;
            let underside = horizontal && a.1 < 0.0;
            let panel = match (middle && flat, underside, vertical, horizontal) {
                (true, false, true, _) => Some(PANEL_HATCH),
                (true, false, _, true) => Some(PANEL_FRAME),
                _ => None,
            };
            let color = if underside {
                TRIM
            } else if panel.is_some() {
                NEUTRAL
            } else {
                YELLOW
            };
            faces.push(Face {
                points,
                color,
                kind: Kind::Hull,
                double_sided: false,
                pivot: [0.0; 3],
                panel,
            });
        }
    }
    let (dome_centre, dome_radius) = DOME;
    for (slice, outward) in [(slices[0], -1.0f32), (slices[3], 1.0)] {
        let z = slice.0;
        let centre = [0.0, 0.08, z];
        for index in 0..count {
            let (a, b) = (at(index, slice), at(index + 1, slice));
            if outward > 0.0 {
                let ring = |p: Vec3| {
                    let offset = [p[0] - dome_centre[0], p[1] - dome_centre[1]];
                    let length = offset[0].hypot(offset[1]).max(0.01);
                    let shrink = (dome_radius / length).min(1.0);
                    [
                        dome_centre[0] + offset[0] * shrink,
                        dome_centre[1] + offset[1] * shrink,
                        z,
                    ]
                };
                faces.push(Face {
                    points: quad_points(a, b, ring(a), ring(b), axis(z - 1.0)),
                    color: YELLOW,
                    kind: Kind::Hull,
                    double_sided: false,
                    pivot: [0.0; 3],
                    panel: None,
                });
            } else {
                faces.push(Face {
                    points: oriented(vec![centre, a, b], axis(z + 1.0)),
                    color: YELLOW,
                    kind: Kind::Hull,
                    double_sided: false,
                    pivot: [0.0; 3],
                    panel: None,
                });
            }
        }
    }
    let rings = [0.0f32, 0.35, 0.65, 0.88, 1.0];
    let segments = 12;
    for pair in rings.windows(2) {
        for side in 0..segments {
            let next = (side + 1) % segments;
            let point = |s: usize, t: f32| {
                let angle = s as f32 / segments as f32 * TAU;
                let spread = (t * std::f32::consts::FRAC_PI_2).cos();
                [
                    dome_centre[0] + angle.cos() * dome_radius * spread,
                    dome_centre[1] + angle.sin() * dome_radius * spread,
                    dome_centre[2] + (t * std::f32::consts::FRAC_PI_2).sin() * dome_radius,
                ]
            };
            let points = if pair[1] >= 1.0 {
                oriented(
                    vec![
                        point(side, pair[0]),
                        point(next, pair[0]),
                        point(side, pair[1]),
                    ],
                    dome_centre,
                )
            } else {
                quad_points(
                    point(side, pair[0]),
                    point(next, pair[0]),
                    point(side, pair[1]),
                    point(next, pair[1]),
                    dome_centre,
                )
            };
            faces.push(Face {
                points,
                color: GLASS,
                kind: Kind::Glass,
                double_sided: false,
                pivot: [0.0; 3],
                panel: None,
            });
        }
    }
    boxed(&mut faces, [-0.1, -0.08, 0.72], [0.1, 0.12, 0.92], STEEL);
    for x in [-0.52f32, 0.52] {
        boxed(
            &mut faces,
            [x - 0.12, -0.32, 0.8],
            [x + 0.12, -0.08, 1.02],
            STEEL,
        );
        faces.push(Face {
            points: quad_points(
                [x - 0.1, -0.1, 1.03],
                [x + 0.1, -0.1, 1.03],
                [x - 0.1, -0.3, 1.03],
                [x + 0.1, -0.3, 1.03],
                [x, -0.2, 0.0],
            ),
            color: LAMP_OFF,
            kind: Kind::Lamp,
            double_sided: false,
            pivot: [0.0; 3],
            panel: None,
        });
    }
    for thruster in THRUSTERS {
        let side = thruster[0].signum();
        let (near, far) = if side < 0.0 {
            (-0.86, -0.74)
        } else {
            (0.74, 0.86)
        };
        boxed(&mut faces, [near, -0.06, -0.85], [far, 0.06, -0.65], STEEL);
        cylinder(
            &mut faces,
            thruster,
            0.23,
            thruster[2] - 0.35,
            thruster[2] + 0.3,
            DUCT,
            [20, 20, 22],
        );
        for blade in 0..3 {
            let angle = blade as f32 / 3.0 * TAU;
            let tip = |radius: f32, offset: f32| {
                let (s, c) = (angle + offset).sin_cos();
                [
                    thruster[0] + c * radius,
                    thruster[1] + s * radius,
                    thruster[2] - 0.1,
                ]
            };
            faces.push(Face {
                points: vec![tip(0.03, 0.0), tip(0.2, -0.35), tip(0.2, 0.35)],
                color: BRASS,
                kind: Kind::Propeller,
                double_sided: true,
                pivot: thruster,
                panel: None,
            });
        }
    }
    let top = [0.0, 0.5, -0.25];
    let hole: Vec<Vec3> = (0..8)
        .map(|index| {
            let angle = index as f32 / 8.0 * TAU;
            [
                top[0] + angle.cos() * 0.26,
                top[1] + 0.01,
                top[2] + angle.sin() * 0.26,
            ]
        })
        .collect();
    for index in 0..8 {
        faces.push(Face {
            points: oriented(
                vec![
                    [top[0], top[1] + 0.01, top[2]],
                    hole[index],
                    hole[(index + 1) % 8],
                ],
                [0.0, 0.0, -0.25],
            ),
            color: [24, 24, 26],
            kind: Kind::Hull,
            double_sided: false,
            pivot: [0.0; 3],
            panel: None,
        });
    }
    for x in [-0.55f32, 0.55] {
        boxed(
            &mut faces,
            [x - 0.05, -0.58, -1.15],
            [x + 0.05, -0.5, 1.0],
            DUCT,
        );
        for z in [-0.8f32, 0.6] {
            boxed(
                &mut faces,
                [x - 0.03, -0.5, z - 0.03],
                [x + 0.03, -0.35, z + 0.03],
                DUCT,
            );
        }
    }
    faces
}

impl Sub {
    pub fn new(position: Vec3, yaw: f32) -> Self {
        Self {
            position,
            velocity: [0.0; 3],
            yaw,
            yaw_rate: 0.0,
            pitch: 0.0,
            roll: 0.0,
            propeller: 0.0,
            thrust: 0.0,
            lights: true,
            trail: vec![position],
            panels: Vec::new(),
            faces: build_faces(),
        }
    }

    pub fn basis(&self) -> Basis {
        let forward = [
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        ];
        let flat_right = normalize(cross([0.0, 1.0, 0.0], forward));
        let flat_up = cross(forward, flat_right);
        let (sin, cos) = self.roll.sin_cos();
        Basis {
            right: add(scale(flat_right, cos), scale(flat_up, sin)),
            up: add(scale(flat_up, cos), scale(flat_right, -sin)),
            forward,
        }
    }

    pub fn update(&mut self, controls: Controls, dt: f32) {
        self.thrust += (controls.thrust - self.thrust) * (dt * 2.5).min(1.0);
        self.yaw_rate += (controls.turn * TURN - self.yaw_rate) * (dt * 2.2).min(1.0);
        self.yaw += self.yaw_rate * dt;
        let heading = [self.yaw.sin(), 0.0, self.yaw.cos()];
        let acceleration = add(
            scale(heading, self.thrust * THRUST),
            [0.0, controls.lift * LIFT, 0.0],
        );
        self.velocity = add(self.velocity, scale(acceleration, dt));
        self.velocity = scale(self.velocity, (-DRAG * dt).exp());
        self.velocity[1] *= (-0.8 * dt).exp();
        let mut next = add(self.position, scale(self.velocity, dt));
        next = wreck::collide(next, RADIUS);
        next = crate::alien::collide(next, RADIUS);
        let caved = crate::alien::in_cave(next);
        let floor = if caved {
            f32::MIN
        } else {
            seabed(next[0], next[2]) + RADIUS
        };
        if next[1] < floor {
            next[1] = floor;
            self.velocity[1] = self.velocity[1].max(0.0);
        }
        let ceiling = SURFACE - 1.5;
        if next[1] > ceiling {
            next[1] = ceiling;
            self.velocity[1] = self.velocity[1].min(0.0);
        }
        let (x0, x1, z0, z1) = BOUNDS;
        if !caved {
            next[0] = next[0].clamp(x0 + 6.0, x1 - 6.0);
            next[2] = next[2].clamp(z0 + 6.0, z1 - 6.0);
        }
        self.velocity = scale(sub(next, self.position), 1.0 / dt.max(1e-4));
        self.position = next;
        let pitch_target = (self.velocity[1] * 0.14).clamp(-0.4, 0.4);
        self.pitch += (pitch_target - self.pitch) * (dt * 2.0).min(1.0);
        let roll_target = (-self.yaw_rate * 0.3).clamp(-0.35, 0.35);
        self.roll += (roll_target - self.roll) * (dt * 2.0).min(1.0);
        self.propeller += self.thrust * dt * 22.0 + dt * 2.0;
        let tail = add(self.position, self.basis().apply([0.0, 0.3, -1.15]));
        if self
            .trail
            .last()
            .is_none_or(|&last| length(sub(last, tail)) > 1.0)
        {
            self.trail.push(tail);
            if self.trail.len() > TETHER_POINTS {
                self.trail.remove(0);
            }
        }
    }

    pub fn lamp_positions(&self) -> [Vec3; 2] {
        let basis = self.basis();
        [-0.52f32, 0.52].map(|x| add(self.position, basis.apply([x, -0.2, 1.05])))
    }

    pub fn draw(&self, renderer: &mut Renderer) {
        let basis = self.basis();
        let origin = self.position;
        renderer.set_object(
            world_to_psx(origin[0], origin[1], origin[2]),
            Mat3Fx12::IDENTITY,
        );
        let filter = water_filter(-origin[1] * 0.4);
        let ambient = add([62.0, 56.0, 44.0], multiply([46.0, 56.0, 60.0], filter));
        let sun = multiply([190.0, 185.0, 165.0], filter);
        let (spin_sin, spin_cos) = self.propeller.sin_cos();
        for face in &self.faces {
            let points: Vec<Vec3> = face
                .points
                .iter()
                .map(|&point| {
                    let point = if face.kind == Kind::Propeller {
                        let (dx, dy) = (point[0] - face.pivot[0], point[1] - face.pivot[1]);
                        [
                            face.pivot[0] + dx * spin_cos - dy * spin_sin,
                            face.pivot[1] + dx * spin_sin + dy * spin_cos,
                            point[2],
                        ]
                    } else {
                        point
                    };
                    basis.apply(point)
                })
                .collect();
            let normal = normalize(cross(sub(points[1], points[0]), sub(points[2], points[0])));
            let color = match (face.kind, self.lights) {
                (Kind::Lamp, true) => LAMP_ON,
                (Kind::Glass, true) => LIT_GLASS,
                _ => {
                    let facing = dot(normal, sun_direction())
                        .abs()
                        .max(if face.double_sided {
                            0.0
                        } else {
                            dot(normal, sun_direction())
                        });
                    let light = add(ambient, scale(sun, facing.max(0.0) * 0.8));
                    to_rgb(multiply(face.color.map(|c| c as f32 / 128.0), light))
                }
            };
            let image = face
                .panel
                .and_then(|index| self.panels.get(index))
                .filter(|_| points.len() == 4);
            let uvs: [(u8, u8); 4] = match image {
                Some(image) => {
                    let (u0, v0) = (image.u, image.v);
                    let (u1, v1) = (u0 + image.width - 1, v0 + image.height - 1);
                    [(u0, v0), (u1, v0), (u0, v1), (u1, v1)]
                }
                None => [(0, 0); 4],
            };
            let converted: Vec<Point> = points
                .iter()
                .zip(uvs)
                .map(|(&point, uv)| Point::new(local(add(origin, point), origin), color, uv))
                .collect();
            if !face.double_sided && renderer.backface(&converted) {
                continue;
            }
            let surface = match image {
                Some(image) => Surface::textured(image.texture),
                None => Surface::gouraud(),
            };
            renderer.polygon(&converted, surface, -6, 1);
        }
    }
}
