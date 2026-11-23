use std::f32::consts::TAU;

use crate::art::Art;
use crate::biome::Rgb;
use crate::cave;
use crate::crypt;
use crate::grotto;
use crate::lighting::{sun_direction, Light, LightKind, Lighting};
use crate::mesh::{
    add, cross, length, normalize, scale, steps, sub, Builder, Cell, Material, Mesh, Vec3, Zone,
};
use crate::path::{key, Path};
use crate::rng::{hash_float, Rng};
use crate::ruins;
use crate::secrets;
use crate::terrain::{
    cliff_x, ground_height, plateau_height, pool_distance, smoothstep, value_noise, CLIFF_TOP,
    CLIFF_X, FALLS_Z, TEMPLE_Z, WATER_Y,
};
use crate::textures::Image;

pub const TIER_Y: [f32; 6] = [0.0, 4.0, 6.5, 9.0, 11.5, 14.0];
pub const ALTAR_TOP: f32 = 1.1;
const CANOPY_CELL: f32 = 6.0;
const CANOPY_HEIGHT: f32 = 20.0;
const TREELINE_ROWS: usize = 4;
const TIER_MOTIFS: [usize; 5] = [0, 0, 2, 1, 3];
const BRAZIER_X: f32 = 2.5;
const BRAZIER_Z: f32 = TEMPLE_Z + 2.5;
pub const SHRINE_ROOF: f32 = TIER_Y[5] + 5.0;
const GIANT_TREES: [(f32, f32); 6] = [
    (-24.0, 44.0),
    (26.0, 22.0),
    (22.0, 58.0),
    (-20.0, -6.0),
    (8.0, -14.0),
    (-12.0, 66.0),
];

pub struct Billboard {
    pub image: Image,
    pub position: Vec3,
    pub width: f32,
    pub height: f32,
    pub color: Rgb,
    pub hanging: bool,
}

pub struct Shaft {
    pub top: Vec3,
    pub bottom: Vec3,
    pub top_width: f32,
    pub bottom_width: f32,
    pub top_color: Rgb,
    pub bottom_color: Rgb,
}

pub struct Scene {
    pub mesh: Mesh,
    pub billboards: Vec<Billboard>,
    pub lighting: Lighting,
    pub pool: Vec<(f32, f32)>,
    pub shafts: Vec<Shaft>,
}

pub fn tour() -> Path {
    let mut keys = vec![
        key([30.0, 42.0, -20.0], [0.0, 6.0, 40.0], 5.6),
        key([16.0, 18.0, -4.0], [0.0, 8.0, 40.0], 4.8),
        key([6.0, 6.5, 8.0], [0.0, 9.0, 34.0], 4.0),
        key([-10.0, 4.5, 10.0], [-30.0, 5.0, 14.0], 3.6),
        key([-21.0, 2.2, 9.0], [-38.0, 8.0, 14.0], 3.2),
        key([-28.0, 1.6, 15.0], [-38.0, 12.0, 14.0], 2.4),
    ];
    keys.extend(cave::keys());
    keys.extend(grotto::keys());
    keys.extend([
        key([-27.0, 5.5, 28.0], [-14.0, 2.0, 40.0], 3.2),
        key([-24.0, 2.2, 40.0], [-10.0, 2.0, 40.0], 2.4),
        key([-14.0, 2.0, 40.0], [0.0, 1.8, 40.0], 2.0),
        key([-8.0, 2.0, 40.0], [0.0, 1.5, 40.0], 1.6),
        key([-3.2, 2.4, 37.8], [0.0, 1.4, 40.0], 1.28),
        key([0.0, 3.0, 36.6], [0.0, 1.3, 40.0], 1.28),
    ]);
    keys.extend(crypt::keys());
    keys.extend([
        key([8.0, 2.0, 40.0], [20.0, 2.0, 40.0], 1.76),
        key([14.0, 2.0, 40.0], [26.0, 3.0, 40.0], 2.24),
        key([22.0, 3.2, 40.0], [30.0, 6.0, 36.0], 2.8),
        key([30.0, 12.0, 30.0], [0.0, 10.0, 40.0], 4.0),
        key([12.0, 17.5, 26.0], [0.0, 16.0, 40.0], 3.2),
        key([0.0, 16.0, 30.5], [0.0, 15.4, 42.0], 1.4),
        key([6.0, 24.0, 28.0], [0.0, 14.0, 40.0], 3.0),
        key([40.0, 36.0, 8.0], [0.0, 8.0, 40.0], 5.2),
    ]);
    Path::new(keys)
}

pub fn insideness(position: Vec3) -> f32 {
    temple_amount(position)
        .max(cave::amount(position))
        .max(grotto::amount(position))
        .max(crypt::amount(position))
}

pub fn temple_amount(position: Vec3) -> f32 {
    let within =
        position[0].abs() < 15.0 && (position[2] - TEMPLE_Z).abs() < 6.5 && position[1] < 7.5;
    if within {
        smoothstep(-1.0, 5.0, 14.0 - position[0].abs())
    } else {
        0.0
    }
}

pub fn build(art: &Art, path: &Path) -> Scene {
    let mut lights = torches();
    lights.extend(cave::lights());
    lights.extend(grotto::lights());
    lights.extend(crypt::lights());
    let lighting = Lighting { lights };
    let samples = path.points(48);
    let mesh = {
        let mut builder = Builder::new(&lighting);
        ground(&mut builder, art);
        cliff(&mut builder, art);
        pyramid(&mut builder, art);
        stairs(&mut builder, art);
        shrine(&mut builder, art);
        interior(&mut builder, art);
        brackets(&mut builder, &lighting);
        giant_trees(&mut builder, art, &samples);
        secrets::geometry(&mut builder, art);
        ruins::geometry(&mut builder, art, &samples);
        canopy(&mut builder, art, &samples);
        cave::geometry(&mut builder, art);
        grotto::geometry(&mut builder, art);
        crypt::geometry(&mut builder, art);
        builder.build()
    };
    let mut billboards = props(art, &samples, &lighting);
    cave::crystals(art, &mut billboards);
    secrets::props(art, &mut billboards);
    ruins::props(art, &mut billboards);
    secrets::treasure(art, &lighting, &mut billboards);
    cave::treasure(art, &lighting, &mut billboards);
    crypt::treasure(art, &lighting, &mut billboards);
    grotto::props(art, &lighting, &mut billboards);
    crowns(art, &samples, &lighting, &mut billboards);
    let mut shafts = shafts();
    shafts.push(secrets::shaft());
    shafts.extend(grotto::shafts());
    Scene {
        mesh,
        billboards,
        pool: pool_cells(),
        shafts,
        lighting,
    }
}

