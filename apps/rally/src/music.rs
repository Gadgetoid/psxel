use psxel_engine::{Audio, SoundId};

use crate::daytime;

const SEQUENCE: &str = include_str!("../../../assets/sounds/music/rally_music.seq");
const LOOKAHEAD: f64 = 0.35;
const START_DELAY: f64 = 0.1;
const SEND: f32 = 0.6;
pub const LAYERS: [&str; 6] = ["pads", "bass", "arpeggio", "lead", "drums", "rhythm"];
const LAYER_GAINS: [f32; 6] = [0.6, 1.4, 1.0, 0.8, 0.9, 0.8];
const NIGHT_FROM: f32 = 0.45;
const NIGHT_TO: f32 = 0.25;
const RHYTHM: usize = 5;
const ARPEGGIO: usize = 2;
const LEAD: usize = 3;
const RUSH_GAIN: f32 = 1.2;
const ALTERNATE: &str = "_b";

macro_rules! strip {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../../../assets/sounds/music/", $name, ".spu")).as_slice(),
        )
    };
}

const STRIPS: [(&str, &[u8]); 22] = [
    strip!("music_pads_horn"),
    strip!("music_pads_organ"),
    strip!("music_pads_pluck"),
    strip!("music_bass_saw"),
    strip!("music_bass_sine"),
    strip!("music_arpeggio_pulse"),
    strip!("music_arpeggio_bells"),
    strip!("music_arpeggio_saw"),
    strip!("music_lead_saw"),
    strip!("music_lead_square"),
    strip!("music_lead_twinkle"),
    strip!("music_lead_oud"),
    strip!("music_lead_rush"),
    strip!("music_lead_saw_b"),
    strip!("music_lead_square_b"),
    strip!("music_lead_twinkle_b"),
    strip!("music_lead_oud_b"),
    strip!("music_lead_rush_b"),
    strip!("music_rhythm_chug"),
    strip!("music_rhythm_pluck"),
    strip!("music_drums_full"),
    strip!("music_drums_light"),
];

pub struct Strip {
    pub layer: usize,
    pub variant: String,
    name: String,
    order: Vec<i32>,
}

pub struct Score {
    pub bar: f64,
    slot: f64,
    pub bars: usize,
    pub strips: Vec<Strip>,
    arpeggio_gains: Vec<f32>,
    pub styles: Vec<[usize; 6]>,
    pub nights: Vec<[usize; 6]>,
    pub dawns: Vec<[usize; 6]>,
    alternates: Vec<Option<usize>>,
}

