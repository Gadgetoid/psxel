use psx_vram::{bgr555, Vram};

use crate::art::{Ground, Road};
use crate::biome::BIOMES;
use crate::rng::hash_float;
use crate::sprites::{decode, Decoded, SpriteSheet};
use crate::textures::Image;

pub const STAGES: usize = 3;
const SIZE: usize = 64;
const PALETTE: usize = 16;
const OCTAVES: [(usize, f32); 3] = [(4, 0.5), (8, 0.3), (16, 0.2)];
const JITTER: f32 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ground,
    Road,
    Cliff,
}

const KINDS: [Kind; 3] = [Kind::Ground, Kind::Road, Kind::Cliff];

#[derive(Clone, Copy, Debug)]
pub enum Choice {
    First,
    Second,
    Mix(Image),
}

pub struct Sources<'a> {
    pub grounds: &'a [(Ground, &'a [u8])],
    pub roads: &'a [(Road, &'a [u8])],
    pub desert_cliff: &'a [u8],
    pub rock_cliff: &'a [u8],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Ground(Ground),
    Road(Road),
    Cliff(bool),
}

pub struct Blends {
    grounds: Vec<(Ground, Decoded)>,
    roads: Vec<(Road, Decoded)>,
    desert_cliff: Decoded,
    rock_cliff: Decoded,
    masks: [Vec<f32>; 3],
    slots: [[Image; STAGES]; 3],
    active: [bool; 3],
    pair: Option<(usize, usize)>,
}

impl Blends {
    pub fn new(sheet: &mut SpriteSheet, sources: Sources) -> Self {
        let slots = KINDS.map(|_| [(); STAGES].map(|_| sheet.reserve(SIZE, SIZE, PALETTE * 2)));
        let masks = [0x6a0d, 0x40ad, 0xc11f].map(mask);
        Self {
            grounds: sources
                .grounds
                .iter()
                .map(|&(ground, bytes)| (ground, decode(bytes)))
                .collect(),
            roads: sources
                .roads
                .iter()
                .map(|&(road, bytes)| (road, decode(bytes)))
                .collect(),
            desert_cliff: decode(sources.desert_cliff),
            rock_cliff: decode(sources.rock_cliff),
            masks,
            slots,
            active: [false; 3],
            pair: None,
        }
    }

    pub fn prepare(&mut self, vram: &mut Vram, pair: (usize, usize)) {
        if self.pair == Some(pair) {
            return;
        }
        self.pair = Some(pair);
        let (first, second) = (&BIOMES[pair.0], &BIOMES[pair.1]);
        for (slot, kind) in KINDS.iter().enumerate() {
            let source = |biome: &crate::biome::Biome| match kind {
                Kind::Ground => Source::Ground(biome.ground),
                Kind::Road => Source::Road(biome.road),
                Kind::Cliff => Source::Cliff(biome.ground == Ground::Earth),
            };
            let (from, to) = (source(first), source(second));
            self.active[slot] = from != to;
            if !self.active[slot] {
                continue;
            }
            let (from, to) = (self.decoded(from), self.decoded(to));
            let colors: Vec<u16> = [from, to]
                .iter()
                .flat_map(|texture| {
                    (0..PALETTE).map(|index| {
                        let rgb = texture.palette.get(index).copied().unwrap_or([0, 0, 0]);
                        let value = bgr555(rgb[0], rgb[1], rgb[2]);
                        if value == 0 {
                            0x8000
                        } else {
                            value
                        }
                    })
                })
                .collect();
            for (stage, image) in self.slots[slot].iter().enumerate() {
                let threshold = (stage + 1) as f32 / (STAGES + 1) as f32;
                let indices: Vec<u8> = self.masks[slot]
                    .iter()
                    .enumerate()
                    .map(|(texel, &value)| {
                        if value < threshold {
                            to.indices[texel] + PALETTE as u8
                        } else {
                            from.indices[texel]
                        }
                    })
                    .collect();
                vram.write_texture(
                    image.texture.page,
                    image.u as usize,
                    image.v as usize,
                    SIZE,
                    &indices,
                );
                vram.write_clut(image.texture.clut, &colors);
            }
        }
    }

