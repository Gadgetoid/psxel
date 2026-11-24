use psxel_vram::{Clut, Vram};

use crate::biome::Rgb;
use crate::textures::Image;

pub const BANDS: u16 = 8;
const AREA: (u16, u16, u16, u16) = (320, 310, 512, 478);

pub struct FogBands {
    sources: Vec<(Clut, [u16; 16])>,
    cursor: (u16, u16),
}

impl Default for FogBands {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            cursor: (AREA.0, AREA.1),
        }
    }
}

impl FogBands {
    pub fn adopt(&mut self, vram: &mut Vram, image: &mut Image) {
        let original = image.texture.clut;
        let colors: [u16; 16] =
            std::array::from_fn(|index| vram.get(original.x as usize + index, original.y as usize));
        if self.cursor.0 + 16 > AREA.2 {
            self.cursor = (AREA.0, self.cursor.1 + BANDS);
        }
        assert!(self.cursor.1 + BANDS <= AREA.3, "out of fog band space");
        let clut = Clut::new(self.cursor.0, self.cursor.1);
        self.cursor.0 += 16;
        vram.write_clut(clut, &colors);
        self.sources.push((clut, colors));
        image.texture.clut = clut;
    }

    pub fn update(&self, vram: &mut Vram, fog: Rgb) {
        let fog = fog.map(|channel| (channel >> 3) as f32);
        for (clut, colors) in &self.sources {
            for band in 1..BANDS {
                let amount = band as f32 / (BANDS - 1) as f32;
                let banded = colors.map(|entry| {
                    if entry == 0 {
                        return 0;
                    }
                    let mix = |shift: u16, target: f32| {
                        let channel = ((entry >> shift) & 0x1f) as f32;
                        ((channel + (target - channel) * amount).round() as u16) << shift
                    };
                    let value =
                        (entry & 0x8000) | mix(0, fog[0]) | mix(5, fog[1]) | mix(10, fog[2]);
                    if value == 0 {
                        0x8000
                    } else {
                        value
                    }
                });
                vram.write_clut(Clut::new(clut.x, clut.y + band), &banded);
            }
        }
    }
}
