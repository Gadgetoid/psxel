use std::f32::consts::{FRAC_PI_2, TAU};

use crate::art::Art;
use crate::mesh::{add, cross, length, normalize, scale, sub, Builder, Cell, Material, Vec3, Zone};
use crate::rng::hash_float;
use crate::scene::Billboard;
use crate::terrain::{ground_height, TEMPLE_Z};
use crate::textures::Image;

const CAUSEWAY: (f32, f32) = (-20.0, 17.4);
const TUNNEL_MOUTH: f32 = -6.0;
const TUNNEL_BACK: f32 = -26.0;
const TUNNEL_HALF: f32 = 4.0;
const TUNNEL_HEIGHT: f32 = 5.0;
const GAP_HALF: f32 = 6.0;
const CAUSEWAY_HALF: f32 = 3.5;
const SLAB: f32 = 1.75;
const WALLS: [((f32, f32), (f32, f32)); 5] = [
    ((-22.0, 26.0), (-22.5, 34.0)),
    ((24.0, 46.0), (24.5, 56.0)),
    ((-10.0, 62.0), (12.0, 62.5)),
    ((-18.0, 54.0), (-11.0, 60.0)),
    ((12.0, 8.0), (20.0, 14.0)),
];
const STELAE: [(f32, f32, Stela); 6] = [
    (-5.6, -4.0, Stela::Standing),
    (5.6, -4.0, Stela::Fallen),
    (-5.6, 5.0, Stela::Broken),
    (5.6, 5.0, Stela::Standing),
    (-5.6, 13.0, Stela::Standing),
    (5.6, 13.0, Stela::Broken),
];
const PLATFORM: (f32, f32) = (-16.5, 23.0);

#[derive(Clone, Copy)]
enum Stela {
    Standing,
    Broken,
    Fallen,
}

#[derive(Clone, Copy)]
pub struct Faces {
    pub stone: Material,
    pub front: Option<Image>,
    pub back: Option<Image>,
}

fn transform(local: Vec3, centre: Vec3, yaw: f32, tilt: f32) -> Vec3 {
    let (st, ct) = tilt.sin_cos();
    let tilted = [
        local[0],
        local[1] * ct - local[2] * st,
        local[1] * st + local[2] * ct,
    ];
    let (sy, cy) = yaw.sin_cos();
    add(
        centre,
        [
            tilted[0] * cy + tilted[2] * sy,
            tilted[1],
            -tilted[0] * sy + tilted[2] * cy,
        ],
    )
}

pub fn slab(
    builder: &mut Builder,
    zone: Zone,
    centre: Vec3,
    half: Vec3,
    yaw: f32,
    tilt: f32,
    faces: Faces,
) {
    let [hx, hy, hz] = half;
    let at = |x: f32, y: f32, z: f32| transform([x * hx, y * hy, z * hz], centre, yaw, tilt);
    let quads = [
        (
            [
                at(-1.0, 1.0, -1.0),
                at(1.0, 1.0, -1.0),
                at(-1.0, -1.0, -1.0),
                at(1.0, -1.0, -1.0),
            ],
            hx,
            hy,
            faces.front,
        ),
        (
            [
                at(1.0, 1.0, 1.0),
                at(-1.0, 1.0, 1.0),
                at(1.0, -1.0, 1.0),
                at(-1.0, -1.0, 1.0),
            ],
            hx,
            hy,
            faces.back,
        ),
        (
            [
                at(1.0, 1.0, -1.0),
                at(1.0, 1.0, 1.0),
                at(1.0, -1.0, -1.0),
                at(1.0, -1.0, 1.0),
            ],
            hz,
            hy,
            None,
        ),
        (
            [
                at(-1.0, 1.0, 1.0),
                at(-1.0, 1.0, -1.0),
                at(-1.0, -1.0, 1.0),
                at(-1.0, -1.0, -1.0),
            ],
            hz,
            hy,
            None,
        ),
        (
            [
                at(-1.0, 1.0, 1.0),
                at(1.0, 1.0, 1.0),
                at(-1.0, 1.0, -1.0),
                at(1.0, 1.0, -1.0),
            ],
            hx,
            hz,
            None,
        ),
    ];
    for (corners, width, height, panel) in quads {
        let (material, uvs) = match panel {
            Some(image) => {
                let cell = Cell::panel(&image, faces.stone.tint);
                let (u0, v0) = (image.u as f32, image.v as f32);
                let (u1, v1) = (
                    u0 + image.width as f32 - 1.0,
                    v0 + image.height as f32 - 1.0,
                );
                (cell.material, [(u0, v0), (u1, v0), (u0, v1), (u1, v1)])
            }
            None => {
                let (u, v) = ((width * 48.0).min(190.0), (height * 48.0).min(190.0));
                (faces.stone, [(0.0, 0.0), (u, 0.0), (0.0, v), (u, v)])
            }
        };
        builder.quad(zone, corners, uvs, material);
    }
}

