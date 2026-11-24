use std::f32::consts::TAU;

use crate::art::Art;
use crate::mesh::{add, length, scale, steps, sub, Builder, Cell, Material, Vec3, Zone};
use crate::rng::hash_float;
use crate::terrain::{seabed, smoothstep, value_noise};

const MOUTH: (f32, f32) = (6.0, 175.0);
const TUNNEL_LENGTH: f32 = 30.0;
const TUNNEL_RISE: f32 = 9.0;
const TUNNEL_RADIUS: f32 = 3.0;
const SIDES: usize = 8;
const CHAMBER: (f32, f32, f32, f32) = (-8.0, 20.0, 205.0, 231.0);
const CHAMBER_HEIGHT: f32 = 15.0;
const POCKET: f32 = 9.5;
const LEDGE: (f32, f32, f32) = (-8.0, 7.0, 9.0);
const CLIFF: (f32, f32) = (-11.0, 23.0);
pub const RIM_LIGHTS: usize = 12;
const TILT: f32 = 0.18;
const SAUCER: [(f32, f32); 8] = [
    (0.0, 1.9),
    (1.2, 1.8),
    (2.0, 1.25),
    (3.8, 0.55),
    (6.2, 0.0),
    (3.8, -0.45),
    (1.4, -0.85),
    (0.0, -0.95),
];
const HULL: [u8; 3] = [118, 124, 128];
const UNDERSIDE: [u8; 3] = [70, 76, 82];
const CUPOLA: [u8; 3] = [60, 150, 140];
const PANEL: [u8; 3] = [96, 100, 108];
const SEAM: [u8; 3] = [52, 56, 62];
const PORTAL: [u8; 3] = [70, 80, 86];
const GROOVE: [u8; 3] = [30, 120, 112];

fn mouth_floor() -> f32 {
    seabed(MOUTH.0, MOUTH.1 - 3.0)
}

pub fn chamber_floor() -> f32 {
    mouth_floor() + TUNNEL_RISE
}

pub fn pocket() -> f32 {
    chamber_floor() + POCKET
}

fn ceiling() -> f32 {
    chamber_floor() + CHAMBER_HEIGHT
}

pub fn chamber_bounds() -> (f32, f32, f32, f32) {
    CHAMBER
}

fn tunnel_axis(along: f32) -> Vec3 {
    let t = (along / TUNNEL_LENGTH).clamp(0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t) * TUNNEL_RISE;
    let sway = (t * std::f32::consts::PI).sin() * 1.5;
    [
        MOUTH.0 + sway,
        mouth_floor() + TUNNEL_RADIUS + rise,
        MOUTH.1 + along,
    ]
}

fn ring(along: f32, side: usize) -> Vec3 {
    let axis = tunnel_axis(along);
    let angle = (side as f32 + 0.5) / SIDES as f32 * TAU;
    let ends = along <= 0.0 || along >= TUNNEL_LENGTH;
    let wobble = if ends {
        0.0
    } else {
        0.08 * (value_noise(along * 0.5, side as f32, 61) - 0.5)
    };
    let radius = TUNNEL_RADIUS * (1.0 + wobble);
    [
        axis[0] + angle.cos() * radius,
        axis[1] + angle.sin() * radius,
        axis[2],
    ]
}

pub fn in_cutout(x: f32, z: f32) -> bool {
    (CLIFF.0..CLIFF.1).contains(&x) && z > MOUTH.1 - 1.0 && z < CHAMBER.3 + 4.0
}

pub fn in_cave(position: Vec3) -> bool {
    let [x, y, z] = position;
    if z > MOUTH.1 - 0.5 && z < MOUTH.1 + TUNNEL_LENGTH {
        let axis = tunnel_axis(z - MOUTH.1);
        return (x - axis[0]).hypot(y - axis[1]) < TUNNEL_RADIUS + 0.5;
    }
    x > CHAMBER.0
        && x < CHAMBER.1
        && z >= CHAMBER.2 - 0.5
        && z < CHAMBER.3
        && y > chamber_floor() - 1.0
        && y < ceiling() + 1.0
}

