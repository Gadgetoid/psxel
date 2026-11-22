use super::*;
use psx_vram::{Clut, TexPage, TextureDepth};

fn env() -> DrawEnv {
    let mut env = DrawEnv::new(0, 0, 64, 64);
    env.dither = false;
    env
}

fn quad(x0: i16, y0: i16, x1: i16, y1: i16, color: [u8; 3], blend: Option<BlendMode>) -> Primitive {
    Primitive::Quad {
        vertices: [
            Vertex::new(x0, y0).with_color(color),
            Vertex::new(x1, y0).with_color(color),
            Vertex::new(x0, y1).with_color(color),
            Vertex::new(x1, y1).with_color(color),
        ],
        gouraud: false,
        texture: None,
        blend,
    }
}

fn covered(vram: &Vram) -> Vec<(usize, usize)> {
    let mut pixels = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            if vram.get(x, y) != 0 {
                pixels.push((x, y));
            }
        }
    }
    pixels
}

#[test]
fn right_and_bottom_edges_excluded() {
    let mut vram = Vram::new();
    draw(&mut vram, &env(), &quad(2, 3, 6, 7, [255, 255, 255], None));
    let pixels = covered(&vram);
    assert_eq!(pixels.len(), 16);
    assert!(pixels
        .iter()
        .all(|&(x, y)| (2..6).contains(&x) && (3..7).contains(&y)));
}

#[test]
fn shared_edges_neither_overlap_nor_gap() {
    let mut vram = Vram::new();
    let one = [8, 8, 8];
    let shapes = [
        [Vertex::new(1, 1), Vertex::new(40, 5), Vertex::new(9, 37)],
        [Vertex::new(40, 5), Vertex::new(9, 37), Vertex::new(50, 50)],
        [Vertex::new(1, 1), Vertex::new(40, 5), Vertex::new(33, 0)],
    ];
    for vertices in shapes {
        draw(
            &mut vram,
            &env(),
            &Primitive::Triangle {
                vertices: vertices.map(|v| v.with_color(one)),
                gouraud: false,
                texture: None,
                blend: Some(BlendMode::Add),
            },
        );
    }
    assert!(covered(&vram)
        .iter()
        .all(|&(x, y)| vram.get(x, y) == 0x0421));
    assert!(covered(&vram).len() > 500);
}

#[test]
fn both_windings_rasterise_identically() {
    let mut forward = Vram::new();
    let mut backward = Vram::new();
    let a = Vertex::new(3, 2).with_color([200, 0, 0]);
    let b = Vertex::new(30, 9).with_color([200, 0, 0]);
    let c = Vertex::new(12, 40).with_color([200, 0, 0]);
    let triangle = |vertices| Primitive::Triangle {
        vertices,
        gouraud: false,
        texture: None,
        blend: None,
    };
    draw(&mut forward, &env(), &triangle([a, b, c]));
    draw(&mut backward, &env(), &triangle([c, b, a]));
    assert_eq!(forward.pixels, backward.pixels);
    assert!(!covered(&forward).is_empty());
}

#[test]
fn oversized_polygon_dropped() {
    let mut vram = Vram::new();
    draw(
        &mut vram,
        &env(),
        &quad(-600, 0, 600, 10, [255, 255, 255], None),
    );
    assert!(covered(&vram).is_empty());
}

#[test]
fn gouraud_dither_follows_matrix() {
    let mut vram = Vram::new();
    let mut dithered = env();
    dithered.dither = true;
    let gray = [100, 100, 100];
    draw(
        &mut vram,
        &dithered,
        &Primitive::Quad {
            vertices: [
                Vertex::new(0, 0).with_color(gray),
                Vertex::new(8, 0).with_color(gray),
                Vertex::new(0, 8).with_color(gray),
                Vertex::new(8, 8).with_color(gray),
            ],
            gouraud: true,
            texture: None,
            blend: None,
        },
    );
    for y in 0..8 {
        for x in 0..8 {
            let expected = ((100 + DITHER[y & 3][x & 3]) >> 3) as u16;
            assert_eq!(vram.get(x, y) & 0x1f, expected, "pixel {x},{y}");
        }
    }
}

