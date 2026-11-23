pub type SoundId = usize;

#[derive(Clone)]
pub struct Sound {
    pub rate: u32,
    pub samples: Vec<f32>,
}

pub fn decode_wav(bytes: &[u8]) -> Option<Sound> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let read_u32 =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (mut channels, mut rate, mut bits) = (0u16, 0u32, 0u16);
    let mut cursor = 12;
    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let size = read_u32(cursor + 4) as usize;
        let body = cursor + 8;
        if body + size > bytes.len() {
            return None;
        }
        if id == b"fmt " {
            channels = read_u16(body + 2);
            rate = read_u32(body + 4);
            bits = read_u16(body + 14);
        } else if id == b"data" {
            if bits != 16 || channels == 0 {
                return None;
            }
            let frame = channels as usize * 2;
            let samples = bytes[body..body + size]
                .chunks_exact(frame)
                .map(|frame| i16::from_le_bytes([frame[0], frame[1]]) as f32 / 32768.0)
                .collect();
            return Some(Sound { rate, samples });
        }
        cursor = body + size + (size & 1);
    }
    None
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use std::collections::HashMap;

    use web_sys::{
        AudioBuffer, AudioBufferSourceNode, AudioContext, ConvolverNode, GainNode, StereoPannerNode,
    };

    use super::{Sound, SoundId};

    const REVERB_SECONDS: f32 = 2.6;
    const REVERB_DECAY: f32 = 6.0;
    const SEND: f32 = 0.35;

    struct Voice {
        sound: SoundId,
        source: AudioBufferSourceNode,
        gain: GainNode,
        panner: Option<StereoPannerNode>,
    }

    pub struct Backend {
        context: AudioContext,
        master: GainNode,
        reverb: Option<(ConvolverNode, GainNode)>,
        buffers: Vec<Option<AudioBuffer>>,
        voices: HashMap<usize, Voice>,
    }

    impl Backend {
        pub fn new() -> Option<Self> {
            let context = AudioContext::new().ok()?;
            let master = context.create_gain().ok()?;
            master
                .connect_with_audio_node(&context.destination())
                .ok()?;
            let reverb = Self::reverb(&context, &master);
            Some(Self {
                context,
                master,
                reverb,
                buffers: Vec::new(),
                voices: HashMap::new(),
            })
        }

        fn reverb(context: &AudioContext, master: &GainNode) -> Option<(ConvolverNode, GainNode)> {
            let rate = context.sample_rate();
            let length = (rate * REVERB_SECONDS) as usize;
            let response = context.create_buffer(2, length as u32, rate).ok()?;
            for channel in 0..2 {
                let mut seed = 0x9e37_79b9u32 ^ (channel as u32 * 0x85eb_ca6b);
                let samples: Vec<f32> = (0..length)
                    .map(|index| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let noise = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
                        let time = index as f32 / length as f32;
                        noise * (-time * REVERB_DECAY).exp()
                    })
                    .collect();
                response.copy_to_channel(&samples, channel).ok()?;
            }
            let convolver = context.create_convolver().ok()?;
            convolver.set_buffer(Some(&response));
            let wet = context.create_gain().ok()?;
            wet.gain().set_value(0.0);
            convolver.connect_with_audio_node(&wet).ok()?;
            wet.connect_with_audio_node(master).ok()?;
            Some((convolver, wet))
        }

        pub fn now(&self) -> f64 {
            self.context.current_time()
        }

        pub fn set_reverb(&mut self, level: f32) {
            if let Some((_, wet)) = &self.reverb {
                let now = self.context.current_time();
                let _ = wet.gain().set_target_at_time(level, now, 0.3);
            }
        }

        pub fn play_at(
            &mut self,
            sounds: &[Sound],
            id: SoundId,
            when: f64,
            volume: f32,
            send: f32,
        ) {
            let Some((source, gain, _)) = self.source(sounds, id, false) else {
                return;
            };
            gain.gain().set_value(volume);
            self.send(&gain, send);
            let _ = source.start_with_when(when);
        }

        fn send(&self, gain: &GainNode, amount: f32) {
            let Some((convolver, _)) = &self.reverb else {
                return;
            };
            let Ok(send) = self.context.create_gain() else {
                return;
            };
            send.gain().set_value(amount);
            if gain.connect_with_audio_node(&send).is_ok() {
                let _ = send.connect_with_audio_node(convolver);
            }
        }

        pub fn resume(&self) {
            if self.context.state() == web_sys::AudioContextState::Running {
                return;
            }
            let _ = self.context.resume();
        }

        fn buffer(&mut self, sounds: &[Sound], id: SoundId) -> Option<AudioBuffer> {
            if self.buffers.len() <= id {
                self.buffers.resize(id + 1, None);
            }
            if self.buffers[id].is_none() {
                let sound = sounds.get(id)?;
                let buffer = self
                    .context
                    .create_buffer(1, sound.samples.len() as u32, sound.rate as f32)
                    .ok()?;
                buffer.copy_to_channel(&sound.samples, 0).ok()?;
                self.buffers[id] = Some(buffer);
            }
            self.buffers[id].clone()
        }

        fn source(
            &mut self,
            sounds: &[Sound],
            id: SoundId,
            looping: bool,
        ) -> Option<(AudioBufferSourceNode, GainNode, Option<StereoPannerNode>)> {
            let buffer = self.buffer(sounds, id)?;
            let source = self.context.create_buffer_source().ok()?;
            source.set_buffer(Some(&buffer));
            source.set_loop(looping);
            let gain = self.context.create_gain().ok()?;
            source.connect_with_audio_node(&gain).ok()?;
            let panner = self.context.create_stereo_panner().ok();
            match &panner {
                Some(panner) => {
                    gain.connect_with_audio_node(panner).ok()?;
                    panner.connect_with_audio_node(&self.master).ok()?;
                }
                None => {
                    gain.connect_with_audio_node(&self.master).ok()?;
                }
            }
            Some((source, gain, panner))
        }

        pub fn set_loop(
            &mut self,
            sounds: &[Sound],
            channel: usize,
            id: SoundId,
            volume: f32,
            rate: f32,
            pan: f32,
        ) {
            let now = self.context.current_time();
            let restart = self
                .voices
                .get(&channel)
                .is_none_or(|voice| voice.sound != id);
            if restart {
                if let Some(old) = self.voices.remove(&channel) {
                    let _ = old.gain.gain().set_target_at_time(0.0, now, 0.02);
                    let _ =
                        web_sys::AudioScheduledSourceNode::stop_with_when(&old.source, now + 0.1);
                }
                let Some((source, gain, panner)) = self.source(sounds, id, true) else {
                    return;
                };
                gain.gain().set_value(0.0);
                self.send(&gain, SEND);
                let _ = source.start();
                self.voices.insert(
                    channel,
                    Voice {
                        sound: id,
                        source,
                        gain,
                        panner,
                    },
                );
            }
            if let Some(voice) = self.voices.get(&channel) {
                let _ = voice.gain.gain().set_target_at_time(volume, now, 0.04);
                let _ = voice
                    .source
                    .playback_rate()
                    .set_target_at_time(rate.max(0.01), now, 0.03);
                if let Some(panner) = &voice.panner {
                    let _ = panner
                        .pan()
                        .set_target_at_time(pan.clamp(-1.0, 1.0), now, 0.05);
                }
            }
        }

        pub fn play(&mut self, sounds: &[Sound], id: SoundId, volume: f32, rate: f32) {
            let Some((source, gain, _)) = self.source(sounds, id, false) else {
                return;
            };
            gain.gain().set_value(volume);
            source.playback_rate().set_value(rate);
            self.send(&gain, SEND);
            let _ = source.start();
        }

        pub fn set_master(&mut self, volume: f32) {
            let now = self.context.current_time();
            let _ = self.master.gain().set_target_at_time(volume, now, 0.05);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::{Sound, SoundId};

    pub struct Backend;

    impl Backend {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn resume(&self) {}

        pub fn now(&self) -> f64 {
            0.0
        }

        pub fn set_reverb(&mut self, _level: f32) {}

        pub fn play_at(
            &mut self,
            _sounds: &[Sound],
            _id: SoundId,
            _when: f64,
            _volume: f32,
            _send: f32,
        ) {
        }

        pub fn set_loop(
            &mut self,
            _sounds: &[Sound],
            _channel: usize,
            _id: SoundId,
            _volume: f32,
            _rate: f32,
            _pan: f32,
        ) {
        }

        pub fn play(&mut self, _sounds: &[Sound], _id: SoundId, _volume: f32, _rate: f32) {}

        pub fn set_master(&mut self, _volume: f32) {}
    }
}

