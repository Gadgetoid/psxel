use psxel_math::Mat3Fx12;
use psxel_prim::BlendMode;

use crate::art::Art;
use crate::biome::Rgb;
use crate::cave;
use crate::grotto;
use crate::lighting::LightKind;
use crate::mesh::{
    add, cross, dot, length, local, normalize, scale, steps, sub, tiled, Vec3, View,
};
use crate::render::{world_to_psx, Point, Renderer, Surface, OT_SIZE};
use crate::rng::hash_float;
use crate::scene::{Billboard, Scene, Shaft, ALTAR_TOP, SHRINE_ROOF};
use crate::secrets;
use crate::terrain::{CLIFF_X, FALLS_HALF_WIDTH, FALLS_Z, POOL_CENTER, RIVER_Y, TEMPLE_Z, WATER_Y};
use crate::textures::Image;

const BLACK: Rgb = [0, 0, 0];
const HANGING_BIAS: i32 = -48;
const STANDING_BIAS: i32 = -8;
const SURFACE_BIAS: i32 = -12;
const GOLD: Vec3 = [235.0, 178.0, 62.0];
const JADE: Vec3 = [70.0, 200.0, 130.0];
const FALLS_PROFILE: [(f32, f32); 7] = [
    (CLIFF_X, RIVER_Y),
    (-39.3, 15.9),
    (-38.9, 15.0),
    (-38.7, 13.0),
    (-38.6, 9.0),
    (-38.5, 4.5),
    (-38.5, WATER_Y),
];
const IDOL: Vec3 = [0.0, ALTAR_TOP + 0.34, TEMPLE_Z];

#[derive(Clone, Copy)]
enum Anchor {
    Base,
    Top,
    Centre,
}

fn set_origin(renderer: &mut Renderer, origin: Vec3) {
    renderer.set_object(
        world_to_psx(origin[0], origin[1], origin[2]),
        Mat3Fx12::IDENTITY,
    );
}

fn scaled(color: Rgb, amount: f32) -> Rgb {
    color.map(|channel| (channel as f32 * amount).clamp(0.0, 255.0) as u8)
}

#[allow(clippy::too_many_arguments)]
fn billboard(
    renderer: &mut Renderer,
    view: &View,
    image: &Image,
    anchor: Vec3,
    size: (f32, f32),
    color: Rgb,
    surface: Surface,
    placement: Anchor,
) {
    set_origin(renderer, anchor);
    let half = size.0 * 0.5;
    let right = [view.right.0 * half, 0.0, view.right.1 * half];
    let (top, bottom) = match placement {
        Anchor::Base => (size.1, 0.0),
        Anchor::Top => (0.0, -size.1),
        Anchor::Centre => (size.1 * 0.5, -size.1 * 0.5),
    };
    let corner =
        |side: f32, height: f32| local([right[0] * side, height, right[2] * side], [0.0; 3]);
    let (u0, u1) = (image.u, image.u.wrapping_add(image.width.saturating_sub(1)));
    let (v0, v1) = (image.v, image.v.wrapping_add(image.height).wrapping_sub(1));
    let points = [
        Point::new(corner(-1.0, top), color, (u0, v0)),
        Point::new(corner(1.0, top), color, (u1, v0)),
        Point::new(corner(-1.0, bottom), color, (u0, v1)),
        Point::new(corner(1.0, bottom), color, (u1, v1)),
    ];
    let middle = local([0.0, (top + bottom) * 0.5, 0.0], [0.0; 3]);
    renderer.force_otz = Some(renderer.depth_of(middle) as u32);
    let bias = match placement {
        Anchor::Top => HANGING_BIAS,
        _ => STANDING_BIAS,
    };
    renderer.polygon(&points, surface, bias, 1);
    renderer.force_otz = None;
}

fn additive(image: &Image) -> Surface {
    Surface::textured(image.texture)
        .blended(BlendMode::Add)
        .tinted(BLACK)
}