pub fn in_air(position: Vec3) -> bool {
    in_cave(position) && position[1] > pocket() && position[2] > CHAMBER.2
}

pub fn inside(position: Vec3) -> f32 {
    if !in_cave(position) {
        return 0.0;
    }
    smoothstep(MOUTH.1, MOUTH.1 + 8.0, position[2])
}

fn saucer_origin() -> Vec3 {
    [
        (LEDGE.0 + LEDGE.1) * 0.5,
        chamber_floor() + LEDGE.2 + 0.8,
        (CHAMBER.2 + CHAMBER.3) * 0.5 + 3.0,
    ]
}

fn saucer_point(radius: f32, height: f32, angle: f32) -> Vec3 {
    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
    let (sin, cos) = TILT.sin_cos();
    let (y, x) = (height * cos - x * sin, height * sin + x * cos);
    add(saucer_origin(), [x, y, z])
}

pub fn rim_lights() -> Vec<Vec3> {
    (0..RIM_LIGHTS)
        .map(|index| saucer_point(6.25, 0.0, index as f32 / RIM_LIGHTS as f32 * TAU))
        .collect()
}

pub fn cupola() -> Vec3 {
    saucer_point(0.0, 2.0, 0.0)
}

pub fn portal_lights() -> Vec<Vec3> {
    let axis = tunnel_axis(0.0);
    (0..SIDES)
        .map(|side| {
            let angle = side as f32 / SIDES as f32 * TAU;
            [
                axis[0] + angle.cos() * (TUNNEL_RADIUS + 0.8),
                axis[1] + angle.sin() * (TUNNEL_RADIUS + 0.8),
                MOUTH.1 - 1.2,
            ]
        })
        .collect()
}

pub fn glow(position: Vec3, time: f32) -> Vec3 {
    let pulse = 0.75 + 0.25 * (time * 1.3).sin();
    let offset = sub(position, saucer_origin());
    let distance = (offset[0].hypot(offset[2]) - 6.2).hypot(offset[1]);
    let saucer = (1.0 - distance / 20.0).max(0.0).powi(2) * pulse * 1.1;
    let portal = (1.0 - length(sub(position, tunnel_axis(0.0))) / 9.0)
        .max(0.0)
        .powi(2);
    let along = position[2] - MOUTH.1;
    let groove = if (0.0..TUNNEL_LENGTH).contains(&along) && in_cave(position) {
        let phase = (along * 0.4 - time * 2.0).sin().max(0.0);
        0.35 * phase * phase
    } else {
        0.0
    };
    let amount = saucer + portal * 0.8 + groove;
    [30.0 * amount, 110.0 * amount, 100.0 * amount]
}

fn cliff_top(x: f32) -> f32 {
    seabed(x, MOUTH.1 + 6.0).max(tunnel_axis(0.0)[1] + TUNNEL_RADIUS + 3.0)
}

