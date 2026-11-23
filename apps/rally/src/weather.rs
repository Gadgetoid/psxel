use crate::art::Ground;
use crate::biome::{Weather, BIOMES};
use crate::rng::value_noise;
use crate::track::Track;

const FRONT_LENGTH: f32 = 650.0;
const GUST_LENGTH: f32 = 180.0;
const LIGHTEST: f32 = 0.2;
const STORM_START: f32 = 0.66;
const STORM_RANGE: f32 = 0.22;
const DUST_STORM_START: f32 = 0.76;
const DUST_STORM_RANGE: f32 = 0.14;
const DUST_HAZE: f32 = 0.35;
const RAIN_OVERCAST: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    pub precipitation: f32,
    pub storm: f32,
    pub thunder: f32,
    pub overcast: f32,
    pub dust: f32,
    pub dust_storm: f32,
    pub wind: f32,
    pub birds: f32,
    pub insects: f32,
}

fn wind_base(ground: Ground) -> f32 {
    match ground {
        Ground::Snow => 0.7,
        Ground::Earth => 0.6,
        Ground::Wheat => 0.45,
        Ground::Leaves => 0.35,
        Ground::Grass => 0.3,
        Ground::Jungle => 0.15,
    }
}

fn birdsong(ground: Ground) -> f32 {
    match ground {
        Ground::Grass => 1.0,
        Ground::Leaves => 0.8,
        Ground::Wheat => 0.7,
        Ground::Snow => 0.2,
        Ground::Earth => 0.1,
        Ground::Jungle => 0.0,
    }
}

fn crickets(ground: Ground) -> f32 {
    match ground {
        Ground::Wheat => 1.0,
        Ground::Earth => 0.8,
        Ground::Leaves => 0.6,
        Ground::Grass => 0.5,
        Ground::Snow | Ground::Jungle => 0.0,
    }
}

pub fn conditions(track: &Track, distance: f32, daylight: f32) -> Conditions {
    let (a, b, t) = track.biomes.at(distance);
    let (biome, settled) = if t < 0.5 {
        (a, 1.0 - t * 2.0)
    } else {
        (b, t * 2.0 - 1.0)
    };
    let mix = |field: fn(Ground) -> f32| {
        field(BIOMES[a].ground) * (1.0 - t) + field(BIOMES[b].ground) * t
    };
    let weather = BIOMES[biome].weather;
    let front = value_noise(track.seed ^ 0x3e47, distance / FRONT_LENGTH);
    let wet = matches!(weather, Weather::Rain | Weather::Snow);
    let precipitation = if wet {
        settled * (LIGHTEST + (1.0 - LIGHTEST) * front)
    } else {
        0.0
    };
    let ramp = |start: f32, range: f32| {
        let amount = ((front - start) / range).clamp(0.0, 1.0);
        settled * amount * amount * (3.0 - 2.0 * amount)
    };
    let dusty = weather == Weather::Dust;
    let storm = if wet {
        ramp(STORM_START, STORM_RANGE)
    } else if dusty {
        ramp(DUST_STORM_START, DUST_STORM_RANGE)
    } else {
        0.0
    };
    let dust_storm = if dusty { storm } else { 0.0 };
    let gust = value_noise(track.seed ^ 0x9057, distance / GUST_LENGTH);
    let wind = (mix(wind_base) * (0.5 + gust) + storm * 0.5).min(1.0);
    let shelter = 1.0 - precipitation * 0.7;
    Conditions {
        precipitation,
        storm,
        thunder: if weather == Weather::Rain { storm } else { 0.0 },
        overcast: if wet {
            storm.max(precipitation * RAIN_OVERCAST)
        } else {
            0.0
        },
        dust: if dusty {
            settled * (DUST_HAZE + (1.0 - DUST_HAZE) * storm)
        } else {
            0.0
        },
        dust_storm,
        wind,
        birds: mix(birdsong) * daylight * shelter,
        insects: mix(crickets) * (1.0 - daylight) * shelter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_varies_along_the_stage() {
        let mut track = Track::new(4);
        track.ensure(40);
        let samples: Vec<(Weather, Conditions)> = (0..4000)
            .map(|step| {
                let distance = step as f32 * 25.0;
                let (a, b, t) = track.biomes.at(distance);
                let weather = BIOMES[if t < 0.5 { a } else { b }].weather;
                (weather, conditions(&track, distance, 1.0))
            })
            .collect();
        let dry = samples
            .iter()
            .filter(|(weather, _)| matches!(weather, Weather::Clear | Weather::Dust));
        assert!(dry
            .clone()
            .all(|(_, conditions)| conditions.precipitation == 0.0));
        let wet: Vec<f32> = samples
            .iter()
            .filter(|(weather, _)| *weather == Weather::Rain)
            .map(|(_, conditions)| conditions.precipitation)
            .collect();
        let lightest = wet.iter().copied().fold(1.0, f32::min);
        let heaviest = wet.iter().copied().fold(0.0, f32::max);
        assert!(heaviest - lightest > 0.4, "{lightest} {heaviest}");
        assert!(samples.iter().any(|(_, conditions)| conditions.storm > 0.5));
        assert!(samples
            .iter()
            .all(|(_, conditions)| (0.0..=1.0).contains(&conditions.wind)));
    }
}
