use std::f32::consts::TAU;

use crate::art::Art;
use crate::grotto;
use crate::lighting::Lighting;
use crate::mesh::{steps, Builder, Cell, Material, Vec3, Zone};
use crate::rng::hash_float;
use crate::scene::{Billboard, Shaft};
use crate::terrain::{ground_height, plateau_height, value_noise};

pub const RUIN: (f32, f32) = (-66.0, 34.0);
pub const RUIN_CLEARING: f32 = 9.0;
pub const PIT_RADIUS: f32 = 8.0;
pub const PIT_FLOOR: f32 = 12.5;
const PIT_SEGMENTS: usize = 16;
const POOL_TREASURE: (f32, f32) = (-33.5, 17.5);
const PEDESTAL: f32 = 1.0;

pub fn pool_idol() -> Vec3 {
    let (x, z) = POOL_TREASURE;
    [x, ground_height(x, z) + 0.28, z]
}

pub fn jade_idol() -> Vec3 {
    let (x, z) = RUIN;
    [x, ruin_floor() + PEDESTAL + 0.34, z]
}

fn ruin_floor() -> f32 {
    PIT_FLOOR + 0.5
}

pub fn near_ruin(x: f32, z: f32, margin: f32) -> bool {
    (x - RUIN.0).hypot(z - RUIN.1) < RUIN_CLEARING + margin
}

pub fn shaft() -> Shaft {
    let floor = ruin_floor();
    Shaft {
        top: [RUIN.0 + 10.0, floor + 26.0, RUIN.1 - 9.0],
        bottom: [RUIN.0, floor, RUIN.1],
        top_width: 2.0,
        bottom_width: 3.0,
        top_color: [34, 34, 22],
        bottom_color: [8, 8, 5],
    }
}

fn treasure_pile(
    art: &Art,
    lighting: &Lighting,
    image: usize,
    position: Vec3,
    size: f32,
) -> Billboard {
    let lit = lighting.billboard(
        [position[0], position[1] + size * 0.4, position[2]],
        Zone::Exterior,
    );
    Billboard {
        image: art.treasure[image % art.treasure.len()],
        position,
        width: size,
        height: size * 0.75,
        color: lit,
        hanging: false,
    }
}

pub fn treasure(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    let (x, z) = POOL_TREASURE;
    list.push(treasure_pile(
        art,
        lighting,
        0,
        [x + 1.3, ground_height(x + 1.3, z - 0.6) - 0.1, z - 0.6],
        2.2,
    ));
    list.push(treasure_pile(
        art,
        lighting,
        3,
        [x - 1.1, ground_height(x - 1.1, z + 0.9) - 0.1, z + 0.9],
        1.7,
    ));
    let (rx, rz) = RUIN;
    list.push(treasure_pile(
        art,
        lighting,
        2,
        [rx + 1.5, ruin_floor(), rz + 1.3],
        1.8,
    ));
}

fn pit_point(azimuth: f32, height: f32) -> Vec3 {
    let wobble = 1.0 + 0.18 * (value_noise(azimuth * 2.5, height * 0.4, 41) - 0.5);
    let radius = PIT_RADIUS * wobble;
    let (x, z) = (
        RUIN.0 + azimuth.sin() * radius,
        RUIN.1 + azimuth.cos() * radius,
    );
    let rim = plateau_height(x, z) + 0.05;
    [x, PIT_FLOOR + (rim - PIT_FLOOR) * height, z]
}