pub fn draw(renderer: &mut Renderer, view: &View, art: &Art, scene: &Scene, flicker: &[f32]) {
    horizon(renderer, view, art);
    billboards(renderer, view, &scene.billboards);
    lights(renderer, view, art, scene, flicker);
    waterfall(renderer, view, art);
    indoor_pool(renderer, view, art);
    cascades(renderer, view, art);
    pool(renderer, view, art, &scene.pool);
    mist(renderer, view, art);
    shafts(renderer, view, &scene.shafts);
    skylight(renderer, view);
    motes(renderer, view, art);
    idol(renderer, view, art, IDOL, GOLD, 1.0);
    idol(renderer, view, art, cave::IDOL, GOLD, 1.0);
    idol(renderer, view, art, secrets::pool_idol(), GOLD, 2.2);
    idol(renderer, view, art, secrets::jade_idol(), JADE, 1.6);
    idol(renderer, view, art, grotto::IDOL, GOLD, 1.3);
    drips(renderer, view, art);
    glints(
        renderer,
        view,
        art,
        [cave::IDOL[0], 0.55, cave::IDOL[2]],
        3.0,
        57,
    );
    glints(
        renderer,
        view,
        art,
        [cave::IDOL[0], 1.15, cave::IDOL[2]],
        0.8,
        59,
    );
    glints(renderer, view, art, secrets::pool_idol(), 1.6, 58);
    glints(renderer, view, art, grotto::IDOL, 2.6, 60);
    jade_glow(renderer, view, art);
}

fn horizon(renderer: &mut Renderer, view: &View, art: &Art) {
    const SEGMENTS: usize = 16;
    const REPEATS: usize = 2;
    const RADIUS: f32 = 220.0;
    let image = &art.horizon;
    let haze = [0, 1, 2].map(|c| (view.fog[c] as f32 * 0.6 + 100.0) as u8);
    if view.fog[0] < 40 {
        return;
    }
    renderer.set_sky_object(Mat3Fx12::IDENTITY);
    renderer.force_bucket = Some(OT_SIZE - 1);
    renderer.fog_enabled = false;
    let per_segment = 256 / (SEGMENTS / REPEATS);
    let (v0, v1) = (image.v, image.v.wrapping_add(image.height).wrapping_sub(1));
    let (top, bottom) = (-RADIUS * 0.3, RADIUS * 0.02 + view.position[1] * 0.5);
    for segment in 0..SEGMENTS {
        let angle = |index: usize| index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (a0, a1) = (angle(segment), angle(segment + 1));
        let corner = |angle: f32, height: f32| {
            local(
                [angle.sin() * RADIUS, -height, angle.cos() * RADIUS],
                [0.0; 3],
            )
        };
        let u0 = image.u as usize + (segment % (SEGMENTS / REPEATS)) * per_segment;
        let u1 = (u0 + per_segment).min(image.u as usize + image.width as usize) as u8;
        let u0 = u0 as u8;
        let points = [
            Point::new(corner(a0, top), haze, (u0, v0)),
            Point::new(corner(a1, top), haze, (u1, v0)),
            Point::new(corner(a0, bottom), haze, (u0, v1)),
            Point::new(corner(a1, bottom), haze, (u1, v1)),
        ];
        renderer.polygon(&points, Surface::textured(image.texture), 0, 0);
    }
    renderer.fog_enabled = true;
    renderer.force_bucket = None;
}

fn billboards(renderer: &mut Renderer, view: &View, list: &[Billboard]) {
    for item in list {
        let offset = if item.hanging { -0.5 } else { 0.5 } * item.height;
        let centre = [
            item.position[0],
            item.position[1] + offset,
            item.position[2],
        ];
        if !view.visible(centre, item.height) {
            continue;
        }
        let surface = Surface::textured(item.image.texture).tinted(view.fog);
        let placement = if item.hanging {
            Anchor::Top
        } else {
            Anchor::Base
        };
        billboard(
            renderer,
            view,
            &item.image,
            item.position,
            (item.width, item.height),
            item.color,
            surface,
            placement,
        );
    }
}

