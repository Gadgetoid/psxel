use std::f32::consts::PI;

use crate::art::Art;
use crate::lighting::{Light, LightKind, Lighting};
use crate::mesh::{steps, Builder, Cell, Material, Vec3, Zone};
use crate::path::{key, Key};
use crate::ruins::{slab, Faces};
use crate::scene::{Billboard, Shaft};
use crate::secrets;
use crate::terrain::{smoothstep, CLIFF_X, FALLS_Z, RIVER_Y};

pub const HILL: (f32, f32) = (CLIFF_X - 8.0, FALLS_Z);
pub const LEDGE: f32 = 16.0;
pub const CEILING: f32 = 21.5;
pub const WATER: f32 = RIVER_Y;
const BASIN: f32 = 13.5;
const WEST: f32 = CLIFF_X - 16.0;
const EAST: f32 = CLIFF_X - 0.6;
const SOUTH: f32 = FALLS_Z - 9.0;
const NORTH: f32 = FALLS_Z + 9.0;
pub const POOL: (f32, f32, f32, f32) = (CLIFF_X - 13.0, EAST, FALLS_Z - 5.0, FALLS_Z + 5.0);
pub const MOUTH: (f32, f32, f32) = (FALLS_Z - 2.0, FALLS_Z + 2.0, LEDGE + 3.0);
pub const CRACKS: [(f32, f32); 2] = [
    (CLIFF_X - 11.0, FALLS_Z - 2.0),
    (CLIFF_X - 5.0, FALLS_Z + 4.0),
];
const DOOR: (f32, f32, f32) = (FALLS_Z + 4.5, FALLS_Z + 7.5, LEDGE + 2.9);
const PASSAGE_HEIGHT: f32 = 2.9;
const PASSAGE_TURN: f32 = CLIFF_X - 24.8;
const PASSAGE_STEPS: usize = 8;
const PIT_DOOR_Z: f32 = secrets::RUIN.1 - secrets::PIT_RADIUS + 0.5;
pub const IDOL: Vec3 = [CLIFF_X - 7.0, BASIN + 1.4 + 0.34, FALLS_Z + 1.5];

pub fn hill(x: f32, z: f32) -> f32 {
    6.5 * (1.0 - smoothstep(12.0, 22.0, (x - HILL.0).hypot(z - HILL.1)))
}

pub fn in_mouth(z0: f32, z1: f32, y0: f32, y1: f32) -> bool {
    z0 >= MOUTH.0 && z1 <= MOUTH.1 && y0 >= LEDGE && y1 <= MOUTH.2 + 0.01
}

fn passage_floor(x: f32) -> f32 {
    let along = ((WEST - 0.6 - x) / (WEST - 0.6 - PASSAGE_TURN)).clamp(0.0, 1.0);
    let step = (along * PASSAGE_STEPS as f32).ceil();
    LEDGE - step * (LEDGE - secrets::PIT_FLOOR) / PASSAGE_STEPS as f32
}

pub fn tunnel_end() -> Vec3 {
    [PASSAGE_TURN - 1.5, secrets::PIT_FLOOR, PIT_DOOR_Z]
}

pub fn mouth() -> Vec3 {
    [EAST - 0.5, LEDGE + 1.5, FALLS_Z]
}

pub fn skylights() -> [Vec3; 2] {
    CRACKS.map(|(x, z)| [x, CEILING - 0.5, z])
}

pub fn amount(position: Vec3) -> f32 {
    let [x, y, z] = position;
    let in_cave = x > WEST - 0.3
        && x < EAST + 0.3
        && z > SOUTH
        && z < NORTH
        && y > BASIN - 0.5
        && y < CEILING;
    if in_cave {
        return smoothstep(EAST + 0.3, EAST - 2.5, x);
    }
    let west_leg = x < WEST
        && x > PASSAGE_TURN - 3.2
        && z > DOOR.0 - 0.3
        && z < DOOR.1 + 0.3
        && y < LEDGE + 3.0;
    let north_leg = x < PASSAGE_TURN + 0.1
        && x > PASSAGE_TURN - 3.2
        && z > DOOR.0
        && z < PIT_DOOR_Z
        && y < LEDGE;
    if west_leg || north_leg {
        1.0
    } else {
        0.0
    }
}

