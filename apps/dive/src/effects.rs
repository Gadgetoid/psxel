use std::f32::consts::TAU;

use psxel_math::Mat3Fx12;
use psxel_prim::BlendMode;

use crate::art::Art;
use crate::biome::Rgb;
use crate::lighting::{sun_direction, water_filter, Lighting};
use crate::mesh::{add, cross, dot, length, local, normalize, scale, sub, tiled, Vec3, View, Zone};
use crate::render::{world_to_psx, Point, Renderer, Surface};
use crate::rng::hash_float;
use crate::scene::Scene;
use crate::sub::Sub;
use crate::terrain::{normal, seabed, smoothstep, SURFACE};
use crate::textures::Image;
use crate::wreck;

const BLACK: Rgb = [0, 0, 0];
const BEAM_LENGTH: f32 = 18.0;
const LAMP: [f32; 3] = [255.0, 240.0, 200.0];
const WRECK_VENTS: [(f32, f32, f32); 3] = [(0.0, 2.6, -6.0), (-3.0, 3.0, 9.0), (2.0, -1.0, 14.0)];

fn set_origin(renderer: &mut Renderer, origin: Vec3) {
    renderer.set_object(
        world_to_psx(origin[0], origin[1], origin[2]),
        Mat3Fx12::IDENTITY,
    );
}

fn scaled(color: Rgb, amount: f32) -> Rgb {
    color.map(|channel| (channel as f32 * amount).clamp(0.0, 255.0) as u8)
}

fn neutralise(color: Rgb, amount: f32) -> Rgb {
    color.map(|channel| (channel as f32 + (128.0 - channel as f32) * amount) as u8)
}

fn additive(image: &Image) -> Surface {
    Surface::textured(image.texture)
        .blended(BlendMode::Add)
        .tinted(BLACK)
}

#[derive(Clone, Copy)]
enum Anchor {
    Base,
    Centre,
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
    sway: Vec3,
    mirror: bool,
) {
    set_origin(renderer, anchor);
    let half = size.0 * 0.5;
    let right = [view.right.0 * half, 0.0, view.right.1 * half];
    let (top, bottom) = match placement {
        Anchor::Base => (size.1, 0.0),
        Anchor::Centre => (size.1 * 0.5, -size.1 * 0.5),
    };
    let corner = |side: f32, height: f32, bend: f32| {
        local(
            add(
                [right[0] * side, height, right[2] * side],
                scale(sway, bend),
            ),
            [0.0; 3],
        )
    };
    let (mut u0, mut u1) = (image.u, image.u.wrapping_add(image.width.saturating_sub(1)));
    if mirror {
        std::mem::swap(&mut u0, &mut u1);
    }
    let (v0, v1) = (image.v, image.v.wrapping_add(image.height).wrapping_sub(1));
    let middle_v = ((v0 as u16 + v1 as u16) / 2) as u8;
    let middle = (top + bottom) * 0.5;
    let rows = [(top, v0, 1.0), (middle, middle_v, 0.35), (bottom, v1, 0.0)];
    let depth = local([0.0, middle, 0.0], [0.0; 3]);
    renderer.force_otz = Some(renderer.depth_of(depth) as u32);
    for pair in rows.windows(2) {
        let ((ya, va, ba), (yb, vb, bb)) = (pair[0], pair[1]);
        let points = [
            Point::new(corner(-1.0, ya, ba), color, (u0, va)),
            Point::new(corner(1.0, ya, ba), color, (u1, va)),
            Point::new(corner(-1.0, yb, bb), color, (u0, vb)),
            Point::new(corner(1.0, yb, bb), color, (u1, vb)),
        ];
        renderer.polygon(&points, surface, -8, 1);
    }
    renderer.force_otz = None;
}

pub fn headlight(craft: &Sub, position: Vec3) -> f32 {
    if !craft.lights {
        return 0.0;
    }
    let forward = craft.basis().forward;
    let offset = sub(position, craft.position);
    let distance = length(offset);
    if distance > BEAM_LENGTH + 4.0 || distance < 0.01 {
        return 0.0;
    }
    let cone = (dot(scale(offset, 1.0 / distance), forward) - 0.78) / 0.14;
    cone.clamp(0.0, 1.0) * (1.0 - distance / (BEAM_LENGTH + 4.0)).powi(2)
}