fn lights(renderer: &mut Renderer, view: &View, art: &Art, scene: &Scene, flicker: &[f32]) {
    for (index, light) in scene.lighting.lights.iter().enumerate() {
        if !view.visible(light.position, 2.0) {
            continue;
        }
        let strength = flicker[index];
        match light.kind {
            LightKind::Torch => {
                let frame = ((view.time * 14.0) as usize + index * 5) % art.procedural.flames.len();
                let flame = &art.procedural.flames[frame];
                billboard(
                    renderer,
                    view,
                    flame,
                    light.position,
                    (0.4, 0.8),
                    scaled([128, 128, 128], strength),
                    additive(flame),
                    Anchor::Base,
                );
                let glow = &art.procedural.glow;
                let centre = [
                    light.position[0],
                    light.position[1] + 0.35,
                    light.position[2],
                ];
                billboard(
                    renderer,
                    view,
                    glow,
                    centre,
                    (2.6, 2.6),
                    scaled([70, 46, 26], strength),
                    additive(glow),
                    Anchor::Centre,
                );
            }
            LightKind::Crystal => {
                let glow = &art.procedural.cold_glow;
                billboard(
                    renderer,
                    view,
                    glow,
                    light.position,
                    (3.2, 3.2),
                    scaled([60, 70, 72], strength),
                    additive(glow),
                    Anchor::Centre,
                );
            }
            LightKind::Shimmer => {}
        }
    }
}

fn drips(renderer: &mut Renderer, view: &View, art: &Art) {
    let image = &art.procedural.mote;
    let spots: [(f32, f32, f32); 8] = [
        (-43.0, 13.2, 6.0),
        (-46.5, 15.1, 6.0),
        (-52.5, 9.0, 8.2),
        (-58.0, 19.0, 7.6),
        (-55.0, 6.5, 7.9),
        (-60.0, 11.0, 7.4),
        (-53.0, 20.5, 8.0),
        (-57.5, 16.8, 7.5),
    ];
    if !view.visible([-52.0, 4.0, 14.0], 14.0) || cave::amount(view.position) <= 0.0 {
        return;
    }
    for (index, &(x, z, top)) in spots.iter().enumerate() {
        let phase = (view.time * 0.9 + index as f32 * 0.37).fract();
        let fall = phase.powi(2);
        if fall < 1.0 && phase < 0.85 {
            let y = top - fall / 0.7225 * top;
            billboard(
                renderer,
                view,
                image,
                [x, y.max(0.05), z],
                (0.05, 0.22),
                [70, 90, 100],
                additive(image),
                Anchor::Centre,
            );
        } else {
            let splash = 1.0 - (phase - 0.85) / 0.15;
            billboard(
                renderer,
                view,
                &art.procedural.cold_glow,
                [x, 0.08, z],
                (0.5, 0.25),
                scaled([60, 80, 90], splash),
                additive(&art.procedural.cold_glow),
                Anchor::Centre,
            );
        }
    }
}

fn glints(renderer: &mut Renderer, view: &View, art: &Art, centre: Vec3, spread: f32, seed: u64) {
    if !view.visible(centre, spread + 1.0) {
        return;
    }
    let glow = &art.procedural.glow;
    for index in 0..7u64 {
        let random = |salt: u64| hash_float(seed, index, salt);
        let sparkle = ((view.time * (0.9 + random(1)) + random(2) * 6.0)
            .sin()
            .max(0.0))
        .powi(12);
        if sparkle < 0.05 {
            continue;
        }
        let position = [
            centre[0] - spread + random(3) * spread * 2.0,
            centre[1] - 0.2 + random(5) * 0.3,
            centre[2] - spread + random(4) * spread * 2.0,
        ];
        billboard(
            renderer,
            view,
            glow,
            position,
            (0.35, 0.35),
            scaled([255, 220, 140], sparkle),
            additive(glow),
            Anchor::Centre,
        );
    }
}

fn jade_glow(renderer: &mut Renderer, view: &View, art: &Art) {
    let centre = secrets::jade_idol();
    if !view.visible(centre, 2.0) {
        return;
    }
    let pulse = 0.7 + 0.3 * (view.time * 1.1).sin();
    let glow = &art.procedural.cold_glow;
    billboard(
        renderer,
        view,
        glow,
        centre,
        (2.4, 2.4),
        scaled([40, 90, 60], pulse),
        additive(glow),
        Anchor::Centre,
    );
}

