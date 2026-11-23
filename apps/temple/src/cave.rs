use crate::art::Art;
use crate::lighting::Lighting;
use crate::lighting::{Light, LightKind};
use crate::mesh::{steps, Builder, Cell, Material, Vec3, Zone};
use crate::path::{key, Key};
use crate::scene::Billboard;
use crate::terrain::{smoothstep, value_noise, CLIFF_X, FALLS_Z};

pub const MOUTH: (f32, f32) = (FALLS_Z - 2.0, FALLS_Z + 2.0);
pub const MOUTH_TOP: f32 = 6.0;
const MOUTH_X: f32 = CLIFF_X + 0.1;
const TUNNEL_END: f32 = -50.0;
const CHAMBER_X: (f32, f32) = (-62.0, TUNNEL_END);
const CHAMBER_Z: (f32, f32) = (4.0, 24.0);
const CHEST_TOP: f32 = 1.0;
pub const OVERLAY_BIAS: i32 = -16;
pub const IDOL: Vec3 = [-56.0, CHEST_TOP + 0.34, FALLS_Z];
const ROWS: [f32; 5] = [0.0, 2.0, 4.0, MOUTH_TOP, MOUTH_TOP + 2.0];
const CRYSTALS: [(f32, f32, f32); 7] = [
    (-61.0, 7.0, 1.6),
    (-60.8, 21.5, 1.4),
    (-51.2, 5.5, 1.2),
    (-52.0, 22.6, 1.5),
    (-45.5, 12.5, 0.9),
    (-47.5, 15.5, 1.0),
    (-61.2, 14.0, 1.1),
];

pub fn amount(position: Vec3) -> f32 {
    let [x, y, z] = position;
    let within = x < CLIFF_X + 0.5
        && x > CHAMBER_X.0 - 1.0
        && z > CHAMBER_Z.0 - 1.0
        && z < CHAMBER_Z.1 + 1.0
        && y < 10.0;
    if within {
        smoothstep(-0.5, 3.0, CLIFF_X - x)
    } else {
        0.0
    }
}

pub fn keys() -> Vec<Key> {
    let idol = [IDOL[0], IDOL[1] + 0.1, IDOL[2]];
    vec![
        key([-33.5, 1.9, 14.2], [-44.0, 2.0, 14.0], 2.86),
        key([-44.0, 2.0, 14.0], idol, 2.34),
        key([-51.0, 2.2, 14.8], idol, 1.82),
        key([-54.5, 2.6, 18.0], idol, 1.56),
        key([-59.5, 2.8, 15.0], idol, 1.56),
        key([-56.5, 2.4, 10.4], [-48.0, 2.0, 13.0], 1.69),
        key([-50.5, 2.1, 13.4], [-40.0, 2.0, 14.0], 2.08),
        key([-44.0, 2.0, 13.8], [-30.0, 2.5, 15.0], 2.6),
        key([-34.0, 2.2, 14.6], [-24.0, 4.0, 22.0], 3.38),
        key([-29.0, 4.0, 18.0], [-30.0, 9.0, 6.0], 3.0),
    ]
}

pub fn in_mouth(z0: f32, z1: f32, y0: f32, y1: f32) -> bool {
    z0 >= MOUTH.0 && z1 <= MOUTH.1 && y0 >= 0.0 && y1 <= MOUTH_TOP
}

pub fn lights() -> Vec<Light> {
    let mut list: Vec<Light> = CRYSTALS
        .iter()
        .map(|&(x, z, size)| Light {
            position: [x, size * 0.5, z],
            zone: Zone::Cave,
            kind: LightKind::Crystal,
        })
        .collect();
    list.push(Light {
        position: [CLIFF_X - 1.5, 2.5, FALLS_Z],
        zone: Zone::Cave,
        kind: LightKind::Shimmer,
    });
    list
}

pub fn treasure(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    for (index, (dx, dz, size)) in [(-2.2, -2.4, 2.2), (-2.2, 2.4, 2.0), (2.4, 1.8, 1.6)]
        .into_iter()
        .enumerate()
    {
        let position = [IDOL[0] + dx, 0.05, IDOL[2] + dz];
        let lit = lighting.billboard([position[0], 0.8, position[2]], Zone::Cave);
        list.push(Billboard {
            image: art.treasure[(index + 1) % art.treasure.len()],
            position,
            width: size,
            height: size * 0.75,
            color: [0, 1, 2].map(|c| (lit[c] as f32 * [1.7, 1.25, 0.7][c]).min(255.0) as u8),
            hanging: false,
        });
    }
}

pub fn crystals(art: &Art, list: &mut Vec<Billboard>) {
    for &(x, z, size) in &CRYSTALS {
        list.push(Billboard {
            image: art.procedural.crystal,
            position: [x, -0.1, z],
            width: size,
            height: size,
            color: [150, 150, 150],
            hanging: false,
        });
    }
}

fn jitter(position: Vec3) -> Vec3 {
    let [x, y, z] = position;
    let amount = 0.55 * smoothstep(CLIFF_X - 0.5, CLIFF_X - 2.5, x);
    let noise = |seed: u64| value_noise(x / 1.7 + y * 0.31, z / 1.7 - y * 0.27, seed) - 0.5;
    [
        x + noise(81) * 2.0 * amount,
        y + noise(82) * amount,
        z + noise(83) * 2.0 * amount,
    ]
}

fn ceiling(x: f32, z: f32) -> f32 {
    if x > TUNNEL_END {
        MOUTH_TOP
    } else {
        9.0 - 2.2 * value_noise(x / 4.0, z / 4.0, 84)
    }
}

fn level(x: f32, z: f32, row: f32) -> f32 {
    if row > MOUTH_TOP {
        ceiling(x, z)
    } else {
        row
    }
}

