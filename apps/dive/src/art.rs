use psxel_vram::Vram;

use crate::fog::FogBands;

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
    pub sand: Vec<Image>,
    pub seabed: Vec<Image>,
    pub rock: Vec<Image>,
    pub coral: Vec<Image>,
    pub hull_rust: Vec<Image>,
    pub hull_barnacle: Vec<Image>,
    pub deck: Vec<Image>,
    pub coins: Vec<Image>,
    pub treasure: Vec<Image>,
    pub fish: Vec<Image>,
    pub kelp: Vec<Image>,
    pub clumps: Vec<Image>,
    pub sign: Image,
    pub rov: Vec<Image>,
    pub flotsam: Vec<Image>,
    pub drifting: Vec<Image>,
    pub portholes: Vec<Image>,
    pub relics: Vec<Image>,
    pub rubble: Vec<Image>,
    pub plaques: Vec<Image>,
    pub procedural: Procedural,
    pub bands: FogBands,
}

impl Art {
    pub fn load(vram: &mut Vram) -> Self {
        let mut sheet = SpriteSheet::default();
        let sand = family!(
            sheet,
            vram,
            false,
            "sand_rippled_0",
            "sand_rippled_1",
            "sand_rippled_2",
            "sand_rippled_3"
        );
        let seabed = family!(sheet, vram, false, "seabed_0", "seabed_1", "seabed_2");
        let rock = family!(
            sheet,
            vram,
            false,
            "reef_rock_0",
            "reef_rock_1",
            "reef_rock_2",
            "reef_rock_3",
            "rocks_dense_0"
        );
        let coral = family!(sheet, vram, false, "coral_0", "coral_1", "coral_2", "coral_3");
        let hull_rust = family!(
            sheet,
            vram,
            false,
            "hull_rust_0",
            "hull_rust_1",
            "hull_rust_2",
            "hull_rust_3"
        );
        let hull_barnacle = family!(
            sheet,
            vram,
            false,
            "hull_barnacle_0",
            "hull_barnacle_1",
            "hull_barnacle_2",
            "hull_barnacle_3"
        );
        let deck = family!(sheet, vram, false, "deck_0", "deck_1", "deck_2", "deck_3");
        let coins = family!(sheet, vram, false, "coins_1", "coins_large");
        let treasure = family!(
            sheet,
            vram,
            true,
            "treasure_0",
            "treasure_1",
            "treasure_2",
            "treasure_3"
        );
        let fish = family!(
            sheet,
            vram,
            true,
            "fish_clown",
            "fish_tang_blue",
            "fish_tang_yellow",
            "fish_parrot",
            "fish_damsel",
            "fish_jack",
            "fish_red",
            "fish_angel",
            "fish_grouper",
            "fish_barracuda"
        );
        let mut fish = fish;
        fish.extend(family!(sheet, vram, true, "fish_lion", "fish_idol"));
        let kelp = family!(sheet, vram, true, "kelp_0", "kelp_1", "kelp_2");
        let clumps = family!(
            sheet,
            vram,
            true,
            "kelp_clump_0",
            "kelp_clump_1",
            "kelp_clump_2",
            "seaweed_0"
        );
        let sign = sheet.add(vram, sprite!("sign_oceanic"), false);
        let rov = family!(
            sheet,
            vram,
            false,
            "rov_plain",
            "rov_seam",
            "rov_frame",
            "rov_hatch"
        );
        let flotsam = family!(
            sheet,
            vram,
            true,
            "flotsam_driftwood_0",
            "flotsam_driftwood_1",
            "flotsam_driftwood_2",
            "flotsam_rope",
            "flotsam_pot",
            "flotsam_timber",
            "flotsam_net",
            "flotsam_can",
            "flotsam_shells",
            "flotsam_rubble",
            "flotsam_tangle"
        );
        let drifting = family!(
            sheet,
            vram,
            true,
            "flotsam_wrack",
            "flotsam_float",
            "flotsam_buoy"
        );
        let portholes = family!(
            sheet,
            vram,
            true,
            "porthole_0",
            "porthole_1",
            "porthole_2",
            "porthole_3"
        );
        let relics = family!(
            sheet,
            vram,
            true,
            "relic_anchor",
            "relic_helm",
            "relic_frame",
            "relic_hatch",
            "relic_pipe",
            "relic_ladder"
        );
        let rubble = family!(
            sheet,
            vram,
            true,
            "relic_coral_0",
            "relic_coral_1",
            "relic_debris_0",
            "relic_debris_1",
            "relic_chain",
            "relic_bollard"
        );
        let plaques = family!(
            sheet,
            vram,
            true,
            "relic_plaque_name",
            "relic_plaque_engine"
        );
        let procedural = Procedural::build(vram);
        let mut art = Self {
            sand,
            seabed,
            rock,
            coral,
            hull_rust,
            hull_barnacle,
            deck,
            coins,
            treasure,
            fish,
            kelp,
            clumps,
            sign,
            rov,
            flotsam,
            drifting,
            portholes,
            relics,
            rubble,
            plaques,
            procedural,
            bands: FogBands::default(),
        };
        let mut bands = FogBands::default();
        for list in [
            &mut art.sand,
            &mut art.seabed,
            &mut art.rock,
            &mut art.coral,
            &mut art.hull_rust,
            &mut art.hull_barnacle,
            &mut art.deck,
            &mut art.coins,
            &mut art.treasure,
            &mut art.fish,
            &mut art.kelp,
            &mut art.clumps,
            &mut art.portholes,
            &mut art.relics,
            &mut art.rubble,
            &mut art.plaques,
            &mut art.flotsam,
            &mut art.drifting,
        ] {
            for image in list.iter_mut() {
                bands.adopt(vram, image);
            }
        }
        bands.adopt(vram, &mut art.sign);
        bands.adopt(vram, &mut art.procedural.fan);
        bands.adopt(vram, &mut art.procedural.sponge);
        bands.adopt(vram, &mut art.procedural.anemone);
        bands.adopt(vram, &mut art.procedural.water);
        art.bands = bands;
        art
    }
}