fn random(seed: u64, a: f32, b: f32) -> f32 {
    hash_float(seed, (a * 7.0 + 1000.0) as u64, (b * 7.0 + 1000.0) as u64)
}

fn moss(position: Vec3) -> Vec3 {
    let amount = random(301, position[0], position[2]);
    [1.0 - 0.25 * amount, 1.0, 1.0 - 0.4 * amount]
}

fn clear(samples: &[Vec3], x: f32, z: f32, top: f32, radius: f32) -> bool {
    samples
        .iter()
        .all(|point| (point[0] - x).hypot(point[2] - z) > radius + 1.2 || point[1] > top + 1.2)
}

pub fn footprints() -> Vec<(f32, f32, f32)> {
    let mut list = vec![(PLATFORM.0, PLATFORM.1, 3.5)];
    list.extend(STELAE.iter().map(|&(x, z, _)| (x, z, 2.0)));
    for (a, b) in WALLS {
        let steps = ((b.0 - a.0).hypot(b.1 - a.1) / 3.0).ceil() as usize;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            list.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, 1.8));
        }
    }
    list.push((0.0, (CAUSEWAY.0 + CAUSEWAY.1) * 0.5, 4.5));
    list
}

pub fn tunnel_gap(x: f32, z: f32) -> bool {
    x.abs() < GAP_HALF + 0.1 && (z - TUNNEL_MOUTH).abs() < 1.0
}

pub fn near(x: f32, z: f32, margin: f32) -> bool {
    if x.abs() < GAP_HALF + 1.0 + margin && z > TUNNEL_BACK - 2.0 && z < TUNNEL_MOUTH + 2.0 {
        return true;
    }
    let along =
        (CAUSEWAY.0 - margin..CAUSEWAY.1).contains(&z) && x.abs() < CAUSEWAY_HALF + 0.6 + margin;
    along
        || footprints()
            .iter()
            .any(|&(fx, fz, radius)| (fx - x).hypot(fz - z) < radius + margin)
}

fn prism(builder: &mut Builder, start: Vec3, end: Vec3, radius: (f32, f32), material: Material) {
    let axis = normalize(sub(end, start));
    let across = normalize(cross(axis, [0.0, 1.0, 0.0]));
    let up = cross(across, axis);
    let ring = |centre: Vec3, r: f32, side: usize| {
        let angle = side as f32 / 8.0 * TAU;
        add(
            centre,
            add(scale(across, angle.cos() * r), scale(up, angle.sin() * r)),
        )
    };
    let span = (length(sub(end, start)) * 12.0).min(190.0);
    for side in 0..8 {
        let next = (side + 1) % 8;
        builder.quad(
            Zone::Exterior,
            [
                ring(start, radius.0, side),
                ring(end, radius.1, side),
                ring(start, radius.0, next),
                ring(end, radius.1, next),
            ],
            [(0.0, 0.0), (span, 0.0), (0.0, 16.0), (span, 16.0)],
            material,
        );
    }
}

