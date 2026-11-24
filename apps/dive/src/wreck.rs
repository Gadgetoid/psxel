use std::f32::consts::{FRAC_PI_2, TAU};

use crate::art::Art;
use crate::lighting::Lighting;
use crate::mesh::{add, length, scale, sub, Builder, Material, Vec3, Zone};
use crate::scene::Billboard;
use crate::terrain::{seabed, smoothstep};

pub const CENTRE: (f32, f32) = (6.0, 84.0);
const YAW: f32 = 0.35;
const LIST: f32 = 0.24;
const LENGTH: f32 = 62.0;
const BEAM: f32 = 7.0;
const DECK: f32 = 3.0;
const KEEL: f32 = -5.0;
const STATIONS: usize = 26;
const ROWS: [f32; 8] = [0.0, 0.18, 0.36, 0.55, 0.7, 0.82, 0.93, 1.0];
const BREACH: (f32, f32, f32, f32) = (0.4, 0.56, 0.15, 0.8);
const DECK_HOLE: (f32, f32, f32) = (0.37, 0.63, 0.72);
const HOLD: (f32, f32) = (0.3, 0.7);
const FLOOR: f32 = KEEL + 0.9;
const THICKNESS: f32 = 0.35;
const CRATES: [(f32, f32, f32, f32, f32); 9] = [
    (0.34, -3.2, 0.7, 0.3, 0.0),
    (0.35, -2.0, 0.55, 1.1, 0.0),
    (0.345, -2.6, 0.5, 0.6, 0.35),
    (0.5, 3.0, 0.8, 0.2, 0.0),
    (0.53, 3.4, 0.6, 0.9, 0.5),
    (0.58, -3.0, 0.75, 0.5, 0.0),
    (0.66, 2.8, 0.6, 1.4, 0.0),
    (0.665, 2.4, 0.5, 0.2, 0.0),
    (0.42, 0.6, 0.6, 0.8, 0.9),
];
const FALLEN_BEAMS: [usize; 2] = [11, 15];
const BOXES: [(f32, f32, f32, f32, f32); 4] = [
    (0.73, 0.55, 1.4, 4.0, 0.0),
    (0.77, 0.32, 1.0, 2.2, 2.8),
    (0.2, 0.5, 1.4, 4.5, 0.0),
    (0.52, 0.16, 0.5, 1.6, 0.0),
];
const RAISED: [(f32, f32, f32); 2] = [(0.88, 1.0, 2.0), (0.0, 0.12, 2.3)];
const BULWARK: f32 = 1.1;
const BULWARK_RANGE: (f32, f32) = (0.12, 0.88);

fn origin() -> Vec3 {
    [CENTRE.0, seabed(CENTRE.0, CENTRE.1) + 4.8, CENTRE.1]
}

pub fn to_world(local: Vec3) -> Vec3 {
    let (sl, cl) = LIST.sin_cos();
    let (sy, cy) = YAW.sin_cos();
    let x = local[0] * cl - local[1] * sl;
    let y = local[0] * sl + local[1] * cl;
    let z = local[2];
    add(origin(), [x * cy + z * sy, y, -x * sy + z * cy])
}

pub fn to_local(world: Vec3) -> Vec3 {
    let offset = sub(world, origin());
    let (sl, cl) = LIST.sin_cos();
    let (sy, cy) = YAW.sin_cos();
    let x = offset[0] * cy - offset[2] * sy;
    let z = offset[0] * sy + offset[2] * cy;
    let y = offset[1];
    [x * cl + y * sl, -x * sl + y * cl, z]
}

fn station_z(s: f32) -> f32 {
    (s - 0.5) * LENGTH
}

fn half_beam(s: f32) -> f32 {
    let bow = ((s - 0.55) / 0.45).max(0.0);
    let stern = ((0.25 - s) / 0.25).max(0.0);
    (BEAM * (1.0 - bow.powf(1.8)).max(0.0).sqrt() * (1.0 - stern.powi(3) * 0.75)).max(0.25)
}

fn bilge_radius(s: f32) -> f32 {
    ((DECK - KEEL) * 0.4).min(half_beam(s) * 0.9)
}

fn section(s: f32, u: f32) -> (f32, f32) {
    let beam = half_beam(s);
    let radius = bilge_radius(s);
    let straight = DECK - KEEL - radius;
    let total = straight + radius * FRAC_PI_2 + (beam - radius);
    let along = u * total;
    if along <= straight {
        (beam, DECK - along)
    } else if along <= straight + radius * FRAC_PI_2 {
        let angle = (along - straight) / radius;
        (
            beam - radius + radius * angle.cos(),
            KEEL + radius - radius * angle.sin(),
        )
    } else {
        let flat = along - straight - radius * FRAC_PI_2;
        ((beam - radius - flat).max(0.0), KEEL)
    }
}

fn width_at(s: f32, y: f32) -> f32 {
    let beam = half_beam(s);
    let radius = bilge_radius(s);
    if y >= KEEL + radius {
        beam
    } else if y > KEEL {
        let dy = KEEL + radius - y;
        beam - radius + (radius * radius - dy * dy).max(0.0).sqrt()
    } else {
        0.0
    }
}

fn in_breach(s: f32, u: f32, side: f32) -> bool {
    side > 0.0 && s > BREACH.0 && s < BREACH.1 && u > BREACH.2 && u < BREACH.3
}

fn in_deck_hole(s: f32, fraction: f32) -> bool {
    s > DECK_HOLE.0 && s < DECK_HOLE.1 && fraction.abs() < DECK_HOLE.2
}

