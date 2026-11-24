use psxel_engine::{Audio, SoundId};

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/sounds/", $name, ".wav"))
    };
}

#[derive(Clone, Copy, Default)]
pub struct Voice {
    pub volume: f32,
    pub pan: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Mix {
    pub ambience: Voice,
    pub engine: Voice,
    pub engine_rate: f32,
    pub bubbles: Voice,
    pub whale: Voice,
    pub creak: Voice,
    pub alien: Voice,
    pub sonar: bool,
}

struct Bank {
    ambience: SoundId,
    engine: SoundId,
    bubbles: SoundId,
    whale: SoundId,
    creak: SoundId,
    alien: SoundId,
    sonar: SoundId,
}

#[derive(Default)]
pub struct SoundSystem {
    bank: Option<Bank>,
    pub muted: bool,
    master: f32,
}

impl SoundSystem {
    pub fn update(&mut self, audio: &mut Audio, mix: &Mix) {
        let bank = self.bank.get_or_insert_with(|| Bank {
            ambience: audio.load_wav(asset!("dive_ambience")),
            engine: audio.load_wav(asset!("dive_engine")),
            bubbles: audio.load_wav(asset!("dive_bubbles")),
            whale: audio.load_wav(asset!("dive_whale")),
            creak: audio.load_wav(asset!("dive_creak")),
            alien: audio.load_wav(asset!("dive_alien")),
            sonar: audio.load_wav(asset!("dive_sonar")),
        });
        if !audio.ready() {
            return;
        }
        let target = if self.muted { 0.0 } else { 0.8 };
        if (target - self.master).abs() > 0.001 {
            self.master = target;
            audio.set_master(target);
        }
        let loops = [
            (bank.ambience, mix.ambience, 1.0),
            (bank.engine, mix.engine, mix.engine_rate),
            (bank.bubbles, mix.bubbles, 1.0),
            (bank.whale, mix.whale, 1.0),
            (bank.creak, mix.creak, 1.0),
            (bank.alien, mix.alien, 1.0),
        ];
        for (channel, (sound, voice, rate)) in loops.into_iter().enumerate() {
            audio.set_loop_panned(channel, sound, voice.volume, rate, voice.pan);
        }
        if mix.sonar {
            audio.play(bank.sonar, 0.35, 1.0);
        }
    }
}