fn waterfall(renderer: &mut Renderer, view: &View, art: &Art) {
    let origin = [-39.0, 8.0, FALLS_Z];
    if !view.visible(origin, 12.0) {
        return;
    }
    set_origin(renderer, origin);
    let image = tiled(&art.procedural.waterfall);
    let zs = [
        FALLS_Z - FALLS_HALF_WIDTH,
        FALLS_Z - 0.8,
        FALLS_Z + 0.8,
        FALLS_Z + FALLS_HALF_WIDTH,
    ];
    let layers = [
        (0.0, 1.0, BlendMode::Half, [140, 150, 155], view.fog),
        (0.3, 1.7, BlendMode::Add, [40, 44, 48], BLACK),
    ];
    let density = 20.0;
    for (layer, (offset, speed, blend, color, tint)) in layers.into_iter().enumerate() {
        let surface = Surface::textured(image.texture).blended(blend).tinted(tint);
        let scroll = view.time * speed * 3.5 * density;
        let mut travelled = 0.0;
        for segment in FALLS_PROFILE.windows(2) {
            let ((xa, ya), (xb, yb)) = (segment[0], segment[1]);
            let span = (xb - xa).hypot(yb - ya);
            let v0 = (travelled * density - scroll).rem_euclid(64.0);
            let v1 = v0 + span * density;
            travelled += span;
            for zw in zs.windows(2) {
                let u0 = (zw[0] * 14.0 + layer as f32 * 21.0).rem_euclid(64.0);
                let u1 = u0 + (zw[1] - zw[0]) * 14.0;
                let at = |x: f32, y: f32, z: f32| local([x + offset, y, z], origin);
                let uv = |u: f32, v: f32| (u.round() as u8, v.round().min(255.0) as u8);
                let points = [
                    Point::new(at(xa, ya, zw[0]), color, uv(u0, v0)),
                    Point::new(at(xa, ya, zw[1]), color, uv(u1, v0)),
                    Point::new(at(xb, yb, zw[0]), color, uv(u0, v1)),
                    Point::new(at(xb, yb, zw[1]), color, uv(u1, v1)),
                ];
                renderer.polygon(&points, surface, -4, 2);
            }
        }
    }
}

fn indoor_pool(renderer: &mut Renderer, view: &View, art: &Art) {
    let (x0, x1, z0, z1) = grotto::POOL;
    let origin = [(x0 + x1) * 0.5, grotto::WATER, (z0 + z1) * 0.5];
    if !view.visible(origin, 9.0) {
        return;
    }
    set_origin(renderer, origin);
    let image = tiled(&art.procedural.water);
    let surface = Surface::textured(image.texture)
        .blended(BlendMode::Half)
        .tinted(view.fog);
    let time = view.time;
    let splash = |x: f32, z: f32| {
        grotto::CRACKS
            .iter()
            .map(|&(cx, cz)| (-((x - cx).powi(2) + (z - cz).powi(2)) / 3.0).exp())
            .sum::<f32>()
    };
    let height = |x: f32, z: f32| {
        grotto::WATER
            + 0.03 * (x * 1.1 + time * 1.5).sin()
            + 0.08 * splash(x, z) * (time * 9.0 + x * 3.0 + z * 2.0).sin()
    };
    let color = |x: f32, z: f32| {
        let shimmer = (x * 0.8 + z * 0.5 + time * 1.2).sin() * 10.0 + splash(x, z) * 50.0;
        [80.0 + shimmer, 100.0 + shimmer, 98.0 + shimmer].map(|c| c.clamp(0.0, 255.0) as u8)
    };
    let mut xs = steps(x0, x1 - 0.4, 2.0);
    xs.push(x1);
    xs.push(CLIFF_X);
    let zs = steps(z0, z1, 2.0);
    let density = 12.0;
    for zw in zs.windows(2) {
        for xw in xs.windows(2) {
            let (ax, bx, az, bz) = (xw[0], xw[1], zw[0], zw[1]);
            let mouth = ax >= x1 - 0.01;
            if mouth && (az < grotto::MOUTH.0 - 0.01 || bz > grotto::MOUTH.1 + 0.01) {
                continue;
            }
            let u0 = (ax * density + time * 2.0).rem_euclid(64.0);
            let v0 = (-bz * density).rem_euclid(64.0);
            let (u1, v1) = (u0 + (bx - ax) * density, v0 + (bz - az) * density);
            let at = |x: f32, z: f32| local([x, height(x, z), z], origin);
            let points = [
                Point::new(at(ax, bz), color(ax, bz), (u0 as u8, v0 as u8)),
                Point::new(at(bx, bz), color(bx, bz), (u1 as u8, v0 as u8)),
                Point::new(at(ax, az), color(ax, az), (u0 as u8, v1 as u8)),
                Point::new(at(bx, az), color(bx, az), (u1 as u8, v1 as u8)),
            ];
            renderer.polygon(&points, surface, SURFACE_BIAS, 2);
        }
    }
}