pub fn near(x: f32, z: f32, margin: f32) -> bool {
    x > WEST - margin && x < EAST + margin && z > SOUTH - margin && z < NORTH + margin
}

pub fn keys() -> Vec<Key> {
    let ruin = secrets::jade_idol();
    let pit = |angle: f32, radius: f32, height: f32| {
        [
            secrets::RUIN.0 + angle.sin() * radius,
            secrets::PIT_FLOOR + height,
            secrets::RUIN.1 + angle.cos() * radius,
        ]
    };
    let at = |dx: f32, dz: f32, height: f32| [CLIFF_X + dx, LEDGE + height, FALLS_Z + dz];
    let door_z = (DOOR.0 + DOOR.1) * 0.5;
    let passage = |x: f32| [x, passage_floor(x) + 1.35, door_z];
    let turn = PASSAGE_TURN - 1.5;
    let entry = PI;
    let crack = [CRACKS[0].0, LEDGE, CRACKS[0].1];
    vec![
        key([-31.0, 6.0, 15.0], [-38.0, 15.0, 14.0], 3.0),
        key([-35.0, 15.0, FALLS_Z - 0.6], at(-6.0, 0.0, 1.8), 2.4),
        key(at(-0.8, -0.4, 1.6), at(-11.0, -2.0, 3.0), 1.8),
        key(at(-4.5, -2.0, 2.2), crack, 1.4),
        key(at(-8.0, -6.6, 2.4), [WEST, LEDGE + 3.0, FALLS_Z - 2.0], 1.3),
        key(at(-13.6, -3.0, 2.2), IDOL, 1.3),
        key(at(-13.4, 3.6, 2.2), [WEST, LEDGE + 1.4, door_z], 1.3),
        key([WEST + 1.8, LEDGE + 1.4, door_z], passage(WEST - 4.0), 1.2),
        key(passage(WEST - 2.0), passage(WEST - 6.0), 1.2),
        key(
            passage(PASSAGE_TURN + 1.8),
            [turn, secrets::PIT_FLOOR + 1.3, door_z],
            1.1,
        ),
        key(
            [turn, secrets::PIT_FLOOR + 1.35, door_z - 0.2],
            [turn, secrets::PIT_FLOOR + 1.3, PIT_DOOR_Z],
            0.9,
        ),
        key(
            [turn, secrets::PIT_FLOOR + 1.35, door_z + 2.2],
            [turn, secrets::PIT_FLOOR + 1.2, PIT_DOOR_Z + 2.0],
            1.1,
        ),
        key(
            [turn, secrets::PIT_FLOOR + 1.35, PIT_DOOR_Z - 1.5],
            ruin,
            1.2,
        ),
        key(pit(entry - 0.3, secrets::PIT_RADIUS - 1.5, 1.6), ruin, 1.3),
        key(pit(entry - 1.4, 5.0, 2.4), ruin, 1.3),
        key(pit(entry - 2.6, 5.0, 3.2), ruin, 1.4),
        key(pit(entry - 3.6, 4.0, 7.0), ruin, 2.0),
        key(pit(entry - 4.2, 2.0, 16.0), [-30.0, 10.0, 30.0], 3.0),
        key([-45.0, 26.0, 34.0], [-14.0, 6.0, 40.0], 4.0),
    ]
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    let rock = Material::textured(&art.cliff[0]).tinted([0.95, 1.0, 0.95]);
    let moss = Material::textured(&art.rockface[1]).tinted([1.1, 1.15, 1.05]);
    let wet = Material::textured(&art.riverbed).tinted([0.8, 0.85, 0.8]);
    let stone = Material::textured(&art.stone[2]).tinted([0.9, 0.95, 0.9]);
    let paving = Material::textured(&art.paving).tinted([0.8, 0.85, 0.8]);
    let gold = Material::textured(&art.coins[1]).biased(-16);
    let rocky = |middle: Vec3| {
        let roll = crate::rng::hash_float(
            77,
            (middle[0] * 2.0 + 500.0) as u64,
            (middle[2] * 2.0 + middle[1] * 5.0 + 500.0) as u64,
        );
        Some(Cell::new(if roll < 0.35 { moss } else { rock }))
    };
    let (px0, px1, pz0, pz1) = POOL;
    let wall_rows = [LEDGE, LEDGE + 2.75, CEILING];

    builder.wall(
        Zone::Grotto,
        (WEST, SOUTH),
        (WEST, DOOR.0 - 1.0),
        &wall_rows,
        2.0,
        16.0,
        rocky,
    );
    builder.wall(
        Zone::Grotto,
        (WEST, DOOR.0 - 1.0),
        (WEST, DOOR.1 + 1.0),
        &[LEDGE, DOOR.2 + 0.8, CEILING],
        4.4,
        16.0,
        |middle| {
            if middle[1] < DOOR.2 + 0.8 {
                None
            } else {
                rocky(middle)
            }
        },
    );
    builder.wall(
        Zone::Grotto,
        (WEST, DOOR.1 + 1.0),
        (WEST, NORTH),
        &wall_rows,
        2.0,
        16.0,
        rocky,
    );
    builder.wall(
        Zone::Grotto,
        (EAST, NORTH),
        (EAST, SOUTH),
        &[BASIN, LEDGE, MOUTH.2, CEILING],
        2.0,
        16.0,
        |middle| {
            let pool_side = middle[2] > pz0 && middle[2] < pz1;
            let mouth = middle[2] > MOUTH.0
                && middle[2] < MOUTH.1
                && middle[1] > LEDGE
                && middle[1] < MOUTH.2;
            if mouth || (!pool_side && middle[1] < LEDGE) {
                None
            } else {
                rocky(middle)
            }
        },
    );
    builder.wall(
        Zone::Grotto,
        (EAST, SOUTH),
        (WEST, SOUTH),
        &wall_rows,
        2.0,
        16.0,
        rocky,
    );
    builder.wall(
        Zone::Grotto,
        (WEST, NORTH),
        (EAST, NORTH),
        &wall_rows,
        2.0,
        16.0,
        rocky,
    );
    for (a, b) in [
        ((CLIFF_X, MOUTH.0), (EAST, MOUTH.0)),
        ((EAST, MOUTH.1), (CLIFF_X, MOUTH.1)),
    ] {
        builder.wall(Zone::Grotto, a, b, &[LEDGE, MOUTH.2], 1.0, 16.0, |_| {
            Some(Cell::new(rock))
        });
    }
    builder.floor(
        Zone::Grotto,
        &[EAST, CLIFF_X],
        &[MOUTH.0, MOUTH.1],
        |_, _| MOUTH.2,
        false,
        16.0,
        |_| Some(Cell::new(rock)),
    );

    let mut xs = steps(WEST, EAST - 0.4, 2.0);
    xs.push(EAST);
    let zs = steps(SOUTH, NORTH, 2.0);
    let crack = |middle: Vec3| {
        CRACKS
            .iter()
            .any(|&(x, z)| (middle[0] - x).abs() < 1.0 && (middle[2] - z).abs() < 1.0)
    };
    builder.floor(
        Zone::Grotto,
        &xs,
        &zs,
        |_, _| CEILING,
        false,
        16.0,
        |middle| {
            if crack(middle) {
                None
            } else {
                rocky(middle)
            }
        },
    );
    for &(x, z) in &CRACKS {
        for (a, b) in [
            ((x - 1.0, z - 1.0), (x - 1.0, z + 1.0)),
            ((x + 1.0, z + 1.0), (x + 1.0, z - 1.0)),
            ((x + 1.0, z - 1.0), (x - 1.0, z - 1.0)),
            ((x - 1.0, z + 1.0), (x + 1.0, z + 1.0)),
        ] {
            builder.wall(
                Zone::Grotto,
                a,
                b,
                &[CEILING, CEILING + 1.2],
                2.0,
                16.0,
                |_| Some(Cell::new(moss)),
            );
        }
    }

    let in_pool =
        |middle: Vec3| middle[0] > px0 && middle[0] < px1 && middle[2] > pz0 && middle[2] < pz1;
    let ledge_xs = {
        let mut list = steps(WEST, px0, 1.5);
        list.extend(steps(px0, EAST - 0.4, 2.0).into_iter().skip(1));
        list.push(EAST);
        list
    };
    let ledge_zs = {
        let mut list = steps(SOUTH, pz0, 2.0);
        list.extend(steps(pz0, pz1, 2.0).into_iter().skip(1));
        list.extend(steps(pz1, NORTH, 2.0).into_iter().skip(1));
        list
    };
    builder.floor(
        Zone::Grotto,
        &ledge_xs,
        &ledge_zs,
        |_, _| LEDGE,
        true,
        16.0,
        |middle| (!in_pool(middle)).then(|| Cell::new(wet)),
    );
    builder.floor(
        Zone::Grotto,
        &ledge_xs,
        &ledge_zs,
        |_, _| BASIN,
        true,
        16.0,
        |middle| in_pool(middle).then(|| Cell::new(wet.tinted([0.8, 0.9, 0.85]))),
    );
    builder.wall(
        Zone::Grotto,
        (px0, pz0),
        (px0, pz1),
        &[BASIN, LEDGE],
        2.0,
        16.0,
        |middle| {
            let stairs = (middle[2] - FALLS_Z).abs() < 1.5;
            (!stairs).then(|| Cell::new(stone))
        },
    );
    builder.wall(
        Zone::Grotto,
        (px1, pz0),
        (px0, pz0),
        &[BASIN, LEDGE],
        2.0,
        16.0,
        |_| Some(Cell::new(stone)),
    );
    builder.wall(
        Zone::Grotto,
        (px0, pz1),
        (px1, pz1),
        &[BASIN, LEDGE],
        2.0,
        16.0,
        |_| Some(Cell::new(stone)),
    );

    let tread = 0.62;
    let rise = (LEDGE - BASIN) / 4.0;
    for step in 0..4 {
        let x = px0 + step as f32 * tread;
        let top = LEDGE - (step + 1) as f32 * rise;
        let previous = LEDGE - step as f32 * rise;
        builder.floor(
            Zone::Grotto,
            &[x, x + tread],
            &[FALLS_Z - 1.5, FALLS_Z, FALLS_Z + 1.5],
            |_, _| top,
            true,
            24.0,
            |_| Some(Cell::new(paving)),
        );
        builder.wall(
            Zone::Grotto,
            (x, FALLS_Z - 1.5),
            (x, FALLS_Z + 1.5),
            &[top, previous],
            3.0,
            24.0,
            |_| Some(Cell::new(paving)),
        );
        for (a, b) in [
            ((x, FALLS_Z - 1.5), (x + tread, FALLS_Z - 1.5)),
            ((x + tread, FALLS_Z + 1.5), (x, FALLS_Z + 1.5)),
        ] {
            builder.wall(Zone::Grotto, a, b, &[BASIN, top], 1.0, 24.0, |_| {
                Some(Cell::new(stone))
            });
        }
    }

    for (cx, cz, half) in [
        (IDOL[0], IDOL[2], 2.8),
        (CLIFF_X - 10.5, FALLS_Z - 3.0, 1.6),
        (CLIFF_X - 4.0, FALLS_Z - 3.4, 1.4),
    ] {
        builder.floor(
            Zone::Grotto,
            &[cx - half, cx, cx + half],
            &[cz - half, cz, cz + half],
            |_, _| BASIN + 0.05,
            true,
            16.0,
            |_| Some(Cell::new(gold)),
        );
    }
    builder.block(
        Zone::Grotto,
        [IDOL[0] - 0.5, BASIN, IDOL[2] - 0.5],
        [IDOL[0] + 0.5, BASIN + 1.4, IDOL[2] + 0.5],
        24.0,
        stone,
    );

    let carved = |front: usize, back: usize| Faces {
        stone,
        front: Some(art.carvings[front]),
        back: Some(art.carvings[back]),
    };
    for (centre, half, yaw, tilt, faces) in [
        (
            [CLIFF_X - 9.5, BASIN + 3.0, FALLS_Z - 3.6],
            [0.5, 3.0, 0.5],
            0.3,
            0.0,
            carved(0, 1),
        ),
        (
            [CLIFF_X - 3.5, BASIN + 1.5, FALLS_Z + 3.4],
            [0.5, 1.5, 0.5],
            -0.4,
            0.06,
            carved(2, 3),
        ),
        (
            [CLIFF_X - 5.0, BASIN + 0.5, FALLS_Z + 2.0],
            [1.4, 0.45, 0.45],
            1.1,
            0.25,
            carved(1, 2),
        ),
        (
            [CLIFF_X - 11.5, BASIN + 0.4, FALLS_Z + 3.2],
            [0.5, 0.5, 1.4],
            0.5,
            -0.3,
            carved(3, 0),
        ),
        (
            [CLIFF_X - 6.2, BASIN + 0.35, FALLS_Z - 3.8],
            [0.6, 0.35, 0.6],
            2.2,
            0.4,
            carved(0, 2),
        ),
    ] {
        slab(builder, Zone::Grotto, centre, half, yaw, tilt, faces);
    }

    let relief = WEST + 0.06;
    builder.wall(
        Zone::Grotto,
        (relief, FALLS_Z - 2.5),
        (relief, FALLS_Z + 1.5),
        &[LEDGE + 0.6, LEDGE + 4.6],
        4.0,
        16.0,
        |_| Some(Cell::panel(&art.carvings[0], [0.95, 1.0, 0.95]).biased(-16)),
    );
    for z in [FALLS_Z - 3.8, FALLS_Z + 1.8] {
        builder.wall(
            Zone::Grotto,
            (relief, z),
            (relief, z + 1.1),
            &[LEDGE, LEDGE + 4.0],
            2.0,
            16.0,
            |_| Some(Cell::panel(&art.serpent, [0.9; 3]).biased(-16)),
        );
    }
    for (z0, z1) in [(DOOR.0 - 0.9, DOOR.0), (DOOR.1, DOOR.1 + 0.9)] {
        builder.wall(
            Zone::Grotto,
            (relief, z0),
            (relief, z1),
            &[LEDGE, DOOR.2],
            1.0,
            16.0,
            |_| Some(Cell::panel(&art.serpent, [0.95; 3]).biased(-16)),
        );
    }
    builder.wall(
        Zone::Grotto,
        (relief, DOOR.0 - 0.9),
        (relief, DOOR.1 + 0.9),
        &[DOOR.2, DOOR.2 + 0.8],
        4.2,
        16.0,
        |_| Some(Cell::panel(&art.carvings[3], [0.95; 3]).biased(-16)),
    );
    passage(builder, stone, paving);
}

