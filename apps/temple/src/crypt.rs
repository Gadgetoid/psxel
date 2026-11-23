use crate::art::Art;
use crate::lighting::{Light, LightKind, Lighting};
use crate::mesh::{steps, Builder, Cell, Material, Vec3, Zone};
use crate::path::{key, Key};
use crate::scene::Billboard;
use crate::terrain::TEMPLE_Z;

pub const STAIR: (f32, f32) = (TEMPLE_Z + 2.0, TEMPLE_Z + 6.0);
pub const STAIR_HALF: f32 = 2.0;
const FLOOR: f32 = -4.0;
const CEILING: f32 = -0.5;
const ROOM_X: f32 = 4.5;
const ROOM_Z: (f32, f32) = (TEMPLE_Z + 6.0, TEMPLE_Z + 15.0);
const TOMB: (f32, f32) = (0.0, TEMPLE_Z + 10.5);
const WATER: f32 = FLOOR + 0.18;

pub fn in_stairwell(x: f32, z: f32) -> bool {
    x.abs() < STAIR_HALF && z > STAIR.0 && z < STAIR.1
}

pub fn amount(position: Vec3) -> f32 {
    let [x, y, z] = position;
    if y < -0.2 && x.abs() < ROOM_X + 0.5 && z > STAIR.0 && z < ROOM_Z.1 + 0.5 {
        1.0
    } else {
        0.0
    }
}

pub fn keys() -> Vec<Key> {
    let tomb = [TOMB.0, FLOOR + 1.0, TOMB.1];
    vec![
        key([3.0, 2.4, TEMPLE_Z - 1.5], [0.0, -2.0, STAIR.1], 1.4),
        key([1.2, 1.5, STAIR.0 + 0.6], [0.0, -3.0, STAIR.1 + 2.0], 1.2),
        key([0.0, -1.5, STAIR.1 - 0.6], tomb, 1.2),
        key([-2.8, -2.3, ROOM_Z.0 + 2.6], tomb, 1.1),
        key([-3.0, -2.2, ROOM_Z.0 + 6.6], tomb, 1.1),
        key([0.0, -2.0, ROOM_Z.1 - 1.4], tomb, 1.1),
        key([3.0, -2.2, ROOM_Z.0 + 6.6], tomb, 1.1),
        key([2.8, -2.3, ROOM_Z.0 + 2.6], [0.0, -1.0, STAIR.0], 1.2),
        key([0.6, -1.7, STAIR.1 - 0.4], [0.0, 2.0, STAIR.0 - 2.0], 1.3),
        key([0.0, 1.5, STAIR.0 + 0.8], [8.0, 2.0, TEMPLE_Z], 1.5),
    ]
}

