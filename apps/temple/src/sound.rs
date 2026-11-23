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

impl Voice {
    pub fn ambient(volume: f32) -> Self {
        Self { volume, pan: 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Mix {
    pub jungle: Voice,
    pub waterfall: Voice,
    pub stream: Voice,
    pub drone: Voice,
    pub drips: Voice,
    pub torch: Voice,
    pub cascade: Voice,
}

struct Bank {
    jungle: SoundId,
    waterfall: SoundId,
    stream: SoundId,
    drone: SoundId,
    drips: SoundId,
    torch: SoundId,
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
            jungle: audio.load_wav(asset!("temple_jungle")),
            waterfall: audio.load_wav(asset!("temple_waterfall")),
            stream: audio.load_wav(asset!("temple_stream")),
            drone: audio.load_wav(asset!("temple_drone")),
            drips: audio.load_wav(asset!("temple_drips")),
            torch: audio.load_wav(asset!("temple_torch")),
        });
        if !audio.ready() {
            return;
        }
        let target = if self.muted { 0.0 } else { 0.8 };
        if (target - self.master).abs() > 0.001 {
            self.master = target;
            audio.set_master(target);
        }
        let channels = [
            (bank.jungle, mix.jungle),
            (bank.waterfall, mix.waterfall),
            (bank.stream, mix.stream),
            (bank.drone, mix.drone),
            (bank.drips, mix.drips),
            (bank.torch, mix.torch),
        ];
        for (channel, (sound, voice)) in channels.into_iter().enumerate() {
            audio.set_loop_panned(channel, sound, voice.volume, 1.0, voice.pan);
        }
        audio.set_loop_panned(
            channels.len(),
            bank.waterfall,
            mix.cascade.volume,
            1.35,
            mix.cascade.pan,
        );
    }
}