fn cascades(renderer: &mut Renderer, view: &View, art: &Art) {
    let image = tiled(&art.procedural.waterfall);
    let mist = &art.procedural.mist;
    for (index, &(x, z)) in grotto::CRACKS.iter().enumerate() {
        let top = grotto::CEILING + 1.0;
        let middle = [x, (top + grotto::WATER) * 0.5, z];
        if !view.visible(middle, 4.0) {
            continue;
        }
        set_origin(renderer, middle);
        let half = 0.6;
        let side = [view.right.0 * half, 0.0, view.right.1 * half];
        let corner = |sign: f32, y: f32| local([x + side[0] * sign, y, z + side[2] * sign], middle);
        let density = 20.0;
        for (layer, (speed, blend, color, tint)) in [
            (1.0, BlendMode::Half, [150, 160, 165], view.fog),
            (1.8, BlendMode::Add, [40, 46, 50], BLACK),
        ]
        .into_iter()
        .enumerate()
        {
            let scroll =
                view.time * speed * 3.5 * density + layer as f32 * 17.0 + index as f32 * 23.0;
            let v0 = (-scroll).rem_euclid(64.0);
            let v1 = v0 + (top - grotto::WATER) * density * 0.5;
            let points = [
                Point::new(corner(-1.0, top), color, (0, v0 as u8)),
                Point::new(corner(1.0, top), color, (24, v0 as u8)),
                Point::new(corner(-1.0, grotto::WATER), color, (0, v1.min(255.0) as u8)),
                Point::new(corner(1.0, grotto::WATER), color, (24, v1.min(255.0) as u8)),
            ];
            let surface = Surface::textured(image.texture).blended(blend).tinted(tint);
            renderer.polygon(&points, surface, -4, 2);
        }
        for puff in 0..4 {
            let phase = (view.time * 0.35 + puff as f32 / 4.0 + index as f32 * 0.5).fract();
            let centre = [
                x + (phase - 0.5) * 0.8,
                grotto::WATER + 0.2 + phase * 1.2,
                z,
            ];
            let size = 0.8 + phase * 1.6;
            billboard(
                renderer,
                view,
                mist,
                centre,
                (size, size),
                scaled([40, 44, 46], (1.0 - phase) * (phase * 5.0).min(1.0)),
                additive(mist),
                Anchor::Centre,
            );
        }
    }
}