fn pick<T: Copy>(list: &[T], position: Vec3, seed: u64) -> T {
    let hash = hash_float(
        seed,
        (position[0] * 2.0 + 1000.0) as u64,
        (position[2] * 2.0 + position[1] * 7.0 + 1000.0) as u64,
    );
    list[((hash * list.len() as f32) as usize).min(list.len() - 1)]
}

fn moss(position: Vec3, amount: f32) -> Vec3 {
    let noise = value_noise(
        position[0] / 2.5 + position[2] / 5.0,
        position[1] / 2.0 + position[2] / 3.0,
        41,
    );
    let mix = (noise * amount).clamp(0.0, 1.0);
    [1.0 - 0.3 * mix, 1.0 - 0.05 * mix, 1.0 - 0.45 * mix]
}

fn tier_half(tier: usize) -> f32 {
    14.0 - 2.0 * tier as f32
}

fn torches() -> Vec<Light> {
    let mut list = Vec::new();
    for side in [-1.0f32, 1.0] {
        for z in [37.0, 43.0] {
            list.push(Light {
                position: [side * 14.6, 2.6, z],
                zone: Zone::Exterior,
                kind: LightKind::Torch,
            });
        }
        list.push(Light {
            position: [side * 10.0, 2.4, TEMPLE_Z - side * 1.65],
            zone: Zone::Interior,
            kind: LightKind::Torch,
        });
        for z in [34.4, 45.6] {
            list.push(Light {
                position: [side * 5.6, 3.0, z],
                zone: Zone::Interior,
                kind: LightKind::Torch,
            });
        }
    }
    for side in [-1.0f32, 1.0] {
        list.push(Light {
            position: [side * BRAZIER_X, TIER_Y[5] + 1.4, BRAZIER_Z],
            zone: Zone::Interior,
            kind: LightKind::Torch,
        });
    }
    list
}

fn shafts() -> Vec<Shaft> {
    let sun = sun_direction();
    let ray = |bottom: Vec3, width: f32| {
        let top = add(bottom, scale(sun, 30.0));
        Shaft {
            top,
            bottom,
            top_width: width * 0.7,
            bottom_width: width,
            top_color: [30, 30, 20],
            bottom_color: [5, 5, 3],
        }
    };
    vec![
        Shaft {
            top: [0.0, SHRINE_ROOF - 0.2, TEMPLE_Z],
            bottom: [0.0, TIER_Y[5], TEMPLE_Z],
            top_width: 0.8,
            bottom_width: 0.95,
            top_color: [64, 60, 44],
            bottom_color: [40, 38, 28],
        },
        Shaft {
            top: [0.0, 7.0, TEMPLE_Z],
            bottom: [0.0, 0.0, TEMPLE_Z],
            top_width: 0.95,
            bottom_width: 1.35,
            top_color: [72, 68, 50],
            bottom_color: [24, 22, 15],
        },
        ray([-4.0, 0.0, 18.0], 3.2),
        ray([17.0, 0.0, 12.0], 2.6),
        ray([-25.0, WATER_Y, 19.0], 3.0),
        ray([20.0, 0.0, 46.0], 2.8),
    ]
}

fn open_ground(x: f32, z: f32, margin: f32) -> bool {
    let temple = x.abs().max((z - TEMPLE_Z).abs());
    let stairs = x.abs() < 4.0 + margin && z > 16.0 - margin && z < 27.0;
    pool_distance(x, z) > 1.0 + margin * 0.06
        && temple > 14.5 + margin
        && !stairs
        && x > CLIFF_X + 1.0 + margin * 0.3
}

fn ground(builder: &mut Builder, art: &Art) {
    let riverbed = Material::textured(&art.riverbed);
    let swamp = Material::textured(&art.swamp);
    let floors: Vec<Material> = art
        .floor
        .iter()
        .map(|image| Material::textured(image).tinted([0.75, 0.8, 0.72]))
        .collect();
    let surface = |middle: Vec3| {
        if middle[1] < WATER_Y + 0.25 && pool_distance(middle[0], middle[2]) < 1.3 {
            riverbed
        } else if pool_distance(middle[0], middle[2]) < 1.5 {
            swamp
        } else {
            pick(&floors, middle, 3)
        }
    };
    builder.floor(
        Zone::Exterior,
        &steps(-42.0, 102.0, 4.0),
        &steps(-70.0, 110.0, 4.0),
        ground_height,
        true,
        16.0,
        |middle| {
            let under_temple = middle[0].abs() < 14.0 && (middle[2] - TEMPLE_Z).abs() < 14.0;
            (!under_temple).then(|| Cell::new(surface(middle)))
        },
    );
    let mut xs = steps(-102.0, -42.0, 4.0);
    xs.push(CLIFF_X);
    builder.floor(
        Zone::Exterior,
        &xs,
        &steps(-70.0, 110.0, 4.0),
        plateau_height,
        true,
        16.0,
        |middle| {
            let hidden = secrets::near_ruin(middle[0], middle[2], 0.5);
            (!hidden).then(|| Cell::new(surface(middle)))
        },
    );
    let outer = |x: f32, z: f32| floor_height(x, z) - 0.6;
    for (xs, zs) in [
        (steps(102.0, 150.0, 12.0), steps(-70.0, 110.0, 12.0)),
        (steps(-150.0, -102.0, 12.0), steps(-70.0, 110.0, 12.0)),
        (steps(-150.0, 150.0, 12.0), steps(-130.0, -70.0, 12.0)),
        (steps(-150.0, 150.0, 12.0), steps(110.0, 158.0, 12.0)),
    ] {
        builder.floor(Zone::Exterior, &xs, &zs, outer, true, 12.0, |middle| {
            Some(Cell::new(pick(&floors, middle, 3)))
        });
    }
}