pub fn opening_light(position: Vec3) -> f32 {
    let local = to_local(position);
    let breach = [
        width_at(0.48, 0.0),
        (DECK + KEEL) * 0.5,
        station_z((BREACH.0 + BREACH.1) * 0.5),
    ];
    let hole = [0.0, DECK, station_z(0.5)];
    let reach =
        |point: Vec3, range: f32| (1.0 - length(sub(local, point)) / range).max(0.0).powi(2);
    (reach(breach, 9.0) + reach(hole, 12.0)).min(1.2)
}

fn inset(point: Vec3) -> Vec3 {
    let centre = [0.0, (DECK + KEEL) * 0.5, point[2]];
    let inward = sub(centre, point);
    let distance = length(inward).max(0.01);
    add(
        point,
        scale(inward, THICKNESS.min(distance * 0.5) / distance),
    )
}

fn axis(s: f32) -> Vec3 {
    [0.0, (DECK + KEEL) * 0.5, station_z(s)]
}

fn rusted(material: Material, position: Vec3) -> Material {
    let streak =
        crate::terrain::value_noise(position[0] * 0.4 + position[2] * 0.2, position[1] * 0.5, 31);
    material.tinted([
        1.0 + streak * 0.2,
        0.92 + streak * 0.05,
        0.85 - streak * 0.15,
    ])
}

pub fn geometry(builder: &mut Builder, art: &Art) {
    let lowers: Vec<Material> = art.hull_barnacle.iter().map(Material::textured).collect();
    let uppers: Vec<Material> = art.hull_rust.iter().map(Material::textured).collect();
    let inner = Material::textured(&art.hull_rust[2]).tinted([0.6, 0.6, 0.62]);
    let deck = Material::textured(&art.deck[0]);
    let station = |index: usize| index as f32 / STATIONS as f32;
    for index in 0..STATIONS {
        let (s0, s1) = (station(index), station(index + 1));
        let middle_s = (s0 + s1) * 0.5;
        for side in [1.0f32, -1.0] {
            for rows in ROWS.windows(2) {
                let (u0, u1) = (rows[0], rows[1]);
                if in_breach(middle_s, (u0 + u1) * 0.5, side) {
                    continue;
                }
                let point = |s: f32, u: f32| {
                    let (x, y) = section(s, u);
                    [x * side, y, station_z(s)]
                };
                let corners = [point(s0, u0), point(s1, u0), point(s0, u1), point(s1, u1)];
                let centre = scale(corners.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 0.25);
                let outward = sub(centre, axis(middle_s));
                let span = length(sub(corners[1], corners[0])) * 10.0;
                let height = length(sub(corners[2], corners[0])) * 10.0;
                let uvs = [(0.0, 0.0), (span, 0.0), (0.0, height), (span, height)];
                let pick = (index * 7 + (u0 * 10.0) as usize * 3) % 4;
                let base = if centre[1] < DECK - 3.0 {
                    lowers[pick]
                } else {
                    uppers[pick]
                };
                builder.quad_facing(
                    Zone::Exterior,
                    corners.map(to_world),
                    uvs,
                    rusted(base, to_world(centre)),
                    to_world(add(centre, outward)),
                );
                builder.quad_facing(
                    Zone::Interior,
                    corners.map(|corner| to_world(inset(corner))),
                    uvs,
                    inner,
                    to_world(axis(middle_s)),
                );
            }
        }
        let fractions = [-1.0f32, -0.72, -0.36, 0.0, 0.36, 0.72, 1.0];
        for pair in fractions.windows(2) {
            let (f0, f1) = (pair[0], pair[1]);
            if in_deck_hole(middle_s, (f0 + f1) * 0.5) {
                continue;
            }
            let point = |s: f32, fraction: f32| [half_beam(s) * fraction, DECK, station_z(s)];
            let corners = [point(s0, f0), point(s1, f0), point(s0, f1), point(s1, f1)];
            let centre = scale(corners.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 0.25);
            let uvs = [(0.0, 0.0), (40.0, 0.0), (0.0, 40.0), (40.0, 40.0)];
            builder.quad_facing(
                Zone::Exterior,
                corners.map(to_world),
                uvs,
                Material::textured(&art.deck[(index + (f0 * 3.0 + 3.0) as usize) % art.deck.len()]),
                to_world(add(centre, [0.0, 3.0, 0.0])),
            );
            builder.quad_facing(
                Zone::Interior,
                corners.map(|corner| to_world(add(corner, [0.0, -0.06, 0.0]))),
                uvs,
                inner,
                to_world(add(centre, [0.0, -3.0, 0.0])),
            );
        }
        if (HOLD.0..HOLD.1).contains(&middle_s) {
            let width = |s: f32| width_at(s, FLOOR) - 0.1;
            let corners = [
                [-width(s0), FLOOR, station_z(s0)],
                [-width(s1), FLOOR, station_z(s1)],
                [width(s0), FLOOR, station_z(s0)],
                [width(s1), FLOOR, station_z(s1)],
            ];
            builder.quad_facing(
                Zone::Interior,
                corners.map(to_world),
                [(0.0, 0.0), (30.0, 0.0), (0.0, 60.0), (30.0, 60.0)],
                deck.tinted([0.6, 0.6, 0.6]),
                to_world([0.0, FLOOR + 3.0, station_z(middle_s)]),
            );
        }
    }
    for (s, towards) in [(HOLD.0, 1.0f32), (HOLD.1, -1.0)] {
        let z = station_z(s);
        let width = width_at(s, FLOOR) - 0.1;
        let rows = [FLOOR, (FLOOR + DECK) * 0.5, DECK];
        for pair in rows.windows(2) {
            let (y0, y1) = (pair[0], pair[1]);
            let corners = [
                [-width, y1, z],
                [width, y1, z],
                [-width, y0, z],
                [width, y0, z],
            ];
            builder.quad_facing(
                Zone::Interior,
                corners.map(to_world),
                [(0.0, 0.0), (90.0, 0.0), (0.0, 40.0), (90.0, 40.0)],
                inner,
                to_world([0.0, (y0 + y1) * 0.5, z + towards * 3.0]),
            );
        }
    }
    let panels = Material::textured(&art.hull_rust[1]).tinted([0.9, 0.9, 0.9]);
    for &(s, width, depth, height, base) in &BOXES {
        let centre = [0.0, DECK + base + height * 0.5, station_z(s)];
        oriented_box(
            builder,
            centre,
            [half_beam(s) * width, height * 0.5, depth * 2.0],
            rusted(panels, to_world(centre)),
        );
    }
    portholes(builder, art);
    bulwarks(builder, art);
    breach_edges(builder, art);
    hold_structure(builder, art);
    raised_decks(builder, art);
    coaming(builder, art);
    windows(builder, art);
    name_board(builder, art);
    let funnel = Material::textured(&art.hull_rust[3]).double_sided();
    let base = to_world([-half_beam(0.52) - 3.5, 0.0, station_z(0.5)]);
    let ground = seabed(base[0], base[2]);
    prism(
        builder,
        [base[0], ground + 1.3, base[2] - 3.0],
        [base[0] - 2.0, ground + 1.6, base[2] + 4.0],
        1.5,
        funnel,
    );
    let mast = Material::textured(&art.deck[1]).tinted([0.7, 0.68, 0.64]);
    prism(
        builder,
        to_world([0.0, DECK, station_z(0.86)]),
        to_world([0.4, DECK + 12.0, station_z(0.87)]),
        0.22,
        mast,
    );
    prism(
        builder,
        to_world([-half_beam(0.45), DECK + 0.4, station_z(0.43)]),
        to_world([half_beam(0.55) + 1.0, DECK - 1.0, station_z(0.58)]),
        0.2,
        mast,
    );
}