fn cliff(builder: &mut Builder, pick: &dyn Fn(Vec3) -> Material) {
    let mouth = tunnel_axis(0.0);
    let face_z = MOUTH.1 - 1.0;
    let top_of = cliff_top;
    let bottom = |x: f32| seabed(x, face_z) - 0.5;
    let columns = 8;
    let rows = 8;
    for column in 0..columns {
        let x0 = CLIFF.0 + (CLIFF.1 - CLIFF.0) * column as f32 / columns as f32;
        let x1 = CLIFF.0 + (CLIFF.1 - CLIFF.0) * (column + 1) as f32 / columns as f32;
        for row in 0..rows {
            let level =
                |x: f32, r: usize| bottom(x) + (top_of(x) - bottom(x)) * r as f32 / rows as f32;
            let corners = [
                [x0, level(x0, row + 1), face_z],
                [x1, level(x1, row + 1), face_z],
                [x0, level(x0, row), face_z],
                [x1, level(x1, row), face_z],
            ];
            let middle = scale(
                add(add(corners[0], corners[1]), add(corners[2], corners[3])),
                0.25,
            );
            if (middle[0] - mouth[0]).hypot(middle[1] - mouth[1]) < TUNNEL_RADIUS + 1.6 {
                continue;
            }
            builder.quad_facing(
                Zone::Exterior,
                corners,
                [(0.0, 0.0), (24.0, 0.0), (0.0, 24.0), (24.0, 24.0)],
                pick(middle),
                [middle[0], middle[1], face_z - 20.0],
            );
        }
    }
    for x in [CLIFF.0, CLIFF.1] {
        let zs = [face_z, MOUTH.1 + 8.0, MOUTH.1 + 18.0, CHAMBER.3 + 4.0];
        for pair in zs.windows(2) {
            let (z0, z1) = (pair[0], pair[1]);
            let low = |z: f32| seabed(x, z).min(mouth_floor()) - 1.0;
            let high = |z: f32| seabed(x, z) + 0.5;
            let corners = [
                [x, high(z0), z0],
                [x, high(z1), z1],
                [x, low(z0), z0],
                [x, low(z1), z1],
            ];
            let outward = if x < MOUTH.0 { x - 20.0 } else { x + 20.0 };
            builder.quad_facing(
                Zone::Exterior,
                corners,
                [(0.0, 0.0), (48.0, 0.0), (0.0, 48.0), (48.0, 48.0)],
                pick(corners[0]),
                [outward, corners[0][1], z0],
            );
        }
    }
    let lid_xs = [CLIFF.0, 0.0, 4.0, 8.0, 12.0, CLIFF.1];
    let lid_zs = steps(face_z, CHAMBER.3 + 4.0, 6.0);
    builder.floor(
        Zone::Exterior,
        &lid_xs,
        &lid_zs,
        |x, z| seabed(x, z).max(top_of(x).min(seabed(x, z) + 30.0)) + 0.5,
        true,
        12.0,
        |middle| Some(Cell::new(pick(middle))),
    );
}

fn portal(builder: &mut Builder) {
    let smooth = Material::flat(PORTAL);
    let axis = tunnel_axis(0.0);
    let face_z = MOUTH.1 - 1.0;
    let angle = |s: usize| (s as f32 + 0.5) / SIDES as f32 * TAU;
    let point = |s: usize, radius: f32, z: f32| {
        [
            axis[0] + angle(s).cos() * radius,
            axis[1] + angle(s).sin() * radius,
            z,
        ]
    };
    for side in 0..SIDES {
        let next = side + 1;
        let face = [
            point(side, TUNNEL_RADIUS + 1.7, face_z - 0.3),
            point(next, TUNNEL_RADIUS + 1.7, face_z - 0.3),
            point(side, TUNNEL_RADIUS, face_z - 0.3),
            point(next, TUNNEL_RADIUS, face_z - 0.3),
        ];
        builder.quad_facing(
            Zone::Exterior,
            face,
            [(0.0, 0.0); 4],
            smooth,
            [axis[0], axis[1], face_z - 20.0],
        );
        let rim = [
            point(side, TUNNEL_RADIUS + 1.7, face_z - 0.3),
            point(next, TUNNEL_RADIUS + 1.7, face_z - 0.3),
            point(side, TUNNEL_RADIUS + 1.7, face_z + 0.2),
            point(next, TUNNEL_RADIUS + 1.7, face_z + 0.2),
        ];
        let outward = point(side, TUNNEL_RADIUS + 9.0, face_z);
        builder.quad_facing(Zone::Exterior, rim, [(0.0, 0.0); 4], smooth, outward);
        let reveal = [
            point(side, TUNNEL_RADIUS, face_z - 0.3),
            point(next, TUNNEL_RADIUS, face_z - 0.3),
            point(side, TUNNEL_RADIUS, MOUTH.1),
            point(next, TUNNEL_RADIUS, MOUTH.1),
        ];
        builder.quad_facing(Zone::Exterior, reveal, [(0.0, 0.0); 4], smooth, axis);
    }
}