impl Score {
    pub fn parse(text: &str) -> Self {
        let mut score = Score {
            bar: 2.4,
            slot: 2.7,
            bars: 0,
            strips: Vec::new(),
            arpeggio_gains: Vec::new(),
            styles: Vec::new(),
            nights: Vec::new(),
            dawns: Vec::new(),
            alternates: Vec::new(),
        };
        let mut styles: Vec<(usize, Vec<(String, String)>)> = Vec::new();
        let mut nights: Vec<(usize, Vec<(String, String)>)> = Vec::new();
        let mut dawns: Vec<(usize, Vec<(String, String)>)> = Vec::new();
        let pairs = |words: &[&str]| -> Vec<(String, String)> {
            words
                .iter()
                .filter_map(|word| word.split_once('='))
                .map(|(layer, variant)| (layer.to_string(), variant.to_string()))
                .collect()
        };
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["bar", value] => score.bar = value.parse().unwrap_or(score.bar),
                ["slot", value] => score.slot = value.parse().unwrap_or(score.slot),
                ["bars", value] => score.bars = value.parse().unwrap_or(0),
                ["strip", layer, variant, name, _] => {
                    if let Some(layer) = LAYERS.iter().position(|known| known == layer) {
                        score.strips.push(Strip {
                            layer,
                            variant: variant.to_string(),
                            name: name.to_string(),
                            order: Vec::new(),
                        });
                    }
                }
                ["order", layer, variant, slots @ ..] => {
                    if let Some(strip) = score
                        .strips
                        .iter_mut()
                        .find(|strip| LAYERS[strip.layer] == *layer && strip.variant == *variant)
                    {
                        strip.order = slots
                            .iter()
                            .map(|slot| slot.parse().unwrap_or(-1))
                            .collect();
                    }
                }
                ["gain", "arpeggio", gains @ ..] => {
                    score.arpeggio_gains = gains
                        .iter()
                        .map(|gain| gain.parse().unwrap_or(1.0))
                        .collect();
                }
                ["style", biome, rest @ ..] => {
                    styles.push((biome.parse().unwrap_or(0), pairs(rest)));
                }
                ["night", biome, rest @ ..] => {
                    nights.push((biome.parse().unwrap_or(0), pairs(rest)));
                }
                ["dawn", biome, rest @ ..] => {
                    dawns.push((biome.parse().unwrap_or(0), pairs(rest)));
                }
                _ => {}
            }
        }
        let resolve = |choices: &[(String, String)]| {
            let mut picked = [0usize; 6];
            for (layer, slot) in picked.iter_mut().enumerate() {
                let variant = choices
                    .iter()
                    .find(|(name, _)| name == LAYERS[layer])
                    .map(|(_, variant)| variant.as_str());
                *slot = score
                    .strips
                    .iter()
                    .position(|strip| {
                        strip.layer == layer
                            && variant.is_none_or(|variant| strip.variant == variant)
                    })
                    .unwrap_or(0);
            }
            picked
        };
        let table = |mut entries: Vec<(usize, Vec<(String, String)>)>| {
            entries.sort_by_key(|(biome, _)| *biome);
            entries
                .iter()
                .map(|(_, choices)| resolve(choices))
                .collect::<Vec<_>>()
        };
        score.styles = table(styles);
        score.nights = table(nights);
        score.dawns = table(dawns);
        score.alternates = score
            .strips
            .iter()
            .map(|strip| {
                let wanted = format!("{}{}", strip.variant, ALTERNATE);
                score
                    .strips
                    .iter()
                    .position(|other| other.layer == strip.layer && other.variant == wanted)
            })
            .collect();
        score
    }

    pub fn alternate(&self, strip: usize) -> usize {
        self.alternates[strip].unwrap_or(strip)
    }

    pub fn blend(&self, biomes: (usize, usize, f32), clock: f32) -> Vec<([usize; 6], f32)> {
        let daylight = daytime::phase(clock).daylight;
        let night = ((NIGHT_FROM - daylight) / (NIGHT_FROM - NIGHT_TO)).clamp(0.0, 1.0);
        let dawn = daytime::dawn(clock);
        let day = (1.0 - night) * (1.0 - dawn);
        let night = night * (1.0 - dawn);
        let (first, second, t) = biomes;
        let pick = |table: &Vec<[usize; 6]>, biome: usize| table[biome % table.len()];
        let mut blend = Vec::new();
        for (table, weight) in [
            (&self.styles, day),
            (&self.nights, night),
            (&self.dawns, dawn),
        ] {
            blend.push((pick(table, first), (1.0 - t) * weight));
            blend.push((pick(table, second), t * weight));
        }
        blend
    }
}

pub struct Music {
    pub score: Score,
    clips: Vec<Vec<SoundId>>,
    next_bar: usize,
    next_time: Option<f64>,
    pass: usize,
}

pub struct Cue {
    pub blend: Vec<([usize; 6], f32)>,
    pub rhythm: f32,
    pub volume: f32,
}

impl Music {
    pub fn load(audio: &mut Audio) -> Self {
        let score = Score::parse(SEQUENCE);
        let clips = score
            .strips
            .iter()
            .map(|strip| {
                let bytes = STRIPS
                    .iter()
                    .find(|(name, _)| *name == strip.name)
                    .map(|(_, bytes)| *bytes)
                    .expect("music strip");
                let stream = psxel_spu::decode_file(bytes).expect("music stream");
                let slot = (score.slot * stream.rate as f64).round() as usize;
                let count = stream.samples.len() / slot;
                (1..count)
                    .map(|index| {
                        let samples = stream.samples[index * slot..(index + 1) * slot]
                            .iter()
                            .map(|&sample| sample as f32 / 32768.0)
                            .collect();
                        audio.load_samples(stream.rate, samples)
                    })
                    .collect()
            })
            .collect();
        Self {
            score,
            clips,
            next_bar: 0,
            next_time: None,
            pass: 0,
        }
    }

