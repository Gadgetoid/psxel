use crate::art::{Ground, Road};
use crate::rng::Rng;

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    Fir,
    Pine,
    SnowFir,
    Broadleaf,
    AutumnTree,
    BareTree,
    Joshua,
    Cactus,
    Rock,
    SnowRock,
    SmallRock,
    Boulder,
    Bush,
    Scrub,
    HayBale,
    FencePost,
    Spectator,
}

pub fn ground_traits(ground: Ground) -> (f32, f32) {
    match ground {
        Ground::Grass => (0.55, 3.0),
        Ground::Wheat => (0.6, 3.6),
        Ground::Leaves => (0.45, 3.4),
        Ground::Snow => (0.25, 7.0),
        Ground::Earth => (0.4, 5.5),
    }
}

impl Prop {
    pub fn is_tree(self) -> bool {
        matches!(
            self,
            Self::Fir
                | Self::Pine
                | Self::SnowFir
                | Self::Broadleaf
                | Self::AutumnTree
                | Self::BareTree
                | Self::Joshua
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Clear,
    Rain,
    Snow,
    Dust,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horizon {
    Hills,
    Peaks,
    Mesas,
    Flat,
}

#[derive(Clone, Copy, Debug)]
pub struct Biome {
    pub name: &'static str,
    pub surface: &'static str,
    pub road: Road,
    pub ground: Ground,
    pub sky_top: Rgb,
    pub sky_horizon: Rgb,
    pub fog: Rgb,
    pub sun: Rgb,
    pub ambient: Rgb,
    pub ground_dark: Rgb,
    pub ground_light: Rgb,
    pub road_dark: Rgb,
    pub road_light: Rgb,
    pub road_paint: Rgb,
    pub foliage_dark: Rgb,
    pub foliage_light: Rgb,
    pub horizon_color: Rgb,
    pub horizon: Horizon,
    pub dust: Rgb,
    pub props: &'static [(Prop, f32)],
    pub density: f32,
    pub weather: Weather,
    pub grip: f32,
    pub hilliness: f32,
    pub bank_height: f32,
}

pub const BIOMES: [Biome; 5] = [
    Biome {
        name: "PINE FOREST",
        surface: "GRAVEL",
        road: Road::Gravel,
        ground: Ground::Grass,
        sky_top: [70, 100, 150],
        sky_horizon: [170, 185, 195],
        fog: [150, 165, 170],
        sun: [210, 205, 190],
        ambient: [70, 80, 85],
        ground_dark: [30, 60, 25],
        ground_light: [80, 120, 50],
        road_dark: [90, 75, 60],
        road_light: [170, 150, 120],
        road_paint: [150, 130, 105],
        foliage_dark: [15, 45, 25],
        foliage_light: [50, 110, 60],
        horizon_color: [60, 85, 90],
        horizon: Horizon::Hills,
        dust: [150, 130, 100],
        props: &[
            (Prop::Fir, 5.0),
            (Prop::Pine, 2.0),
            (Prop::Bush, 1.5),
            (Prop::Rock, 0.8),
            (Prop::SmallRock, 1.2),
            (Prop::Spectator, 0.4),
        ],
        density: 0.75,
        weather: Weather::Rain,
        grip: 0.7,
        hilliness: 1.0,
        bank_height: 2.5,
    },
    Biome {
        name: "FARMLAND",
        surface: "TARMAC",
        road: Road::Tarmac,
        ground: Ground::Wheat,
        sky_top: [60, 110, 200],
        sky_horizon: [190, 210, 230],
        fog: [190, 205, 220],
        sun: [240, 230, 200],
        ambient: [80, 80, 90],
        ground_dark: [150, 120, 40],
        ground_light: [230, 200, 100],
        road_dark: [60, 60, 65],
        road_light: [110, 110, 115],
        road_paint: [235, 235, 225],
        foliage_dark: [30, 70, 20],
        foliage_light: [90, 150, 50],
        horizon_color: [110, 140, 110],
        horizon: Horizon::Flat,
        dust: [180, 170, 140],
        props: &[
            (Prop::Broadleaf, 3.0),
            (Prop::HayBale, 2.0),
            (Prop::FencePost, 3.0),
            (Prop::Bush, 1.0),
            (Prop::Spectator, 0.6),
        ],
        density: 0.45,
        weather: Weather::Clear,
        grip: 1.0,
        hilliness: 0.5,
        bank_height: 0.3,
    },
    Biome {
        name: "RED CANYON",
        surface: "SAND",
        road: Road::Sand,
        ground: Ground::Earth,
        sky_top: [80, 130, 210],
        sky_horizon: [240, 200, 160],
        fog: [230, 190, 150],
        sun: [255, 230, 190],
        ambient: [100, 70, 60],
        ground_dark: [150, 70, 35],
        ground_light: [230, 150, 90],
        road_dark: [170, 110, 70],
        road_light: [235, 180, 120],
        road_paint: [220, 160, 105],
        foliage_dark: [40, 80, 40],
        foliage_light: [100, 150, 70],
        horizon_color: [170, 80, 50],
        horizon: Horizon::Mesas,
        dust: [220, 160, 110],
        props: &[
            (Prop::Joshua, 2.0),
            (Prop::Cactus, 2.0),
            (Prop::Scrub, 3.0),
            (Prop::Rock, 2.0),
            (Prop::SmallRock, 2.0),
            (Prop::Boulder, 0.8),
            (Prop::Spectator, 0.3),
        ],
        density: 0.35,
        weather: Weather::Dust,
        grip: 0.5,
        hilliness: 1.4,
        bank_height: 6.0,
    },
    Biome {
        name: "ALPINE PASS",
        surface: "SNOW",
        road: Road::Snow,
        ground: Ground::Snow,
        sky_top: [110, 130, 160],
        sky_horizon: [200, 210, 225],
        fog: [205, 215, 230],
        sun: [220, 225, 240],
        ambient: [100, 110, 130],
        ground_dark: [150, 160, 185],
        ground_light: [240, 245, 255],
        road_dark: [150, 155, 170],
        road_light: [225, 230, 240],
        road_paint: [120, 110, 100],
        foliage_dark: [20, 45, 45],
        foliage_light: [60, 100, 90],
        horizon_color: [150, 160, 185],
        horizon: Horizon::Peaks,
        dust: [240, 245, 255],
        props: &[
            (Prop::SnowFir, 6.0),
            (Prop::SnowRock, 2.0),
            (Prop::Boulder, 0.5),
            (Prop::Spectator, 0.3),
        ],
        density: 0.6,
        weather: Weather::Snow,
        grip: 0.32,
        hilliness: 1.8,
        bank_height: 4.0,
    },
    Biome {
        name: "AUTUMN VALLEY",
        surface: "MUD",
        road: Road::Mud,
        ground: Ground::Leaves,
        sky_top: [90, 90, 120],
        sky_horizon: [210, 160, 130],
        fog: [170, 130, 110],
        sun: [240, 190, 140],
        ambient: [80, 60, 60],
        ground_dark: [90, 55, 25],
        ground_light: [170, 110, 40],
        road_dark: [70, 55, 45],
        road_light: [140, 115, 90],
        road_paint: [125, 100, 80],
        foliage_dark: [110, 35, 10],
        foliage_light: [235, 140, 30],
        horizon_color: [95, 60, 70],
        horizon: Horizon::Hills,
        dust: [120, 95, 70],
        props: &[
            (Prop::AutumnTree, 4.0),
            (Prop::BareTree, 2.0),
            (Prop::Pine, 1.0),
            (Prop::Bush, 1.0),
            (Prop::FencePost, 1.0),
            (Prop::Spectator, 0.5),
        ],
        density: 0.65,
        weather: Weather::Rain,
        grip: 0.58,
        hilliness: 1.2,
        bank_height: 1.5,
    },
];

pub const SECTION_LENGTH: f32 = 1800.0;
pub const BLEND_LENGTH: f32 = 350.0;

pub struct BiomeSchedule {
    order: Vec<usize>,
}

impl BiomeSchedule {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0xb10e);
        let mut order = vec![0];
        while order.len() < 256 {
            let mut cycle: Vec<usize> = (0..BIOMES.len()).collect();
            for index in (1..cycle.len()).rev() {
                cycle.swap(index, rng.below(index as u32 + 1) as usize);
            }
            for biome in cycle {
                if *order.last().unwrap() != biome {
                    order.push(biome);
                }
            }
        }
        Self { order }
    }

    pub fn section(&self, distance: f32) -> usize {
        (distance.max(0.0) / SECTION_LENGTH) as usize
    }

    pub fn biome_index(&self, section: usize) -> usize {
        self.order[section % self.order.len()]
    }

    pub fn at(&self, distance: f32) -> (usize, usize, f32) {
        let section = self.section(distance);
        let into = distance.max(0.0) - section as f32 * SECTION_LENGTH;
        let current = self.biome_index(section);
        let next = self.biome_index(section + 1);
        let blend_start = SECTION_LENGTH - BLEND_LENGTH;
        let t = ((into - blend_start) / BLEND_LENGTH).clamp(0.0, 1.0);
        (current, next, t * t * (3.0 - 2.0 * t))
    }

    pub fn mix<T>(&self, distance: f32, field: impl Fn(&Biome) -> T) -> (T, T, f32) {
        let (a, b, t) = self.at(distance);
        (field(&BIOMES[a]), field(&BIOMES[b]), t)
    }

    pub fn color(&self, distance: f32, field: impl Fn(&Biome) -> Rgb) -> Rgb {
        let (a, b, t) = self.mix(distance, field);
        lerp_rgb(a, b, t)
    }

    pub fn scalar(&self, distance: f32, field: impl Fn(&Biome) -> f32) -> f32 {
        let (a, b, t) = self.mix(distance, field);
        a + (b - a) * t
    }
}

pub fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|c| (a[c] as f32 + (b[c] as f32 - a[c] as f32) * t).round() as u8)
}

pub fn scale_rgb(color: Rgb, factor: f32) -> Rgb {
    color.map(|c| (c as f32 * factor).clamp(0.0, 255.0) as u8)
}