fn passage(builder: &mut Builder, stone: Material, paving: Material) {
    let (z0, z1) = (DOOR.0, DOOR.1);
    let plain = |material: Material| move |_: Vec3| Some(Cell::new(material));
    builder.wall(
        Zone::Grotto,
        (WEST, z0),
        (WEST - 0.6, z0),
        &[LEDGE, DOOR.2],
        1.0,
        24.0,
        plain(stone),
    );
    builder.wall(
        Zone::Grotto,
        (WEST - 0.6, z1),
        (WEST, z1),
        &[LEDGE, DOOR.2],
        1.0,
        24.0,
        plain(stone),
    );
    builder.floor(
        Zone::Grotto,
        &[WEST - 0.6, WEST],
        &[z0, z1],
        |_, _| DOOR.2,
        false,
        24.0,
        plain(stone),
    );
    builder.floor(
        Zone::Grotto,
        &[WEST - 0.6, WEST],
        &[z0, z1],
        |_, _| LEDGE,
        true,
        24.0,
        plain(paving),
    );
    builder.wall(
        Zone::Grotto,
        (WEST - 0.6, z0),
        (WEST - 0.6, z1),
        &[
            LEDGE + PASSAGE_HEIGHT,
            DOOR.2.max(LEDGE + PASSAGE_HEIGHT + 0.01),
        ],
        2.4,
        24.0,
        plain(stone),
    );
    let run = (WEST - 0.6 - PASSAGE_TURN) / PASSAGE_STEPS as f32;
    for step in 0..PASSAGE_STEPS {
        let east = WEST - 0.6 - step as f32 * run;
        let west = east - run;
        let floor = passage_floor(east - run * 0.5);
        let above = if step == 0 {
            LEDGE
        } else {
            passage_floor(east + run * 0.5)
        };
        let roof = floor + PASSAGE_HEIGHT;
        let roof_above = above + PASSAGE_HEIGHT;
        builder.floor(
            Zone::Grotto,
            &[west, east],
            &[z0, z1],
            |_, _| floor,
            true,
            24.0,
            plain(paving),
        );
        builder.floor(
            Zone::Grotto,
            &[west, east],
            &[z0, z1],
            |_, _| roof,
            false,
            24.0,
            plain(stone),
        );
        builder.wall(
            Zone::Grotto,
            (east, z1),
            (east, z0),
            &[floor, above],
            2.4,
            24.0,
            plain(paving),
        );
        if roof_above > roof + 0.01 {
            builder.wall(
                Zone::Grotto,
                (east, z0),
                (east, z1),
                &[roof, roof_above],
                2.4,
                24.0,
                plain(stone),
            );
        }
        builder.wall(
            Zone::Grotto,
            (west, z1),
            (east, z1),
            &[floor, roof],
            2.0,
            24.0,
            plain(stone),
        );
        builder.wall(
            Zone::Grotto,
            (east, z0),
            (west, z0),
            &[floor, roof],
            2.0,
            24.0,
            plain(stone),
        );
    }
    let low = secrets::PIT_FLOOR;
    let roof = low + PASSAGE_HEIGHT;
    let west = PASSAGE_TURN - 3.0;
    builder.wall(
        Zone::Grotto,
        (PASSAGE_TURN, z0),
        (west, z0),
        &[low, roof],
        2.4,
        24.0,
        plain(stone),
    );
    builder.wall(
        Zone::Grotto,
        (west, z0),
        (west, PIT_DOOR_Z),
        &[low, roof],
        2.0,
        24.0,
        plain(stone),
    );
    builder.wall(
        Zone::Grotto,
        (PASSAGE_TURN, PIT_DOOR_Z),
        (PASSAGE_TURN, z1),
        &[low, roof],
        2.0,
        24.0,
        plain(stone),
    );
    let zs = steps(z0, PIT_DOOR_Z, 2.0);
    builder.floor(
        Zone::Grotto,
        &[west, PASSAGE_TURN],
        &zs,
        |_, _| low,
        true,
        24.0,
        plain(paving),
    );
    builder.floor(
        Zone::Grotto,
        &[west, PASSAGE_TURN],
        &zs,
        |_, _| roof,
        false,
        24.0,
        plain(stone),
    );
    let z = PIT_DOOR_Z;
    for (min, max) in [
        ([west - 0.5, low, z - 0.3], [west, roof, z + 0.4]),
        (
            [PASSAGE_TURN, low, z - 0.3],
            [PASSAGE_TURN + 0.5, roof, z + 0.4],
        ),
        (
            [west - 0.5, roof, z - 0.3],
            [PASSAGE_TURN + 0.5, roof + 0.6, z + 0.4],
        ),
    ] {
        builder.block(Zone::Exterior, min, max, 24.0, stone);
    }
}