    pub fn update(&mut self, audio: &mut Audio, cue: &Cue) {
        let now = audio.now();
        let next = match self.next_time {
            Some(time) if time > now => time,
            _ => now + START_DELAY,
        };
        let mut time = next;
        while time < now + LOOKAHEAD {
            let mut weights: Vec<(usize, usize, f32)> = Vec::new();
            for (style, weight) in &cue.blend {
                for (layer, &strip_index) in style.iter().enumerate() {
                    match weights
                        .iter_mut()
                        .find(|(_, strip, _)| *strip == strip_index)
                    {
                        Some(entry) => entry.2 += weight,
                        None => weights.push((layer, strip_index, *weight)),
                    }
                }
            }
            if let Some(rush) = self
                .score
                .strips
                .iter()
                .position(|strip| strip.layer == LEAD && strip.variant == "rush")
            {
                weights.push((LEAD, rush, cue.rhythm * RUSH_GAIN));
            }
            for &(layer, strip_index, weight) in &weights {
                if weight <= 0.001 {
                    continue;
                }
                let strip_index = if self.pass % 2 == 1 {
                    self.score.alternate(strip_index)
                } else {
                    strip_index
                };
                let strip = &self.score.strips[strip_index];
                let Some(&slot) = strip.order.get(self.next_bar) else {
                    continue;
                };
                let Some(&clip) = usize::try_from(slot)
                    .ok()
                    .and_then(|slot| self.clips[strip_index].get(slot))
                else {
                    continue;
                };
                let mut volume = cue.volume * LAYER_GAINS[layer] * weight;
                if layer == RHYTHM {
                    volume *= cue.rhythm;
                }
                if layer == ARPEGGIO {
                    volume *= self
                        .score
                        .arpeggio_gains
                        .get(self.next_bar)
                        .copied()
                        .unwrap_or(1.0);
                }
                if volume > 0.001 {
                    audio.play_at(clip, time, volume, SEND);
                }
            }
            self.next_bar = (self.next_bar + 1) % self.score.bars.max(1);
            if self.next_bar == 0 {
                self.pass += 1;
            }
            time += self.score.bar;
        }
        self.next_time = Some(time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_covers_every_layer_and_bar() {
        let score = Score::parse(SEQUENCE);
        assert_eq!(score.strips.len(), STRIPS.len());
        assert!(score.bars > 0);
        for strip in &score.strips {
            assert_eq!(strip.order.len(), score.bars, "{}", strip.name);
            assert!(STRIPS.iter().any(|(name, _)| *name == strip.name));
        }
        assert_eq!(score.styles.len(), 6);
        for style in score.styles.iter().chain(&score.nights).chain(&score.dawns) {
            for (layer, &strip) in style.iter().enumerate() {
                assert_eq!(score.strips[strip].layer, layer);
            }
        }
        for step in 0..100 {
            let clock = step as f32 / 100.0;
            let blend = score.blend((0, 2, 0.5), clock);
            let total: f32 = blend.iter().map(|(_, weight)| weight).sum();
            assert!((total - 1.0).abs() < 1e-5, "{clock} {total}");
        }
        let midnight = score.blend((0, 2, 0.0), 0.75);
        assert_eq!(midnight[2].1, 1.0);
        let sunrise = score.blend((0, 2, 0.0), 0.96);
        assert!(sunrise[4].1 > 0.5, "{:?}", sunrise[4].1);
        let saw = score.styles[0][3];
        assert_eq!(score.strips[score.alternate(saw)].variant, "saw_b");
        let desert = score.styles[2][3];
        assert_eq!(score.strips[desert].variant, "oud");
    }
}
