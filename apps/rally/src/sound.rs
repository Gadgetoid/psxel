use psxel_engine::{Audio, SoundId};

use crate::biome::Weather;
use crate::car::Ground;

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/sounds/", $name, ".wav"))
    };
}

const ENGINE_RPMS: [f32; 6] = [1000.0, 2000.0, 3000.0, 4500.0, 6000.0, 7500.0];
const ENGINE_CHANNEL: usize = 0;
const SURFACE_CHANNEL: usize = 10;
const SKID_LOOSE_CHANNEL: usize = 20;
const SKID_TARMAC_CHANNEL: usize = 21;
const WIND_CHANNEL: usize = 22;
const RAIN_CHANNEL: usize = 23;
const STREAM_CHANNEL: usize = 24;
const STREAM_VOLUME: f32 = 0.5;
const MUSIC_CHANNEL: usize = 30;
const MUSIC_VOLUME: f32 = 0.35;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Gravel,
    Tarmac,
    Sand,
    Snow,
}

impl Surface {
    pub fn from_name(name: &str) -> Self {
        match name {
            "TARMAC" => Self::Tarmac,
            "SAND" => Self::Sand,
            "SNOW" => Self::Snow,
            _ => Self::Gravel,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Effect {
    Thud,
    Pop,
    Call,
    Splash,
}

#[derive(Clone, Copy, Default)]
pub struct Mix {
    pub rpm: f32,
    pub throttle: f32,
    pub speed: f32,
    pub slide: f32,
    pub handbrake: bool,
    pub airborne: bool,
    pub ground: Option<Ground>,
    pub surface: Option<Surface>,
    pub weather: Option<Weather>,
    pub weather_amount: f32,
    pub engine_volume: f32,
    pub stream: (f32, f32),
}

struct Bank {
    engines: Vec<SoundId>,
    surfaces: [(Surface, SoundId); 4],
    skid_loose: SoundId,
    skid_tarmac: SoundId,
    wind: SoundId,
    rain: SoundId,
    stream: SoundId,
    thud: SoundId,
    pop: SoundId,
    call: SoundId,
    splash: SoundId,
    music: SoundId,
}

pub struct SoundSystem {
    bank: Option<Bank>,
    pending: Vec<(Effect, f32, f32)>,
    pub muted: bool,
    pub music: bool,
    master: f32,
}

impl Default for SoundSystem {
    fn default() -> Self {
        Self {
            bank: None,
            pending: Vec::new(),
            muted: false,
            music: true,
            master: 0.0,
        }
    }
}

impl SoundSystem {
    pub fn trigger(&mut self, effect: Effect, volume: f32, rate: f32) {
        if self.pending.len() < 8 {
            self.pending.push((effect, volume, rate));
        }
    }

    fn load(audio: &mut Audio) -> Bank {
        let engines = vec![
            audio.load_wav(asset!("rally_engine_1000")),
            audio.load_wav(asset!("rally_engine_2000")),
            audio.load_wav(asset!("rally_engine_3000")),
            audio.load_wav(asset!("rally_engine_4500")),
            audio.load_wav(asset!("rally_engine_6000")),
            audio.load_wav(asset!("rally_engine_7500")),
        ];
        Bank {
            engines,
            surfaces: [
                (Surface::Gravel, audio.load_wav(asset!("rally_gravel"))),
                (Surface::Tarmac, audio.load_wav(asset!("rally_tarmac"))),
                (Surface::Sand, audio.load_wav(asset!("rally_sand"))),
                (Surface::Snow, audio.load_wav(asset!("rally_snow"))),
            ],
            skid_loose: audio.load_wav(asset!("rally_skid_loose")),
            skid_tarmac: audio.load_wav(asset!("rally_skid_tarmac")),
            wind: audio.load_wav(asset!("rally_wind")),
            rain: audio.load_wav(asset!("amb_rain")),
            stream: audio.load_wav(asset!("temple_stream")),
            thud: audio.load_wav(asset!("rally_thud")),
            pop: audio.load_wav(asset!("rally_pop")),
            call: audio.load_wav(asset!("rally_call")),
            splash: audio.load_wav(asset!("rally_splash")),
            music: {
                let stream = psxel_spu::decode_file(include_bytes!(
                    "../../../assets/sounds/rally_music.spu"
                ))
                .expect("music stream");
                audio.load_samples(
                    stream.rate,
                    stream
                        .samples
                        .iter()
                        .map(|&sample| sample as f32 / 32768.0)
                        .collect(),
                )
            },
        }
    }

    pub fn update(&mut self, audio: &mut Audio, mix: &Mix) {
        if self.bank.is_none() {
            self.bank = Some(Self::load(audio));
        }
        if !audio.ready() {
            self.pending.clear();
            return;
        }
        let target = if self.muted { 0.0 } else { 0.8 };
        if (target - self.master).abs() > 0.001 {
            self.master = target;
            audio.set_master(target);
        }
        let bank = self.bank.as_ref().unwrap();

        let rpm = mix.rpm.clamp(ENGINE_RPMS[0] * 0.8, ENGINE_RPMS[5] * 1.1);
        let upper = ENGINE_RPMS
            .iter()
            .position(|&r| r >= rpm)
            .unwrap_or(ENGINE_RPMS.len() - 1)
            .max(1);
        let lower = upper - 1;
        let blend = ((rpm - ENGINE_RPMS[lower]) / (ENGINE_RPMS[upper] - ENGINE_RPMS[lower]))
            .clamp(0.0, 1.0);
        let load = 0.45 + 0.55 * mix.throttle.max(0.0);
        for (index, &sound) in bank.engines.iter().enumerate() {
            let weight = if index == lower {
                1.0 - blend
            } else if index == upper {
                blend
            } else {
                0.0
            };
            let volume = weight * load * mix.engine_volume * 0.55;
            audio.set_loop(
                ENGINE_CHANNEL + index,
                sound,
                volume,
                rpm / ENGINE_RPMS[index],
            );
        }

        let grounded = !mix.airborne;
        let road_surface = match mix.ground {
            Some(Ground::Road) => mix.surface,
            Some(_) => Some(if mix.surface == Some(Surface::Snow) {
                Surface::Snow
            } else {
                Surface::Gravel
            }),
            None => None,
        };
        let speed_level = (mix.speed / 28.0).clamp(0.0, 1.0);
        for (index, (surface, sound)) in bank.surfaces.iter().enumerate() {
            let active = grounded && road_surface == Some(*surface);
            let volume = if active { speed_level * 0.7 } else { 0.0 };
            audio.set_loop(
                SURFACE_CHANNEL + index,
                *sound,
                volume,
                0.8 + speed_level * 0.5,
            );
        }

        let slide = if grounded {
            ((mix.slide - 1.5) / 7.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let slide = if mix.handbrake && grounded && mix.speed > 3.0 {
            slide.max(0.6)
        } else {
            slide
        };
        let tarmac = mix.ground == Some(Ground::Road) && mix.surface == Some(Surface::Tarmac);
        audio.set_loop(
            SKID_LOOSE_CHANNEL,
            bank.skid_loose,
            if tarmac { 0.0 } else { slide * 0.6 },
            0.9 + speed_level * 0.3,
        );
        audio.set_loop(
            SKID_TARMAC_CHANNEL,
            bank.skid_tarmac,
            if tarmac { slide * 0.45 } else { 0.0 },
            0.95 + slide * 0.1,
        );
        audio.set_loop(
            WIND_CHANNEL,
            bank.wind,
            speed_level * speed_level * 0.5,
            0.8 + speed_level * 0.6,
        );
        let rain = if matches!(mix.weather, Some(Weather::Rain)) {
            mix.weather_amount * 0.32
        } else {
            0.0
        };
        audio.set_loop(RAIN_CHANNEL, bank.rain, rain, 1.0);
        audio.set_loop_panned(
            STREAM_CHANNEL,
            bank.stream,
            mix.stream.0 * STREAM_VOLUME,
            1.0,
            mix.stream.1,
        );
        audio.set_loop(
            MUSIC_CHANNEL,
            bank.music,
            if self.music { MUSIC_VOLUME } else { 0.0 },
            1.0,
        );

        for (effect, volume, rate) in self.pending.drain(..) {
            let sound = match effect {
                Effect::Thud => bank.thud,
                Effect::Pop => bank.pop,
                Effect::Call => bank.call,
                Effect::Splash => bank.splash,
            };
            audio.play(sound, volume, rate);
        }
    }
}
