use std::f32::consts::TAU;

use crate::art::Art;
use crate::biome::Rgb;
use crate::lighting::{sun_direction, Lighting, Torch};
use crate::mesh::{add, scale, steps, Builder, Cell, Material, Mesh, Vec3, Zone};
use crate::path::{key, Path};
use crate::rng::{hash_float, Rng};
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
    Path::new(vec![
        key([30.0, 42.0, -20.0], [0.0, 6.0, 40.0], 5.6),
        key([16.0, 18.0, -4.0], [0.0, 8.0, 40.0], 4.8),
        key([6.0, 6.5, 8.0], [0.0, 9.0, 34.0], 4.0),
        key([-10.0, 4.5, 10.0], [-30.0, 5.0, 14.0], 3.6),
        key([-21.0, 2.2, 9.0], [-38.0, 8.0, 14.0], 3.2),
        key([-28.0, 1.6, 15.0], [-38.0, 12.0, 14.0], 2.4),
        key([-27.0, 5.5, 28.0], [-14.0, 2.0, 40.0], 3.2),
        key([-24.0, 2.2, 40.0], [-10.0, 2.0, 40.0], 2.4),
        key([-14.0, 2.0, 40.0], [0.0, 1.8, 40.0], 2.0),
        key([-8.0, 2.0, 40.0], [0.0, 1.5, 40.0], 1.6),
        key([-3.2, 2.4, 37.8], [0.0, 1.4, 40.0], 1.28),
        key([0.0, 3.0, 36.6], [0.0, 1.3, 40.0], 1.28),
        key([3.2, 2.4, 37.8], [8.0, 2.0, 40.0], 1.44),
        key([8.0, 2.0, 40.0], [20.0, 2.0, 40.0], 1.76),
        key([14.0, 2.0, 40.0], [26.0, 3.0, 40.0], 2.24),
        key([22.0, 3.2, 40.0], [30.0, 6.0, 36.0], 2.8),
        key([30.0, 12.0, 30.0], [0.0, 10.0, 40.0], 4.0),
        key([40.0, 36.0, 8.0], [0.0, 8.0, 40.0], 5.2),
    ])
}

pub fn insideness(position: Vec3) -> f32 {
    let within =
        position[0].abs() < 15.0 && (position[2] - TEMPLE_Z).abs() < 6.5 && position[1] < 7.5;
    if within {
        smoothstep(-1.0, 5.0, 14.0 - position[0].abs())
    } else {
        0.0
    }
}