fn tunnel(builder: &mut Builder) {
    let smooth = Material::flat(PORTAL).tinted([0.85, 0.9, 0.9]);
    let groove = Material::flat(GROOVE);
    let count = TUNNEL_LENGTH as usize / 2;
    for step in 0..count {
        let a = step as f32 / count as f32 * TUNNEL_LENGTH;
        let b = (step + 1) as f32 / count as f32 * TUNNEL_LENGTH;
        let centre = tunnel_axis((a + b) * 0.5);
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            let corners = [ring(a, side), ring(a, next), ring(b, side), ring(b, next)];
            let material = if (side == 1 || side == 5) && step % 2 == 0 {
                groove
            } else {
                smooth
            };
            builder.quad_facing(Zone::Interior, corners, [(0.0, 0.0); 4], material, centre);
        }
    }
}

fn chamber(builder: &mut Builder, pick: &dyn Fn(Vec3) -> Material) {
    let (x0, x1, z0, z1) = CHAMBER;
    let floor = chamber_floor();
    let top = ceiling();
    let centre = [(x0 + x1) * 0.5, (floor + top) * 0.5, (z0 + z1) * 0.5];
    let jagged = |p: Vec3| {
        let n = value_noise(p[0] * 0.35 + p[2] * 0.2, p[1] * 0.4, 81) - 0.5;
        let m = value_noise(p[2] * 0.35, p[0] * 0.3 + p[1] * 0.2, 82) - 0.5;
        [p[0] + n * 1.6, p[1], p[2] + m * 1.6]
    };
    let walls = [
        ((x0, z0), (x0, z1)),
        ((x0, z1), (x1, z1)),
        ((x1, z1), (x1, z0)),
        ((x1, z0), (x0, z0)),
    ];
    let heights = [
        floor - 0.5,
        floor + 3.0,
        floor + 6.0,
        floor + 9.0,
        top - 2.5,
        top,
    ];
    let exit = tunnel_axis(TUNNEL_LENGTH);
    for (a, b) in walls {
        let span = (b.0 - a.0).hypot(b.1 - a.1);
        let columns = (span / 2.0).ceil() as usize;
        for column in 0..columns {
            let t0 = column as f32 / columns as f32;
            let t1 = (column + 1) as f32 / columns as f32;
            for pair in heights.windows(2) {
                let at = |t: f32, y: f32| [a.0 + (b.0 - a.0) * t, y, a.1 + (b.1 - a.1) * t];
                let corners = [
                    at(t0, pair[1]),
                    at(t1, pair[1]),
                    at(t0, pair[0]),
                    at(t1, pair[0]),
                ];
                let middle = scale(add(corners[0], corners[3]), 0.5);
                let opening = (middle[2] - z0).abs() < 0.1
                    && (middle[0] - exit[0]).hypot(middle[1] - exit[1]) < TUNNEL_RADIUS + 0.6;
                if opening {
                    continue;
                }
                let corners = if (middle[2] - z0).abs() < 0.1 {
                    corners
                } else {
                    corners.map(jagged)
                };
                builder.quad_facing(
                    Zone::Interior,
                    corners,
                    [(0.0, 0.0), (24.0, 0.0), (0.0, 36.0), (24.0, 36.0)],
                    pick(middle),
                    centre,
                );
            }
        }
    }
    let xs = steps(x0, x1, 2.0);
    let zs = steps(z0, z1, 2.0);
    let on_ledge = |x: f32, z: f32| x > LEDGE.0 && x < LEDGE.1 && z > z0 + 6.0;
    let floor_at = |x: f32, z: f32| {
        let base = if on_ledge(x - 0.01, z - 0.01) || on_ledge(x + 0.01, z + 0.01) {
            floor + LEDGE.2
        } else {
            floor
        };
        base + (value_noise(x * 0.3, z * 0.3, 91) - 0.5) * 0.6
    };
    builder.floor(Zone::Interior, &xs, &zs, floor_at, true, 12.0, |middle| {
        Some(Cell::new(pick(middle)))
    });
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |x, z| top + (value_noise(x * 0.25, z * 0.25, 93) - 0.5) * 2.5,
        false,
        12.0,
        |middle| Some(Cell::new(pick(middle))),
    );
    let step_z = z0 + 6.0;
    let ledge_rows = [floor - 0.3, floor + LEDGE.2 * 0.5, floor + LEDGE.2 + 0.3];
    builder.wall(
        Zone::Interior,
        (LEDGE.0, step_z),
        (LEDGE.1, step_z),
        &ledge_rows,
        2.0,
        12.0,
        |middle| Some(Cell::new(pick(middle)).biased(-4)),
    );
    builder.wall(
        Zone::Interior,
        (LEDGE.1, step_z),
        (LEDGE.1, z1),
        &ledge_rows,
        2.0,
        12.0,
        |middle| Some(Cell::new(pick(middle)).biased(-4)),
    );
}

