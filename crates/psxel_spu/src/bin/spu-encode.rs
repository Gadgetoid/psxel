fn read_wav(bytes: &[u8]) -> (u32, Vec<i16>) {
    let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let read_u32 =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert!(
        &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "not a WAV file"
    );
    let (mut rate, mut channels) = (0, 1);
    let mut cursor = 12;
    while cursor + 8 <= bytes.len() {
        let size = read_u32(cursor + 4) as usize;
        let body = cursor + 8;
        match &bytes[cursor..cursor + 4] {
            b"fmt " => {
                channels = read_u16(body + 2) as usize;
                rate = read_u32(body + 4);
                assert_eq!(read_u16(body + 14), 16, "16-bit PCM only");
            }
            b"data" => {
                let samples = bytes[body..body + size]
                    .chunks_exact(2 * channels)
                    .map(|frame| i16::from_le_bytes([frame[0], frame[1]]))
                    .collect();
                return (rate, samples);
            }
            _ => {}
        }
        cursor = body + size + (size & 1);
    }
    panic!("no data chunk");
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 {
        eprintln!("usage: spu-encode input.wav output.spu");
        std::process::exit(2);
    }
    let (rate, samples) = read_wav(&std::fs::read(&arguments[1]).expect("read input"));
    let encoded = psxel_spu::encode_file(rate, &samples, true);
    std::fs::write(&arguments[2], &encoded).expect("write output");
    println!(
        "{} samples at {} Hz -> {} bytes",
        samples.len(),
        rate,
        encoded.len()
    );
}
