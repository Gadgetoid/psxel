pub const BLOCK_BYTES: usize = 16;
pub const BLOCK_SAMPLES: usize = 28;
pub const FLAG_LOOP_END: u8 = 1;
pub const FLAG_LOOP_REPEAT: u8 = 2;
pub const FLAG_LOOP_START: u8 = 4;

const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];
const MAGIC: &[u8; 4] = b"SPU1";

#[derive(Clone, Copy, Default)]
struct History {
    old: i32,
    older: i32,
}

fn decode_nibble(nibble: u8, shift: u8, filter: usize, history: &mut History) -> i16 {
    let shift = if shift > 12 { 9 } else { shift };
    let raw = (((nibble as i32) << 28) >> 28) << 12 >> shift;
    let (positive, negative) = FILTERS[filter.min(4)];
    let sample = (raw + ((history.old * positive + history.older * negative + 32) >> 6))
        .clamp(-32768, 32767);
    history.older = history.old;
    history.old = sample;
    sample as i16
}

pub fn decode_blocks(data: &[u8]) -> Vec<i16> {
    let mut history = History::default();
    let mut samples = Vec::with_capacity(data.len() / BLOCK_BYTES * BLOCK_SAMPLES);
    for block in data.as_chunks::<BLOCK_BYTES>().0 {
        let shift = block[0] & 0x0f;
        let filter = (block[0] >> 4) as usize;
        for &byte in &block[2..] {
            samples.push(decode_nibble(byte & 0x0f, shift, filter, &mut history));
            samples.push(decode_nibble(byte >> 4, shift, filter, &mut history));
        }
    }
    samples
}

fn encode_block(
    samples: &[i16; BLOCK_SAMPLES],
    history: &mut History,
    flags: u8,
) -> [u8; BLOCK_BYTES] {
    let mut best: Option<(i64, u8, usize, [u8; BLOCK_SAMPLES], History)> = None;
    for (filter, &(positive, negative)) in FILTERS.iter().enumerate() {
        for shift in 0..=12u8 {
            let mut state = *history;
            let mut nibbles = [0u8; BLOCK_SAMPLES];
            let mut error = 0i64;
            for (index, &target) in samples.iter().enumerate() {
                let predicted = (state.old * positive + state.older * negative + 32) >> 6;
                let residual = target as i32 - predicted;
                let scaled = ((residual << shift) + if residual >= 0 { 2048 } else { -2048 }) >> 12;
                let nibble = scaled.clamp(-8, 7);
                nibbles[index] = (nibble & 0x0f) as u8;
                let decoded = decode_nibble(nibbles[index], shift, filter, &mut state);
                let difference = decoded as i64 - target as i64;
                error += difference * difference;
            }
            if best.as_ref().is_none_or(|current| error < current.0) {
                best = Some((error, shift, filter, nibbles, state));
            }
        }
    }
    let (_, shift, filter, nibbles, state) = best.unwrap();
    *history = state;
    let mut block = [0u8; BLOCK_BYTES];
    block[0] = shift | ((filter as u8) << 4);
    block[1] = flags;
    for (index, pair) in nibbles.chunks(2).enumerate() {
        block[2 + index] = pair[0] | (pair[1] << 4);
    }
    block
}

pub fn encode_blocks(samples: &[i16], looping: bool) -> Vec<u8> {
    let mut history = History::default();
    let blocks = samples.len().div_ceil(BLOCK_SAMPLES);
    let mut out = Vec::with_capacity(blocks * BLOCK_BYTES);
    for (index, chunk) in samples.chunks(BLOCK_SAMPLES).enumerate() {
        let mut padded = [0i16; BLOCK_SAMPLES];
        padded[..chunk.len()].copy_from_slice(chunk);
        let mut flags = 0;
        if looping && index == 0 {
            flags |= FLAG_LOOP_START;
        }
        if index + 1 == blocks {
            flags |= FLAG_LOOP_END | if looping { FLAG_LOOP_REPEAT } else { 0 };
        }
        out.extend_from_slice(&encode_block(&padded, &mut history, flags));
    }
    out
}

pub struct Stream {
    pub rate: u32,
    pub samples: Vec<i16>,
}

pub fn encode_file(rate: u32, samples: &[i16], looping: bool) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    out.extend_from_slice(&encode_blocks(samples, looping));
    out
}

pub fn decode_file(bytes: &[u8]) -> Option<Stream> {
    if bytes.len() < 12 || &bytes[0..4] != MAGIC {
        return None;
    }
    let rate = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let count = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    let mut samples = decode_blocks(&bytes[12..]);
    samples.truncate(count);
    Some(Stream { rate, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_known_block() {
        let mut block = [0u8; BLOCK_BYTES];
        block[0] = 0x0c;
        block[2] = 0x71;
        block[3] = 0x08;
        let samples = decode_blocks(&block);
        assert_eq!(&samples[..4], &[1, 7, -8, 0]);
    }

    #[test]
    fn filter_prediction_matches_spx() {
        let mut block = [0u8; BLOCK_BYTES];
        block[0] = 0x10;
        block[2] = 0x01;
        let samples = decode_blocks(&block);
        assert_eq!(samples[0], 4096);
        assert_eq!(samples[1] as i32, (4096 * 60 + 32) >> 6);
    }

    #[test]
    fn round_trip_is_faithful() {
        let rate = 22050.0;
        let samples: Vec<i16> = (0..22050)
            .map(|index| {
                let t = index as f32 / rate;
                ((t * 220.0 * std::f32::consts::TAU).sin() * 9000.0
                    + (t * 1330.0 * std::f32::consts::TAU).sin() * 3000.0) as i16
            })
            .collect();
        let file = encode_file(22050, &samples, true);
        assert!(file.len() < samples.len() * 2 / 3);
        let decoded = decode_file(&file).unwrap();
        assert_eq!(decoded.samples.len(), samples.len());
        let signal: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
        let noise: f64 = samples
            .iter()
            .zip(&decoded.samples)
            .map(|(&a, &b)| (a as f64 - b as f64).powi(2))
            .sum();
        let snr = 10.0 * (signal / noise).log10();
        assert!(snr > 30.0, "snr {snr:.1} dB");
        let last = &file[12 + (file.len() - 12 - BLOCK_BYTES)..];
        assert_eq!(
            last[1] & (FLAG_LOOP_END | FLAG_LOOP_REPEAT),
            FLAG_LOOP_END | FLAG_LOOP_REPEAT
        );
    }
}