fn saucer(builder: &mut Builder) {
    let hull = Material::flat(HULL);
    let belly = Material::flat(UNDERSIDE);
    let glass = Material::flat(CUPOLA);
    let panel = Material::flat(PANEL);
    let seam = Material::flat(SEAM);
    let segments = 20;
    let origin = saucer_origin();
    for segment in 0..segments {
        let a0 = segment as f32 / segments as f32 * TAU;
        let a1 = (segment + 1) as f32 / segments as f32 * TAU;
        for pair in SAUCER.windows(2) {
            let ((ra, ya), (rb, yb)) = (pair[0], pair[1]);
            let band = (segment + (ra * 2.0) as usize).is_multiple_of(2);
            let material = if ya > 1.2 && yb > 1.2 {
                glass
            } else if ya <= 0.0 && yb <= 0.0 {
                if band {
                    belly
                } else {
                    seam
                }
            } else if band {
                hull
            } else {
                panel
            };
            let corners = [
                saucer_point(ra, ya, a0),
                saucer_point(ra, ya, a1),
                saucer_point(rb, yb, a0),
                saucer_point(rb, yb, a1),
            ];
            let middle = scale(
                add(add(corners[0], corners[1]), add(corners[2], corners[3])),
                0.25,
            );
            let outward = add(
                middle,
                sub(middle, add(origin, [0.0, (ya + yb) * 0.4, 0.0])),
            );
            let quad = if ra < 0.01 {
                [corners[2], corners[3], corners[0], corners[0]]
            } else if rb < 0.01 {
                [corners[0], corners[1], corners[2], corners[2]]
            } else {
                corners
            };
            builder.quad_facing(Zone::Interior, quad, [(0.0, 0.0); 4], material, outward);
        }
    }
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    let rocks: Vec<Material> = art.rock.iter().map(Material::textured).collect();
    let pick = |position: Vec3| {
        let roll = hash_float(
            201,
            (position[0] * 2.0 + 500.0) as u64,
            (position[2] * 2.0 + position[1] * 3.0 + 500.0) as u64,
        );
        rocks[(roll * rocks.len() as f32) as usize % rocks.len()]
    };
    cliff(builder, &pick);
    portal(builder);
    tunnel(builder);
    chamber(builder, &pick);
    saucer(builder);
}

pub fn surface_cells() -> Vec<(f32, f32)> {
    let (x0, x1, z0, z1) = CHAMBER;
    let mut cells = Vec::new();
    let mut z = z0;
    while z < z1 - 0.01 {
        let mut x = x0;
        while x < x1 - 0.01 {
            let on_ledge = x + 1.0 > LEDGE.0 && x + 1.0 < LEDGE.1 && z + 1.0 > z0 + 6.0;
            if !on_ledge {
                cells.push((x, z));
            }
            x += 2.0;
        }
        z += 2.0;
    }
    cells
}

