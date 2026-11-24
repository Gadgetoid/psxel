use crate::alien;
use crate::art::Art;
use crate::biome::Rgb;
use crate::lighting::Lighting;
use crate::mesh::{steps, Builder, Cell, Material, Mesh, Vec3, Zone};
use crate::rng::{hash_float, Rng};
use crate::terrain::{normal, reef, seabed, BOUNDS};
use crate::textures::Image;
use crate::wreck;

pub struct Billboard {
    pub image: Image,
    pub position: Vec3,
    pub width: f32,
    pub height: f32,
    pub color: Rgb,
    pub sway: f32,
}

pub struct School {
    pub centre: Vec3,
    pub radius: f32,
    pub count: usize,
    pub species: usize,
    pub speed: f32,
}

pub struct Scene {
    pub mesh: Mesh,
    pub billboards: Vec<Billboard>,
    pub schools: Vec<School>,
}

fn near_wreck(x: f32, z: f32, margin: f32) -> bool {
    let local = wreck::to_local([x, seabed(x, z), z]);
    local[0].abs() < 8.0 + margin && local[2].abs() < 32.0 + margin
}

pub fn build(art: &Art, lighting: &Lighting) -> Scene {
    let mut builder = Builder::new(lighting);
    let materials = |images: &[Image]| images.iter().map(Material::textured).collect::<Vec<_>>();
    let sands = materials(&art.sand);
    let silts = materials(&art.seabed);
    let rocks = materials(&art.rock);
    let corals = materials(&art.coral);
    let (x0, x1, z0, z1) = BOUNDS;
    builder.floor(
        Zone::Exterior,
        &steps(x0, x1, 4.0),
        &steps(z0, z1, 4.0),
        seabed,
        true,
        12.0,
        |middle| {
            let (x, z) = (middle[0], middle[2]);
            if alien::in_cutout(x, z) {
                return None;
            }
            if wreck::under_hull(x, z) {
                return None;
            }
            let steep = normal(x, z)[1] < 0.82;
            let pick = hash_float(3, (x + 500.0) as u64, (z + 500.0) as u64);
            let choose = |list: &[Material]| list[(pick * list.len() as f32) as usize % list.len()];
            let reefy = reef(x, z);
            Some(Cell::new(
                if reefy > 0.35 && !steep && middle[1] > -30.0 && pick > 0.35 {
                    choose(&corals)
                } else if steep || reefy > 0.35 || x > 40.0 {
                    choose(&rocks)
                } else if middle[1] < -45.0 || (pick < 0.25 && middle[1] < -25.0) {
                    choose(&silts)
                } else {
                    choose(&sands)
                },
            ))
        },
    );
    wreck::geometry(&mut builder, art);
    wreck::coin_floor(&mut builder, art);
    alien::geometry(&mut builder, art);
    let mesh = builder.build();

    let mut billboards = Vec::new();
    let mut rng = Rng::new(77);
    let mut place = |image: Image, x: f32, z: f32, size: (f32, f32), sway: f32, tint: [f32; 3]| {
        if alien::in_cutout(x, z) {
            return;
        }
        let base = seabed(x, z) - 0.15;
        let lit = lighting.billboard([x, base + size.1 * 0.5, z], Zone::Exterior);
        billboards.push(Billboard {
            image,
            position: [x, base, z],
            width: size.0,
            height: size.1,
            color: [0, 1, 2].map(|c| (lit[c] as f32 * tint[c]).min(255.0) as u8),
            sway,
        });
    };
    let forests = [
        (12.0, 10.0, 14.0),
        (-8.0, 40.0, 12.0),
        (24.0, 52.0, 10.0),
        (-14.0, 4.0, 9.0),
        (30.0, 22.0, 8.0),
    ];
    for &(cx, cz, radius) in &forests {
        for _ in 0..55 {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let distance = rng.float().sqrt() * radius;
            let (x, z) = (cx + angle.cos() * distance, cz + angle.sin() * distance);
            if near_wreck(x, z, 4.0) || reef(x, z) > 0.2 {
                continue;
            }
            if rng.chance(0.3) {
                let size = rng.range(2.0, 3.6);
                let clump = art.clumps[rng.below(art.clumps.len() as u32) as usize];
                place(
                    clump,
                    x,
                    z,
                    (size, size),
                    rng.range(0.25, 0.5),
                    [1.0, 1.0, 1.0],
                );
            } else {
                let height = rng.range(6.0, 12.0).min(-seabed(x, z) - 2.0);
                let kelp = art.kelp[rng.below(art.kelp.len() as u32) as usize];
                place(
                    kelp,
                    x,
                    z,
                    (height / 3.0, height),
                    rng.range(0.6, 1.2),
                    [1.0, 1.0, 1.0],
                );
            }
        }
    }
    for _ in 0..2600 {
        if rng.chance(0.5) {
            continue;
        }
        let x = rng.range(-34.0, 46.0);
        let z = rng.range(-40.0, 176.0);
        let rocky = reef(x, z) > 0.1 || x > 36.0;
        let near_rock = (0..6).any(|step| {
            let angle = step as f32;
            reef(x + angle.cos() * 6.0, z + angle.sin() * 6.0) > 0.25
        });
        if near_wreck(x, z, 1.0) {
            continue;
        }
        let pick = rng.float();
        if rocky || near_rock {
            if pick < 0.35 {
                let size = rng.range(1.2, 2.6);
                place(
                    art.procedural.fan,
                    x,
                    z,
                    (size, size),
                    0.12,
                    [1.0, 1.0, 1.0],
                );
            } else if pick < 0.55 {
                let size = rng.range(0.8, 1.6);
                place(
                    art.procedural.sponge,
                    x,
                    z,
                    (size * 0.5, size),
                    0.0,
                    [1.0, 1.0, 1.0],
                );
            } else if pick < 0.75 {
                let size = rng.range(0.6, 1.1);
                place(
                    art.procedural.anemone,
                    x,
                    z,
                    (size, size),
                    0.08,
                    [1.0, 1.0, 1.0],
                );
            } else {
                let size = rng.range(1.6, 2.8);
                let image = art.rubble[rng.below(2) as usize];
                place(image, x, z, (size, size * 0.66), 0.0, [1.0, 1.0, 1.0]);
            }
        } else if pick < 0.06 {
            let size = rng.range(0.6, 1.0);
            place(
                art.procedural.anemone,
                x,
                z,
                (size, size),
                0.08,
                [1.0, 1.0, 1.0],
            );
        } else if pick < 0.09 {
            let size = rng.range(1.2, 2.2);
            let image = art.rubble[rng.below(2) as usize];
            place(image, x, z, (size, size * 0.66), 0.0, [1.0, 1.0, 1.0]);
        }
    }
    for index in 0..140 {
        let near = index % 2 == 0;
        let (x, z) = if near {
            let local = [rng.range(-16.0, 16.0), 0.0, rng.range(-38.0, 38.0)];
            let world = wreck::to_world(local);
            (world[0], world[2])
        } else {
            (rng.range(-30.0, 44.0), rng.range(-40.0, 176.0))
        };
        if near_wreck(x, z, 0.5) || reef(x, z) > 0.5 {
            continue;
        }
        let image = art.flotsam[rng.below(art.flotsam.len() as u32) as usize];
        let width = rng.range(0.8, 1.8) * image.width as f32 / 32.0;
        let height = width * image.height as f32 / image.width as f32;
        place(image, x, z, (width, height), 0.0, [1.0, 1.0, 1.0]);
    }
    for _ in 0..14 {
        let (x, z) = (rng.range(-26.0, 40.0), rng.range(-36.0, 120.0));
        let y = rng.range(seabed(x, z) + 3.0, -2.0);
        let image = art.drifting[rng.below(art.drifting.len() as u32) as usize];
        let width = rng.range(0.8, 1.4) * image.width as f32 / 32.0;
        let height = width * image.height as f32 / image.width as f32;
        let lit = lighting.billboard([x, y, z], Zone::Exterior);
        billboards.push(Billboard {
            image,
            position: [x, y, z],
            width,
            height,
            color: lit,
            sway: 0.5,
        });
    }
    wreck::props(art, lighting, &mut billboards);
    wreck::relics(art, lighting, &mut billboards);

    let mut schools = vec![
        School {
            centre: [10.0, -14.0, 12.0],
            radius: 6.0,
            count: 22,
            species: 2,
            speed: 0.6,
        },
        School {
            centre: [-16.0, -22.0, 34.0],
            radius: 5.0,
            count: 18,
            species: 1,
            speed: 0.8,
        },
        School {
            centre: [14.0, -34.0, 76.0],
            radius: 9.0,
            count: 30,
            species: 5,
            speed: 0.45,
        },
        School {
            centre: [-2.0, -38.0, 96.0],
            radius: 4.0,
            count: 10,
            species: 7,
            speed: 0.9,
        },
        School {
            centre: [26.0, -26.0, 50.0],
            radius: 7.0,
            count: 16,
            species: 4,
            speed: 0.5,
        },
        School {
            centre: [-24.0, -12.0, 2.0],
            radius: 3.0,
            count: 8,
            species: 0,
            speed: 0.7,
        },
        School {
            centre: [4.0, -20.0, 30.0],
            radius: 6.0,
            count: 6,
            species: 3,
            speed: 0.4,
        },
        School {
            centre: [-10.0, -44.0, 120.0],
            radius: 10.0,
            count: 6,
            species: 9,
            speed: 0.3,
        },
        School {
            centre: [20.0, -40.0, 100.0],
            radius: 3.0,
            count: 4,
            species: 8,
            speed: 0.25,
        },
        School {
            centre: [10.0, -38.0, 78.0],
            radius: 2.5,
            count: 2,
            species: 10,
            speed: 0.2,
        },
        School {
            centre: [-6.0, -34.0, 70.0],
            radius: 3.0,
            count: 5,
            species: 11,
            speed: 0.35,
        },
        School {
            centre: [-30.0, -18.0, 22.0],
            radius: 4.0,
            count: 7,
            species: 6,
            speed: 0.55,
        },
    ];
    schools.push(School {
        centre: wreck::hold_centre(),
        radius: 2.5,
        count: 9,
        species: 4,
        speed: 0.5,
    });
    Scene {
        mesh,
        billboards,
        schools,
    }
}
