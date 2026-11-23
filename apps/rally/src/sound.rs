use psxel_engine::{Audio, SoundId};

use crate::biome::Weather;
use crate::car::Ground;
use crate::music::{Cue, Music};

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
const JUNGLE_CHANNEL: usize = 24;
const BIRDS_CHANNEL: usize = 25;
const STREAM_CHANNEL: usize = 26;
const JUNGLE_VOLUME: f32 = 0.4;
const BIRDS_VOLUME: f32 = 0.3;
const STREAM_VOLUME: f32 = 0.5;
const STORM_CHANNEL: usize = 27;
const GUSTS_CHANNEL: usize = 28;
const COUNTRY_BIRDS_CHANNEL: usize = 29;
const NIGHT_CHANNEL: usize = 33;
const STORM_VOLUME: f32 = 0.45;
const GUSTS_VOLUME: f32 = 0.35;
const COUNTRY_BIRDS_VOLUME: f32 = 0.22;
const NIGHT_VOLUME: f32 = 0.3;
const RAIN_VOLUME: f32 = 0.32;
const MUSIC_VOLUME: f32 = 0.7;
const MUSIC_SPEED_LIFT: f32 = 0.25;

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
    Crackle,
    Thunder,
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
    pub precipitation: f32,
    pub storm: f32,
    pub gusts: f32,
    pub country_birds: f32,
    pub insects: f32,
    pub biomes: (usize, usize, f32),
    pub clock: f32,
    pub rhythm: f32,
    pub reverb: f32,
    pub engine_volume: f32,
    pub jungle: f32,
    pub birds: f32,
    pub stream: (f32, f32),
}

struct Bank {
    engines: Vec<SoundId>,
    surfaces: [(Surface, SoundId); 4],
    skid_loose: SoundId,
    skid_tarmac: SoundId,
    wind: SoundId,
    rain: SoundId,
    jungle: SoundId,
    birds: SoundId,
    stream: SoundId,
    thud: SoundId,
    pop: SoundId,
    call: SoundId,
    splash: SoundId,
    storm: SoundId,
    gusts: SoundId,
    country_birds: SoundId,
    night: SoundId,
    crackle: SoundId,
    thunder: SoundId,
}

pub struct SoundSystem {
    bank: Option<Bank>,
    player: Option<Music>,
    pending: Vec<(Effect, f32, f32)>,
    scheduled: Vec<(u32, Effect, f32, f32)>,
    pub muted: bool,
    pub music: bool,
    master: f32,
}

impl Default for SoundSystem {
    fn default() -> Self {
        Self {
            bank: None,
            player: None,
            pending: Vec::new(),
            scheduled: Vec::new(),
            muted: false,
            music: true,
            master: 0.0,
        }
    }
}

impl SoundSystem {
    pub fn trigger_after(&mut self, frames: u32, effect: Effect, volume: f32, rate: f32) {
        if self.scheduled.len() < 16 {
            self.scheduled.push((frames, effect, volume, rate));
        }
    }

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
            jungle: audio.load_wav(asset!("temple_jungle")),
            birds: audio.load_wav(asset!("temple_birds")),
            stream: audio.load_wav(asset!("temple_stream")),
            thud: audio.load_wav(asset!("rally_thud")),
            pop: audio.load_wav(asset!("rally_pop")),
            call: audio.load_wav(asset!("rally_call")),
            splash: audio.load_wav(asset!("rally_splash")),
            storm: audio.load_wav(asset!("rally_storm")),
            gusts: audio.load_wav(asset!("rally_gusts")),
            country_birds: audio.load_wav(asset!("rally_birds")),
            night: audio.load_wav(asset!("rally_night")),
            crackle: audio.load_wav(asset!("rally_crackle")),
            thunder: audio.load_wav(asset!("rally_thunder")),
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
            mix.precipitation * (1.0 - mix.storm * 0.6) * RAIN_VOLUME
        } else {
            0.0
        };
        audio.set_loop(RAIN_CHANNEL, bank.rain, rain, 1.0);
        let storm = if matches!(mix.weather, Some(Weather::Rain)) {
            mix.storm
        } else {
            0.0
        };
        audio.set_loop(STORM_CHANNEL, bank.storm, storm * STORM_VOLUME, 1.0);
        audio.set_loop(
            GUSTS_CHANNEL,
            bank.gusts,
            mix.gusts * GUSTS_VOLUME,
            0.9 + mix.gusts * 0.2,
        );
        audio.set_loop(
            COUNTRY_BIRDS_CHANNEL,
            bank.country_birds,
            mix.country_birds * COUNTRY_BIRDS_VOLUME,
            1.0,
        );
        audio.set_loop(NIGHT_CHANNEL, bank.night, mix.insects * NIGHT_VOLUME, 1.0);
        audio.set_loop(JUNGLE_CHANNEL, bank.jungle, mix.jungle * JUNGLE_VOLUME, 1.0);
        audio.set_loop(BIRDS_CHANNEL, bank.birds, mix.birds * BIRDS_VOLUME, 1.0);
        audio.set_loop_panned(
            STREAM_CHANNEL,
            bank.stream,
            mix.stream.0 * STREAM_VOLUME,
            1.0,
            mix.stream.1,
        );
        audio.set_reverb(mix.reverb);
        let player = self.player.get_or_insert_with(|| Music::load(audio));
        if self.music {
            let cue = Cue {
                blend: player.score.blend(mix.biomes, mix.clock),
                rhythm: mix.rhythm,
                volume: MUSIC_VOLUME * (1.0 + MUSIC_SPEED_LIFT * mix.rhythm),
            };
            player.update(audio, &cue);
        }

        for scheduled in &mut self.scheduled {
            scheduled.0 = scheduled.0.saturating_sub(1);
        }
        let due: Vec<(Effect, f32, f32)> = self
            .scheduled
            .iter()
            .filter(|scheduled| scheduled.0 == 0)
            .map(|&(_, effect, volume, rate)| (effect, volume, rate))
            .collect();
        self.scheduled.retain(|scheduled| scheduled.0 > 0);
        self.pending.extend(due);
        for (effect, volume, rate) in self.pending.drain(..) {
            let sound = match effect {
                Effect::Thud => bank.thud,
                Effect::Pop => bank.pop,
                Effect::Crackle => bank.crackle,
                Effect::Thunder => bank.thunder,
                Effect::Call => bank.call,
                Effect::Splash => bank.splash,
            };
            audio.play(sound, volume, rate);
        }
    }
}
