//! Writes the engine as the game plays it to a WAV file, to hear it without the game: an idle,
//! a slow pull through the revs under load, a lift, blips of the throttle, and a cruise. It is
//! made from the steady-rev loops in `assets/sounds`, as the game's is.
//!
//!     cargo run --release -p viewer --example engine_wav -- assets/sounds/_engine_preview.wav
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "engine_preview.wav".to_owned());
    let rate = 48_000.0_f32;
    let seconds = 22.0;
    let folder = std::path::Path::new("assets/sounds");
    let samples = solstice_viewer::sound::engine_preview(folder, rate, seconds, |t| match t {
        t if t < 3.0 => (700.0, 0.0),
        // A pull from idle to the limit, foot down.
        t if t < 11.0 => (700.0 + (t - 3.0) / 8.0 * 6300.0, 0.9),
        // Off the throttle, the revs falling.
        t if t < 15.0 => (7000.0 - (t - 11.0) / 4.0 * 5800.0, 0.0),
        // Blips.
        t if t < 19.0 => {
            let beat = ((t - 15.0) * 1.5).fract();
            if beat < 0.35 {
                (1200.0 + beat / 0.35 * 2600.0, 1.0)
            } else {
                (3800.0 - (beat - 0.35) / 0.65 * 2600.0, 0.0)
            }
        }
        // A steady cruise.
        _ => (2200.0, 0.3),
    });
    let mut bytes = Vec::new();
    let data = samples.len() as u32 * 2;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&(rate as u32).to_le_bytes());
    bytes.extend_from_slice(&(rate as u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    let loudest = samples.iter().fold(0.0_f32, |most, s| most.max(s.abs()));
    for sample in &samples {
        let scaled = (sample * 0.7).tanh();
        bytes.extend_from_slice(&((scaled * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(&path, bytes).expect("the file can be written");
    println!("{path}: {seconds} s, loudest sample {loudest:.3}");
}
#[cfg(target_arch = "wasm32")]
fn main() {}
