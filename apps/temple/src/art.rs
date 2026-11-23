use psxel_vram::Vram;

use crate::procedural::Procedural;
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

pub struct Art {
    pub stone: Vec<Image>,
    pub floor: Vec<Image>,
    pub bark: Vec<Image>,
    pub rockface: Vec<Image>,
    pub foliage: Image,
    pub canopy: Image,
    pub paving: Image,
    pub riverbed: Image,
    pub swamp: Image,
    pub walls: Vec<Image>,
    pub horizon: Image,
    pub carvings: Vec<Image>,
    pub serpent: Image,
    pub vines: Vec<Image>,
    pub trees: Vec<Image>,
    pub bushes: Vec<Image>,
    pub boulders: Vec<Image>,
    pub procedural: Procedural,
}

impl Art {
    pub fn load(vram: &mut Vram) -> Self {
        let mut sheet = SpriteSheet::default();
        let stone = family!(
            sheet,
            vram,
            false,
            "temple_stone_0",
            "temple_stone_1",
            "temple_stone_2",
            "temple_stone_3"
        );
        let floor = family!(
            sheet,
            vram,
            false,
            "jungle_floor_0",
            "jungle_floor_1",
            "jungle_floor_2",
            "jungle_floor_3"
        );
        let bark = family!(sheet, vram, false, "bark_0", "bark_1", "bark_2", "bark_3");
        let rockface = family!(
            sheet,
            vram,
            false,
            "rockface_0",
            "rockface_1",
            "rockface_2",
            "rockface_3"
        );
        let foliage = sheet.add(vram, sprite!("foliage"), false);
        let canopy = sheet.add(vram, sprite!("canopy"), false);
        let paving = sheet.add(vram, sprite!("wall_tunnel"), false);
        let riverbed = sheet.add(vram, sprite!("ground_riverbed"), false);
        let swamp = sheet.add(vram, sprite!("ground_swamp"), false);
        let walls = family!(sheet, vram, false, "treeline_0", "treeline_1");
        let horizon = sheet.add_wide(vram, sprite!("horizon_jungle"));
        let carvings = family!(
            sheet,
            vram,
            false,
            "carved_face",
            "carved_sun",
            "carved_rosette",
            "carved_star"
        );
        let serpent = sheet.add(vram, sprite!("carved_serpent"), false);
        let vines = family!(sheet, vram, true, "vine_0", "vine_1", "vine_2", "vine_3");
        let trees = family!(
            sheet,
            vram,
            true,
            "broadleaf_0",
            "broadleaf_1",
            "broadleaf_2",
            "broadleaf_3",
            "kapok_0",
            "kapok_1"
        );
        let bushes = family!(sheet, vram, true, "bush_0", "bush_1", "bush_2", "bush_3");
        let boulders = family!(sheet, vram, true, "rock_0", "rock_1", "rock_2", "rock_3");
        Self {
            stone,
            floor,
            bark,
            rockface,
            foliage,
            canopy,
            paving,
            riverbed,
            swamp,
            walls,
            horizon,
            carvings,
            serpent,
            vines,
            trees,
            bushes,
            boulders,
            procedural: Procedural::build(vram),
        }
    }
}