pub fn collide(position: Vec3, radius: f32) -> Vec3 {
    let mut position = position;
    let centre = add(saucer_origin(), [0.0, 0.4, 0.0]);
    let offset = sub(position, centre);
    let flat = offset[0].hypot(offset[2]);
    let stretched = (flat / (6.4 + radius)).hypot(offset[1] / (1.4 + radius));
    if stretched < 1.0 {
        position = add(centre, scale(offset, 1.0 / stretched.max(0.05)));
    }
    let face_z = MOUTH.1 - 1.0;
    let (x, z) = (position[0], position[2]);
    let mouth = tunnel_axis(0.0);
    let in_portal = (x - mouth[0]).hypot(position[1] - mouth[1]) < TUNNEL_RADIUS - radius;
    let below_top = position[1] < cliff_top(x);
    if (CLIFF.0..CLIFF.1).contains(&x) && below_top && z > face_z - radius && !in_portal {
        let inside_tunnel = z > MOUTH.1 - 0.5 && in_cave(position);
        let inside_chamber = z >= CHAMBER.2 && in_cave(position);
        if !inside_tunnel && !inside_chamber {
            position[2] = face_z - radius;
            return position;
        }
    }
    let along = position[2] - MOUTH.1;
    if (-0.5..TUNNEL_LENGTH).contains(&along) {
        let axis = tunnel_axis(along.max(0.0));
        let (dx, dy) = (position[0] - axis[0], position[1] - axis[1]);
        let distance = dx.hypot(dy).max(0.001);
        let limit = TUNNEL_RADIUS - radius;
        if distance > limit && distance < TUNNEL_RADIUS + 4.0 {
            position[0] = axis[0] + dx / distance * limit;
            position[1] = axis[1] + dy / distance * limit;
        }
        return position;
    }
    let (x0, x1, z0, z1) = CHAMBER;
    if position[2] >= z0 && position[2] < z1 + 3.0 && position[1] > chamber_floor() - 2.0 {
        position[0] = position[0].clamp(x0 + radius + 0.8, x1 - radius - 0.8);
        let exit = tunnel_axis(TUNNEL_LENGTH);
        let through = (position[0] - exit[0]).hypot(position[1] - exit[1]) < TUNNEL_RADIUS - radius;
        let front = if through { z0 } else { z0 + radius };
        position[2] = position[2].min(z1 - radius - 0.8).max(front);
        let step_z = z0 + 6.0;
        let ledge_top = chamber_floor() + LEDGE.2;
        let on_ledge = position[0] > LEDGE.0
            && position[0] < LEDGE.1 + radius
            && position[2] > step_z - radius;
        if on_ledge && position[1] < ledge_top + radius - 0.3 {
            let out_x = LEDGE.1 + radius - position[0];
            let out_z = position[2] - (step_z - radius);
            if out_x < out_z {
                position[0] = LEDGE.1 + radius;
            } else {
                position[2] = step_z - radius;
            }
        }
        let on_top = position[0] > LEDGE.0 && position[0] < LEDGE.1 && position[2] > step_z;
        let floor = chamber_floor() + if on_top { LEDGE.2 } else { 0.0 } + radius + 0.4;
        position[1] = position[1].min(pocket() + 0.2).max(floor);
    }
    position
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sub::{Controls, Sub};

    #[test]
    fn rov_enters_tunnel_without_escaping() {
        let mouth = tunnel_axis(0.0);
        for (dx, dy) in [
            (0.0, 0.0),
            (1.2, 0.0),
            (-1.4, 0.8),
            (2.6, 1.8),
            (0.0, -1.0),
            (4.5, 2.0),
        ] {
            let start = [mouth[0] + dx, mouth[1] + dy, MOUTH.1 - 8.0];
            let mut rov = Sub::new(start, 0.0);
            for tick in 0..60 * 60 {
                let before = rov.position;
                let turn = if (60 * 18..60 * 24).contains(&tick) {
                    0.6
                } else {
                    0.0
                };
                let thrust = if tick > 60 * 24 { -1.0 } else { 1.0 };
                rov.update(
                    Controls {
                        thrust,
                        turn,
                        lift: if tick > 60 * 24 { -0.3 } else { 0.15 },
                    },
                    1.0 / 60.0,
                );
                let jump = length(sub(rov.position, before));
                assert!(
                    jump < 0.5
                        && (!in_cutout(rov.position[0], rov.position[2])
                            || rov.position[1] < pocket() + 1.0
                            || rov.position[1] > cliff_top(rov.position[0])),
                    "jumped {jump} to {:?} from offset {dx},{dy}",
                    rov.position
                );
            }
        }
    }
}