fn lit(color: Rgb, boost: f32) -> Rgb {
    if boost <= 0.0 {
        return color;
    }
    [0, 1, 2].map(|c| (color[c] as f32 + LAMP[c] * boost * 0.9).min(255.0) as u8)
}

pub fn draw(
    renderer: &mut Renderer,
    view: &View,
    art: &Art,
    scene: &Scene,
    craft: &Sub,
    lighting: &Lighting,
) {
    tether(renderer, view, craft);
    silt(renderer, view, art, craft);
    billboards(renderer, view, scene, craft);
    surface(renderer, view, art);
    sunbeams(renderer, view);
    caustics(renderer, view, art);
    fish(renderer, view, art, scene, craft, lighting);
    bubbles(renderer, view, art, craft);
    snow(renderer, view, art, craft);
    beams(renderer, view, art, craft);
    alien(renderer, view, art);
    hold_shafts(renderer, view);
}

fn billboards(renderer: &mut Renderer, view: &View, scene: &Scene, craft: &Sub) {
    renderer.fog_enabled = false;
    for item in &scene.billboards {
        let centre = add(item.position, [0.0, item.height * 0.5, 0.0]);
        if !view.visible(centre, item.height) {
            continue;
        }
        let phase = item.position[0] * 0.37 + item.position[2] * 0.21;
        let sway = [
            (view.time * 0.7 + phase).sin() * item.sway,
            0.0,
            (view.time * 0.5 + phase * 1.3).cos() * item.sway * 0.6,
        ];
        let color = lit(item.color, headlight(craft, centre));
        let mut texture = item.image.texture;
        let band = view.band(length(sub(centre, view.position)));
        texture.clut.y += band;
        let surface = Surface::textured(texture);
        let color = neutralise(color, view.band_amount(band));
        billboard(
            renderer,
            view,
            &item.image,
            item.position,
            (item.width, item.height),
            color,
            surface,
            Anchor::Base,
            sway,
            false,
        );
    }
    renderer.fog_enabled = true;
}

fn hold_shafts(renderer: &mut Renderer, view: &View) {
    renderer.fog_enabled = false;
    for (index, (top, bottom, width)) in wreck::shafts().into_iter().enumerate() {
        let pulse = 0.75 + 0.25 * (view.time * 0.7 + index as f32 * 1.9).sin();
        beam(
            renderer,
            view,
            top,
            bottom,
            (width * 0.7, width),
            (scaled([40, 56, 58], pulse), scaled([8, 10, 10], pulse)),
        );
    }
    renderer.fog_enabled = true;
}