#[test]
fn flat_untextured_not_dithered() {
    let mut vram = Vram::new();
    let mut dithered = env();
    dithered.dither = true;
    draw(
        &mut vram,
        &dithered,
        &quad(0, 0, 4, 4, [100, 100, 100], None),
    );
    assert!(covered(&vram)
        .iter()
        .all(|&(x, y)| vram.get(x, y) & 0x1f == 12));
}

fn textured_setup() -> (Vram, Texture) {
    let mut vram = Vram::new();
    let page = TexPage::new(4, 0, TextureDepth::Clut4);
    let clut = Clut::new(0, 500);
    vram.write_clut(clut, &[0x0000, 0x0010, 0x8010, 0x7fff]);
    let indices: Vec<u8> = (0..16 * 16).map(|i| ((i % 16) / 4) as u8).collect();
    vram.write_texture(page, 0, 0, 16, &indices);
    (
        vram,
        Texture {
            page,
            clut,
            raw: false,
            window: None,
        },
    )
}

fn textured_sprite(texture: Texture, color: [u8; 3], blend: Option<BlendMode>) -> Primitive {
    Primitive::Sprite {
        x: 0,
        y: 0,
        width: 16,
        height: 1,
        u: 0,
        v: 0,
        color,
        texture: Some(texture),
        blend,
    }
}

#[test]
fn texture_modulation_and_transparency() {
    let (mut vram, texture) = textured_setup();
    vram.set(0, 0, 0x1234);
    draw(
        &mut vram,
        &env(),
        &textured_sprite(texture, [128, 128, 128], None),
    );
    assert_eq!(vram.get(0, 0), 0x1234, "index 0 is transparent");
    assert_eq!(vram.get(4, 0), 0x0010);
    assert_eq!(vram.get(8, 0), 0x8010, "mask bit comes from the texel");
    assert_eq!(vram.get(12, 0), 0x7fff);

    let (mut vram, texture) = textured_setup();
    draw(
        &mut vram,
        &env(),
        &textured_sprite(texture, [64, 255, 255], None),
    );
    assert_eq!(vram.get(4, 0), 0x0008, "half intensity");
    assert_eq!(
        vram.get(12, 0),
        0x7fef,
        "red halved, green and blue saturate"
    );
}

#[test]
fn only_semi_transparent_texels_blend() {
    let (mut vram, texture) = textured_setup();
    vram.fill_rect(0, 0, 16, 1, 0x0008);
    draw(
        &mut vram,
        &env(),
        &textured_sprite(texture, [128, 128, 128], Some(BlendMode::Add)),
    );
    assert_eq!(vram.get(4, 0), 0x0010, "opaque texel replaces");
    assert_eq!(vram.get(8, 0), 0x8018, "semi-transparent texel adds");
}

#[test]
fn blend_modes_on_5_bit_values() {
    let background: u16 = 0x0010 | (20 << 5) | (4 << 10);
    let foreground = [80, 40, 160];
    let expect = |mode, red: u16, green: u16, blue: u16| {
        let mut vram = Vram::new();
        vram.fill_rect(0, 0, 4, 4, background);
        draw(&mut vram, &env(), &quad(0, 0, 4, 4, foreground, Some(mode)));
        assert_eq!(
            vram.get(1, 1),
            (blue << 10) | (green << 5) | red,
            "{mode:?}"
        );
    };
    expect(BlendMode::Half, 13, 12, 12);
    expect(BlendMode::Add, 26, 25, 24);
    expect(BlendMode::Subtract, 6, 15, 0);
    expect(BlendMode::AddQuarter, 18, 21, 9);
}

#[test]
fn mask_bit_check_and_set() {
    let mut vram = Vram::new();
    vram.set(1, 1, 0x8000);
    let mut masked = env();
    masked.check_mask = true;
    masked.set_mask = true;
    draw(&mut vram, &masked, &quad(0, 0, 4, 4, [255, 0, 0], None));
    assert_eq!(vram.get(1, 1), 0x8000);
    assert_eq!(vram.get(2, 2), 0x801f);
}