pub fn build(art: &Art, path: &Path) -> Scene {
    let lighting = Lighting { torches: torches() };
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
        giant_trees(&mut builder, art);
        canopy(&mut builder, art, &samples);
        builder.build()
    };
    let billboards = props(art, &samples, &lighting);
    Scene {
        mesh,
        billboards,
        pool: pool_cells(),
        shafts: shafts(),
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

fn torches() -> Vec<Torch> {
    let mut list = Vec::new();
    for side in [-1.0f32, 1.0] {
        for z in [37.0, 43.0] {
            list.push(Torch {
                position: [side * 14.6, 2.6, z],
                zone: Zone::Exterior,
            });
        }
        list.push(Torch {
            position: [side * 10.0, 2.4, TEMPLE_Z - side * 1.65],
            zone: Zone::Interior,
        });
        for z in [34.4, 45.6] {
            list.push(Torch {
                position: [side * 5.6, 3.0, z],
                zone: Zone::Interior,
            });
        }
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
        if middle[1] < WATER_Y + 0.25 {
            riverbed
        } else if pool_distance(middle[0], middle[2]) < 1.5 {
            swamp
        } else {
            pick(&floors, middle, 3)
        }
    };
    builder.floor(
        Zone::Exterior,
        &steps(-42.0, 98.0, 4.0),
        &steps(-70.0, 110.0, 4.0),
        ground_height,
        true,
        16.0,
        |middle| {
            let under_temple = middle[0].abs() < 14.0 && (middle[2] - TEMPLE_Z).abs() < 14.0;
            (!under_temple).then(|| Cell::new(surface(middle)))
        },
    );
    let mut xs = steps(-98.0, -42.0, 4.0);
    xs.push(CLIFF_X);
    builder.floor(
        Zone::Exterior,
        &xs,
        &steps(-70.0, 110.0, 4.0),
        plateau_height,
        true,
        16.0,
        |middle| Some(Cell::new(surface(middle))),
    );
}

fn cliff(builder: &mut Builder, art: &Art) {
    let rocks: Vec<Material> = art
        .rockface
        .iter()
        .map(|image| Material::textured(image).tinted([1.25, 1.25, 1.2]))
        .collect();
    let rows = [-3.0, 0.0, 3.0, 6.0, 9.0, 12.0, 14.5, CLIFF_TOP];
    for zw in steps(-70.0, 110.0, 2.0).windows(2) {
        let (z0, z1) = (zw[0], zw[1]);
        for row in rows.windows(2) {
            let (y0, y1) = (row[0], row[1]);
            let at = |z: f32, y: f32| [cliff_x(z, y), y, z];
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
                    let carved = tier > 0
                        && hash_float(
                            5,
                            (middle[0] * 4.0 + 400.0) as u64,
                            (middle[2] * 4.0 + middle[1] * 31.0 + 400.0) as u64,
                        ) < 0.22;
                    Some(if carved {
                        Cell::panel(&pick(&art.carvings, middle, 6), tint)
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
    let dark = Material::flat([4, 3, 2]);
    let (y0, y1) = (TIER_Y[5], TIER_Y[5] + 5.0);
    let corners = [
        (-4.0, TEMPLE_Z - 4.0),
        (4.0, TEMPLE_Z - 4.0),
        (4.0, TEMPLE_Z + 4.0),
        (-4.0, TEMPLE_Z + 4.0),
    ];
    for side in 0..4 {
        builder.wall(
            Zone::Exterior,
            corners[side],
            corners[(side + 1) % 4],
            &[y0, y0 + 3.0, y1],
            2.0,
            24.0,
            |middle| {
                let tint = moss(middle, 0.8);
                if side == 0 && middle[0].abs() < 2.0 && middle[1] < y0 + 3.0 {
                    None
                } else if middle[1] > y0 + 3.0 {
                    Some(Cell::panel(&pick(&art.carvings, middle, 8), tint))
                } else {
                    Some(Cell::new(stone.tinted(tint)))
                }
            },
        );
    }
    builder.floor(
        Zone::Exterior,
        &steps(-4.0, 4.0, 2.0),
        &steps(TEMPLE_Z - 4.0, TEMPLE_Z + 4.0, 2.0),
        |_, _| y1,
        true,
        16.0,
        |middle| Some(Cell::new(stone.tinted(moss(middle, 1.2)))),
    );
    let back = TEMPLE_Z - 3.2;
    let front = TEMPLE_Z - 4.0;
    let rows = [y0, y0 + 3.0];
    builder.wall(
        Zone::Exterior,
        (-2.0, back),
        (2.0, back),
        &rows,
        4.0,
        24.0,
        |_| Some(Cell::new(dark)),
    );
    builder.wall(
        Zone::Exterior,
        (-2.0, front),
        (-2.0, back),
        &rows,
        1.0,
        24.0,
        |_| Some(Cell::new(dark)),
    );
    builder.wall(
        Zone::Exterior,
        (2.0, back),
        (2.0, front),
        &rows,
        1.0,
        24.0,
        |_| Some(Cell::new(dark)),
    );
    builder.floor(
        Zone::Exterior,
        &[-2.0, 2.0],
        &[front, back],
        |_, _| y0 + 3.0,
        false,
        24.0,
        |_| Some(Cell::new(dark)),
    );
    let comb = [
        (-3.0, TEMPLE_Z - 0.6),
        (3.0, TEMPLE_Z - 0.6),
        (3.0, TEMPLE_Z + 0.6),
        (-3.0, TEMPLE_Z + 0.6),
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
        &[TEMPLE_Z - 0.6, TEMPLE_Z + 0.6],
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
                Cell::panel(&pick(&art.carvings, middle, 12), damp(middle))
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
        |middle| Some(Cell::new(paving.tinted(damp(middle)))),
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
                    Some(Cell::panel(&pick(&art.carvings, middle, 15), [0.9; 3]))
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
        builder.wall(Zone::Interior, a, b, &[7.0, 10.0], 2.0, 24.0, |_| {
            Some(Cell::new(stones[2]))
        });
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
    builder.block(
        Zone::Interior,
        [-1.0, 0.0, TEMPLE_Z - 1.0],
        [1.0, ALTAR_TOP, TEMPLE_Z + 1.0],
        24.0,
        paving,
    );
}

fn brackets(builder: &mut Builder, lighting: &Lighting) {
    for torch in &lighting.torches {
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

fn giant_trees(builder: &mut Builder, art: &Art) {
    const SIDES: usize = 8;
    for (index, &(x, z)) in GIANT_TREES.iter().enumerate() {
        let bark = Material::textured(&art.bark[index % art.bark.len()]);
        let base = ground_height(x, z) - 0.5;
        let top = ground_height(x, z) + CANOPY_HEIGHT + 1.0;
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
                        at(angle0, y1),
                        at(angle1, y1),
                        at(angle0, y0),
                        at(angle1, y0),
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
}

fn canopy_clear(x: f32, z: f32, samples: &[Vec3]) -> bool {
    let temple = x.abs().max((z - TEMPLE_Z).abs());
    let stairs = x.abs() < 14.0 && z > -6.0 && z < 30.0;
    let height = canopy_height(x, z);
    temple < 26.0
        || pool_distance(x, z) < 1.7
        || stairs
        || samples
            .iter()
            .any(|point| (point[0] - x).hypot(point[2] - z) < 12.0 && point[1] < height + 4.0)
}

fn canopy_height(x: f32, z: f32) -> f32 {
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
                if !clear(cx + dx, cz + dz) {
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
        if trunks.len() >= 180 {
            break;
        }
        let plateau = rng.chance(0.22);
        let x = if plateau {
            rng.range(-96.0, -45.0)
        } else {
            rng.range(-38.0, 96.0)
        };
        let z = rng.range(-68.0, 108.0);
        let height = rng.range(13.0, 22.0);
        let image = art.trees[rng.below(art.trees.len() as u32) as usize];
        let width = height * image.width as f32 / image.height as f32;
        let base = if plateau {
            plateau_height(x, z)
        } else {
            ground_height(x, z)
        } - 0.4;
        if (!plateau && !open_ground(x, z, 6.0))
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
        if !open_ground(x, z, 0.6) || !clear(x, z, base, base + height, width * 0.5) {
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