fn portholes(builder: &mut Builder, art: &Art) {
    for (index, step) in (2..STATIONS - 3).step_by(2).enumerate() {
        let s = (step as f32 + 0.5) / STATIONS as f32;
        for side in [1.0f32, -1.0] {
            if in_breach(s, 0.12, side) {
                continue;
            }
            let image =
                art.portholes[(index + if side > 0.0 { 0 } else { 2 }) % art.portholes.len()];
            let (x, y) = section(s, 0.12);
            let half = 0.55;
            let z = station_z(s);
            let x = (x + 0.05) * side;
            let (u0, v0) = (image.u as f32, image.v as f32);
            let (u1, v1) = (
                u0 + image.width as f32 - 1.0,
                v0 + image.height as f32 - 1.0,
            );
            let corners = [
                [x, y + half, z - half * side],
                [x, y + half, z + half * side],
                [x, y - half, z - half * side],
                [x, y - half, z + half * side],
            ];
            let mut material = Material::textured(&image);
            material.surface = crate::render::Surface::textured(image.texture);
            builder.quad_facing(
                Zone::Exterior,
                corners.map(to_world),
                [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
                material.biased(-16),
                to_world([x * 2.0, y, z]),
            );
        }
    }
}

fn name_board(builder: &mut Builder, art: &Art) {
    let image = art.sign;
    let s = 0.86;
    let (x, top) = section(s, 0.05);
    let (u0, v0) = (image.u as f32, image.v as f32);
    let (u1, v1) = (
        u0 + image.width as f32 - 1.0,
        v0 + image.height as f32 - 1.0,
    );
    for side in [1.0f32, -1.0] {
        let x = (x + 0.06) * side;
        let (z0, z1) = (station_z(s) - 2.4 * side, station_z(s) + 2.4 * side);
        let (y0, y1) = (top - 2.6, top - 0.2);
        let corners = [[x, y1, z0], [x, y1, z1], [x, y0, z0], [x, y0, z1]];
        let mut material = Material::textured(&image).tinted([1.1, 1.1, 1.1]);
        material.surface = crate::render::Surface::textured(image.texture);
        builder.quad_facing(
            Zone::Exterior,
            corners.map(to_world),
            [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
            material.biased(-16),
            to_world([x * 2.0, (y0 + y1) * 0.5, (z0 + z1) * 0.5]),
        );
    }
}

fn bulwark_height(index: usize) -> f32 {
    let roll = crate::rng::hash_float(211, index as u64, 0);
    if roll < 0.12 {
        0.0
    } else if roll < 0.28 {
        BULWARK * 0.45
    } else {
        BULWARK
    }
}

fn bulwarks(builder: &mut Builder, art: &Art) {
    let plating = Material::textured(&art.hull_rust[0]);
    let inner = Material::textured(&art.hull_rust[2]).tinted([0.8, 0.8, 0.8]);
    let steel = Material::textured(&art.hull_rust[3]).tinted([0.7, 0.7, 0.7]);
    for index in 0..STATIONS {
        let (s0, s1) = (
            index as f32 / STATIONS as f32,
            (index + 1) as f32 / STATIONS as f32,
        );
        let middle = (s0 + s1) * 0.5;
        if middle < BULWARK_RANGE.0 || middle > BULWARK_RANGE.1 {
            continue;
        }
        let height = bulwark_height(index);
        if height <= 0.0 {
            continue;
        }
        for side in [1.0f32, -1.0] {
            if side > 0.0 && in_breach(middle, 0.05, side) {
                continue;
            }
            let x = |s: f32, inset: f32| (half_beam(s) - inset) * side;
            let (z0, z1) = (station_z(s0), station_z(s1));
            let top = DECK + height;
            let span = (z1 - z0).abs() * 10.0;
            let uv = [
                (0.0, 0.0),
                (span, 0.0),
                (0.0, height * 10.0),
                (span, height * 10.0),
            ];
            let outer = [
                [x(s0, 0.0), top, z0],
                [x(s1, 0.0), top, z1],
                [x(s0, 0.0), DECK, z0],
                [x(s1, 0.0), DECK, z1],
            ];
            builder.quad_facing(
                Zone::Exterior,
                outer.map(to_world),
                uv,
                plating,
                to_world([side * 20.0, DECK, (z0 + z1) * 0.5]),
            );
            let inside = [
                [x(s0, 0.15), top, z0],
                [x(s1, 0.15), top, z1],
                [x(s0, 0.15), DECK, z0],
                [x(s1, 0.15), DECK, z1],
            ];
            builder.quad_facing(
                Zone::Exterior,
                inside.map(to_world),
                uv,
                inner,
                to_world([0.0, DECK, (z0 + z1) * 0.5]),
            );
            let cap = [
                [x(s0, 0.0), top, z0],
                [x(s1, 0.0), top, z1],
                [x(s0, 0.15), top, z0],
                [x(s1, 0.15), top, z1],
            ];
            builder.quad_facing(
                Zone::Exterior,
                cap.map(to_world),
                [(0.0, 0.0), (span, 0.0), (0.0, 2.0), (span, 2.0)],
                steel,
                to_world([x(middle, 0.07), top + 5.0, (z0 + z1) * 0.5]),
            );
            if height >= BULWARK && index % 2 == 0 {
                let post = [x(s0, 0.08), top + 0.45, z0];
                oriented_box(builder, post, [0.04, 0.45, 0.04], steel);
                let next = (index + 2).min(STATIONS);
                if middle + 2.0 / STATIONS as f32 <= BULWARK_RANGE.1
                    && bulwark_height(index + 1) >= BULWARK
                {
                    let z_end = station_z(next as f32 / STATIONS as f32);
                    let rail = [x(s0, 0.08), top + 0.9, (z0 + z_end) * 0.5];
                    oriented_box(builder, rail, [0.04, 0.04, (z_end - z0).abs() * 0.5], steel);
                }
            }
        }
    }
}

fn raised_decks(builder: &mut Builder, art: &Art) {
    let plating = Material::textured(&art.hull_rust[0]);
    let deck = Material::textured(&art.deck[2]);
    let steel = Material::textured(&art.hull_rust[3]).tinted([0.7, 0.7, 0.7]);
    for &(start, end, height) in &RAISED {
        let steps = 6;
        let top = DECK + height;
        let at = |step: usize| start + (end - start) * step as f32 / steps as f32;
        for step in 0..steps {
            let (s0, s1) = (at(step), at(step + 1));
            let (z0, z1) = (station_z(s0), station_z(s1));
            let (w0, w1) = (half_beam(s0), half_beam(s1));
            let span = (z1 - z0).abs() * 10.0;
            for side in [1.0f32, -1.0] {
                let wall = [
                    [w0 * side, top, z0],
                    [w1 * side, top, z1],
                    [w0 * side, DECK, z0],
                    [w1 * side, DECK, z1],
                ];
                builder.quad_facing(
                    Zone::Exterior,
                    wall.map(to_world),
                    [
                        (0.0, 0.0),
                        (span, 0.0),
                        (0.0, height * 10.0),
                        (span, height * 10.0),
                    ],
                    plating,
                    to_world([side * 20.0, DECK, (z0 + z1) * 0.5]),
                );
                if step % 2 == 0 {
                    oriented_box(
                        builder,
                        [(w0 - 0.08) * side, top + 0.45, z0],
                        [0.04, 0.45, 0.04],
                        steel,
                    );
                }
            }
            let roof = [[-w0, top, z0], [w0, top, z0], [-w1, top, z1], [w1, top, z1]];
            builder.quad_facing(
                Zone::Exterior,
                roof.map(to_world),
                [(0.0, 0.0), (w0 * 20.0, 0.0), (0.0, span), (w1 * 20.0, span)],
                deck,
                to_world([0.0, top + 5.0, (z0 + z1) * 0.5]),
            );
        }
        let inner = if start > 0.5 { start } else { end };
        let z = station_z(inner);
        let w = half_beam(inner);
        let face = [[-w, top, z], [w, top, z], [-w, DECK, z], [w, DECK, z]];
        let towards = if start > 0.5 { z - 5.0 } else { z + 5.0 };
        builder.quad_facing(
            Zone::Exterior,
            face.map(to_world),
            [
                (0.0, 0.0),
                (w * 20.0, 0.0),
                (0.0, height * 10.0),
                (w * 20.0, height * 10.0),
            ],
            plating,
            to_world([0.0, DECK, towards]),
        );
    }
}

fn breach_edges(builder: &mut Builder, art: &Art) {
    let edge = Material::textured(&art.hull_rust[2])
        .tinted([0.75, 0.7, 0.65])
        .double_sided();
    let station = |index: usize| index as f32 / STATIONS as f32;
    let point = |s: f32, u: f32| {
        let (x, y) = section(s, u);
        [x, y, station_z(s)]
    };
    let open = |index: usize, row: usize| {
        if index >= STATIONS || row + 1 >= ROWS.len() {
            return false;
        }
        let s = (station(index) + station(index + 1)) * 0.5;
        in_breach(s, (ROWS[row] + ROWS[row + 1]) * 0.5, 1.0)
    };
    let mut rim = |a: Vec3, b: Vec3| {
        let quad = [a, b, inset(a), inset(b)];
        builder.quad(
            Zone::Exterior,
            quad.map(to_world),
            [(0.0, 0.0), (12.0, 0.0), (0.0, 4.0), (12.0, 4.0)],
            edge,
        );
    };
    for index in 0..STATIONS {
        for row in 0..ROWS.len() - 1 {
            if !open(index, row) {
                continue;
            }
            let (s0, s1) = (station(index), station(index + 1));
            let (u0, u1) = (ROWS[row], ROWS[row + 1]);
            if index == 0 || !open(index - 1, row) {
                rim(point(s0, u0), point(s0, u1));
            }
            if !open(index + 1, row) {
                rim(point(s1, u0), point(s1, u1));
            }
            if row == 0 || !open(index, row - 1) {
                rim(point(s0, u0), point(s1, u0));
            }
            if !open(index, row + 1) {
                rim(point(s0, u1), point(s1, u1));
            }
        }
    }
    let width = half_beam(0.5) * DECK_HOLE.2;
    let (z0, z1) = (station_z(DECK_HOLE.0), station_z(DECK_HOLE.1));
    for (a, b) in [
        ([-width, z0], [width, z0]),
        ([width, z0], [width, z1]),
        ([width, z1], [-width, z1]),
        ([-width, z1], [-width, z0]),
    ] {
        let quad = [
            [a[0], DECK, a[1]],
            [b[0], DECK, b[1]],
            [a[0], DECK - THICKNESS, a[1]],
            [b[0], DECK - THICKNESS, b[1]],
        ];
        builder.quad(
            Zone::Interior,
            quad.map(to_world),
            [(0.0, 0.0), (40.0, 0.0), (0.0, 4.0), (40.0, 4.0)],
            edge,
        );
    }
}

pub fn under_hull(x: f32, z: f32) -> bool {
    let local = to_local([x, origin()[1], z]);
    let s = local[2] / LENGTH + 0.5;
    (0.05..0.95).contains(&s) && local[0].abs() < half_beam(s) - 1.6
}

pub fn hold_centre() -> Vec3 {
    to_world([0.0, FLOOR + 2.2, station_z(0.5)])
}

pub fn shafts() -> Vec<(Vec3, Vec3, f32)> {
    let sun = crate::lighting::sun_direction();
    let mut list = Vec::new();
    for (x, s) in [(-1.6f32, 0.43f32), (1.2, 0.52), (-0.4, 0.6)] {
        let bottom = to_world([x, FLOOR, station_z(s)]);
        let rise = DECK + 0.6 - FLOOR;
        list.push((add(bottom, scale(sun, rise / sun[1])), bottom, 1.1));
    }
    let (bx, by) = section((BREACH.0 + BREACH.1) * 0.5, (BREACH.2 + BREACH.3) * 0.5);
    let opening = to_world([bx, by, station_z((BREACH.0 + BREACH.1) * 0.5)]);
    let inside = to_world([0.5, FLOOR, station_z(0.47)]);
    list.push((opening, inside, 1.4));
    list
}

fn crate_box(
    builder: &mut Builder,
    centre: Vec3,
    half: f32,
    yaw: f32,
    tilt: f32,
    material: Material,
) {
    let (sy, cy) = yaw.sin_cos();
    let (st, ct) = tilt.sin_cos();
    let rotate = |p: Vec3| {
        let (x, y) = (p[0] * ct - p[1] * st, p[0] * st + p[1] * ct);
        add(centre, [x * cy + p[2] * sy, y, -x * sy + p[2] * cy])
    };
    let corner = |x: f32, y: f32, z: f32| rotate([x * half, y * half, z * half]);
    let faces = [
        [
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
        ],
        [
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(-1.0, -1.0, -1.0),
        ],
        [
            corner(1.0, 1.0, 1.0),
            corner(1.0, 1.0, -1.0),
            corner(1.0, -1.0, 1.0),
            corner(1.0, -1.0, -1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(-1.0, -1.0, -1.0),
            corner(-1.0, -1.0, 1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
        ],
    ];
    let size = half * 2.0 * 24.0;
    for face in faces {
        let middle = scale(face.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 0.25);
        builder.quad_facing(
            Zone::Interior,
            face.map(to_world),
            [(0.0, 0.0), (size, 0.0), (0.0, size), (size, size)],
            material,
            to_world(add(middle, sub(middle, centre))),
        );
    }
}

fn hold_structure(builder: &mut Builder, art: &Art) {
    let frame = Material::textured(&art.hull_rust[1])
        .tinted([0.7, 0.68, 0.66])
        .double_sided();
    let beam = Material::textured(&art.deck[3]).tinted([0.7, 0.66, 0.6]);
    let wood = Material::textured(&art.deck[1]).tinted([0.95, 0.85, 0.7]);
    let station = |index: usize| index as f32 / STATIONS as f32;
    for index in 0..=STATIONS {
        let s = station(index);
        if !(HOLD.0 + 0.01..HOLD.1 - 0.01).contains(&s) {
            continue;
        }
        let z = station_z(s);
        for side in [1.0f32, -1.0] {
            for pair in ROWS.windows(2) {
                let (u0, u1) = (pair[0], pair[1]);
                if side > 0.0 && in_breach(s, (u0 + u1) * 0.5, side) {
                    continue;
                }
                let at = |u: f32, depth: f32| {
                    let (x, y) = section(s, u);
                    let base = [x * side, y, z];
                    let inner = inset(base);
                    let inward = sub(inner, base);
                    add(inner, scale(inward, depth / THICKNESS))
                };
                for offset in [-0.08f32, 0.08] {
                    let shift = |p: Vec3| [p[0], p[1], p[2] + offset];
                    let quad = [
                        shift(at(u0, 0.0)),
                        shift(at(u1, 0.0)),
                        shift(at(u0, 0.3)),
                        shift(at(u1, 0.3)),
                    ];
                    builder.quad(
                        Zone::Interior,
                        quad.map(to_world),
                        [(0.0, 0.0), (6.0, 0.0), (0.0, 24.0), (6.0, 24.0)],
                        frame,
                    );
                }
                let face = [
                    [at(u0, 0.3)[0], at(u0, 0.3)[1], z - 0.08],
                    [at(u0, 0.3)[0], at(u0, 0.3)[1], z + 0.08],
                    [at(u1, 0.3)[0], at(u1, 0.3)[1], z - 0.08],
                    [at(u1, 0.3)[0], at(u1, 0.3)[1], z + 0.08],
                ];
                builder.quad_facing(
                    Zone::Interior,
                    face.map(to_world),
                    [(0.0, 0.0), (4.0, 0.0), (0.0, 24.0), (4.0, 24.0)],
                    frame,
                    to_world([0.0, (DECK + KEEL) * 0.5, z]),
                );
            }
        }
        if index % 2 == 0 {
            let width = half_beam(s) - THICKNESS - 0.1;
            let y = DECK - THICKNESS - 0.18;
            if FALLEN_BEAMS.contains(&index) {
                let low = [-width * 0.2, FLOOR + 0.6, z + 0.4];
                let high = [width, y, z];
                let middle = scale(add(low, high), 0.5);
                let tilt = (high[1] - low[1]).atan2(high[0] - low[0]);
                let length = length(sub(high, low)) * 0.5;
                beam_box(builder, middle, [length, 0.16, 0.16], tilt, beam);
            } else {
                beam_box(builder, [0.0, y, z], [width, 0.16, 0.16], 0.0, beam);
            }
        }
    }
    for &(s, x, half, yaw, tilt) in &CRATES {
        crate_box(
            builder,
            [x, FLOOR + half * 0.98, station_z(s)],
            half,
            yaw,
            tilt,
            wood,
        );
    }
}

fn beam_box(builder: &mut Builder, centre: Vec3, half: Vec3, tilt: f32, material: Material) {
    let (st, ct) = tilt.sin_cos();
    let rotate = |p: Vec3| add(centre, [p[0] * ct - p[1] * st, p[0] * st + p[1] * ct, p[2]]);
    let corner = |x: f32, y: f32, z: f32| rotate([x * half[0], y * half[1], z * half[2]]);
    let faces = [
        [
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
        ],
        [
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(-1.0, -1.0, -1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
        ],
        [
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
            corner(-1.0, -1.0, -1.0),
            corner(1.0, -1.0, -1.0),
        ],
    ];
    let span = half[0] * 2.0 * 16.0;
    for face in faces {
        let middle = scale(face.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 0.25);
        builder.quad_facing(
            Zone::Interior,
            face.map(to_world),
            [
                (0.0, 0.0),
                (span.min(250.0), 0.0),
                (0.0, 8.0),
                (span.min(250.0), 8.0),
            ],
            material,
            to_world(add(middle, sub(middle, centre))),
        );
    }
}

fn coaming(builder: &mut Builder, art: &Art) {
    let steel = Material::textured(&art.hull_rust[1]).tinted([0.85, 0.85, 0.85]);
    let (s0, s1) = (DECK_HOLE.0, DECK_HOLE.1);
    let width = half_beam(0.5) * DECK_HOLE.2;
    let (z0, z1) = (station_z(s0), station_z(s1));
    let height = 0.6;
    let sides = [
        ([-width, z0], [width, z0]),
        ([width, z0], [width, z1]),
        ([width, z1], [-width, z1]),
        ([-width, z1], [-width, z0]),
    ];
    for (a, b) in sides {
        let middle = [(a[0] + b[0]) * 0.5, DECK, (a[1] + b[1]) * 0.5];
        let span = (b[0] - a[0]).hypot(b[1] - a[1]) * 10.0;
        let uv = [(0.0, 0.0), (span, 0.0), (0.0, 6.0), (span, 6.0)];
        let quad = [
            [a[0], DECK + height, a[1]],
            [b[0], DECK + height, b[1]],
            [a[0], DECK, a[1]],
            [b[0], DECK, b[1]],
        ];
        let outward = [
            middle[0] * 2.0,
            DECK,
            (middle[2] - station_z(0.5)) * 2.0 + station_z(0.5),
        ];
        builder.quad_facing(
            Zone::Exterior,
            quad.map(to_world),
            uv,
            steel,
            to_world(outward),
        );
        let inward = [0.0, DECK, station_z(0.5)];
        builder.quad_facing(
            Zone::Interior,
            quad.map(to_world),
            uv,
            steel.tinted([0.6, 0.6, 0.6]),
            to_world(inward),
        );
        let (ia, ib) = ([a[0] * 1.06, a[1]], [b[0] * 1.06, b[1]]);
        let cap = [
            [a[0], DECK + height, a[1]],
            [b[0], DECK + height, b[1]],
            [ia[0], DECK + height, ia[1]],
            [ib[0], DECK + height, ib[1]],
        ];
        builder.quad_facing(
            Zone::Exterior,
            cap.map(to_world),
            [(0.0, 0.0), (span, 0.0), (0.0, 2.0), (span, 2.0)],
            steel,
            to_world([middle[0], DECK + 5.0, middle[2]]),
        );
    }
}

fn windows(builder: &mut Builder, art: &Art) {
    for (box_index, &(s, width, depth, height, base)) in BOXES.iter().enumerate() {
        if height < 2.0 || width < 0.3 {
            continue;
        }
        let half = [half_beam(s) * width, height * 0.5, depth * 2.0];
        let y = DECK + base + height * 0.62;
        let count = (half[2] * 2.0 / 1.4).floor() as usize;
        for side in [1.0f32, -1.0] {
            for slot in 0..count {
                let z = station_z(s) - half[2] + (slot as f32 + 0.5) * half[2] * 2.0 / count as f32;
                let image = art.portholes[(box_index + slot) % art.portholes.len()];
                let x = (half[0] + 0.04) * side;
                let size = 0.32;
                let (u0, v0) = (image.u as f32, image.v as f32);
                let (u1, v1) = (
                    u0 + image.width as f32 - 1.0,
                    v0 + image.height as f32 - 1.0,
                );
                let quad = [
                    [x, y + size, z - size * side],
                    [x, y + size, z + size * side],
                    [x, y - size, z - size * side],
                    [x, y - size, z + size * side],
                ];
                let mut material = Material::textured(&image);
                material.surface = crate::render::Surface::textured(image.texture);
                builder.quad_facing(
                    Zone::Exterior,
                    quad.map(to_world),
                    [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
                    material.biased(-16),
                    to_world([x * 3.0, y, z]),
                );
            }
        }
    }
}

fn oriented_box(builder: &mut Builder, centre: Vec3, half: Vec3, material: Material) {
    let corner = |x: f32, y: f32, z: f32| add(centre, [x * half[0], y * half[1], z * half[2]]);
    let faces = [
        [
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(-1.0, -1.0, 1.0),
            corner(1.0, -1.0, 1.0),
        ],
        [
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, -1.0),
            corner(1.0, -1.0, -1.0),
            corner(-1.0, -1.0, -1.0),
        ],
        [
            corner(1.0, 1.0, 1.0),
            corner(1.0, 1.0, -1.0),
            corner(1.0, -1.0, 1.0),
            corner(1.0, -1.0, -1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(-1.0, -1.0, -1.0),
            corner(-1.0, -1.0, 1.0),
        ],
        [
            corner(-1.0, 1.0, -1.0),
            corner(1.0, 1.0, -1.0),
            corner(-1.0, 1.0, 1.0),
            corner(1.0, 1.0, 1.0),
        ],
    ];
    for face in faces {
        let middle = scale(face.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 0.25);
        let outward = add(middle, sub(middle, centre));
        let span = length(sub(face[1], face[0])) * 12.0;
        let height = length(sub(face[2], face[0])) * 12.0;
        builder.quad_facing(
            Zone::Exterior,
            face.map(to_world),
            [(0.0, 0.0), (span, 0.0), (0.0, height), (span, height)],
            material,
            to_world(outward),
        );
    }
}

fn prism(builder: &mut Builder, start: Vec3, end: Vec3, radius: f32, material: Material) {
    let axis = sub(end, start);
    let along = scale(axis, 1.0 / length(axis).max(0.01));
    let reference = if along[1].abs() > 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let across = crate::mesh::normalize(crate::mesh::cross(along, reference));
    let up = crate::mesh::cross(across, along);
    let ring = |centre: Vec3, side: usize| {
        let angle = side as f32 / 8.0 * TAU;
        add(
            centre,
            add(
                scale(across, angle.cos() * radius),
                scale(up, angle.sin() * radius),
            ),
        )
    };
    let span = (length(axis) * 10.0).min(190.0);
    for side in 0..8 {
        let next = (side + 1) % 8;
        let corners = [
            ring(start, side),
            ring(end, side),
            ring(start, next),
            ring(end, next),
        ];
        let middle = scale(
            add(add(corners[0], corners[1]), add(corners[2], corners[3])),
            0.25,
        );
        let centre = add(start, scale(axis, 0.5));
        builder.quad_facing(
            Zone::Exterior,
            corners,
            [(0.0, 0.0), (span, 0.0), (0.0, 24.0), (span, 24.0)],
            material,
            add(middle, sub(middle, centre)),
        );
    }
}

pub fn collide(position: Vec3, radius: f32) -> Vec3 {
    let mut local = to_local(position);
    let s = local[2] / LENGTH + 0.5;
    if !(-0.02..=1.02).contains(&s) {
        return position;
    }
    let s = s.clamp(0.0, 1.0);
    let width = width_at(s, local[1].min(DECK));
    let inside = local[0].abs() < width;
    if inside && (local[1] - DECK).abs() < radius {
        let fraction = local[0] / half_beam(s).max(0.1);
        if !in_deck_hole(s, fraction) {
            local[1] = if local[1] > DECK {
                DECK + radius
            } else {
                DECK - radius
            };
        }
    }
    if local[1] > KEEL && local[1] < DECK + BULWARK + 0.2 {
        let distance = local[0].abs() - (width - THICKNESS * 0.5);
        if distance.abs() < radius {
            let side = local[0].signum();
            let u = row_of(s, local[1]);
            if !in_breach(s, u, side) {
                let outside = distance > 0.0;
                local[0] = side
                    * if outside {
                        width + radius
                    } else {
                        width - radius
                    };
            }
        }
    }
    if inside && (HOLD.0..HOLD.1).contains(&s) && local[1] < FLOOR + radius && local[1] > KEEL - 1.0
    {
        local[1] = FLOOR + radius;
    }
    if inside && local[1] < DECK {
        for wall in [HOLD.0, HOLD.1] {
            let z = station_z(wall);
            if (local[2] - z).abs() < radius {
                local[2] = z + (local[2] - z).signum() * radius;
            }
        }
    }
    for &(s, x, half, _, _) in &CRATES {
        let centre = [x, FLOOR + half, station_z(s)];
        let offset = sub(local, centre);
        let reach = half + radius;
        if (0..3).all(|axis| offset[axis].abs() < reach) {
            let push = (0..3)
                .min_by(|&a, &b| (reach - offset[a].abs()).total_cmp(&(reach - offset[b].abs())))
                .unwrap();
            local[push] = centre[push] + offset[push].signum() * reach;
        }
    }
    for &(start, end, height) in &RAISED {
        let s = local[2] / LENGTH + 0.5;
        let top = DECK + height;
        if (start..end).contains(&s)
            && local[0].abs() < half_beam(s.clamp(0.0, 1.0))
            && local[1] < top + radius
            && local[1] > DECK - 0.5
        {
            local[1] = top + radius;
        }
    }
    for &(box_s, width, depth, height, base) in &BOXES {
        let centre = [0.0, DECK + base + height * 0.5, station_z(box_s)];
        let half = [
            half_beam(box_s) * width + radius,
            height * 0.5 + radius,
            depth * 2.0 + radius,
        ];
        let offset = sub(local, centre);
        if (0..3).all(|axis| offset[axis].abs() < half[axis]) {
            let push = (0..3)
                .min_by(|&a, &b| {
                    (half[a] - offset[a].abs()).total_cmp(&(half[b] - offset[b].abs()))
                })
                .unwrap();
            local[push] = centre[push] + offset[push].signum() * half[push];
        }
    }
    to_world(local)
}

fn row_of(s: f32, y: f32) -> f32 {
    let mut best = (0.0, f32::MAX);
    for step in 0..=40 {
        let u = step as f32 / 40.0;
        let distance = (section(s, u).1 - y).abs();
        if distance < best.1 {
            best = (u, distance);
        }
    }
    best.0
}

pub fn inside(position: Vec3) -> f32 {
    let local = to_local(position);
    let s = local[2] / LENGTH + 0.5;
    if !(HOLD.0..HOLD.1).contains(&s) || local[1] > DECK || local[1] < KEEL {
        return 0.0;
    }
    let width = width_at(s, local[1]);
    smoothstep(width + 0.5, width - 1.5, local[0].abs())
        * smoothstep(DECK + 0.5, DECK - 1.5, local[1])
}

pub fn props(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    for (index, (x, s, size)) in [
        (-2.0, 0.42, 2.2),
        (2.2, 0.47, 1.8),
        (-1.0, 0.6, 2.0),
        (2.6, 0.64, 1.5),
    ]
    .into_iter()
    .enumerate()
    {
        let position = to_world([x, FLOOR, station_z(s)]);
        list.push(Billboard {
            image: art.treasure[index % art.treasure.len()],
            position,
            width: size,
            height: size * 0.75,
            color: lighting.billboard(add(position, [0.0, 0.6, 0.0]), Zone::Interior),
            sway: 0.0,
        });
    }
}

pub fn relics(art: &Art, lighting: &Lighting, list: &mut Vec<Billboard>) {
    let mut place = |image: crate::textures::Image, position: Vec3, size: f32, zone: Zone| {
        let ratio = image.height as f32 / image.width as f32;
        list.push(Billboard {
            image,
            position,
            width: size,
            height: size * ratio,
            color: lighting.billboard(add(position, [0.0, size * 0.4, 0.0]), zone),
            sway: 0.0,
        });
    };
    let ground = |x: f32, z: f32| [x, seabed(x, z) - 0.1, z];
    let near = |dx: f32, dz: f32| {
        let base = to_world([dx, 0.0, dz]);
        ground(base[0], base[2])
    };
    place(art.relics[0], near(-9.5, 26.0), 2.6, Zone::Exterior);
    place(art.relics[1], near(10.5, -10.0), 1.8, Zone::Exterior);
    place(art.relics[2], near(11.0, 6.0), 1.4, Zone::Exterior);
    place(art.relics[3], near(-10.5, -18.0), 1.4, Zone::Exterior);
    place(art.relics[4], near(12.5, 14.0), 2.2, Zone::Exterior);
    place(
        art.relics[5],
        to_world([1.5, FLOOR, station_z(0.36)]),
        1.4,
        Zone::Interior,
    );
    place(art.plaques[0], near(-9.0, -26.0), 1.8, Zone::Exterior);
    place(
        art.plaques[1],
        to_world([-2.5, FLOOR, station_z(0.66)]),
        1.1,
        Zone::Interior,
    );
    for (index, (dx, dz, size)) in [
        (11.0, -2.0, 2.4),
        (12.0, 3.0, 1.8),
        (9.5, 9.0, 2.2),
        (-9.5, 12.0, 2.0),
        (-11.0, -6.0, 1.6),
        (13.0, -14.0, 2.0),
        (-8.5, 30.0, 1.6),
    ]
    .into_iter()
    .enumerate()
    {
        place(
            art.rubble[2 + index % 4],
            near(dx, dz),
            size,
            Zone::Exterior,
        );
    }
}

pub fn coin_floor(builder: &mut Builder, art: &Art) {
    let gold = Material::textured(&art.coins[0]).biased(-16);
    for (x, s) in [(-1.5f32, 0.44f32), (1.8, 0.62)] {
        let z = station_z(s);
        let corners = [
            [x - 2.0, FLOOR + 0.04, z + 2.0],
            [x + 2.0, FLOOR + 0.04, z + 2.0],
            [x - 2.0, FLOOR + 0.04, z - 2.0],
            [x + 2.0, FLOOR + 0.04, z - 2.0],
        ];
        builder.quad_facing(
            Zone::Interior,
            corners.map(to_world),
            [(0.0, 0.0), (63.0, 0.0), (0.0, 63.0), (63.0, 63.0)],
            gold,
            to_world([x, FLOOR + 3.0, z]),
        );
    }
}