#[test]
fn affine_texture_mapping_ignores_depth() {
    let mut vram = Vram::new();
    let page = TexPage::new(4, 0, TextureDepth::Direct15);
    let ramp: Vec<u16> = (0..64).map(|u| (u as u16 / 2) | 0x0400).collect();
    for v in 0..64 {
        vram.write_rect(256, v, 64, &ramp);
    }
    let texture = Texture {
        page,
        clut: Clut::default(),
        raw: true,
        window: None,
    };
    draw(
        &mut vram,
        &env(),
        &Primitive::Triangle {
            vertices: [
                Vertex::new(0, 0).with_uv(0, 0),
                Vertex::new(40, 0).with_uv(40, 0),
                Vertex::new(0, 40).with_uv(0, 40),
            ],
            gouraud: false,
            texture: Some(texture),
            blend: None,
        },
    );
    for x in 0..30 {
        assert_eq!(vram.get(x, 5) & 0x1f, (x / 2) as u16);
    }
}

#[test]
fn clip_rectangle_and_offset() {
    let mut vram = Vram::new();
    let mut clipped = DrawEnv::new(10, 10, 4, 4);
    clipped.dither = false;
    draw(
        &mut vram,
        &clipped,
        &quad(-5, -5, 20, 20, [255, 255, 255], None),
    );
    let pixels = covered(&vram);
    assert_eq!(pixels.len(), 16);
    assert!(pixels
        .iter()
        .all(|&(x, y)| (10..14).contains(&x) && (10..14).contains(&y)));
}

#[test]
fn golden_scene_is_deterministic() {
    let (mut vram, texture) = textured_setup();
    let mut dithered = DrawEnv::new(0, 0, 64, 64);
    dithered.dither = true;
    let scene = [
        Primitive::Quad {
            vertices: [
                Vertex::new(0, 0).with_color([255, 0, 0]),
                Vertex::new(63, 0).with_color([0, 255, 0]),
                Vertex::new(0, 63).with_color([0, 0, 255]),
                Vertex::new(63, 63).with_color([255, 255, 255]),
            ],
            gouraud: true,
            texture: None,
            blend: None,
        },
        Primitive::Triangle {
            vertices: [
                Vertex::new(5, 50).with_uv(0, 0).with_color([200, 128, 90]),
                Vertex::new(60, 10)
                    .with_uv(15, 0)
                    .with_color([128, 128, 128]),
                Vertex::new(30, 62).with_uv(0, 15).with_color([60, 60, 200]),
            ],
            gouraud: true,
            texture: Some(texture),
            blend: Some(BlendMode::Half),
        },
        Primitive::Line {
            vertices: [
                Vertex::new(0, 32).with_color([255, 255, 0]),
                Vertex::new(63, 40).with_color([0, 255, 255]),
            ],
            gouraud: true,
            blend: None,
        },
    ];
    draw_all(&mut vram, &dithered, &scene);
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let pixel = vram.get(x, y);
            rgba.extend([0, 5, 10].map(|shift| (((pixel >> shift) & 0x1f) << 3) as u8));
            rgba.push(0xff);
        }
    }
    compare_golden("soft_scene", 64, 64, &rgba);
}

fn compare_golden(name: &str, width: u32, height: u32, rgba: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../tests/golden/{name}.png"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(rgba)
            .unwrap();
        return;
    }
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&path)
            .unwrap_or_else(|_| panic!("missing {path:?}, run with UPDATE_GOLDEN=1")),
    ));
    let mut reader = decoder.read_info().unwrap();
    let mut expected = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut expected).unwrap();
    assert!(expected == rgba, "{name} differs from golden");
}

#[test]
fn per_primitive_texture_window_repeats_tile() {
    let mut vram = Vram::new();
    let page = TexPage::new(4, 0, TextureDepth::Direct15);
    let ramp: Vec<u16> = (0..64).map(|u| 0x0400 | (u as u16 / 2)).collect();
    for v in 0..64 {
        vram.write_rect(256, v, 64, &ramp);
    }
    let window = psx_vram::TextureWindow {
        mask_x: 0x18,
        mask_y: 0x18,
        offset_x: 0,
        offset_y: 0,
    };
    let texture = Texture {
        page,
        clut: Clut::default(),
        raw: true,
        window: Some(window),
    };
    draw(
        &mut vram,
        &env(),
        &Primitive::Sprite {
            x: 0,
            y: 0,
            width: 64,
            height: 1,
            u: 32,
            v: 0,
            color: [128, 128, 128],
            texture: Some(texture),
            blend: None,
        },
    );
    for x in 0..64u16 {
        let expected = ((32 + x) % 64) / 2;
        assert_eq!(vram.get(x as usize, 0) & 0x1f, expected, "x {x}");
    }
}