fn tunnel(builder: &mut Builder, art: &Art) {
    let wall = Material::textured(&art.walls[0]).tinted([0.55, 0.6, 0.55]);
    let roof = Material::textured(&art.canopy).tinted([0.5, 0.55, 0.5]);
    let floor = Material::textured(&art.floor[1]).tinted([0.6, 0.65, 0.6]);
    let skirt = Material::textured(&art.walls[1]);
    let zs = crate::mesh::steps(TUNNEL_BACK, TUNNEL_MOUTH, 2.0);
    let xs = crate::mesh::steps(-TUNNEL_HALF, TUNNEL_HALF, 2.0);
    let low = -1.5;
    builder.wall(
        Zone::Interior,
        (-TUNNEL_HALF, TUNNEL_BACK),
        (-TUNNEL_HALF, TUNNEL_MOUTH),
        &[low, TUNNEL_HEIGHT],
        2.0,
        12.0,
        |_| Some(Cell::new(wall)),
    );
    builder.wall(
        Zone::Interior,
        (TUNNEL_HALF, TUNNEL_MOUTH),
        (TUNNEL_HALF, TUNNEL_BACK),
        &[low, TUNNEL_HEIGHT],
        2.0,
        12.0,
        |_| Some(Cell::new(wall)),
    );
    builder.wall(
        Zone::Interior,
        (TUNNEL_HALF, TUNNEL_BACK),
        (-TUNNEL_HALF, TUNNEL_BACK),
        &[low, TUNNEL_HEIGHT],
        4.0,
        12.0,
        |_| Some(Cell::new(wall)),
    );
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |_, _| TUNNEL_HEIGHT,
        false,
        10.0,
        |_| Some(Cell::new(roof)),
    );
    builder.floor(
        Zone::Interior,
        &xs,
        &zs,
        |x, z| ground_height(x, z) + 0.03,
        true,
        16.0,
        |middle| {
            (middle[0].abs() > CAUSEWAY_HALF || middle[2] < CAUSEWAY.0).then(|| Cell::new(floor))
        },
    );
    let top = |x: f32| crate::scene::canopy_height(x, TUNNEL_MOUTH) - 0.6;
    let bottom = |x: f32| ground_height(x, TUNNEL_MOUTH) - 0.5;
    for (x0, x1, lintel) in [
        (GAP_HALF, TUNNEL_HALF, false),
        (TUNNEL_HALF, -TUNNEL_HALF, true),
        (-TUNNEL_HALF, -GAP_HALF, false),
    ] {
        let (y0a, y0b) = if lintel {
            (TUNNEL_HEIGHT, TUNNEL_HEIGHT)
        } else {
            (bottom(x0), bottom(x1))
        };
        let span = (x0 - x1).abs() * 5.3;
        builder.quad(
            Zone::Exterior,
            [
                [x0, top(x0), TUNNEL_MOUTH],
                [x1, top(x1), TUNNEL_MOUTH],
                [x0, y0a, TUNNEL_MOUTH],
                [x1, y0b, TUNNEL_MOUTH],
            ],
            [(0.0, 0.0), (span, 0.0), (0.0, 127.0), (span, 127.0)],
            skirt,
        );
    }

    let stone = Material::textured(&art.stone[1]).tinted([0.9, 0.95, 0.88]);
    for side in [-1.0f32, 1.0] {
        let x = side * (TUNNEL_HALF - 0.6);
        let z = TUNNEL_MOUTH + 0.8;
        let ground = ground_height(x, z);
        slab(
            builder,
            Zone::Exterior,
            [x, ground + 1.6, z],
            [0.45, 1.75, 0.45],
            std::f32::consts::PI + side * 0.04,
            side * 0.03,
            Faces {
                stone,
                front: Some(art.carvings[if side < 0.0 { 1 } else { 2 }]),
                back: None,
            },
        );
    }
    let ground = ground_height(0.6, TUNNEL_MOUTH + 2.4);
    slab(
        builder,
        Zone::Exterior,
        [0.6, ground + 0.32, TUNNEL_MOUTH + 2.4],
        [3.1, 0.32, 0.42],
        std::f32::consts::PI + 0.22,
        0.12,
        Faces {
            stone,
            front: Some(art.carvings[3]),
            back: Some(art.carvings[0]),
        },
    );
    let bark = Material::textured(&art.bark[2]).double_sided();
    let start = [-TUNNEL_HALF - 0.8, ground_height(-4.8, -8.2) + 0.7, -8.2];
    let end = [TUNNEL_HALF + 0.6, ground_height(4.6, -10.6) + 1.6, -10.6];
    prism(builder, start, end, (0.8, 0.55), bark);
    let fork = add(start, scale(sub(end, start), 0.6));
    prism(builder, fork, add(fork, [0.8, 1.9, 1.4]), (0.3, 0.1), bark);
}