    fn decoded(&self, source: Source) -> &Decoded {
        match source {
            Source::Ground(ground) => self
                .grounds
                .iter()
                .find(|(entry, _)| *entry == ground)
                .map(|(_, decoded)| decoded)
                .expect("ground blend source"),
            Source::Road(road) => self
                .roads
                .iter()
                .find(|(entry, _)| *entry == road)
                .map(|(_, decoded)| decoded)
                .expect("road blend source"),
            Source::Cliff(true) => &self.desert_cliff,
            Source::Cliff(false) => &self.rock_cliff,
        }
    }

    pub fn choose(&self, kind: Kind, pair: (usize, usize), t: f32, key: f32) -> Choice {
        if t <= 0.0 {
            return Choice::First;
        }
        if t >= 1.0 {
            return Choice::Second;
        }
        let slot = KINDS.iter().position(|&entry| entry == kind).unwrap_or(0);
        if self.pair != Some(pair) || !self.active[slot] {
            return if key < t {
                Choice::Second
            } else {
                Choice::First
            };
        }
        let scaled = t * (STAGES + 1) as f32;
        let stage = (scaled.floor() as usize + usize::from(key < scaled.fract())).min(STAGES + 1);
        match stage {
            0 => Choice::First,
            stage if stage > STAGES => Choice::Second,
            stage => Choice::Mix(self.slots[slot][stage - 1]),
        }
    }
}

fn periodic_noise(seed: u64, cells: usize, x: usize, y: usize) -> f32 {
    let size = SIZE as f32 / cells as f32;
    let (fx, fy) = (x as f32 / size, y as f32 / size);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let corner = |dx: usize, dy: usize| {
        hash_float(
            seed ^ cells as u64,
            ((x0 + dx) % cells) as u64,
            ((y0 + dy) % cells) as u64,
        )
    };
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

fn mask(seed: u64) -> Vec<f32> {
    let values: Vec<f32> = (0..SIZE * SIZE)
        .map(|texel| {
            let (x, y) = (texel % SIZE, texel / SIZE);
            let smooth: f32 = OCTAVES
                .iter()
                .map(|&(cells, weight)| periodic_noise(seed, cells, x, y) * weight)
                .sum();
            smooth + (hash_float(seed, texel as u64, 7) - 0.5) * JITTER
        })
        .collect();
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    for (rank, &texel) in order.iter().enumerate() {
        ranks[texel] = (rank as f32 + 0.5) / values.len() as f32;
    }
    ranks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_are_uniform_and_tile() {
        let values = mask(1);
        for threshold in [0.25f32, 0.5, 0.75] {
            let below = values.iter().filter(|&&value| value < threshold).count();
            assert_eq!(below, (threshold * (SIZE * SIZE) as f32) as usize);
        }
        let edge: f32 = (0..SIZE)
            .map(|y| (values[y * SIZE] - values[y * SIZE + SIZE - 1]).abs())
            .sum::<f32>()
            / SIZE as f32;
        let inner: f32 = (0..SIZE)
            .map(|y| (values[y * SIZE + 31] - values[y * SIZE + 32]).abs())
            .sum::<f32>()
            / SIZE as f32;
        assert!(edge < inner * 2.0 + 0.05, "{edge} {inner}");
    }

    #[test]
    fn choose_steps_through_stages() {
        let mut vram = Vram::new();
        let mut art = crate::art::Art::load(&mut vram);
        let blends = &mut art.blends;
        let forest = BIOMES
            .iter()
            .position(|b| b.ground == Ground::Grass)
            .unwrap();
        let farm = BIOMES
            .iter()
            .position(|b| b.ground == Ground::Wheat)
            .unwrap();
        blends.prepare(&mut vram, (forest, farm));
        assert!(matches!(
            blends.choose(Kind::Ground, (forest, farm), 0.0, 0.5),
            Choice::First
        ));
        assert!(matches!(
            blends.choose(Kind::Ground, (forest, farm), 1.0, 0.5),
            Choice::Second
        ));
        assert!(matches!(
            blends.choose(Kind::Ground, (forest, farm), 0.5, 0.99),
            Choice::Mix(_)
        ));
        assert!(matches!(
            blends.choose(Kind::Ground, (farm, forest), 0.5, 0.99),
            Choice::First
        ));
    }
}