#[derive(Default)]
pub struct Audio {
    sounds: Vec<Sound>,
    backend: Option<backend::Backend>,
}

impl Audio {
    pub fn load_wav(&mut self, bytes: &[u8]) -> SoundId {
        let sound = decode_wav(bytes).unwrap_or(Sound {
            rate: 22050,
            samples: vec![0.0],
        });
        self.sounds.push(sound);
        self.sounds.len() - 1
    }

    pub fn load_samples(&mut self, rate: u32, samples: Vec<f32>) -> SoundId {
        self.sounds.push(Sound { rate, samples });
        self.sounds.len() - 1
    }

    pub fn ready(&self) -> bool {
        self.backend.is_some()
    }

    pub(crate) fn unlock(&mut self) {
        if self.backend.is_none() {
            self.backend = backend::Backend::new();
        }
        if let Some(backend) = &self.backend {
            backend.resume();
        }
    }

    pub fn set_loop(&mut self, channel: usize, sound: SoundId, volume: f32, rate: f32) {
        self.set_loop_panned(channel, sound, volume, rate, 0.0);
    }

    pub fn set_loop_panned(
        &mut self,
        channel: usize,
        sound: SoundId,
        volume: f32,
        rate: f32,
        pan: f32,
    ) {
        if let Some(backend) = &mut self.backend {
            backend.set_loop(&self.sounds, channel, sound, volume, rate, pan);
        }
    }

