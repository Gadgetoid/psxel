use psx_prim::Texture;
use psx_vram::{bgr555, Clut, TexPage, TextureDepth, Vram};

use crate::biome::Rgb;
use crate::textures::{Image, FOG_BANDS};

const COLUMN_WIDTH: usize = 64;
const BANDED_CLUT_AREAS: [(u16, u16, u16, u16); 2] = [(512, 480, 1024, 512), (0, 486, 512, 506)];
const PLAIN_CLUT_AREAS: [(u16, u16, u16, u16); 1] = [(0, 506, 512, 512)];
const WIDE_PAGES: [(u8, u8, usize); 2] = [(15, 1, 224), (15, 0, 256)];
const BANDS: u16 = FOG_BANDS.len() as u16;

pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub indices: Vec<u8>,
    pub palette: Vec<Rgb>,
}

pub fn decode(bytes: &[u8]) -> Decoded {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("sprite png header");
    let info = reader.info();
    assert_eq!(
        info.color_type,
        png::ColorType::Indexed,
        "sprites must be indexed PNGs"
    );
    let bits = match info.bit_depth {
        png::BitDepth::Eight => 8,
        png::BitDepth::Four => 4,
        other => panic!("unsupported sprite bit depth {other:?}"),
    };
    let palette = info
        .palette
        .as_ref()
        .expect("sprite palette")
        .chunks(3)
        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
        .collect();
    let (width, height) = (info.width as usize, info.height as usize);
    let mut raw = vec![0; reader.output_buffer_size().expect("sprite size")];
    let frame = reader.next_frame(&mut raw).expect("sprite pixels");
    let stride = frame.line_size;
    let mut indices = Vec::with_capacity(width * height);
    for row in raw.chunks(stride).take(height) {
        for x in 0..width {
            let index = if bits == 8 {
                row[x]
            } else {
                (row[x / 2] >> (4 - (x % 2) * 4)) & 0xf
            };
            indices.push(index);
        }
    }
    Decoded {
        width,
        height,
        indices,
        palette,
    }
}

fn opaque(rgb: Rgb) -> u16 {
    let value = bgr555(rgb[0], rgb[1], rgb[2]);
    if value == 0 {
        0x8000
    } else {
        value
    }
}

struct ClutArea {
    areas: Vec<(u16, u16, u16, u16)>,
    current: usize,
    cursor: (u16, u16),
    row_height: u16,
}

impl ClutArea {
    fn new(areas: &[(u16, u16, u16, u16)], row_height: u16) -> Self {
        Self {
            areas: areas.to_vec(),
            current: 0,
            cursor: (areas[0].0, areas[0].1),
            row_height,
        }
    }

    fn allocate(&mut self, entries: usize) -> Clut {
        let width = (entries as u16).div_ceil(16) * 16;
        loop {
            let area = self.areas[self.current];
            if self.cursor.0 + width > area.2 {
                self.cursor = (area.0, self.cursor.1 + self.row_height);
            }
            if self.cursor.1 + self.row_height <= area.3 {
                break;
            }
            self.current += 1;
            assert!(self.current < self.areas.len(), "out of CLUT space");
            let next = self.areas[self.current];
            self.cursor = (next.0, next.1);
        }
        let clut = Clut::new(self.cursor.0, self.cursor.1);
        self.cursor.0 += width;
        clut
    }
}

struct Column {
    page: TexPage,
    u: usize,
    used: usize,
    limit: usize,
}

pub struct SpriteSheet {
    banded: Vec<(Clut, Vec<Rgb>)>,
    banded_area: ClutArea,
    plain_area: ClutArea,
    columns: Vec<Column>,
    wide_used: [usize; 2],
}

impl Default for SpriteSheet {
    fn default() -> Self {
        let mut columns = Vec::new();
        for (row, limit) in [(0u8, 256usize), (1, 224)] {
            for page in 8..16u8 {
                if WIDE_PAGES
                    .iter()
                    .any(|&(wide_page, wide_row, _)| (wide_page, wide_row) == (page, row))
                {
                    continue;
                }
                for u in [0, COLUMN_WIDTH] {
                    columns.push(Column {
                        page: TexPage::new(page, row, TextureDepth::Clut8),
                        u,
                        used: 0,
                        limit,
                    });
                }
            }
        }
        Self {
            banded: Vec::new(),
            banded_area: ClutArea::new(&BANDED_CLUT_AREAS, BANDS),
            plain_area: ClutArea::new(&PLAIN_CLUT_AREAS, 1),
            columns,
            wide_used: [0; 2],
        }
    }
}