fn pool(renderer: &mut Renderer, view: &View, art: &Art, cells: &[(f32, f32)]) {
    let origin = [POOL_CENTER.0, WATER_Y, POOL_CENTER.1];
    if !view.visible(origin, 16.0) {
        return;
    }
    set_origin(renderer, origin);
    let image = tiled(&art.procedural.water);
    let surface = Surface::textured(image.texture)
        .blended(BlendMode::Half)
        .tinted(view.fog);
    let time = view.time;
    let falls = |x: f32, z: f32| (-((x + 38.5).powi(2) + (z - FALLS_Z).powi(2)) / 12.0).exp();
    let height = |x: f32, z: f32| {
        WATER_Y
            + 0.05 * (x * 0.9 + time * 1.7).sin()
            + 0.04 * (z * 1.3 - time * 1.3).sin()
            + 0.12 * falls(x, z) * (time * 9.0 + x * 3.0 + z * 2.0).sin()
    };
    let color = |x: f32, z: f32| {
        let shimmer = (x * 0.7 + z * 0.4 + time * 1.1).sin() * 14.0 + falls(x, z) * 60.0;
        [100.0 + shimmer, 118.0 + shimmer, 112.0 + shimmer].map(|c| c.clamp(0.0, 255.0) as u8)
    };
    let density = 12.0;
    for &(x0, z0) in cells {
        let (x1, z1) = (x0 + 2.0, z0 + 2.0);
        if !view.visible([x0 + 1.0, WATER_Y, z0 + 1.0], 2.0) {
            continue;
        }
        let u0 = (x0 * density + time * 3.0).rem_euclid(64.0);
        let v0 = (-z1 * density + time * 2.0).rem_euclid(64.0);
        let (u1, v1) = (u0 + 2.0 * density, v0 + 2.0 * density);
        let at = |x: f32, z: f32| local([x, height(x, z), z], origin);
        let points = [
            Point::new(at(x0, z1), color(x0, z1), (u0 as u8, v0 as u8)),
            Point::new(at(x1, z1), color(x1, z1), (u1 as u8, v0 as u8)),
            Point::new(at(x0, z0), color(x0, z0), (u0 as u8, v1 as u8)),
            Point::new(at(x1, z0), color(x1, z0), (u1 as u8, v1 as u8)),
        ];
        renderer.polygon(&points, surface, SURFACE_BIAS, 2);
    }
}

fn mist(renderer: &mut Renderer, view: &View, art: &Art) {
    let image = &art.procedural.mist;
    for index in 0..12 {
        let phase = (view.time * 0.2 + index as f32 / 12.0).fract();
        let jitter = |salt: u64| hash_float(77, index, salt);
        let centre = [
            -38.2 + phase * 3.5 + jitter(1) * 0.8,
            WATER_Y + 0.4 + phase * 4.5,
            FALLS_Z - 2.2 + 4.4 * jitter(2),
        ];
        if !view.visible(centre, 4.0) {
            continue;
        }
        let size = 2.0 + phase * 4.5;
        let brightness = (1.0 - phase) * (phase * 5.0).min(1.0);
        billboard(
            renderer,
            view,
            image,
            centre,
            (size, size),
            scaled([46, 50, 54], brightness),
            additive(image),
            Anchor::Centre,
        );
    }
}

fn beam(renderer: &mut Renderer, view: &View, shaft: &Shaft, pulse: f32) {
    let middle = scale(add(shaft.top, shaft.bottom), 0.5);
    let reach = length(sub(shaft.top, shaft.bottom));
    if !view.visible(middle, reach * 0.5) {
        return;
    }
    let axis = normalize(sub(shaft.bottom, shaft.top));
    let to_camera = normalize(sub(view.position, middle));
    let side = normalize(cross(axis, to_camera));
    let edge_on = 1.0 - dot(axis, to_camera).abs() * 0.5;
    let fade = (1.0 - view.fog_amount(length(sub(view.position, middle)))) * pulse * edge_on;
    set_origin(renderer, middle);
    let corner =
        |end: Vec3, width: f32, sign: f32| local(add(end, scale(side, width * sign)), middle);
    let (top, bottom) = (
        scaled(shaft.top_color, fade),
        scaled(shaft.bottom_color, fade),
    );
    let points = [
        Point::new(corner(shaft.top, shaft.top_width, -1.0), top, (0, 0)),
        Point::new(corner(shaft.top, shaft.top_width, 1.0), top, (0, 0)),
        Point::new(
            corner(shaft.bottom, shaft.bottom_width, -1.0),
            bottom,
            (0, 0),
        ),
        Point::new(
            corner(shaft.bottom, shaft.bottom_width, 1.0),
            bottom,
            (0, 0),
        ),
    ];
    renderer.polygon(&points, Surface::gouraud().blended(BlendMode::Add), 0, 2);
}

fn shafts(renderer: &mut Renderer, view: &View, list: &[Shaft]) {
    renderer.fog_enabled = false;
    for (index, shaft) in list.iter().enumerate() {
        let pulse = 0.85 + 0.15 * (view.time * 0.6 + index as f32 * 2.1).sin();
        beam(renderer, view, shaft, pulse);
    }
    renderer.fog_enabled = true;
}

