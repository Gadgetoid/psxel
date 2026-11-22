pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;
pub const MASK_BIT: u16 = 0x8000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureDepth {
    Clut4,
    Clut8,
    Direct15,
}

impl TextureDepth {
    pub fn texels_per_halfword(self) -> usize {
        match self {
            Self::Clut4 => 4,
            Self::Clut8 => 2,
            Self::Direct15 => 1,
        }
    }

    pub fn clut_entries(self) -> usize {
        match self {
            Self::Clut4 => 16,
            Self::Clut8 => 256,
            Self::Direct15 => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexPage {
    pub x: u8,
    pub y: u8,
    pub depth: TextureDepth,
}

impl TexPage {
    pub fn new(x: u8, y: u8, depth: TextureDepth) -> Self {
        assert!(x < 16 && y < 2, "texture page out of range");
        Self { x, y, depth }
    }

    pub fn containing(vram_x: usize, vram_y: usize, depth: TextureDepth) -> Self {
        Self::new((vram_x / 64) as u8, (vram_y / 256) as u8, depth)
    }

    pub fn base(self) -> (usize, usize) {
        (self.x as usize * 64, self.y as usize * 256)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Clut {
    pub x: u16,
    pub y: u16,
}

impl Clut {
    pub fn new(x: u16, y: u16) -> Self {
        assert!(x.is_multiple_of(16), "CLUT x must be 16-pixel aligned");
        assert!((x as usize) < VRAM_WIDTH && (y as usize) < VRAM_HEIGHT);
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureWindow {
    pub mask_x: u8,
    pub mask_y: u8,
    pub offset_x: u8,
    pub offset_y: u8,
}

impl TextureWindow {
    pub fn apply(self, u: u8, v: u8) -> (u8, u8) {
        let wrap = |coordinate: u8, mask: u8, offset: u8| {
            (coordinate & !(mask.wrapping_mul(8))) | ((offset & mask).wrapping_mul(8))
        };
        (
            wrap(u, self.mask_x, self.offset_x),
            wrap(v, self.mask_y, self.offset_y),
        )
    }
}

pub fn bgr555(red: u8, green: u8, blue: u8) -> u16 {
    ((blue as u16 >> 3) << 10) | ((green as u16 >> 3) << 5) | (red as u16 >> 3)
}

pub struct Vram {
    pub pixels: Vec<u16>,
}

impl Default for Vram {
    fn default() -> Self {
        Self::new()
    }
}

impl Vram {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> u16 {
        self.pixels[(y & (VRAM_HEIGHT - 1)) * VRAM_WIDTH + (x & (VRAM_WIDTH - 1))]
    }

    pub fn set(&mut self, x: usize, y: usize, value: u16) {
        self.pixels[(y & (VRAM_HEIGHT - 1)) * VRAM_WIDTH + (x & (VRAM_WIDTH - 1))] = value;
    }

    pub fn write_rect(&mut self, x: usize, y: usize, width: usize, data: &[u16]) {
        for (row, line) in data.chunks(width).enumerate() {
            for (column, &value) in line.iter().enumerate() {
                self.set(x + column, y + row, value);
            }
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, value: u16) {
        for row in y..y + height {
            for column in x..x + width {
                self.set(column, row, value);
            }
        }
    }

    pub fn copy_rect(&self, x: usize, y: usize, width: usize, height: usize, out: &mut Vec<u16>) {
        out.clear();
        for row in y..y + height {
            let start = (row & (VRAM_HEIGHT - 1)) * VRAM_WIDTH + x;
            out.extend_from_slice(&self.pixels[start..start + width]);
        }
    }

    pub fn write_texture(
        &mut self,
        page: TexPage,
        x: usize,
        y: usize,
        width: usize,
        indices: &[u8],
    ) {
        let per_halfword = page.depth.texels_per_halfword();
        assert!(
            width.is_multiple_of(per_halfword),
            "texture width must fill whole halfwords"
        );
        let (base_x, base_y) = page.base();
        let bits = 16 / per_halfword;
        for (row, line) in indices.chunks(width).enumerate() {
            for (halfword, group) in line.chunks(per_halfword).enumerate() {
                let packed = group.iter().enumerate().fold(0u16, |acc, (index, &value)| {
                    acc | ((value as u16) << (index * bits))
                });
                self.set(
                    base_x + x / per_halfword + halfword,
                    base_y + y + row,
                    packed,
                );
            }
        }
    }

    pub fn write_clut(&mut self, clut: Clut, colors: &[u16]) {
        for (index, &color) in colors.iter().enumerate() {
            self.set(clut.x as usize + index, clut.y as usize, color);
        }
    }

    pub fn texel(&self, page: TexPage, clut: Clut, u: u8, v: u8) -> u16 {
        let (base_x, base_y) = page.base();
        let (u, v) = (u as usize, v as usize);
        match page.depth {
            TextureDepth::Clut4 => {
                let halfword = self.get(base_x + u / 4, base_y + v);
                let index = (halfword >> ((u & 3) * 4)) & 0xf;
                self.get(clut.x as usize + index as usize, clut.y as usize)
            }
            TextureDepth::Clut8 => {
                let halfword = self.get(base_x + u / 2, base_y + v);
                let index = (halfword >> ((u & 1) * 8)) & 0xff;
                self.get(clut.x as usize + index as usize, clut.y as usize)
            }
            TextureDepth::Direct15 => self.get(base_x + u, base_y + v),
        }
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|&pixel| {
                let red = (pixel & 0x1f) as u8;
                let green = ((pixel >> 5) & 0x1f) as u8;
                let blue = ((pixel >> 10) & 0x1f) as u8;
                [red << 3, green << 3, blue << 3, 0xff]
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Allocator {
    used: Vec<usize>,
    capacity: Vec<usize>,
    clut_cursor: (u16, u16),
    clut_limit: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AllocError {
    TextureSpace { width: usize, height: usize },
    ClutSpace,
}

impl Allocator {
    pub fn new(
        framebuffer_width: usize,
        framebuffer_height: usize,
        clut_rows: std::ops::Range<u16>,
    ) -> Self {
        let pages = VRAM_WIDTH / 64 * 2;
        let mut used = vec![0; pages];
        let mut capacity = vec![256; pages];
        for page in 0..pages {
            let page_x = (page % 16) * 64;
            let page_y = (page / 16) * 256;
            if page_x < framebuffer_width && page_y < framebuffer_height {
                used[page] = (framebuffer_height - page_y).min(256);
            }
            let clut_start = clut_rows.start as usize;
            if clut_start >= page_y && clut_start < page_y + 256 {
                capacity[page] = clut_start - page_y;
            }
        }
        Self {
            used,
            capacity,
            clut_cursor: (0, clut_rows.start),
            clut_limit: clut_rows.end,
        }
    }

    pub fn texture(
        &mut self,
        depth: TextureDepth,
        width: usize,
        height: usize,
    ) -> Result<(TexPage, u8, u8), AllocError> {
        let halfwords = width.div_ceil(depth.texels_per_halfword());
        if halfwords > 64 || height > 256 || width > 256 {
            return Err(AllocError::TextureSpace { width, height });
        }
        for page_index in 0..self.used.len() {
            if self.used[page_index] + height <= self.capacity[page_index] {
                let page = TexPage::new((page_index % 16) as u8, (page_index / 16) as u8, depth);
                let v = self.used[page_index] as u8;
                self.used[page_index] += height;
                return Ok((page, 0, v));
            }
        }
        Err(AllocError::TextureSpace { width, height })
    }

    pub fn clut(&mut self, entries: usize) -> Result<Clut, AllocError> {
        let entries = entries.div_ceil(16) as u16 * 16;
        let (mut x, mut y) = self.clut_cursor;
        if x as usize + entries as usize > VRAM_WIDTH {
            x = 0;
            y += 1;
        }
        if y >= self.clut_limit {
            return Err(AllocError::ClutSpace);
        }
        self.clut_cursor = (x + entries, y);
        Ok(Clut::new(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_4bit() {
        let mut vram = Vram::new();
        let page = TexPage::new(5, 0, TextureDepth::Clut4);
        let clut = Clut::new(0, 480);
        let palette: Vec<u16> = (0..16).map(|index| 0x100 + index).collect();
        vram.write_clut(clut, &palette);
        let indices: Vec<u8> = (0..8 * 4).map(|index| (index * 7 % 16) as u8).collect();
        vram.write_texture(page, 8, 3, 8, &indices);
        for v in 0..4 {
            for u in 0..8 {
                let expected = palette[indices[v * 8 + u] as usize];
                assert_eq!(vram.texel(page, clut, 8 + u as u8, 3 + v as u8), expected);
            }
        }
        let packed = indices[..4]
            .iter()
            .enumerate()
            .fold(0u16, |acc, (index, &value)| {
                acc | (value as u16) << (index * 4)
            });
        assert_eq!(vram.get(5 * 64 + 2, 3), packed);
    }

    #[test]
    fn round_trip_8bit() {
        let mut vram = Vram::new();
        let page = TexPage::new(1, 1, TextureDepth::Clut8);
        let clut = Clut::new(512, 500);
        let palette: Vec<u16> = (0..256).map(|index| index as u16 * 3).collect();
        vram.write_clut(clut, &palette);
        let indices: Vec<u8> = (0..=255).collect();
        vram.write_texture(page, 0, 0, 16, &indices);
        for (index, &value) in indices.iter().enumerate() {
            assert_eq!(
                vram.texel(page, clut, (index % 16) as u8, (index / 16) as u8),
                palette[value as usize]
            );
        }
    }

    #[test]
    fn round_trip_15bit() {
        let mut vram = Vram::new();
        let page = TexPage::new(15, 1, TextureDepth::Direct15);
        vram.write_rect(15 * 64 + 10, 256 + 20, 2, &[0x1234, 0x8001, 0x7fff, 0]);
        assert_eq!(vram.texel(page, Clut::default(), 10, 20), 0x1234);
        assert_eq!(vram.texel(page, Clut::default(), 11, 20), 0x8001);
        assert_eq!(vram.texel(page, Clut::default(), 10, 21), 0x7fff);
    }

    #[test]
    fn texture_window_repeats() {
        let window = TextureWindow {
            mask_x: 0x1c,
            mask_y: 0x1c,
            offset_x: 0x04,
            offset_y: 0x08,
        };
        assert_eq!(window.apply(0, 0), (32, 64));
        assert_eq!(window.apply(31, 31), (63, 95));
        assert_eq!(window.apply(32, 32), (32, 64));
        assert_eq!(window.apply(200, 7), (32 | 8, 64 | 7));
        assert_eq!(TextureWindow::default().apply(200, 7), (200, 7));
    }

    #[test]
    fn allocator_avoids_framebuffer() {
        let mut allocator = Allocator::new(320, 480, 480..512);
        let (page, _, v) = allocator.texture(TextureDepth::Clut4, 64, 64).unwrap();
        assert!(page.x as usize * 64 >= 320 || page.y as usize * 256 + v as usize >= 480);
        assert_eq!((page.x, page.y), (5, 0));
        let clut = allocator.clut(16).unwrap();
        assert_eq!((clut.x, clut.y), (0, 480));
        assert_eq!(allocator.clut(256).unwrap(), Clut::new(16, 480));
    }
}