pub fn geometry(builder: &mut Builder, art: &Art, samples: &[Vec3]) {
    tunnel(builder, art);
    let paving = Material::textured(&art.paving);
    let stone = |index: usize, position: Vec3| {
        Material::textured(&art.stone[index % art.stone.len()]).tinted(moss(position))
    };
    let plain = |material: Material| Faces {
        stone: material,
        front: None,
        back: None,
    };

    let rows = ((CAUSEWAY.1 - CAUSEWAY.0) / SLAB).floor() as usize;
    for row in 0..rows {
        for column in 0..4 {
            let x = -CAUSEWAY_HALF + SLAB * (column as f32 + 0.5);
            let z = CAUSEWAY.0 + SLAB * (row as f32 + 0.5);
            if random(311, x, z) < 0.12 {
                continue;
            }
            let sink = random(312, x, z) * 0.14;
            let centre = [x, ground_height(x, z) + 0.02 - sink, z];
            let yaw = (random(313, x, z) - 0.5) * 0.12;
            let tilt = (random(314, x, z) - 0.5) * 0.08;
            let zone = if z < TUNNEL_MOUTH {
                Zone::Interior
            } else {
                Zone::Exterior
            };
            slab(
                builder,
                zone,
                centre,
                [SLAB * 0.47, 0.15, SLAB * 0.47],
                yaw,
                tilt,
                plain(paving.tinted(moss(centre)).tinted([0.72, 0.74, 0.7])),
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        let mut z = CAUSEWAY.0 + 0.8;
        while z < CAUSEWAY.1 - 0.8 {
            let x = side * (CAUSEWAY_HALF + 0.35);
            if random(321, x, z) > 0.25 {
                let height = 0.25 + random(322, x, z) * 0.25;
                let centre = [x, ground_height(x, z) + height - 0.2, z];
                slab(
                    builder,
                    Zone::Exterior,
                    centre,
                    [0.3, height, 0.78],
                    (random(323, x, z) - 0.5) * 0.1,
                    0.0,
                    plain(stone(0, centre)),
                );
            }
            z += 1.6;
        }
    }

    let carvings = &art.carvings;
    for (index, &(x, z, kind)) in STELAE.iter().enumerate() {
        let facing = if x > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
        let ground = ground_height(x, z);
        let faces = Faces {
            stone: stone(2, [x, ground, z]),
            front: Some(carvings[index % 2]),
            back: Some(carvings[2 + index % 2]),
        };
        match kind {
            Stela::Standing => {
                if clear(samples, x, z, ground + 3.4, 0.8) {
                    slab(
                        builder,
                        Zone::Exterior,
                        [x, ground + 1.5, z],
                        [0.65, 1.7, 0.24],
                        facing + (random(331, x, z) - 0.5) * 0.15,
                        (random(332, x, z) - 0.5) * 0.06,
                        faces,
                    );
                }
            }
            Stela::Broken => {
                slab(
                    builder,
                    Zone::Exterior,
                    [x, ground + 0.6, z],
                    [0.65, 0.8, 0.24],
                    facing,
                    0.0,
                    faces,
                );
                slab(
                    builder,
                    Zone::Exterior,
                    [x + 1.4 * x.signum(), ground + 0.2, z + 0.6],
                    [0.65, 0.6, 0.24],
                    facing + 0.5,
                    1.45,
                    plain(stone(2, [x, ground, z])),
                );
            }
            Stela::Fallen => {
                slab(
                    builder,
                    Zone::Exterior,
                    [x + 0.6 * x.signum(), ground + 0.18, z],
                    [0.65, 1.7, 0.24],
                    facing + 0.3,
                    -1.5,
                    faces,
                );
            }
        }
        slab(
            builder,
            Zone::Exterior,
            [x, ground - 0.1, z],
            [0.95, 0.18, 0.55],
            facing,
            0.0,
            plain(stone(1, [x, ground, z])),
        );
    }

    for (wall_index, (a, b)) in WALLS.into_iter().enumerate() {
        let span = (b.0 - a.0).hypot(b.1 - a.1);
        let yaw = (b.0 - a.0).atan2(b.1 - a.1) + FRAC_PI_2;
        let count = (span / 1.1).floor() as usize;
        for block in 0..count {
            let t = (block as f32 + 0.5) / count as f32;
            let (x, z) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let decay = random(341 + wall_index as u64, x, z);
            if decay < 0.12 {
                continue;
            }
            let height = 0.5 + 1.4 * decay * (1.0 - (t - 0.5).abs());
            let ground = ground_height(x, z);
            if !clear(samples, x, z, ground + height, 0.6) {
                continue;
            }
            let centre = [x, ground + height * 0.5 - 0.15, z];
            slab(
                builder,
                Zone::Exterior,
                centre,
                [0.55, height * 0.5 + 0.15, 0.42],
                yaw + (decay - 0.5) * 0.08,
                0.0,
                plain(stone(block, centre)),
            );
            if decay > 0.75 {
                let offset = 0.9 + random(342, x, z) * 0.8;
                let fallen = [
                    x + yaw.sin() * offset,
                    ground + 0.18,
                    z + yaw.cos() * offset,
                ];
                slab(
                    builder,
                    Zone::Exterior,
                    fallen,
                    [0.45, 0.22, 0.35],
                    yaw + decay * 3.0,
                    (decay - 0.75) * 1.2,
                    plain(stone(block + 1, fallen)),
                );
            }
        }
    }

    let (px, pz) = PLATFORM;
    let ground = ground_height(px, pz);
    slab(
        builder,
        Zone::Exterior,
        [px, ground + 0.15, pz],
        [2.4, 0.45, 2.4],
        0.2,
        0.0,
        plain(stone(3, [px, ground, pz])),
    );
    slab(
        builder,
        Zone::Exterior,
        [px, ground + 0.75, pz],
        [1.6, 0.25, 1.6],
        0.2,
        0.0,
        plain(stone(0, [px, ground, pz])),
    );
    slab(
        builder,
        Zone::Exterior,
        [px - 0.6, ground + 2.2, pz],
        [0.6, 1.3, 0.22],
        0.2 + FRAC_PI_2,
        0.0,
        Faces {
            stone: stone(2, [px, ground, pz]),
            front: Some(carvings[1]),
            back: Some(carvings[0]),
        },
    );

    for index in 0..18u64 {
        let side = index % 4;
        let along = (hash_float(351, index, 1) - 0.5) * 24.0;
        let out = 15.0 + hash_float(351, index, 2) * 2.5;
        let (x, z) = match side {
            0 => (along, TEMPLE_Z - out),
            1 => (out, TEMPLE_Z + along),
            2 => (along, TEMPLE_Z + out),
            _ => (-out, TEMPLE_Z + along),
        };
        let blocked = (side == 0 && x.abs() < 5.0) || (side % 2 == 1 && (z - TEMPLE_Z).abs() < 4.0);
        let ground = ground_height(x, z);
        if blocked || !clear(samples, x, z, ground + 1.4, 0.8) {
            continue;
        }
        let size = 0.45 + hash_float(351, index, 3) * 0.2;
        let yaw = hash_float(351, index, 4) * TAU;
        let tilt = (hash_float(351, index, 5) - 0.5) * 0.9;
        let carved = hash_float(351, index, 6) < 0.35;
        slab(
            builder,
            Zone::Exterior,
            [x, ground + size * 0.7, z],
            [size * 1.1, size, size],
            yaw,
            tilt,
            Faces {
                stone: stone(index as usize, [x, ground, z]),
                front: carved.then(|| carvings[(index % 4) as usize]),
                back: None,
            },
        );
    }
}

pub fn props(art: &Art, list: &mut Vec<Billboard>) {
    for (index, (x, z, size)) in [
        (-2.4, -10.8, 2.4),
        (1.6, -11.6, 2.8),
        (3.3, -9.4, 1.8),
        (-3.4, -12.6, 2.0),
        (0.2, -13.4, 2.6),
    ]
    .into_iter()
    .enumerate()
    {
        list.push(Billboard {
            image: art.boulders[index % art.boulders.len()],
            position: [x, ground_height(x, z) - 0.2, z],
            width: size,
            height: size,
            color: [58, 62, 54],
            hanging: false,
        });
    }
    for (index, x) in [-2.9f32, -1.0, 0.8, 2.6].into_iter().enumerate() {
        let length = 1.6 + (index % 2) as f32 * 0.9;
        list.push(Billboard {
            image: art.vines[index % art.vines.len()],
            position: [x, TUNNEL_HEIGHT + 0.1, TUNNEL_MOUTH + 0.15],
            width: 1.3,
            height: length,
            color: [100, 120, 92],
            hanging: true,
        });
    }
    let mut place = |x: f32, z: f32, width: f32, salt: u64| {
        let image = art.procedural.fern;
        let jitter = |n: u64| (hash_float(361, salt, n) - 0.5) * 1.2;
        let (fx, fz) = (x + jitter(1), z + jitter(2));
        list.push(Billboard {
            image,
            position: [fx, ground_height(fx, fz) - 0.1, fz],
            width,
            height: width * 0.75,
            color: [115, 130, 105],
            hanging: false,
        });
    };
    for (index, &(x, z, _)) in STELAE.iter().enumerate() {
        place(x + 0.9 * x.signum(), z - 0.8, 1.5, index as u64);
        place(x - 0.4 * x.signum(), z + 1.0, 1.2, index as u64 + 20);
    }
    for (index, (a, b)) in WALLS.into_iter().enumerate() {
        for step in 0..3 {
            let t = 0.2 + step as f32 * 0.3;
            place(
                a.0 + (b.0 - a.0) * t + 1.0,
                a.1 + (b.1 - a.1) * t,
                1.6,
                40 + index as u64 * 3 + step,
            );
        }
    }
    for step in 0..8 {
        let z = CAUSEWAY.0 + 2.0 + step as f32 * 3.2;
        let x = if step % 2 == 0 {
            -CAUSEWAY_HALF - 0.9
        } else {
            CAUSEWAY_HALF + 0.9
        };
        place(x, z, 1.3, 80 + step);
    }
    let (px, pz) = PLATFORM;
    place(px + 2.6, pz - 1.8, 1.6, 90);
    place(px - 2.4, pz + 2.2, 1.4, 91);
}