impl SpriteSheet {
    fn place(&mut self, width: usize, height: usize, align: usize) -> (TexPage, usize, usize) {
        assert!(width <= COLUMN_WIDTH, "sprite wider than a column");
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.used.next_multiple_of(align) + height <= column.limit)
            .expect("out of sprite texture space");
        let v = column.used.next_multiple_of(align);
        column.used = v + height;
        (column.page, column.u, v)
    }

    pub fn add(&mut self, vram: &mut Vram, bytes: &[u8], fog: bool) -> Image {
        let sprite = decode(bytes);
        let align = if fog { 1 } else { 64 };
        let (page, column, row) = self.place(sprite.width, sprite.height, align);
        vram.write_texture(page, column, row, sprite.width, &sprite.indices);
        let transparent = fog;
        let colors: Vec<u16> = sprite
            .palette
            .iter()
            .enumerate()
            .map(|(index, &rgb)| {
                if index == 0 && transparent {
                    0
                } else {
                    opaque(rgb)
                }
            })
            .collect();
        let clut = if fog {
            self.banded_area.allocate(colors.len())
        } else {
            self.plain_area.allocate(colors.len())
        };
        vram.write_clut(clut, &colors);
        if fog {
            self.banded.push((clut, sprite.palette));
        }
        Image {
            u: column as u8,
            v: row as u8,
            width: sprite.width as u8,
            height: sprite.height as u8,
            texture: Texture {
                page,
                clut,
                raw: false,
                window: None,
            },
        }
    }

    pub fn add_wide(&mut self, vram: &mut Vram, bytes: &[u8]) -> Image {
        let sprite = decode(bytes);
        assert!(
            sprite.palette.len() <= 16 && sprite.width <= 256,
            "wide images must be 4-bit and at most 256 wide"
        );
        let slot = (0..WIDE_PAGES.len())
            .find(|&slot| self.wide_used[slot] + sprite.height <= WIDE_PAGES[slot].2)
            .expect("out of wide texture space");
        let (page_x, page_y, _) = WIDE_PAGES[slot];
        let page = TexPage::new(page_x, page_y, TextureDepth::Clut4);
        let v = self.wide_used[slot];
        self.wide_used[slot] += sprite.height;
        vram.write_texture(page, 0, v, sprite.width, &sprite.indices);
        let colors: Vec<u16> = sprite
            .palette
            .iter()
            .enumerate()
            .map(|(index, &rgb)| if index == 0 { 0 } else { opaque(rgb) })
            .collect();
        let clut = self.banded_area.allocate(16);
        vram.write_clut(clut, &colors);
        self.banded.push((clut, sprite.palette));
        Image {
            u: 0,
            v: v as u8,
            width: (sprite.width - 1) as u8,
            height: sprite.height as u8,
            texture: Texture {
                page,
                clut,
                raw: false,
                window: None,
            },
        }
    }

    pub fn update_fog(&self, vram: &mut Vram, fog: Rgb) {
        for (clut, palette) in &self.banded {
            for (band, amount) in FOG_BANDS.iter().enumerate().skip(1) {
                let colors: Vec<u16> = palette
                    .iter()
                    .enumerate()
                    .map(|(index, rgb)| {
                        if index == 0 {
                            return 0;
                        }
                        let mixed = [0, 1, 2].map(|c| {
                            (rgb[c] as f32 + (fog[c] as f32 - rgb[c] as f32) * amount) as u8
                        });
                        opaque(mixed)
                    })
                    .collect();
                vram.write_clut(Clut::new(clut.x, clut.y + band as u16), &colors);
            }
        }
    }
}

pub fn band_image(image: &Image, band: u8) -> Image {
    let mut image = *image;
    image.texture.clut.y += band as u16;
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexed_png(width: u32, height: u32, palette: &[u8], pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette.to_vec());
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
        bytes
    }

    #[test]
    fn uploads_indexed_sprite_with_fog_bands() {
        let palette = [255, 255, 255, 10, 200, 30, 0, 0, 0];
        let pixels: Vec<u8> = (0..64 * 128).map(|i| (i % 3) as u8).collect();
        let bytes = indexed_png(64, 128, &palette, &pixels);
        let mut vram = Vram::new();
        let mut sheet = SpriteSheet::default();
        let image = sheet.add(&mut vram, &bytes, true);
        assert_eq!((image.width, image.height), (64, 128));
        let texel = |image: &Image, u: u8, v: u8, vram: &Vram| {
            vram.texel(
                image.texture.page,
                image.texture.clut,
                image.u + u,
                image.v + v,
            )
        };
        assert_eq!(texel(&image, 0, 0, &vram), 0);
        assert_eq!(texel(&image, 1, 0, &vram), bgr555(10, 200, 30));
        assert_eq!(texel(&image, 2, 0, &vram), 0x8000);
        sheet.update_fog(&mut vram, [200, 200, 200]);
        let far = band_image(&image, 4);
        assert_eq!(texel(&far, 0, 0, &vram), 0);
        assert_ne!(texel(&far, 1, 0, &vram), texel(&image, 1, 0, &vram));
        let second = sheet.add(&mut vram, &bytes, true);
        assert_ne!(
            (second.u, second.v, second.texture.page),
            (image.u, image.v, image.texture.page)
        );
    }
}