fn cliff(builder: &mut Builder, art: &Art) {
    let rocks: Vec<Material> = art
        .cliff
        .iter()
        .map(|image| Material::textured(image).tinted([1.1, 1.1, 1.05]))
        .collect();
    for zw in steps(-70.0, 110.0, 2.0).windows(2) {
        let (z0, z1) = (zw[0], zw[1]);
        let top = |z: f32| plateau_height(CLIFF_X, z);
        let tall = top(z0).max(top(z1)) > CLIFF_TOP + 3.5;
        let mut rows = vec![-3.0, 0.0, 3.0, 6.0, 9.0, 12.0, 14.5, CLIFF_TOP];
        if tall {
            rows.push(CLIFF_TOP + 3.0);
        }
        rows.push(f32::MAX);
        for row in rows.windows(2) {
            let (y0, y1) = (row[0], row[1]);
            let level = |z: f32, y: f32| if y == f32::MAX { top(z) } else { y };
            if y1 == f32::MAX && top(z0) <= CLIFF_TOP + 0.05 && top(z1) <= CLIFF_TOP + 0.05 {
                continue;
            }
            let lip = if y1 == f32::MAX {
                top(z0).max(top(z1))
            } else {
                y1
            };
            if cave::in_mouth(z0, z1, y0, lip) || grotto::in_mouth(z0, z1, y0, lip) {
                continue;
            }
            let y1 = lip;
            let at = |z: f32, y: f32| {
                let height = if y == lip && row[1] == f32::MAX {
                    level(z, f32::MAX)
                } else {
                    y
                };
                [cliff_x(z, height), height, z]
            };
            let u0 = (z0 * 16.0).rem_euclid(64.0);
            let v0 = (-y1 * 16.0).rem_euclid(64.0);
            let (u1, v1) = (u0 + (z1 - z0) * 16.0, v0 + (y1 - y0) * 16.0);
            let middle = [CLIFF_X, (y0 + y1) * 0.5, (z0 + z1) * 0.5];
            builder.quad(
                Zone::Exterior,
                [at(z0, y1), at(z1, y1), at(z0, y0), at(z1, y0)],
                [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
                pick(&rocks, middle, 9),
            );
        }
    }
}

fn pyramid(builder: &mut Builder, art: &Art) {
    let stones: Vec<Material> = art.stone.iter().map(Material::textured).collect();
    for tier in 0..5 {
        let half = tier_half(tier);
        let (y0, y1) = (TIER_Y[tier], TIER_Y[tier + 1]);
        let corners = [
            (-half, TEMPLE_Z - half),
            (half, TEMPLE_Z - half),
            (half, TEMPLE_Z + half),
            (-half, TEMPLE_Z + half),
        ];
        for side in 0..4 {
            builder.wall(
                Zone::Exterior,
                corners[side],
                corners[(side + 1) % 4],
                &[y0, y1],
                2.0,
                24.0,
                |middle| {
                    let door_side = tier == 0 && side % 2 == 1;
                    let along = (middle[2] - TEMPLE_Z).abs();
                    let tint = moss(middle, 1.2 - tier as f32 * 0.15);
                    if door_side && along < 2.0 {
                        return None;
                    }
                    if door_side && along < 4.0 {
                        return Some(Cell::panel(&art.serpent, tint));
                    }
                    let offset = if side % 2 == 0 {
                        middle[0]
                    } else {
                        middle[2] - TEMPLE_Z
                    }
                    .abs();
                    let rhythm = (offset.round() as i32) % 4 == 1;
                    let clear_of_stairs = side != 0 || offset > 4.0;
                    let carved = tier > 0 && rhythm && clear_of_stairs && offset < half - 1.5;
                    Some(if carved {
                        Cell::panel(&art.carvings[TIER_MOTIFS[tier]], moss(middle, 0.4))
                    } else {
                        Cell::new(pick(&stones, middle, 4).tinted(tint))
                    })
                },
            );
        }
        let inner = if tier < 4 { tier_half(tier + 1) } else { 4.0 };
        let xs = steps(-half, half, 2.0);
        let zs: Vec<f32> = xs.iter().map(|x| x + TEMPLE_Z).collect();
        builder.floor(
            Zone::Exterior,
            &xs,
            &zs,
            |_, _| y1,
            true,
            24.0,
            |middle| {
                let covered = middle[0].abs() < inner && (middle[2] - TEMPLE_Z).abs() < inner;
                (!covered).then(|| Cell::new(pick(&stones, middle, 5).tinted(moss(middle, 1.4))))
            },
        );
    }
}

fn stairs(builder: &mut Builder, art: &Art) {
    const STEPS: usize = 28;
    let paving = Material::textured(&art.paving);
    let stone = Material::textured(&art.stone[0]);
    let (start, end, rise) = (18.0, 34.0, 0.5);
    let run = (end - start) / STEPS as f32;
    for step in 0..STEPS {
        let z = start + step as f32 * run;
        let (y0, y1) = (step as f32 * rise, (step + 1) as f32 * rise);
        builder.wall(
            Zone::Exterior,
            (-3.0, z),
            (3.0, z),
            &[y0, y1],
            3.0,
            24.0,
            |middle| Some(Cell::new(paving.tinted(moss(middle, 0.7)))),
        );
        builder.floor(
            Zone::Exterior,
            &[-3.0, 0.0, 3.0],
            &[z, z + run],
            |_, _| y1,
            true,
            24.0,
            |middle| Some(Cell::new(paving.tinted(moss(middle, 0.7)))),
        );
    }
    let top = |z: f32| (((z - start) / run + 2.0) * rise).min(TIER_Y[5] + 0.5);
    for side in [-1.0f32, 1.0] {
        let x = side * 3.0;
        for zw in steps(start, end, 2.0).windows(2) {
            let (za, zb) = if side > 0.0 {
                (zw[0], zw[1])
            } else {
                (zw[1], zw[0])
            };
            let (ta, tb) = (top(za), top(zb));
            let rows = (ta.max(tb) / 2.5).ceil().max(1.0) as usize;
            let u0 = (zw[0] * 24.0).rem_euclid(64.0);
            for row in 0..rows {
                let (f0, f1) = (row as f32 / rows as f32, (row + 1) as f32 / rows as f32);
                let (a0, a1, b0, b1) = (ta * f0, ta * f1, tb * f0, tb * f1);
                let highest = a1.max(b1);
                let v_top = (-highest * 24.0).rem_euclid(64.0);
                let v = |y: f32| v_top + (highest - y) * 24.0;
                let middle = [x, (a0 + b1) * 0.5, (za + zb) * 0.5];
                builder.quad(
                    Zone::Exterior,
                    [[x, a1, za], [x, b1, zb], [x, a0, za], [x, b0, zb]],
                    [
                        (u0, v(a1)),
                        (u0 + 48.0, v(b1)),
                        (u0, v(a0)),
                        (u0 + 48.0, v(b0)),
                    ],
                    stone.tinted(moss(middle, 1.0)).double_sided(),
                );
            }
        }
    }
}

fn shrine(builder: &mut Builder, art: &Art) {
    let stone = Material::textured(&art.stone[1]);
    let paving = Material::textured(&art.paving);
    let (y0, y1) = (TIER_Y[5], TIER_Y[5] + 5.0);
    let (door, lintel, ceiling) = (2.0, y0 + 3.0, y1 - 0.6);
    let (outer, inner) = (4.0, 3.4);
    let corners = [
        (-outer, TEMPLE_Z - outer),
        (outer, TEMPLE_Z - outer),
        (outer, TEMPLE_Z + outer),
        (-outer, TEMPLE_Z + outer),
    ];
    for side in 0..4 {
        builder.wall(
            Zone::Exterior,
            corners[side],
            corners[(side + 1) % 4],
            &[y0, lintel, y1],
            2.0,
            24.0,
            |middle| {
                let tint = moss(middle, 0.8);
                if side == 0 && middle[0].abs() < door && middle[1] < lintel {
                    None
                } else if middle[1] > lintel {
                    let offset = if side % 2 == 0 {
                        middle[0]
                    } else {
                        middle[2] - TEMPLE_Z
                    }
                    .abs();
                    let motif = if offset < 2.0 { 1 } else { 0 };
                    Some(Cell::panel(&art.carvings[motif], moss(middle, 0.4)))
                } else {
                    Some(Cell::new(stone.tinted(tint)))
                }
            },
        );
    }
    let open_grid = |half: f32| vec![-half, -2.0, -1.0, 1.0, 2.0, half];
    let oculus = |middle: Vec3| middle[0].abs() < 1.0 && (middle[2] - TEMPLE_Z).abs() < 1.0;
    let outer_z: Vec<f32> = open_grid(outer).iter().map(|z| z + TEMPLE_Z).collect();
    builder.floor(
        Zone::Exterior,
        &open_grid(outer),
        &outer_z,
        |_, _| y1,
        true,
        16.0,
        |middle| (!oculus(middle)).then(|| Cell::new(stone.tinted(moss(middle, 1.2)))),
    );

    let front = TEMPLE_Z - inner;
    let back = TEMPLE_Z + inner;
    let threshold = TEMPLE_Z - outer;
    builder.wall(
        Zone::Exterior,
        (-door, threshold),
        (-door, front),
        &[y0, lintel],
        1.0,
        24.0,
        |middle| Some(Cell::new(stone.tinted(moss(middle, 0.6)))),
    );
    builder.wall(
        Zone::Exterior,
        (door, front),
        (door, threshold),
        &[y0, lintel],
        1.0,
        24.0,
        |middle| Some(Cell::new(stone.tinted(moss(middle, 0.6)))),
    );
    builder.floor(
        Zone::Exterior,
        &[-door, door],
        &[threshold, front],
        |_, _| lintel,
        false,
        24.0,
        |_| Some(Cell::new(stone)),
    );
    builder.floor(
        Zone::Exterior,
        &[-door, 0.0, door],
        &[threshold, front],
        |_, _| y0,
        true,
        24.0,
        |middle| Some(Cell::new(paving.tinted(moss(middle, 0.8)))),
    );

    let plain = |middle: Vec3| Some(Cell::new(stone.tinted(moss(middle, 0.5))));
    builder.wall(
        Zone::Interior,
        (inner, front),
        (door, front),
        &[y0, lintel, ceiling],
        2.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (door, front),
        (-door, front),
        &[lintel, ceiling],
        4.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (-door, front),
        (-inner, front),
        &[y0, lintel, ceiling],
        2.0,
        24.0,
        plain,
    );
    builder.wall(
        Zone::Interior,
        (-inner, back),
        (inner, back),
        &[y0, 16.0, ceiling],
        2.4,
        24.0,
        plain,
    );
    for (a, b, carving) in [
        ((inner, back), (inner, front), 3),
        ((-inner, front), (-inner, back), 2),
    ] {
        builder.wall(
            Zone::Interior,
            a,
            b,
            &[y0, y0 + 1.0, y0 + 3.4, ceiling],
            2.3,
            24.0,
            |middle| {
                if middle[1] > y0 + 1.0
                    && middle[1] < y0 + 3.4
                    && (middle[2] - TEMPLE_Z).abs() < 1.5
                {
                    Some(Cell::panel(&art.carvings[carving], [1.0; 3]))
                } else {
                    plain(middle)
                }
            },
        );
    }
    builder.wall(
        Zone::Interior,
        (-1.5, back - 0.05),
        (1.5, back - 0.05),
        &[y0 + 0.6, y0 + 3.6],
        3.0,
        24.0,
        |_| Some(Cell::panel(&art.carvings[1], [1.05, 1.0, 0.9]).biased(cave::OVERLAY_BIAS)),
    );
    let inner_z: Vec<f32> = open_grid(inner).iter().map(|z| z + TEMPLE_Z).collect();
    builder.floor(
        Zone::Interior,
        &open_grid(inner),
        &inner_z,
        |_, _| y0,
        true,
        24.0,
        |middle| (!oculus(middle)).then(|| Cell::new(paving.tinted(moss(middle, 0.6)))),
    );
    builder.floor(
        Zone::Interior,
        &open_grid(inner),
        &inner_z,
        |_, _| ceiling,
        false,
        24.0,
        |middle| (!oculus(middle)).then(|| Cell::new(stone)),
    );
    for (a, b) in [
        ((-1.0, TEMPLE_Z - 1.0), (-1.0, TEMPLE_Z + 1.0)),
        ((1.0, TEMPLE_Z + 1.0), (1.0, TEMPLE_Z - 1.0)),
        ((1.0, TEMPLE_Z - 1.0), (-1.0, TEMPLE_Z - 1.0)),
        ((-1.0, TEMPLE_Z + 1.0), (1.0, TEMPLE_Z + 1.0)),
    ] {
        builder.wall(Zone::Interior, a, b, &[ceiling, y1], 2.0, 24.0, |_| {
            Some(Cell::new(stone))
        });
    }
    for (min, max) in [
        ([-1.3, TEMPLE_Z - 1.3], [1.3, TEMPLE_Z - 1.0]),
        ([-1.3, TEMPLE_Z + 1.0], [1.3, TEMPLE_Z + 1.3]),
        ([-1.3, TEMPLE_Z - 1.0], [-1.0, TEMPLE_Z + 1.0]),
        ([1.0, TEMPLE_Z - 1.0], [1.3, TEMPLE_Z + 1.0]),
    ] {
        builder.block(
            Zone::Interior,
            [min[0], y0, min[1]],
            [max[0], y0 + 0.3, max[1]],
            24.0,
            paving,
        );
    }
    for side in [-1.0f32, 1.0] {
        let (x, z) = (side * BRAZIER_X, BRAZIER_Z);
        builder.block(
            Zone::Interior,
            [x - 0.35, y0, z - 0.35],
            [x + 0.35, y0 + 0.9, z + 0.35],
            24.0,
            stone,
        );
    }

    let comb_z = TEMPLE_Z + 3.0;
    let comb = [
        (-3.0, comb_z - 0.6),
        (3.0, comb_z - 0.6),
        (3.0, comb_z + 0.6),
        (-3.0, comb_z + 0.6),
    ];
    for side in 0..4 {
        builder.wall(
            Zone::Exterior,
            comb[side],
            comb[(side + 1) % 4],
            &[y1, y1 + 3.0],
            2.0,
            24.0,
            |middle| {
                let tint = moss(middle, 0.6);
                Some(if side % 2 == 0 {
                    Cell::panel(&art.carvings[(middle[0] + 3.0) as usize / 2 % 4], tint)
                } else {
                    Cell::new(stone.tinted(tint))
                })
            },
        );
    }
    builder.floor(
        Zone::Exterior,
        &steps(-3.0, 3.0, 2.0),
        &[comb_z - 0.6, comb_z + 0.6],
        |_, _| y1 + 3.0,
        true,
        24.0,
        |_| Some(Cell::new(stone)),
    );
}

fn interior(builder: &mut Builder, art: &Art) {
    let stones: Vec<Material> = art.stone.iter().map(Material::textured).collect();
    let paving = Material::textured(&art.paving);
    let damp = |middle: Vec3| moss(middle, 0.9);
    let lower = [TEMPLE_Z - 2.0, TEMPLE_Z, TEMPLE_Z + 2.0];
    for side in [-1.0f32, 1.0] {
        let (x0, x1) = if side < 0.0 {
            (-14.0, -6.0)
        } else {
            (6.0, 14.0)
        };
        let xs = steps(x0, x1, 2.0);
        builder.floor(
            Zone::Interior,
            &xs,
            &lower,
            |_, _| 0.0,
            true,
            24.0,
            |middle| Some(Cell::new(paving.tinted(damp(middle)))),
        );
        builder.floor(
            Zone::Interior,
            &xs,
            &lower,
            |_, _| 4.0,
            false,
            24.0,
            |middle| Some(Cell::new(pick(&stones, middle, 11))),
        );
        let corridor = |middle: Vec3| {
            let carved = middle[1] > 2.0 && (middle[0].abs() as i32 / 2) % 2 == 0;
            Some(if carved {
                Cell::panel(
                    &art.carvings[(middle[0].abs() as usize / 4) % 2 * 2 + 1],
                    damp(middle),
                )
            } else {
                Cell::new(pick(&stones, middle, 13).tinted(damp(middle)))
            })
        };
        builder.wall(
            Zone::Interior,
            (x0, TEMPLE_Z + 2.0),
            (x1, TEMPLE_Z + 2.0),
            &[0.0, 2.0, 4.0],
            2.0,
            24.0,
            corridor,
        );
        builder.wall(
            Zone::Interior,
            (x1, TEMPLE_Z - 2.0),
            (x0, TEMPLE_Z - 2.0),
            &[0.0, 2.0, 4.0],
            2.0,
            24.0,
            corridor,
        );
    }

    let chamber_x = steps(-6.0, 6.0, 2.0);
    let chamber_z: Vec<f32> = chamber_x.iter().map(|x| x + TEMPLE_Z).collect();
    builder.floor(
        Zone::Interior,
        &chamber_x,
        &chamber_z,
        |_, _| 0.0,
        true,
        24.0,
        |middle| {
            (!crypt::in_stairwell(middle[0], middle[2]))
                .then(|| Cell::new(paving.tinted(damp(middle))))
        },
    );
    let ceiling_x = [-6.0, -4.0, -2.0, -1.0, 1.0, 2.0, 4.0, 6.0];
    let ceiling_z: Vec<f32> = ceiling_x.iter().map(|x| x + TEMPLE_Z).collect();
    builder.floor(
        Zone::Interior,
        &ceiling_x,
        &ceiling_z,
        |_, _| 7.0,
        false,
        24.0,
        |middle| {
            let hole = middle[0].abs() < 1.0 && (middle[2] - TEMPLE_Z).abs() < 1.0;
            (!hole).then(|| Cell::new(pick(&stones, middle, 14)))
        },
    );
    let walls = [
        ((-6.0, TEMPLE_Z - 6.0), (-6.0, TEMPLE_Z + 6.0)),
        ((6.0, TEMPLE_Z + 6.0), (6.0, TEMPLE_Z - 6.0)),
        ((6.0, TEMPLE_Z - 6.0), (-6.0, TEMPLE_Z - 6.0)),
        ((-6.0, TEMPLE_Z + 6.0), (6.0, TEMPLE_Z + 6.0)),
    ];
    for (index, (a, b)) in walls.into_iter().enumerate() {
        builder.wall(
            Zone::Interior,
            a,
            b,
            &[0.0, 2.0, 4.0, 5.5, 7.0],
            2.0,
            24.0,
            |middle| {
                let door = index < 2 && (middle[2] - TEMPLE_Z).abs() < 2.0 && middle[1] < 4.0;
                if door {
                    None
                } else if middle[1] > 5.5 {
                    Some(Cell::panel(
                        &art.carvings[if (middle[0].abs() + (middle[2] - TEMPLE_Z).abs()) < 6.5 {
                            0
                        } else {
                            2
                        }],
                        [0.9; 3],
                    ))
                } else {
                    Some(Cell::new(pick(&stones, middle, 16).tinted(damp(middle))))
                }
            },
        );
    }
    let hole = [
        ((-1.0, TEMPLE_Z - 1.0), (-1.0, TEMPLE_Z + 1.0)),
        ((1.0, TEMPLE_Z + 1.0), (1.0, TEMPLE_Z - 1.0)),
        ((1.0, TEMPLE_Z - 1.0), (-1.0, TEMPLE_Z - 1.0)),
        ((-1.0, TEMPLE_Z + 1.0), (1.0, TEMPLE_Z + 1.0)),
    ];
    for (a, b) in hole {
        builder.wall(
            Zone::Interior,
            a,
            b,
            &[7.0, 10.5, TIER_Y[5]],
            2.0,
            24.0,
            |_| Some(Cell::new(stones[2])),
        );
    }
    for (x, z) in [(-4.2, 35.8), (4.2, 35.8), (-4.2, 44.2), (4.2, 44.2)] {
        builder.block(
            Zone::Interior,
            [x - 0.5, 0.0, z - 0.5],
            [x + 0.5, 7.0, z + 0.5],
            24.0,
            stones[3],
        );
    }
    builder.dressed_block(
        Zone::Interior,
        [-1.0, 0.0, TEMPLE_Z - 1.0],
        [1.0, ALTAR_TOP, TEMPLE_Z + 1.0],
        24.0,
        [Cell::new(paving); 4],
        Cell::new(Material::textured(&art.coins[2])),
    );
}

fn brackets(builder: &mut Builder, lighting: &Lighting) {
    for torch in lighting
        .lights
        .iter()
        .filter(|light| light.kind == LightKind::Torch)
    {
        let [x, y, z] = torch.position;
        builder.block(
            torch.zone,
            [x - 0.07, y - 0.6, z - 0.07],
            [x + 0.07, y, z + 0.07],
            24.0,
            Material::flat([70, 44, 22]),
        );
    }
}

fn path_clear(samples: &[Vec3], x: f32, z: f32, bottom: f32, top: f32, radius: f32) -> bool {
    samples.iter().all(|point| {
        (point[0] - x).hypot(point[2] - z) > radius + 1.5
            || point[1] > top + 1.5
            || point[1] < bottom - 1.5
    })
}

fn branches(samples: &[Vec3]) -> Vec<(Vec3, Vec3)> {
    let mut list = Vec::new();
    for (index, &(x, z)) in GIANT_TREES.iter().enumerate() {
        let (top, _) = giant_top(x, z, samples);
        let base = ground_height(x, z);
        for branch in 0..3 {
            let height = base + 7.0 + branch as f32 * 3.2;
            if height > top - 1.5 {
                continue;
            }
            let angle = index as f32 * 1.7 + branch as f32 * 2.1;
            let reach = 4.5 + hash_float(71, index as u64, branch) * 2.5;
            let (dx, dz) = (angle.cos(), -angle.sin());
            let start = [x + dx * 0.9, height, z + dz * 0.9];
            let end = [
                start[0] + dx * reach,
                height + reach * 0.25,
                start[2] + dz * reach,
            ];
            let tip_clear = path_clear(samples, end[0], end[2], height - 1.0, end[1] + 1.0, 1.0);
            if tip_clear {
                list.push((start, end));
            }
        }
    }
    list
}

fn giant_top(x: f32, z: f32, samples: &[Vec3]) -> (f32, bool) {
    if canopy_clear(x, z, samples) {
        (ground_height(x, z) + 13.0, true)
    } else {
        (canopy_height(x, z) - 0.6, false)
    }
}

fn giant_trees(builder: &mut Builder, art: &Art, samples: &[Vec3]) {
    const SIDES: usize = 8;
    for (index, &(x, z)) in GIANT_TREES.iter().enumerate() {
        let bark = Material::textured(&art.bark[index % art.bark.len()]);
        let base = ground_height(x, z) - 0.5;
        let (top, _) = giant_top(x, z, samples);
        let rows = steps(base, top, 3.0);
        let radius = |y: f32| 1.6 - 0.6 * ((y - base) / (top - base)).sqrt();
        let twist = index as f32;
        for side in 0..SIDES {
            let angle0 = twist + side as f32 / SIDES as f32 * TAU;
            let angle1 = twist + (side + 1) as f32 / SIDES as f32 * TAU;
            for row in rows.windows(2) {
                let (y0, y1) = (row[0], row[1]);
                let at = |angle: f32, y: f32| {
                    [x + angle.cos() * radius(y), y, z - angle.sin() * radius(y)]
                };
                let u0 = side as f32 * 16.0 % 64.0;
                let v0 = (-y1 * 16.0).rem_euclid(64.0);
                let v1 = v0 + (y1 - y0) * 16.0;
                builder.quad(
                    Zone::Exterior,
                    [
                        at(angle1, y1),
                        at(angle0, y1),
                        at(angle1, y0),
                        at(angle0, y0),
                    ],
                    [(u0, v0), (u0 + 16.0, v0), (u0, v1), (u0 + 16.0, v1)],
                    bark,
                );
            }
        }
        for fin in 0..4 {
            let angle = twist + 0.4 + fin as f32 / 4.0 * TAU;
            let (dx, dz) = (angle.cos(), -angle.sin());
            let inner = [x + dx * 1.2, base, z + dz * 1.2];
            let outer = [x + dx * 4.0, base, z + dz * 4.0];
            let high = [x + dx * 1.2, base + 5.0, z + dz * 1.2];
            let low = [x + dx * 4.0, base + 0.7, z + dz * 4.0];
            builder.quad(
                Zone::Exterior,
                [high, low, inner, outer],
                [(0.0, 0.0), (44.0, 60.0), (0.0, 80.0), (44.0, 80.0)],
                bark.double_sided(),
            );
        }
    }
    for (start, end) in branches(samples) {
        let bark = Material::textured(&art.bark[1]).double_sided();
        let axis = normalize(sub(end, start));
        let across = normalize(cross(axis, [0.0, 1.0, 0.0]));
        let up = cross(across, axis);
        let ring = |centre: Vec3, radius: f32, side: usize| {
            let angle = (side as f32 + 0.5) * TAU / 4.0;
            add(
                centre,
                add(
                    scale(across, angle.cos() * radius),
                    scale(up, angle.sin() * radius),
                ),
            )
        };
        let span = length(sub(end, start)) * 16.0;
        for side in 0..4 {
            let next = (side + 1) % 4;
            builder.quad(
                Zone::Exterior,
                [
                    ring(start, 0.55, side),
                    ring(end, 0.15, side),
                    ring(start, 0.55, next),
                    ring(end, 0.15, next),
                ],
                [(0.0, 0.0), (span, 0.0), (0.0, 16.0), (span, 16.0)],
                bark,
            );
        }
    }
}

fn crowns(art: &Art, samples: &[Vec3], lighting: &Lighting, list: &mut Vec<Billboard>) {
    for (index, (start, end)) in branches(samples).into_iter().enumerate() {
        for (step, t) in [0.45f32, 0.75, 1.0].into_iter().enumerate() {
            let anchor = add(start, scale(sub(end, start), t));
            let ground = ground_height(anchor[0], anchor[2]);
            let length = (3.0 + hash_float(73, index as u64, step as u64) * 4.0)
                .min(anchor[1] - ground - 1.0);
            if length < 1.5
                || !path_clear(
                    samples,
                    anchor[0],
                    anchor[2],
                    anchor[1] - length,
                    anchor[1],
                    0.6,
                )
            {
                continue;
            }
            let lit = lighting.billboard(
                [anchor[0], anchor[1] - length * 0.5, anchor[2]],
                Zone::Exterior,
            );
            list.push(Billboard {
                image: art.vines[(index + step) % art.vines.len()],
                position: [anchor[0], anchor[1] + 0.2, anchor[2]],
                width: length * 0.8,
                height: length,
                color: [0, 1, 2].map(|c| (lit[c] as f32 * [0.82, 1.0, 0.8][c]) as u8),
                hanging: true,
            });
        }
    }
    for &(x, z) in &GIANT_TREES {
        let (top, exposed) = giant_top(x, z, samples);
        if !exposed {
            continue;
        }
        for (index, (dx, dy, dz, size)) in [
            (0.0, -2.0, 0.0, 10.0),
            (2.5, -3.5, 1.5, 7.0),
            (-2.5, -3.0, -1.0, 7.5),
            (0.5, 0.5, -2.0, 6.0),
        ]
        .into_iter()
        .enumerate()
        {
            let position = [x + dx, top + dy, z + dz];
            let lit = lighting.billboard(
                [position[0], position[1] + size * 0.4, position[2]],
                Zone::Exterior,
            );
            list.push(Billboard {
                image: art.bushes[index % art.bushes.len()],
                position,
                width: size,
                height: size * 0.75,
                color: [0, 1, 2].map(|c| (lit[c] as f32 * [0.8, 0.95, 0.75][c]) as u8),
                hanging: false,
            });
        }
    }
}

fn canopy_clear(x: f32, z: f32, samples: &[Vec3]) -> bool {
    let temple = x.abs().max((z - TEMPLE_Z).abs());
    let stairs = x.abs() < 14.0 && z > -6.0 && z < 30.0;
    let height = canopy_height(x, z);
    temple < 26.0
        || secrets::near_ruin(x, z, 0.0)
        || pool_distance(x, z) < 1.7
        || stairs
        || samples.iter().any(|point| {
            point[1] > floor_height(point[0], point[2]) - 1.0
                && (point[0] - x).hypot(point[2] - z) < 12.0
                && point[1] < height + 4.0
        })
}

pub fn canopy_height(x: f32, z: f32) -> f32 {
    floor_height(x, z).max(0.0) + CANOPY_HEIGHT + (value_noise(x / 11.0, z / 11.0, 61) - 0.5) * 4.0
}

fn canopy(builder: &mut Builder, art: &Art, samples: &[Vec3]) {
    let leaves = Material::textured(&art.foliage).tinted([0.55, 0.62, 0.52]);
    let under = Material::textured(&art.canopy).tinted([0.8, 0.8, 0.8]);
    let zs = steps(-126.0, 156.0, CANOPY_CELL);
    for (xs, height_of) in [
        (
            steps(-42.0, 144.0, CANOPY_CELL),
            canopy_height as fn(f32, f32) -> f32,
        ),
        (steps(-150.0, -42.0, CANOPY_CELL), canopy_height),
    ] {
        let select = |middle: Vec3, material: Material| {
            (!canopy_clear(middle[0], middle[2], samples)).then(|| Cell::new(material))
        };
        builder.floor(Zone::Exterior, &xs, &zs, height_of, true, 14.0, |middle| {
            select(middle, leaves)
        });
        builder.floor(
            Zone::Exterior,
            &xs,
            &zs,
            |x, z| height_of(x, z) - 0.6,
            false,
            14.0,
            |middle| select(middle, under),
        );
    }
    treeline(builder, art, samples);
}

fn floor_height(x: f32, z: f32) -> f32 {
    if x < CLIFF_X {
        plateau_height(x, z)
    } else {
        ground_height(x, z)
    }
}

fn treeline(builder: &mut Builder, art: &Art, samples: &[Vec3]) {
    let walls: Vec<Material> = art.walls.iter().map(Material::textured).collect();
    let xs = steps(-150.0, 144.0, CANOPY_CELL);
    let zs = steps(-126.0, 156.0, CANOPY_CELL);
    let clear = |x: f32, z: f32| canopy_clear(x, z, samples);
    for zw in zs.windows(2) {
        for xw in xs.windows(2) {
            let (x0, x1, z0, z1) = (xw[0], xw[1], zw[0], zw[1]);
            let (cx, cz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5);
            if clear(cx, cz) {
                continue;
            }
            let edges = [
                ((CANOPY_CELL, 0.0), (x1, z0), (x1, z1)),
                ((-CANOPY_CELL, 0.0), (x0, z1), (x0, z0)),
                ((0.0, CANOPY_CELL), (x1, z1), (x0, z1)),
                ((0.0, -CANOPY_CELL), (x0, z0), (x1, z0)),
            ];
            for ((dx, dz), a, b) in edges {
                if !clear(cx + dx, cz + dz)
                    || ruins::tunnel_gap((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
                {
                    continue;
                }
                let top = |p: (f32, f32)| canopy_height(p.0, p.1) - 0.6;
                let bottom = |p: (f32, f32)| floor_height(p.0, p.1) - 0.5;
                let u0 = ((a.0 + a.1) * 3.5).rem_euclid(64.0);
                let material = pick(&walls, [cx, 0.0, cz], 17);
                let level = |p: (f32, f32), f: f32| top(p) + (bottom(p) - top(p)) * f;
                for row in 0..TREELINE_ROWS {
                    let (f0, f1) = (
                        row as f32 / TREELINE_ROWS as f32,
                        (row + 1) as f32 / TREELINE_ROWS as f32,
                    );
                    let (v0, v1) = (f0 * 127.0, f1 * 127.0);
                    builder.quad(
                        Zone::Exterior,
                        [
                            [a.0, level(a, f0), a.1],
                            [b.0, level(b, f0), b.1],
                            [a.0, level(a, f1), a.1],
                            [b.0, level(b, f1), b.1],
                        ],
                        [(u0, v0), (u0 + 32.0, v0), (u0, v1), (u0 + 32.0, v1)],
                        material,
                    );
                }
            }
        }
    }
}

fn pool_cells() -> Vec<(f32, f32)> {
    let mut cells = Vec::new();
    for x0 in steps(-42.0, -20.0, 2.0).iter().take(11) {
        for z0 in steps(2.0, 26.0, 2.0).iter().take(12) {
            let (x0, z0) = (*x0, *z0);
            let centre = pool_distance(x0 + 1.0, z0 + 1.0);
            let highest = [(0.0, 0.0), (2.0, 0.0), (0.0, 2.0), (2.0, 2.0)]
                .iter()
                .map(|(dx, dz)| ground_height(x0 + dx, z0 + dz))
                .fold(f32::MIN, f32::max);
            let submerged = centre < 1.0 && highest < WATER_Y - 0.05;
            let under_cliff = x0 + 1.0 < -38.5 && centre < 1.1;
            if submerged || under_cliff {
                cells.push((x0, z0));
            }
        }
    }
    cells
}

fn props(art: &Art, samples: &[Vec3], lighting: &Lighting) -> Vec<Billboard> {
    let clear = |x: f32, z: f32, bottom: f32, top: f32, radius: f32| {
        samples.iter().all(|point| {
            (point[0] - x).hypot(point[2] - z) > radius + 1.5
                || point[1] > top + 1.5
                || point[1] < bottom - 1.5
        })
    };
    let mut rng = Rng::new(2024);
    let mut list = Vec::new();
    let place = |list: &mut Vec<Billboard>,
                 image: Image,
                 position: Vec3,
                 size: (f32, f32),
                 hanging: bool,
                 tint: Vec3| {
        let middle = [
            position[0],
            position[1] + if hanging { -size.1 } else { size.1 } * 0.5,
            position[2],
        ];
        let lit = lighting.billboard(middle, Zone::Exterior);
        list.push(Billboard {
            image,
            position,
            width: size.0,
            height: size.1,
            color: [0, 1, 2].map(|c| (lit[c] as f32 * tint[c]).min(255.0) as u8),
            hanging,
        });
    };
    let jungle = [0.82, 1.0, 0.8];

    let mut trunks: Vec<(f32, f32)> = GIANT_TREES.to_vec();
    for _ in 0..5000 {
        if trunks.len() >= 320 {
            break;
        }
        let plateau = rng.chance(0.3);
        let x = if plateau {
            rng.range(-146.0, -45.0)
        } else {
            rng.range(-38.0, 146.0)
        };
        let z = rng.range(-126.0, 154.0);
        let height = rng.range(13.0, 22.0);
        let image = art.trees[rng.below(art.trees.len() as u32) as usize];
        let width = height * image.width as f32 / image.height as f32;
        let base = if plateau {
            plateau_height(x, z)
        } else {
            ground_height(x, z)
        } - 0.4;
        if (!plateau && !open_ground(x, z, 6.0))
            || secrets::near_ruin(x, z, 2.0)
            || grotto::near(x, z, 3.0)
            || ruins::near(x, z, 3.0)
            || trunks.iter().any(|&(tx, tz)| (tx - x).hypot(tz - z) < 5.0)
            || !clear(x, z, base, base + height, width * 0.5)
        {
            continue;
        }
        trunks.push((x, z));
        place(
            &mut list,
            image,
            [x, base, z],
            (width, height),
            false,
            jungle,
        );
        if rng.chance(0.35) {
            let length = rng.range(4.0, 8.0);
            let vine = art.vines[rng.below(4) as usize];
            let anchor = [
                x + rng.range(-1.5, 1.5),
                base + height * 0.72,
                z + rng.range(-1.5, 1.5),
            ];
            if clear(
                anchor[0],
                anchor[2],
                anchor[1] - length,
                anchor[1],
                length * 0.4,
            ) {
                place(
                    &mut list,
                    vine,
                    anchor,
                    (length * 0.8, length),
                    true,
                    jungle,
                );
            }
        }
    }

    for index in 0..420 {
        let fern = index % 2 == 0;
        let (x, z) = if index % 3 == 0 {
            let angle = rng.range(0.0, TAU);
            let distance = rng.range(15.5, 24.0);
            (angle.cos() * distance, TEMPLE_Z + angle.sin() * distance)
        } else {
            (rng.range(-38.0, 70.0), rng.range(-40.0, 90.0))
        };
        let width = if fern {
            rng.range(1.4, 2.4)
        } else {
            rng.range(2.0, 3.4)
        };
        let height = width * 0.75;
        let base = ground_height(x, z) - 0.15;
        if !open_ground(x, z, 0.6)
            || ruins::near(x, z, 0.3)
            || !clear(x, z, base, base + height, width * 0.5)
        {
            continue;
        }
        let image = if fern {
            art.procedural.fern
        } else {
            art.bushes[rng.below(4) as usize]
        };
        place(
            &mut list,
            image,
            [x, base, z],
            (width, height),
            false,
            if fern { [1.0; 3] } else { jungle },
        );
    }

    for (tier, &y) in TIER_Y.iter().enumerate().take(5).skip(1) {
        let (half, inner) = (tier_half(tier - 1), tier_half(tier));
        for _ in 0..14 {
            let x = rng.range(-half + 0.5, half - 0.5);
            let z = TEMPLE_Z + rng.range(-half + 0.5, half - 0.5);
            let on_terrace = x.abs().max((z - TEMPLE_Z).abs()) > inner + 0.4;
            let on_stairs = x.abs() < 4.0 && z < TEMPLE_Z;
            if !on_terrace || on_stairs {
                continue;
            }
            let width = rng.range(1.2, 2.2);
            place(
                &mut list,
                art.procedural.fern,
                [x, y - 0.1, z],
                (width, width * 0.75),
                false,
                [0.9, 1.0, 0.85],
            );
        }
    }

    for _ in 0..40 {
        let near_cliff = rng.chance(0.5);
        let (x, z) = if near_cliff {
            (rng.range(-39.0, -33.0), rng.range(-30.0, 70.0))
        } else {
            let angle = rng.range(0.0, TAU);
            (
                -31.0 + angle.cos() * 9.5 * rng.range(1.0, 1.3),
                14.0 + angle.sin() * 10.0 * rng.range(1.0, 1.3),
            )
        };
        let size = rng.range(1.8, 3.4);
        let base = ground_height(x, z) - 0.3;
        if pool_distance(x, z) < 1.0
            || (z - FALLS_Z).abs() < 4.0
            || !clear(x, z, base, base + size, size * 0.5)
        {
            continue;
        }
        place(
            &mut list,
            art.boulders[rng.below(4) as usize],
            [x, base, z],
            (size, size),
            false,
            [0.85, 0.95, 0.85],
        );
    }

    for tier in 0..5 {
        let half = tier_half(tier);
        let (y0, y1) = (TIER_Y[tier], TIER_Y[tier + 1]);
        let corners = [
            (-half, TEMPLE_Z - half),
            (half, TEMPLE_Z - half),
            (half, TEMPLE_Z + half),
            (-half, TEMPLE_Z + half),
        ];
        let outward = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
        for side in 0..4 {
            let (a, b) = (corners[side], corners[(side + 1) % 4]);
            let mut t = rng.range(0.02, 0.08);
            while t < 0.95 {
                let x = a.0 + (b.0 - a.0) * t + outward[side].0 * 0.2;
                let z = a.1 + (b.1 - a.1) * t + outward[side].1 * 0.2;
                t += rng.range(0.06, 0.14);
                if !rng.chance(0.55) || (side == 0 && x.abs() < 4.2) {
                    continue;
                }
                let door = tier == 0 && side % 2 == 1 && (z - TEMPLE_Z).abs() < 3.0;
                let reach = if door {
                    1.3
                } else {
                    y1 - y0 + if tier > 0 { 2.0 } else { -0.5 }
                };
                let length = rng.range(reach * 0.5, reach).max(1.0);
                let anchor = [x, y1 + 0.3, z];
                if clear(x, z, anchor[1] - length, anchor[1], length * 0.35) {
                    place(
                        &mut list,
                        art.vines[rng.below(4) as usize],
                        anchor,
                        (length * 0.8, length),
                        true,
                        jungle,
                    );
                }
            }
        }
    }

    let mut z = -60.0;
    while z < 100.0 {
        z += rng.range(1.6, 3.2);
        if (z - FALLS_Z).abs() < 4.5 || !rng.chance(0.6) {
            continue;
        }
        let length = rng.range(3.0, 9.0);
        let anchor = [CLIFF_X + 0.5, CLIFF_TOP + 0.4, z];
        place(
            &mut list,
            art.vines[rng.below(4) as usize],
            anchor,
            (length * 0.8, length),
            true,
            jungle,
        );
    }

    for _ in 0..1500 {
        let x = rng.range(-38.0, 90.0);
        let z = rng.range(-66.0, 104.0);
        if canopy_clear(x, z, samples) {
            continue;
        }
        let edge = (0..8).any(|step| {
            let angle = step as f32 / 8.0 * TAU;
            canopy_clear(x + angle.cos() * 7.0, z + angle.sin() * 7.0, samples)
        });
        if !edge || !rng.chance(0.3) {
            continue;
        }
        let length = rng.range(4.0, 11.0);
        let anchor = [x, canopy_height(x, z) - 0.4, z];
        if clear(x, z, anchor[1] - length, anchor[1], length * 0.4) {
            place(
                &mut list,
                art.vines[rng.below(4) as usize],
                anchor,
                (length * 0.8, length),
                true,
                jungle,
            );
        }
    }
    list
}