pub fn lights() -> Vec<Light> {
    let door_z = (DOOR.0 + DOOR.1) * 0.5;
    vec![
        Light {
            position: [WEST - 4.5, passage_floor(WEST - 4.5) + 2.0, DOOR.0 + 0.15],
            zone: Zone::Grotto,
            kind: LightKind::Torch,
        },
        Light {
            position: [PASSAGE_TURN - 0.15, secrets::PIT_FLOOR + 2.0, door_z + 3.0],
            zone: Zone::Grotto,
            kind: LightKind::Torch,
        },
    ]
}

pub fn shafts() -> Vec<Shaft> {
    CRACKS
        .iter()
        .map(|&(x, z)| Shaft {
            top: [x, CEILING + 1.0, z],
            bottom: [x + 0.6, WATER, z + 0.3],
            top_width: 0.8,
            bottom_width: 1.4,
            top_color: [56, 58, 44],
            bottom_color: [16, 17, 12],
        })
        .collect()
}

pub fn props(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    let jamb = WEST + 0.25;
    for (index, z) in [DOOR.0 + 0.3, (DOOR.0 + DOOR.1) * 0.5, DOOR.1 - 0.3]
        .into_iter()
        .enumerate()
    {
        let length = 1.1 + (index % 2) as f32 * 0.4;
        let anchor = [jamb, DOOR.2 + 0.1, z];
        let lit = lighting.billboard(
            [anchor[0], anchor[1] - length * 0.5, anchor[2]],
            Zone::Grotto,
        );
        list.push(Billboard {
            image: art.vines[index % art.vines.len()],
            position: anchor,
            width: 0.9,
            height: length,
            color: [0, 1, 2].map(|c| (lit[c] as f32 * 1.5).min(255.0) as u8),
            hanging: true,
        });
    }
    for (index, &(x, z)) in CRACKS.iter().enumerate() {
        for side in 0..2 {
            let anchor = [
                x - 0.9 + side as f32 * 1.8,
                CEILING + 0.1,
                z + 0.9 - side as f32 * 1.8,
            ];
            list.push(Billboard {
                image: art.vines[(index + side) % art.vines.len()],
                position: anchor,
                width: 0.9,
                height: 1.4,
                color: [120, 140, 110],
                hanging: true,
            });
        }
    }
    for (index, (dx, dz, size)) in [(-12.5, 7.4, 1.8), (-2.4, -7.3, 1.6), (-14.6, -7.0, 1.4)]
        .into_iter()
        .enumerate()
    {
        let position = [CLIFF_X + dx, LEDGE, FALLS_Z + dz];
        let lit = lighting.billboard([position[0], position[1] + 0.6, position[2]], Zone::Grotto);
        list.push(Billboard {
            image: art.treasure[(index + 1) % art.treasure.len()],
            position,
            width: size,
            height: size * 0.75,
            color: [0, 1, 2].map(|c| (lit[c] as f32 * [1.5, 1.3, 0.9][c]).min(255.0) as u8),
            hanging: false,
        });
    }
    for (index, (dx, dz, size)) in [(-8.2, -0.6, 1.8), (-5.6, 2.8, 1.5)]
        .into_iter()
        .enumerate()
    {
        list.push(Billboard {
            image: art.treasure[(index + 2) % art.treasure.len()],
            position: [CLIFF_X + dx, BASIN + 0.05, FALLS_Z + dz],
            width: size,
            height: size * 0.75,
            color: [150, 135, 100],
            hanging: false,
        });
    }
}