pub fn lights() -> Vec<Light> {
    [(-ROOM_X + 0.4, TOMB.1 - 2.5), (ROOM_X - 0.4, TOMB.1 + 2.5)]
        .iter()
        .map(|&(x, z)| Light {
            position: [x, FLOOR + 2.4, z],
            zone: Zone::Interior,
            kind: LightKind::Torch,
        })
        .collect()
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    let stone = Material::textured(&art.stone[2]).tinted([0.85, 0.9, 0.85]);
    let paving = Material::textured(&art.paving).tinted([0.8, 0.85, 0.8]);
    let plain = |_: Vec3| Some(Cell::new(stone));
    let steps_count = 8;
    let run = (STAIR.1 - STAIR.0) / steps_count as f32;
    let rise = FLOOR / steps_count as f32;
    for step in 0..steps_count {
        let z = STAIR.0 + step as f32 * run;
        let (top, bottom) = (step as f32 * rise, (step + 1) as f32 * rise);
        builder.floor(
            Zone::Interior,
            &[-STAIR_HALF, 0.0, STAIR_HALF],
            &[z, z + run],
            |_, _| bottom,
            true,
            24.0,
            |_| Some(Cell::new(paving)),
        );
        builder.wall(
            Zone::Interior,
            (STAIR_HALF, z),
            (-STAIR_HALF, z),
            &[bottom, top],
            2.0,
            24.0,
            |_| Some(Cell::new(paving)),
        );
    }
    builder.wall(
        Zone::Interior,
        (-STAIR_HALF, STAIR.0),
        (-STAIR_HALF, STAIR.1),
        &[FLOOR, -2.0, 0.0],
        2.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (STAIR_HALF, STAIR.1),
        (STAIR_HALF, STAIR.0),
        &[FLOOR, -2.0, 0.0],
        2.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (STAIR_HALF, STAIR.0),
        (-STAIR_HALF, STAIR.0),
        &[FLOOR, 0.0],
        4.0,
        24.0,
        plain,
    );

    let (z0, z1) = ROOM_Z;
    let rows = [FLOOR, -2.0, CEILING];
    builder.wall(
        Zone::Interior,
        (ROOM_X, z0),
        (-ROOM_X, z0),
        &rows,
        1.5,
        24.0,
        |middle| (middle[0].abs() > STAIR_HALF).then(|| Cell::new(stone)),
    );
    builder.wall(
        Zone::Interior,
        (-STAIR_HALF, z0),
        (STAIR_HALF, z0),
        &[CEILING, 0.0],
        4.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (-ROOM_X, z1),
        (ROOM_X, z1),
        &rows,
        3.0,
        24.0,
        |middle| {
            Some(if middle[1] > -2.0 && middle[0].abs() < 1.6 {
                Cell::panel(&art.carvings[0], [0.9; 3])
            } else {
                Cell::new(stone)
            })
        },
    );
    for (a, b) in [((-ROOM_X, z0), (-ROOM_X, z1)), ((ROOM_X, z1), (ROOM_X, z0))] {
        builder.wall(Zone::Interior, a, b, &rows, 1.5, 24.0, |middle| {
            let niche = middle[1] > -2.0 && ((middle[2] - TOMB.1).abs() as i32) % 3 == 1;
            Some(if niche {
                Cell::panel(&art.serpent, [0.85; 3])
            } else {
                Cell::new(stone)
            })
        });
    }
    let xs = steps(-ROOM_X, ROOM_X, 1.5);
    let zs = steps(z0, z1, 1.5);
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |_, _| FLOOR,
        true,
        24.0,
        |_| Some(Cell::new(paving)),
    );
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |_, _| CEILING,
        false,
        24.0,
        |_| Some(Cell::new(stone)),
    );
    for z in [z0 + 3.0, z0 + 6.0] {
        builder.block(
            Zone::Interior,
            [-ROOM_X, CEILING - 0.35, z - 0.25],
            [ROOM_X, CEILING, z + 0.25],
            24.0,
            stone,
        );
    }
    let water = {
        let mut material = Material::textured(&art.procedural.water);
        material.surface = material.surface.blended(psxel_prim::BlendMode::Half);
        material.tinted([0.7, 0.85, 0.8]).biased(-12)
    };
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |_, _| WATER,
        true,
        16.0,
        |middle| {
            let dais = (middle[0] - TOMB.0).abs() < 1.8 && (middle[2] - TOMB.1).abs() < 2.4;
            (!dais).then(|| Cell::new(water))
        },
    );
    builder.block(
        Zone::Interior,
        [TOMB.0 - 1.6, FLOOR, TOMB.1 - 2.2],
        [TOMB.0 + 1.6, FLOOR + 0.35, TOMB.1 + 2.2],
        24.0,
        stone,
    );
    let lid = FLOOR + 1.35;
    let plain = Cell::new(stone);
    builder.dressed_block(
        Zone::Interior,
        [TOMB.0 - 0.75, FLOOR + 0.35, TOMB.1 - 1.3],
        [TOMB.0 + 0.75, lid, TOMB.1 + 1.3],
        24.0,
        [
            plain,
            Cell::panel(&art.carvings[2], [0.9; 3]),
            plain,
            Cell::panel(&art.carvings[3], [0.9; 3]),
        ],
        Cell::panel(&art.carvings[0], [0.95, 1.0, 0.9]),
    );
    for light in lights() {
        let [x, y, z] = light.position;
        builder.block(
            Zone::Interior,
            [x - 0.07, y - 0.6, z - 0.07],
            [x + 0.07, y, z + 0.07],
            24.0,
            Material::flat([70, 44, 22]),
        );
    }
}

pub fn treasure(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    for (index, (dx, dz, size)) in [(-1.3, -2.8, 1.6), (1.3, 2.8, 1.7), (1.4, -2.6, 1.3)]
        .into_iter()
        .enumerate()
    {
        let position = [TOMB.0 + dx, FLOOR + 0.35, TOMB.1 + dz];
        let lit = lighting.billboard(
            [position[0], position[1] + 0.6, position[2]],
            Zone::Interior,
        );
        list.push(Billboard {
            image: art.treasure[(index + 2) % art.treasure.len()],
            position,
            width: size,
            height: size * 0.75,
            color: lit,
            hanging: false,
        });
    }
}