    pub fn play(&mut self, sound: SoundId, volume: f32, rate: f32) {
        if let Some(backend) = &mut self.backend {
            backend.play(&self.sounds, sound, volume, rate);
        }
    }

    pub fn set_master(&mut self, volume: f32) {
        if let Some(backend) = &mut self.backend {
            backend.set_master(volume);
        }
    }

    pub fn now(&self) -> f64 {
        self.backend.as_ref().map_or(0.0, |backend| backend.now())
    }

    pub fn play_at(&mut self, sound: SoundId, when: f64, volume: f32, send: f32) {
        if let Some(backend) = &mut self.backend {
            backend.play_at(&self.sounds, sound, when, volume, send);
        }
    }

    pub fn set_reverb(&mut self, level: f32) {
        if let Some(backend) = &mut self.backend {
            backend.set_reverb(level);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_16_bit_mono_wav() {
        let samples: [i16; 4] = [0, 16384, -32768, 32767];
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&22050u32.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&data);
        let sound = decode_wav(&bytes).unwrap();
        assert_eq!(sound.rate, 22050);
        assert_eq!(sound.samples, vec![0.0, 0.5, -1.0, 32767.0 / 32768.0]);
    }

    #[test]
    fn decodes_rendered_assets() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/sounds/rally_gravel.wav");
        if let Ok(bytes) = std::fs::read(path) {
            let sound = decode_wav(&bytes).unwrap();
            assert_eq!(sound.rate, 22050);
            assert_eq!(sound.samples.len(), 22050);
        }
    }
}
