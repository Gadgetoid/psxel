use psxel_vram::{TextureWindow, Vram};

use crate::blend::{Blends, Sources};
use crate::sprites::SpriteSheet;
use crate::textures::Image;

macro_rules! sprite {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/sprites/", $name, ".png")).as_slice()
    };
}

macro_rules! family {
    ($sheet:expr, $vram:expr, $fog:expr, $($name:literal),+) => {
        vec![$($sheet.add($vram, sprite!($name), $fog)),+]
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Road {
    Gravel,
    Tarmac,
    Sand,
    Snow,
    Mud,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Grass,
    Wheat,
    Leaves,
    Snow,
    Earth,
}

pub struct Art {
    pub sheet: SpriteSheet,
    pub broadleaf: Vec<Image>,
    pub autumn: Vec<Image>,
    pub bare: Vec<Image>,
    pub fir: Vec<Image>,
    pub pine: Vec<Image>,
    pub snow_fir: Vec<Image>,
    pub joshua: Vec<Image>,
    pub cactus: Vec<Image>,
    pub bush: Vec<Image>,
    pub scrub: Vec<Image>,
    pub rock: Vec<Image>,
    pub snow_rock: Vec<Image>,
    pub small_rock: Vec<Image>,
    pub gravel: Vec<Image>,
    pub tarmac: Vec<Image>,
    pub sand: Vec<Image>,
    pub snow_road: Vec<Image>,
    pub mud: Vec<Image>,
    pub hay: Vec<Image>,
    pub grass: Image,
    pub wheat: Image,
    pub leaves: Image,
    pub snow_ground: Image,
    pub earth: Image,
    pub stone_wall: Image,
    pub tunnel_wall: Image,
    pub riverbed: Image,
    pub desert_wall: Image,
    pub concrete: Image,
    pub concrete_panels: Image,
    pub metal_blue: Image,
    pub metal_red: Image,
    pub road_single: Image,
    pub verge_dirt: Image,
    pub puddle_mud: Image,
    pub road_snow_single: Image,
    pub barrier: Image,
    pub ice: Image,
    pub rock_texture: Image,
    pub horizon_forest: Image,
    pub horizon_farmland: Image,
    pub horizon_mesas: Image,
    pub horizon_alpine: Image,
    pub cliff_desert: Vec<Image>,
    pub blends: Blends,
}

impl Art {
    pub fn load(vram: &mut Vram) -> Self {
        let mut sheet = SpriteSheet::default();
        let gravel = family!(
            sheet,
            vram,
            false,
            "road_gravel_0",
            "road_gravel_1",
            "road_gravel_2",
            "road_gravel_3"
        );
        let tarmac = family!(
            sheet,
            vram,
            false,
            "road_tarmac_0",
            "road_tarmac_1",
            "road_tarmac_2",
            "road_tarmac_3"
        );
        let sand = family!(
            sheet,
            vram,
            false,
            "road_sand_0",
            "road_sand_1",
            "road_sand_2",
            "road_sand_3"
        );
        let snow_road = family!(sheet, vram, false, "road_snow_0", "road_snow_1");
        let mud = family!(sheet, vram, false, "road_mud_0", "road_mud_1");
        let grass = sheet.add(vram, sprite!("ground_grass"), false);
        let wheat = sheet.add(vram, sprite!("ground_wheat"), false);
        let leaves = sheet.add(vram, sprite!("ground_leaves"), false);
        let snow_ground = sheet.add(vram, sprite!("ground_snow"), false);
        let earth = sheet.add(vram, sprite!("ground_earth"), false);
        let stone_wall = sheet.add(vram, sprite!("wall_stone"), false);
        let tunnel_wall = sheet.add(vram, sprite!("wall_tunnel"), false);
        let riverbed = sheet.add(vram, sprite!("ground_riverbed"), false);
        let desert_wall = sheet.add(vram, sprite!("wall_desert"), false);
        let concrete = sheet.add(vram, sprite!("wall_concrete"), false);
        let concrete_panels = sheet.add(vram, sprite!("wall_panels"), false);
        let metal_blue = sheet.add(vram, sprite!("metal_blue"), false);
        let metal_red = sheet.add(vram, sprite!("metal_red"), false);
        let road_single = sheet.add(vram, sprite!("road_single"), false);
        let verge_dirt = sheet.add(vram, sprite!("verge_dirt"), false);
        let puddle_mud = sheet.add(vram, sprite!("puddle_mud"), false);
        let road_snow_single = sheet.add(vram, sprite!("road_snow_single"), false);
        let barrier = sheet.add(vram, sprite!("barrier"), false);
        let ice = sheet.add(vram, sprite!("ice"), false);
        let rock_texture = sheet.add(vram, sprite!("rock_texture"), false);
        let cliff_desert = family!(
            sheet,
            vram,
            false,
            "cliff_desert_0",
            "cliff_desert_1",
            "cliff_desert_2",
            "cliff_desert_3",
            "cliff_desert_plain"
        );
        let fir = family!(sheet, vram, true, "fir_0", "fir_1", "fir_2", "fir_3");
        let pine = family!(sheet, vram, true, "pine_0", "pine_1", "pine_2", "pine_3");
        let snow_fir = family!(
            sheet,
            vram,
            true,
            "snow_fir_0",
            "snow_fir_1",
            "snow_fir_2",
            "snow_fir_3"
        );
        let broadleaf = family!(
            sheet,
            vram,
            true,
            "broadleaf_0",
            "broadleaf_1",
            "broadleaf_2",
            "broadleaf_3"
        );
        let autumn = family!(sheet, vram, true, "autumn_0", "autumn_1", "autumn_2");
        let bare = family!(sheet, vram, true, "bare_0", "bare_1", "bare_2", "bare_3");
        let joshua = family!(sheet, vram, true, "joshua_0", "joshua_1", "joshua_2", "joshua_3");
        let hay = family!(sheet, vram, true, "hay_0", "hay_1");
        let rock = family!(sheet, vram, true, "rock_0", "rock_1", "rock_2", "rock_3");
        let snow_rock = family!(
            sheet,
            vram,
            true,
            "snow_rock_0",
            "snow_rock_1",
            "snow_rock_2",
            "snow_rock_3"
        );
        let bush = family!(sheet, vram, true, "bush_0", "bush_1", "bush_2", "bush_3");
        let scrub = family!(sheet, vram, true, "scrub_0", "scrub_1", "scrub_2", "scrub_3");
        let cactus = family!(sheet, vram, true, "cactus_0", "cactus_1", "cactus_2", "cactus_3");
        let small_rock = family!(
            sheet,
            vram,
            true,
            "small_rock_0",
            "small_rock_1",
            "small_rock_2",
            "small_rock_3"
        );
        let blends = Blends::new(
            &mut sheet,
            Sources {
                grounds: &[
                    (Ground::Grass, sprite!("ground_grass")),
                    (Ground::Wheat, sprite!("ground_wheat")),
                    (Ground::Leaves, sprite!("ground_leaves")),
                    (Ground::Snow, sprite!("ground_snow")),
                    (Ground::Earth, sprite!("ground_earth")),
                ],
                roads: &[
                    (Road::Gravel, sprite!("road_gravel_0")),
                    (Road::Tarmac, sprite!("road_tarmac_0")),
                    (Road::Sand, sprite!("road_sand_0")),
                    (Road::Snow, sprite!("road_snow_0")),
                    (Road::Mud, sprite!("road_mud_0")),
                ],
                desert_cliff: sprite!("cliff_desert_0"),
                rock_cliff: sprite!("rock_texture"),
            },
        );
        let horizon_forest = sheet.add_wide(vram, sprite!("horizon_forest"));
        let horizon_farmland = sheet.add_wide(vram, sprite!("horizon_farmland"));
        let horizon_mesas = sheet.add_wide(vram, sprite!("horizon_mesas"));
        let horizon_alpine = sheet.add_wide(vram, sprite!("horizon_alpine"));
        Self {
            sheet,
            broadleaf,
            autumn,
            bare,
            fir,
            pine,
            snow_fir,
            joshua,
            cactus,
            bush,
            scrub,
            rock,
            snow_rock,
            small_rock,
            gravel,
            tarmac,
            sand,
            snow_road,
            mud,
            hay,
            grass,
            wheat,
            leaves,
            snow_ground,
            earth,
            stone_wall,
            tunnel_wall,
            riverbed,
            desert_wall,
            concrete,
            concrete_panels,
            metal_blue,
            metal_red,
            road_single,
            verge_dirt,
            puddle_mud,
            road_snow_single,
            barrier,
            ice,
            rock_texture,
            horizon_forest,
            horizon_farmland,
            horizon_mesas,
            horizon_alpine,
            cliff_desert,
            blends,
        }
    }

    pub fn road(&self, road: Road, variant: u64) -> &Image {
        let family = match road {
            Road::Gravel => &self.gravel,
            Road::Tarmac => &self.tarmac,
            Road::Sand => &self.sand,
            Road::Snow => &self.snow_road,
            Road::Mud => &self.mud,
        };
        &family[(variant % family.len() as u64) as usize]
    }

    pub fn backdrop(&self, biome: usize) -> Option<Image> {
        match biome {
            0 => Some(self.horizon_forest),
            1 | 4 => Some(self.horizon_farmland),
            2 => Some(self.horizon_mesas),
            3 => Some(self.horizon_alpine),
            _ => None,
        }
    }

    pub fn cliff(&self, ground: Ground, variant: u64) -> Image {
        match ground {
            Ground::Earth => {
                tiled(&self.cliff_desert[(variant % self.cliff_desert.len() as u64) as usize])
            }
            _ => tiled(&self.rock_texture),
        }
    }

    pub fn bridge_wall(&self, ground: Ground) -> Image {
        match ground {
            Ground::Earth => self.desert_wall,
            Ground::Wheat | Ground::Snow => self.concrete,
            Ground::Grass | Ground::Leaves => self.stone_wall,
        }
    }

    pub fn tunnel_lining(&self, ground: Ground) -> Image {
        match ground {
            Ground::Earth => self.desert_wall,
            Ground::Wheat | Ground::Snow => self.concrete_panels,
            Ground::Grass | Ground::Leaves => self.tunnel_wall,
        }
    }

    pub fn ground(&self, ground: Ground) -> Option<&Image> {
        match ground {
            Ground::Grass => Some(&self.grass),
            Ground::Wheat => Some(&self.wheat),
            Ground::Leaves => Some(&self.leaves),
            Ground::Snow => Some(&self.snow_ground),
            Ground::Earth => Some(&self.earth),
        }
    }
}

pub fn tiled(image: &Image) -> Image {
    let mut tiled = *image;
    let mask = 0x18;
    tiled.texture.window = Some(TextureWindow {
        mask_x: mask,
        mask_y: mask,
        offset_x: (image.u / 8) & mask,
        offset_y: (image.v / 8) & mask,
    });
    tiled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_art_into_vram() {
        let mut vram = Vram::new();
        let art = Art::load(&mut vram);
        assert_eq!(art.fir.len(), 4);
        assert!(art.horizon_forest.width >= 200);
        let tiled = tiled(&art.grass);
        assert_eq!(
            tiled.texture.window.unwrap().apply(0, 0),
            (art.grass.u, art.grass.v)
        );
        assert_eq!(
            tiled.texture.window.unwrap().apply(200, 130),
            (art.grass.u + 200 % 64, art.grass.v + 130 % 64)
        );
    }
}