fn skylight(renderer: &mut Renderer, view: &View) {
    let origin = [0.0, SHRINE_ROOF, TEMPLE_Z];
    if !view.visible(origin, 2.0) || view.position[1] > SHRINE_ROOF {
        return;
    }
    renderer.fog_enabled = false;
    set_origin(renderer, origin);
    let at = |x: f32, z: f32| local([x, SHRINE_ROOF, TEMPLE_Z + z], origin);
    let color = [240, 238, 210];
    let points = [
        Point::new(at(-1.0, -1.0), color, (0, 0)),
        Point::new(at(1.0, -1.0), color, (0, 0)),
        Point::new(at(-1.0, 1.0), color, (0, 0)),
        Point::new(at(1.0, 1.0), color, (0, 0)),
    ];
    renderer.polygon(&points, Surface::flat(), 0, 1);
    renderer.fog_enabled = true;
}

fn motes(renderer: &mut Renderer, view: &View, art: &Art) {
    let image = &art.procedural.mote;
    if !view.visible([0.0, 3.5, TEMPLE_Z], 4.0) || view.position[0].abs() > 15.0 {
        return;
    }
    for index in 0..28 {
        let random = |salt: u64| hash_float(91, index, salt);
        let rise = (random(1) + view.time * 0.04 * (0.5 + random(2))).fract();
        let angle = random(3) * std::f32::consts::TAU + view.time * 0.1 * (random(4) - 0.5);
        let radius = random(5).sqrt() * 1.1;
        let centre = [
            angle.cos() * radius,
            0.3 + rise * 6.5,
            TEMPLE_Z + angle.sin() * radius,
        ];
        let twinkle = 0.5 + 0.5 * (view.time * (1.0 + random(6) * 2.0) + random(7) * 6.0).sin();
        let fade = (rise * 6.0).min(1.0) * ((1.0 - rise) * 6.0).min(1.0);
        billboard(
            renderer,
            view,
            image,
            centre,
            (0.07, 0.07),
            scaled([120, 112, 80], twinkle * fade),
            additive(image),
            Anchor::Centre,
        );
    }
}

fn idol(renderer: &mut Renderer, view: &View, art: &Art, centre: Vec3, metal: Vec3, size: f32) {
    if !view.visible(centre, 1.0) {
        return;
    }
    set_origin(renderer, centre);
    let top = [0.0, 0.32, 0.0];
    let bottom = [0.0, -0.24, 0.0];
    let ring = [
        [0.2, 0.04, 0.0],
        [0.0, 0.04, 0.2],
        [-0.2, 0.04, 0.0],
        [0.0, 0.04, -0.2],
    ];
    for index in 0..4 {
        let next = ring[(index + 1) % 4];
        for mut triangle in [[top, ring[index], next], [bottom, next, ring[index]]] {
            let mut normal = cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0]));
            let centroid = scale(add(add(triangle[0], triangle[1]), triangle[2]), 1.0 / 3.0);
            if dot(normal, centroid) < 0.0 {
                triangle.swap(1, 2);
                normal = scale(normal, -1.0);
            }
            let normal = normalize(normal);
            let light = 0.28 + 0.72 * normal[1].max(0.0) + 0.25 * normal[0].abs();
            let color = metal.map(|c| (c * light).min(255.0) as u8);
            let points = triangle
                .map(|corner| Point::new(local(scale(corner, size), [0.0; 3]), color, (0, 0)));
            if renderer.backface(&points) {
                continue;
            }
            renderer.polygon(&points, Surface::flat(), 0, 0);
        }
    }
    let sparkle = ((view.time * 1.3).sin().max(0.0)).powi(8);
    if sparkle > 0.02 {
        let glow = &art.procedural.glow;
        billboard(
            renderer,
            view,
            glow,
            [centre[0] + 0.08, centre[1] + 0.22, centre[2] - 0.1],
            (0.6, 0.6),
            scaled([255, 220, 150], sparkle),
            additive(glow),
            Anchor::Centre,
        );
    }
}