fn pit(builder: &mut Builder, art: &Art) {
    let rock = Material::textured(&art.cliff[0]).tinted([1.05, 1.1, 1.0]);
    let moss = Material::textured(&art.rockface[1]).tinted([1.2, 1.25, 1.1]);
    let ground = Material::textured(&art.floor[2]).tinted([0.75, 0.8, 0.72]);
    let rim_ground = ground.biased(-8);
    let tunnel = grotto::tunnel_end();
    let tunnel_angle = (tunnel[0] - RUIN.0).atan2(tunnel[2] - RUIN.1);
    let rows = [0.0f32, 0.35, 0.7, 1.0];
    for segment in 0..PIT_SEGMENTS {
        let a0 = tunnel_angle + (segment as f32 - 0.5) / PIT_SEGMENTS as f32 * TAU;
        let a1 = tunnel_angle + (segment as f32 + 0.5) / PIT_SEGMENTS as f32 * TAU;
        for row in rows.windows(2) {
            if segment == 0 && row[0] < 0.6 {
                continue;
            }
            let corners = [
                pit_point(a0, row[1]),
                pit_point(a1, row[1]),
                pit_point(a0, row[0]),
                pit_point(a1, row[0]),
            ];
            let height = corners[0][1] - corners[2][1];
            let material = if hash_float(43, segment as u64, (row[0] * 10.0) as u64) < 0.4 {
                moss
            } else {
                rock
            };
            builder.quad_facing(
                Zone::Exterior,
                corners,
                [
                    (0.0, 0.0),
                    (40.0, 0.0),
                    (0.0, (height * 16.0).min(120.0)),
                    (40.0, (height * 16.0).min(120.0)),
                ],
                material,
                [RUIN.0, corners[2][1] + height * 0.5, RUIN.1],
            );
        }
        let rings = [0.0f32, 0.5, 1.0];
        for ring in rings.windows(2) {
            let at = |angle: f32, fraction: f32| {
                let edge = pit_point(angle, 0.0);
                [
                    RUIN.0 + (edge[0] - RUIN.0) * fraction,
                    PIT_FLOOR,
                    RUIN.1 + (edge[2] - RUIN.1) * fraction,
                ]
            };
            let corners = [
                at(a0, ring[1]),
                at(a1, ring[1]),
                at(a0, ring[0]),
                at(a1, ring[0]),
            ];
            builder.quad_facing(
                Zone::Exterior,
                corners,
                [(0.0, 0.0), (48.0, 0.0), (0.0, 48.0), (48.0, 48.0)],
                ground,
                [RUIN.0, PIT_FLOOR + 5.0, RUIN.1],
            );
        }
        let lip = |angle: f32, extra: f32| {
            let edge = pit_point(angle, 1.0);
            let (dx, dz) = (angle.sin(), angle.cos());
            let (x, z) = (edge[0] + dx * extra, edge[2] + dz * extra);
            [
                x,
                if extra > 0.0 {
                    plateau_height(x, z) + 0.04
                } else {
                    edge[1]
                },
                z,
            ]
        };
        let corners = [lip(a0, 3.5), lip(a1, 3.5), lip(a0, 0.0), lip(a1, 0.0)];
        builder.quad_facing(
            Zone::Exterior,
            corners,
            [(0.0, 0.0), (48.0, 0.0), (0.0, 48.0), (48.0, 48.0)],
            rim_ground,
            [corners[2][0], corners[2][1] + 5.0, corners[2][2]],
        );
    }
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    pit(builder, art);
    let (px, pz) = POOL_TREASURE;
    let gold = Material::textured(&art.coins[1])
        .tinted([1.1, 1.05, 0.95])
        .biased(crate::cave::OVERLAY_BIAS);
    builder.floor(
        Zone::Exterior,
        &steps(px - 2.5, px + 2.5, 2.5),
        &steps(pz - 2.5, pz + 2.5, 2.5),
        |x, z| ground_height(x, z) + 0.06,
        true,
        16.0,
        |middle| ((middle[0] - px).hypot(middle[2] - pz) < 3.0).then(|| Cell::new(gold)),
    );
    let (x, z) = RUIN;
    let floor = ruin_floor();
    let stone = Material::textured(&art.stone[3]);
    let worn = Material::textured(&art.stone[0]);
    builder.block(
        Zone::Exterior,
        [x - 3.0, floor - 1.5, z - 3.0],
        [x + 3.0, floor, z + 3.0],
        24.0,
        worn,
    );
    for (dx, dz, height) in [
        (-2.3, -2.3, 3.2),
        (2.3, -2.3, 1.4),
        (-2.3, 2.3, 2.1),
        (2.3, 2.3, 3.6),
    ] {
        builder.block(
            Zone::Exterior,
            [x + dx - 0.4, floor, z + dz - 0.4],
            [x + dx + 0.4, floor + height, z + dz + 0.4],
            24.0,
            stone,
        );
    }
    let carved = Cell::panel(&art.carvings[3], [0.9, 1.0, 0.9]);
    builder.dressed_block(
        Zone::Exterior,
        [x - 0.5, floor, z - 0.5],
        [x + 0.5, floor + PEDESTAL, z + 0.5],
        24.0,
        [carved; 4],
        Cell::new(stone),
    );
    builder.block(
        Zone::Exterior,
        [x + 3.4, PIT_FLOOR - 0.3, z + 0.2],
        [x + 5.6, PIT_FLOOR + 0.5, z + 1.2],
        24.0,
        stone,
    );
}

pub fn props(art: &Art, list: &mut Vec<Billboard>) {
    let (x, z) = RUIN;
    let floor = ruin_floor();
    for (dx, dz, top, length) in [
        (-2.3, -2.6, 3.2, 1.6),
        (2.6, 2.3, 3.6, 1.8),
        (-2.6, 2.3, 2.1, 1.2),
    ] {
        list.push(Billboard {
            image: art.vines[(length as usize) % art.vines.len()],
            position: [x + dx, floor + top + 0.2, z + dz],
            width: length * 0.55,
            height: length,
            color: [110, 125, 100],
            hanging: true,
        });
    }
    for index in 0..10 {
        let angle = index as f32 / 10.0 * std::f32::consts::TAU + 0.3;
        let distance = 3.6 + (index % 3) as f32 * 1.1;
        let (fx, fz) = (x + angle.cos() * distance, z + angle.sin() * distance);
        let width = 1.4 + (index % 4) as f32 * 0.3;
        list.push(Billboard {
            image: art.procedural.fern,
            position: [fx, PIT_FLOOR - 0.1, fz],
            width,
            height: width * 0.75,
            color: [120, 135, 110],
            hanging: false,
        });
    }
}