fn range(from: f32, to: f32) -> Vec<f32> {
    let count = ((to - from).abs() / 2.0).round().max(1.0) as usize;
    (0..=count)
        .map(|index| from + (to - from) * index as f32 / count as f32)
        .collect()
}

fn surface(
    builder: &mut Builder,
    s_values: &[f32],
    t_values: &[f32],
    at: impl Fn(f32, f32) -> Vec3,
    select: impl Fn(Vec3) -> Option<Material>,
) {
    for sw in s_values.windows(2) {
        for tw in t_values.windows(2) {
            let (s0, s1, t0, t1) = (sw[0], sw[1], tw[0], tw[1]);
            let corners = [at(s0, t1), at(s1, t1), at(s0, t0), at(s1, t0)];
            let middle = [0, 1, 2].map(|c| corners.iter().map(|p| p[c]).sum::<f32>() * 0.25);
            let Some(material) = select(middle) else {
                continue;
            };
            let u0 = (s0 * 16.0).rem_euclid(64.0);
            let u1 = u0 + (s1 - s0).abs() * 16.0;
            let v0 = (t1 * 16.0).rem_euclid(64.0);
            let v1 = v0 + (t1 - t0).abs().min(10.0) * 16.0;
            builder.quad(
                Zone::Cave,
                corners.map(jitter),
                [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
                material,
            );
        }
    }
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    let rock = Material::textured(&art.rockface[1]).tinted([1.3, 1.3, 1.3]);
    let wet = Material::textured(&art.riverbed).tinted([0.7, 0.75, 0.75]);
    let rows = &ROWS;
    let wall = |builder: &mut Builder,
                along: Vec<f32>,
                at: &dyn Fn(f32, f32) -> Vec3,
                skip: &dyn Fn(Vec3) -> bool| {
        surface(builder, &along, rows, at, |middle| {
            (!skip(middle)).then_some(rock)
        });
    };
    let never = |_: Vec3| false;
    let tunnel_rows = &ROWS[..4];
    surface(
        builder,
        &range(TUNNEL_END, MOUTH_X),
        tunnel_rows,
        |x, y| [x, y, MOUTH.1],
        |_| Some(rock),
    );
    surface(
        builder,
        &range(MOUTH_X, TUNNEL_END),
        tunnel_rows,
        |x, y| [x, y, MOUTH.0],
        |_| Some(rock),
    );
    surface(
        builder,
        &range(TUNNEL_END, MOUTH_X),
        &range(MOUTH.0, MOUTH.1),
        |x, z| [x, 0.0, z],
        |_| Some(wet),
    );
    surface(
        builder,
        &range(TUNNEL_END, MOUTH_X),
        &range(MOUTH.1, MOUTH.0),
        |x, z| [x, MOUTH_TOP, z],
        |_| Some(rock),
    );

    let (x0, x1) = CHAMBER_X;
    let (z0, z1) = CHAMBER_Z;
    wall(
        builder,
        range(z1, z0),
        &|z, row| [x1, level(x1, z, row), z],
        &|middle| middle[2] > MOUTH.0 && middle[2] < MOUTH.1 && middle[1] < MOUTH_TOP,
    );
    wall(
        builder,
        range(z0, z1),
        &|z, row| [x0, level(x0, z, row), z],
        &never,
    );
    wall(
        builder,
        range(x0, x1),
        &|x, row| [x, level(x, z1, row), z1],
        &never,
    );
    wall(
        builder,
        range(x1, x0),
        &|x, row| [x, level(x, z0, row), z0],
        &never,
    );
    surface(
        builder,
        &range(x0, x1),
        &range(z0, z1),
        |x, z| [x, 0.0, z],
        |middle| Some(if middle[0] > -54.0 { wet } else { rock }),
    );
    surface(
        builder,
        &range(x0, x1),
        &range(z1, z0),
        |x, z| [x, ceiling(x, z), z],
        |_| Some(rock),
    );

    let chest = Cell::new(Material::textured(&art.stone[2]));
    builder.dressed_block(
        Zone::Cave,
        [IDOL[0] - 1.0, 0.0, IDOL[2] - 1.0],
        [IDOL[0] + 1.0, CHEST_TOP, IDOL[2] + 1.0],
        24.0,
        [chest, Cell::panel(&art.carvings[2], [1.0; 3]), chest, chest],
        Cell::new(Material::textured(&art.coins[2])),
    );
    let gold = Material::textured(&art.coins[0]).biased(OVERLAY_BIAS);
    builder.floor(
        Zone::Cave,
        &steps(IDOL[0] - 3.0, IDOL[0] + 3.0, 2.0),
        &steps(IDOL[2] - 3.0, IDOL[2] + 3.0, 2.0),
        |_, _| 0.32,
        true,
        16.0,
        |middle| {
            let inside_chest =
                (middle[0] - IDOL[0]).abs() < 1.0 && (middle[2] - IDOL[2]).abs() < 1.0;
            (!inside_chest).then(|| Cell::new(gold))
        },
    );
    let relief = x0 + 0.7;
    builder.wall(
        Zone::Cave,
        (relief, FALLS_Z - 2.0),
        (relief, FALLS_Z + 2.0),
        &[1.5, 5.5],
        4.0,
        24.0,
        |_| Some(Cell::panel(&art.carvings[0], [0.95, 1.0, 0.95])),
    );
    for z in [FALLS_Z - 3.4, FALLS_Z + 2.2] {
        builder.wall(
            Zone::Cave,
            (relief, z),
            (relief, z + 1.2),
            &[1.0, 5.0],
            2.0,
            24.0,
            |_| Some(Cell::panel(&art.serpent, [0.9; 3])),
        );
    }
}