fn alien(renderer: &mut Renderer, view: &View, art: &Art) {
    let glow = &art.procedural.glow;
    renderer.fog_enabled = false;
    for (index, light) in crate::alien::portal_lights().into_iter().enumerate() {
        if !view.visible(light, 2.0) {
            continue;
        }
        let phase = (view.time * 1.1 - index as f32 * 0.8).sin() * 0.5 + 0.5;
        let fade = 1.0 - view.fog_amount(length(sub(light, view.position))) * 0.5;
        billboard(
            renderer,
            view,
            glow,
            light,
            (1.4, 1.4),
            scaled([50, 200, 185], (0.3 + 0.7 * phase) * fade),
            additive(glow),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
    let lights = crate::alien::rim_lights();
    if view.visible(lights[0], 20.0) {
        let count = lights.len() as f32;
        for (index, &light) in lights.iter().enumerate() {
            if !view.visible(light, 2.0) {
                continue;
            }
            let phase = (view.time * 1.6 - index as f32 / count * TAU).sin() * 0.5 + 0.5;
            let size = 0.9 + phase * 1.2;
            billboard(
                renderer,
                view,
                glow,
                light,
                (size, size),
                scaled([60, 220, 200], 0.35 + 0.65 * phase),
                additive(glow),
                Anchor::Centre,
                [0.0; 3],
                false,
            );
        }
        let pulse = 0.7 + 0.3 * (view.time * 1.3).sin();
        billboard(
            renderer,
            view,
            glow,
            crate::alien::cupola(),
            (5.0, 5.0),
            scaled([40, 170, 150], pulse),
            additive(glow),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
    renderer.fog_enabled = true;
    pocket(renderer, view, art);
}

fn pocket(renderer: &mut Renderer, view: &View, art: &Art) {
    let level = crate::alien::pocket();
    let (x0, x1, z0, z1) = crate::alien::chamber_bounds();
    let centre = [(x0 + x1) * 0.5, level, (z0 + z1) * 0.5];
    if !view.visible(centre, 20.0) || !crate::alien::in_cave(view.position) {
        return;
    }
    set_origin(renderer, centre);
    let image = tiled(&art.procedural.water);
    let below = view.position[1] < level;
    let time = view.time;
    let height = |x: f32, z: f32| {
        level + 0.05 * (x * 0.9 + time * 1.3).sin() + 0.04 * (z * 1.1 - time).sin()
    };
    for (x, z) in crate::alien::surface_cells() {
        let (xa, za) = (x + 2.0, z + 2.0);
        let middle = [x + 1.0, level, z + 1.0];
        if !view.visible(middle, 2.0) {
            continue;
        }
        let shimmer = |px: f32, pz: f32| {
            let glow = crate::alien::glow([px, level, pz], time);
            let base = if below {
                [40.0, 90.0, 90.0]
            } else {
                [60.0, 110.0, 105.0]
            };
            [0, 1, 2].map(|c| (base[c] + glow[c] * 1.2).min(255.0) as u8)
        };
        let density = 12.0;
        let u0 = (x * density + time * 2.0).rem_euclid(64.0);
        let v0 = (z * density).rem_euclid(64.0);
        let (u1, v1) = (u0 + 2.0 * density, v0 + 2.0 * density);
        let at = |px: f32, pz: f32| local([px, height(px, pz), pz], centre);
        let points = [
            Point::new(at(x, za), shimmer(x, za), (u0 as u8, v0 as u8)),
            Point::new(at(xa, za), shimmer(xa, za), (u1 as u8, v0 as u8)),
            Point::new(at(x, z), shimmer(x, z), (u0 as u8, v1 as u8)),
            Point::new(at(xa, z), shimmer(xa, z), (u1 as u8, v1 as u8)),
        ];
        renderer.polygon(
            &points,
            Surface::textured(image.texture).blended(BlendMode::Half),
            -6,
            2,
        );
    }
}

fn tether(renderer: &mut Renderer, view: &View, craft: &Sub) {
    let mut points = craft.trail.clone();
    points.push(add(craft.position, craft.basis().apply([0.0, 0.3, -1.15])));
    let count = points.len();
    let sagged: Vec<Vec3> = points
        .iter()
        .enumerate()
        .map(|(index, &point)| {
            let age = (count - 1 - index) as f32;
            let sway = (view.time * 0.8 + index as f32 * 0.4).sin() * 0.05 * age.min(1.0);
            add(point, [0.0, -(age * 0.08).min(1.6) + sway, 0.0])
        })
        .collect();
    let width = 0.035;
    let sides: Vec<Vec3> = (0..count)
        .map(|index| {
            let before = sagged[index.saturating_sub(1)];
            let after = sagged[(index + 1).min(count - 1)];
            let along = sub(after, before);
            let to_camera = sub(view.position, sagged[index]);
            let across = cross(along, to_camera);
            if length(across) > 0.05 * length(along) * length(to_camera) {
                scale(normalize(across), width)
            } else {
                [view.right.0 * width, 0.0, view.right.1 * width]
            }
        })
        .collect();
    let filter = water_filter(-craft.position[1]);
    let color = [0, 1, 2].map(|c| (30.0 + 60.0 * filter[c]) as u8);
    for index in 0..count.saturating_sub(1) {
        let (a, b) = (sagged[index], sagged[index + 1]);
        let middle = scale(add(a, b), 0.5);
        if !view.visible(middle, 1.0) || length(sub(middle, view.position)) < 2.5 {
            continue;
        }
        set_origin(renderer, middle);
        let corner =
            |point: Vec3, side: Vec3, sign: f32| local(add(point, scale(side, sign)), middle);
        let quad = [
            Point::new(corner(a, sides[index], -1.0), color, (0, 0)),
            Point::new(corner(a, sides[index], 1.0), color, (0, 0)),
            Point::new(corner(b, sides[index + 1], -1.0), color, (0, 0)),
            Point::new(corner(b, sides[index + 1], 1.0), color, (0, 0)),
        ];
        renderer.polygon(&quad, Surface::flat(), 0, 0);
    }
}

fn surface(renderer: &mut Renderer, view: &View, art: &Art) {
    let depth = SURFACE - view.position[1];
    if depth > view.far {
        return;
    }
    let image = tiled(&art.procedural.water);
    renderer.fog_enabled = false;
    let cell = 8.0;
    let (cx, cz) = (
        (view.position[0] / cell).floor() * cell,
        (view.position[2] / cell).floor() * cell,
    );
    let origin = [cx, SURFACE, cz];
    set_origin(renderer, origin);
    let time = view.time;
    let height = |x: f32, z: f32| {
        SURFACE + 0.15 * (x * 0.5 + time * 1.1).sin() + 0.12 * (z * 0.6 - time * 0.9).sin()
    };
    let brightness = (1.0 - depth / view.far).max(0.0);
    let color = |x: f32, z: f32| {
        let shimmer = 0.8 + 0.2 * (x * 0.9 + z * 0.4 + time * 1.4).sin();
        scaled([170, 220, 220], brightness * shimmer)
    };
    let reach = 7;
    for zi in -reach..reach {
        for xi in -reach..reach {
            let (x0, z0) = (cx + xi as f32 * cell, cz + zi as f32 * cell);
            let (x1, z1) = (x0 + cell, z0 + cell);
            if !view.visible([x0 + cell * 0.5, SURFACE, z0 + cell * 0.5], cell) {
                continue;
            }
            let density = 6.0;
            let u0 = (x0 * density + time * 3.0).rem_euclid(64.0);
            let v0 = (z0 * density + time * 2.0).rem_euclid(64.0);
            let (u1, v1) = (u0 + cell * density, v0 + cell * density);
            let distance = length(sub(
                [x0 + cell * 0.5, SURFACE, z0 + cell * 0.5],
                view.position,
            ));
            let band = view.band(distance);
            let mut banded = image.texture;
            banded.clut.y += band;
            let texture = Surface::textured(banded).blended(BlendMode::Half);
            let neutral = view.band_amount(band);
            let color = |x: f32, z: f32| neutralise(color(x, z), neutral);
            let at = |x: f32, z: f32| local([x, height(x, z), z], origin);
            let points = [
                Point::new(at(x0, z0), color(x0, z0), (u0 as u8, v0 as u8)),
                Point::new(at(x1, z0), color(x1, z0), (u1 as u8, v0 as u8)),
                Point::new(at(x0, z1), color(x0, z1), (u0 as u8, v1 as u8)),
                Point::new(at(x1, z1), color(x1, z1), (u1 as u8, v1 as u8)),
            ];
            renderer.polygon(&points, texture, 0, 2);
        }
    }
    renderer.fog_enabled = true;
    let sun = sun_direction();
    let window = add(view.position, scale(sun, depth / sun[1]));
    if depth > 6.0 && view.visible(window, 6.0) {
        let glow = &art.procedural.glow;
        billboard(
            renderer,
            view,
            glow,
            window,
            (depth * 1.4, depth * 1.4),
            scaled([200, 230, 220], brightness),
            additive(glow),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
}

fn beam(
    renderer: &mut Renderer,
    view: &View,
    top: Vec3,
    bottom: Vec3,
    widths: (f32, f32),
    colors: (Rgb, Rgb),
) {
    let middle = scale(add(top, bottom), 0.5);
    let reach = length(sub(top, bottom));
    if !view.visible(middle, reach * 0.5) {
        return;
    }
    let axis = normalize(sub(bottom, top));
    let along = dot(sub(view.position, top), axis).clamp(0.0, reach);
    let nearest = add(top, scale(axis, along));
    if length(sub(view.position, nearest)) < 6.0 {
        return;
    }
    let to_camera = normalize(sub(view.position, middle));
    let across = cross(axis, to_camera);
    if length(across) < 0.2 {
        return;
    }
    let side = normalize(across);
    let edge_on = smoothstep(0.2, 0.6, length(across));
    let distance = length(sub(view.position, middle));
    let fade = (1.0 - view.fog_amount(distance))
        * edge_on
        * smoothstep(reach * 0.25, reach * 0.25 + 8.0, distance);
    set_origin(renderer, middle);
    let corner =
        |end: Vec3, width: f32, sign: f32| local(add(end, scale(side, width * sign)), middle);
    let (a, b) = (scaled(colors.0, fade), scaled(colors.1, fade));
    let points = [
        Point::new(corner(top, widths.0, -1.0), a, (0, 0)),
        Point::new(corner(top, widths.0, 1.0), a, (0, 0)),
        Point::new(corner(bottom, widths.1, -1.0), b, (0, 0)),
        Point::new(corner(bottom, widths.1, 1.0), b, (0, 0)),
    ];
    renderer.polygon(&points, Surface::gouraud().blended(BlendMode::Add), 0, 2);
}

fn sunbeams(renderer: &mut Renderer, view: &View) {
    let sun = sun_direction();
    let cell = 11.0;
    let (gx, gz) = (
        (view.position[0] / cell).floor(),
        (view.position[2] / cell).floor(),
    );
    renderer.fog_enabled = false;
    for dz in -2..=2 {
        for dx in -2..=2 {
            let (ix, iz) = (gx + dx as f32, gz + dz as f32);
            let roll = hash_float(41, (ix + 500.0) as u64, (iz + 500.0) as u64);
            if roll < 0.55 {
                continue;
            }
            let sway = (view.time * 0.3 + roll * 9.0).sin() * 1.5;
            let top = [
                ix * cell + roll * cell + sway,
                SURFACE,
                iz * cell + (1.0 - roll) * cell,
            ];
            let depth = 28.0;
            let bottom = sub(top, scale(sun, depth / sun[1]));
            let strength = (1.0 - (-view.position[1]) / 55.0).max(0.0)
                * (0.6 + 0.4 * (view.time * 0.5 + roll * 5.0).sin());
            beam(
                renderer,
                view,
                top,
                bottom,
                (1.0 + roll, 2.5 + roll * 2.0),
                (scaled([60, 80, 75], strength), [0, 0, 0]),
            );
        }
    }
    renderer.fog_enabled = true;
}

fn caustics(renderer: &mut Renderer, view: &View, art: &Art) {
    let image = tiled(&art.procedural.caustic);
    let cell = 4.0;
    let (cx, cz) = (
        (view.position[0] / cell).floor() * cell,
        (view.position[2] / cell).floor() * cell,
    );
    let origin = [cx, view.position[1], cz];
    set_origin(renderer, origin);
    let time = view.time;
    let reach = 6;
    for layer in 0..2 {
        let (drift, density) = if layer == 0 {
            ([1.2, 0.7], 10.0)
        } else {
            ([-0.8, 1.1], 7.0)
        };
        let surface = Surface::textured(image.texture)
            .blended(BlendMode::Add)
            .tinted(BLACK);
        for zi in -reach..reach {
            for xi in -reach..reach {
                let (x0, z0) = (cx + xi as f32 * cell, cz + zi as f32 * cell);
                let (x1, z1) = (x0 + cell, z0 + cell);
                let middle = [
                    x0 + cell * 0.5,
                    seabed(x0 + cell * 0.5, z0 + cell * 0.5),
                    z0 + cell * 0.5,
                ];
                if wreck::inside(middle) > 0.0
                    || normal(middle[0], middle[2])[1] < 0.88
                    || !view.visible(middle, cell)
                {
                    continue;
                }
                let depth = -middle[1];
                let strength = (-depth / 15.0).exp()
                    * 1.4
                    * (1.0 - view.fog_amount(length(sub(middle, view.position))));
                if strength < 0.04 {
                    continue;
                }
                let color = scaled([110, 120, 110], strength);
                let u0 = (x0 * density + time * drift[0] * density).rem_euclid(64.0);
                let v0 = (z0 * density + time * drift[1] * density).rem_euclid(64.0);
                let (u1, v1) = (u0 + cell * density, v0 + cell * density);
                let at = |x: f32, z: f32| local([x, seabed(x, z) + 0.08, z], origin);
                let points = [
                    Point::new(at(x0, z1), color, (u0 as u8, v1.min(255.0) as u8)),
                    Point::new(
                        at(x1, z1),
                        color,
                        (u1.min(255.0) as u8, v1.min(255.0) as u8),
                    ),
                    Point::new(at(x0, z0), color, (u0 as u8, v0 as u8)),
                    Point::new(at(x1, z0), color, (u1.min(255.0) as u8, v0 as u8)),
                ];
                renderer.polygon(&points, surface, -10, 1);
            }
        }
    }
}

fn fish(
    renderer: &mut Renderer,
    view: &View,
    art: &Art,
    scene: &Scene,
    craft: &Sub,
    lighting: &Lighting,
) {
    renderer.fog_enabled = false;
    let time = view.time;
    for (index, school) in scene.schools.iter().enumerate() {
        let offset = index as f32 * 1.7;
        let centre = add(
            school.centre,
            [
                (time * 0.08 + offset).sin() * school.radius,
                (time * 0.11 + offset).sin() * 1.5,
                (time * 0.06 + offset).cos() * school.radius,
            ],
        );
        if !view.visible(centre, school.radius * 2.0) {
            continue;
        }
        let image = &art.fish[school.species % art.fish.len()];
        let scale_by = match school.species {
            8 => 2.2,
            10 | 11 => 1.6,
            9 => 2.6,
            3 => 1.4,
            _ => 1.0,
        };
        for member in 0..school.count as u64 {
            let random = |salt: u64| hash_float(51 + index as u64, member, salt);
            let radius = school.radius * (0.35 + 0.65 * random(1));
            let angle = time * school.speed * (0.8 + 0.4 * random(2)) + random(3) * TAU;
            let height = (random(4) - 0.5) * 3.0 + (time * 0.9 + random(5) * 6.0).sin() * 0.3;
            let position = add(
                centre,
                [angle.cos() * radius, height, angle.sin() * radius * 0.6],
            );
            let tangent = [-angle.sin(), 0.0, angle.cos() * 0.6];
            let mirror = dot(tangent, [view.right.0, 0.0, view.right.1]) < 0.0;
            let size = (0.5 + random(6) * 0.3) * scale_by;
            let base = lighting.billboard(position, Zone::Exterior);
            let band = view.band(length(sub(position, view.position)));
            let color = neutralise(
                lit(scaled(base, 1.25), headlight(craft, position)),
                view.band_amount(band),
            );
            billboard(
                renderer,
                view,
                image,
                position,
                (size, size * 0.5),
                color,
                {
                    let mut texture = image.texture;
                    texture.clut.y += band;
                    Surface::textured(texture)
                },
                Anchor::Centre,
                [0.0; 3],
                mirror,
            );
        }
    }
    renderer.fog_enabled = true;
}

fn bubbles(renderer: &mut Renderer, view: &View, art: &Art, craft: &Sub) {
    let image = &art.procedural.bubble;
    let basis = craft.basis();
    let thrusters = [[-1.05, 0.0, -1.2], [1.05, 0.0, -1.2]]
        .map(|point| add(craft.position, basis.apply(point)));
    let activity = craft.thrust.abs();
    let count = (activity * 16.0) as u64;
    for index in 0..count {
        let random = |salt: u64| hash_float(61, index, salt);
        let age = (view.time * 0.8 + random(1)).fract() * 2.5;
        let position = add(
            thrusters[(index % 2) as usize],
            [
                (random(2) - 0.5) * 0.6 - basis.forward[0] * age * 1.2,
                age * 1.3,
                (random(3) - 0.5) * 0.6 - basis.forward[2] * age * 1.2,
            ],
        );
        let size = 0.07 + random(4) * 0.09;
        billboard(
            renderer,
            view,
            image,
            position,
            (size, size),
            scaled([110, 130, 140], 1.0 - age / 2.5),
            additive(image),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
    for &(x, y, z) in &WRECK_VENTS {
        let base = wreck::to_world([x, y, z]);
        if !view.visible(add(base, [0.0, 8.0, 0.0]), 10.0) {
            continue;
        }
        for index in 0..10u64 {
            let random = |salt: u64| hash_float(67 + x as u64, index, salt);
            let rise = (view.time * 0.12 + random(1)).fract();
            let height = rise * (SURFACE - base[1]);
            let wobble = (view.time * 3.0 + random(2) * 6.0).sin() * 0.2;
            let position = add(base, [wobble, height, wobble * 0.5]);
            if !view.visible(position, 0.5) {
                continue;
            }
            let size = 0.15 + random(3) * 0.2 + rise * 0.15;
            billboard(
                renderer,
                view,
                image,
                position,
                (size, size),
                [120, 140, 150],
                additive(image),
                Anchor::Centre,
                [0.0; 3],
                false,
            );
        }
    }
}

fn snow(renderer: &mut Renderer, view: &View, art: &Art, craft: &Sub) {
    let image = &art.procedural.mote;
    let span = 16.0;
    let brightness = water_filter(-view.position[1]);
    let tint = [0, 1, 2].map(|c| 30.0 + 70.0 * brightness[c]);
    for index in 0..260u64 {
        let random = |salt: u64| hash_float(71, index, salt);
        let wrap = |value: f32, centre: f32| {
            centre + (value - centre + span * 0.5).rem_euclid(span) - span * 0.5
        };
        let drift = [
            random(1) * span * 7.0 + (view.time * 0.2 + random(4) * 6.0).sin() * 0.4,
            random(2) * span * 7.0 - view.time * (0.05 + random(5) * 0.12),
            random(3) * span * 7.0 + (view.time * 0.15 + random(6) * 6.0).cos() * 0.3,
        ];
        let position = [
            wrap(drift[0], view.position[0]),
            wrap(drift[1], view.position[1]),
            wrap(drift[2], view.position[2]),
        ];
        if position[1] > SURFACE || !view.visible(position, 0.2) {
            continue;
        }
        let distance = length(sub(position, view.position));
        let fade = 1.0 - view.fog_amount(distance);
        let lamp = headlight(craft, position) * 3.0;
        let flake = index % 9 == 0;
        let size = if flake { 0.14 } else { 0.05 + random(7) * 0.04 };
        let color = [0, 1, 2].map(|c| ((tint[c] + LAMP[c] * lamp * 0.6) * fade).min(255.0) as u8);
        billboard(
            renderer,
            view,
            image,
            position,
            (size, size),
            color,
            additive(image),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
}

fn silt(renderer: &mut Renderer, view: &View, art: &Art, craft: &Sub) {
    let image = &art.procedural.glow;
    let ground = seabed(craft.position[0], craft.position[2]);
    let height = craft.position[1] - ground;
    let stirred = (1.0 - height / 4.0).clamp(0.0, 1.0) * craft.thrust.abs().max(0.2);
    for index in 0..10u64 {
        let random = |salt: u64| hash_float(81, index, salt);
        let age = (view.time * 0.25 + random(1)).fract();
        let anchor = [
            craft.position[0] + (random(2) - 0.5) * 6.0,
            0.0,
            craft.position[2] + (random(3) - 0.5) * 6.0,
        ];
        let base = seabed(anchor[0], anchor[2]);
        let position = [anchor[0], base + 0.3 + age * 1.5, anchor[2]];
        let size = 1.5 + age * 3.5;
        let strength = stirred * (1.0 - age) * (age * 4.0).min(1.0);
        if strength < 0.02 || !view.visible(position, size) {
            continue;
        }
        let filter = water_filter(-position[1]);
        let color = [0, 1, 2].map(|c| ((40.0 + 50.0 * filter[c]) * strength) as u8);
        billboard(
            renderer,
            view,
            image,
            position,
            (size, size * 0.6),
            color,
            additive(image),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
}

fn beams(renderer: &mut Renderer, view: &View, art: &Art, craft: &Sub) {
    if !craft.lights {
        return;
    }
    let forward = craft.basis().forward;
    renderer.fog_enabled = false;
    for lamp in craft.lamp_positions() {
        let end = add(lamp, scale(forward, BEAM_LENGTH));
        beam(
            renderer,
            view,
            lamp,
            end,
            (0.25, 2.8),
            ([70, 66, 52], [0, 0, 0]),
        );
    }
    let centre = scale(
        add(craft.lamp_positions()[0], craft.lamp_positions()[1]),
        0.5,
    );
    let mut hit = None;
    let mut distance = 0.6;
    while distance < BEAM_LENGTH {
        let point = add(centre, scale(forward, distance));
        let blocked = point[1] < seabed(point[0], point[2]) + 0.1
            || length(sub(wreck::collide(point, 0.25), point)) > 0.01;
        if blocked {
            hit = Some((point, distance));
            break;
        }
        distance += 0.4;
    }
    if let Some((point, distance)) = hit {
        let back = sub(point, scale(forward, 0.4));
        let strength = (1.0 - distance / BEAM_LENGTH).powf(0.7);
        let glow = &art.procedural.glow;
        let size = 1.2 + distance * 0.32;
        billboard(
            renderer,
            view,
            glow,
            back,
            (size, size),
            scaled([200, 190, 160], strength),
            additive(glow),
            Anchor::Centre,
            [0.0; 3],
            false,
        );
    }
    renderer.fog_enabled = true;
}
