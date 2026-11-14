//! Sound, for a game played natively (`docs/sound.md`).
//!
//! * **Recorded sounds** are WAV files in `assets/sounds`, made from descriptions by
//!   `tools/audio/generate.py`: loops (the bush, the rain, what a tyre rolls on) and things that
//!   happen once (a splash, brush snapping). Each is a [`Voice`]: a place in a clip, a volume and
//!   a pitch, each eased towards what the game last asked for, so that nothing clicks.
//! * **The engine** has to follow the revs and the throttle from one moment to the next, which
//!   no one recording does. So it is several: loops of it held at steady revs, from an idle to
//!   flat out, each played faster or slower to the revs it is at now and faded into the next
//!   as the revs pass from one to the other, as a racing game does with recordings of a real
//!   car ([`EngineLayer`]). Without those loops it is synthesised instead ([`Engine`]).
//!
//! The game says what it wants once a frame; the sound card's own thread mixes it. Between
//! them is one lock, held for a frame's worth of numbers on one side and a few milliseconds of
//! samples on the other.

use crate::sound_table::{ENGINE_LAYERS, ENGINE_OVERRUN};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A recorded sound: its samples, mono, and how many of them a second.
struct Clip {
    samples: Arc<Vec<f32>>,
    rate: f32,
}

/// One sound playing.
struct Voice {
    clip: usize,
    /// Where in the clip it is, in samples.
    at: f64,
    /// Its volume and pitch now, and what they are eased towards.
    gain: f32,
    gain_to: f32,
    pitch: f32,
    pitch_to: f32,
    looping: bool,
}

/// Everything that sounds, as the sound card's thread mixes it.
struct Mix {
    clips: Vec<Clip>,
    /// Each loop that has been asked for, by its clip: it keeps its place while it is silent.
    loops: HashMap<usize, Voice>,
    shots: Vec<Voice>,
    engine: Engine,
    /// The engine's recorded loops, from the lowest revs up, and the one for a shut throttle.
    layers: Vec<EngineLayer>,
    overrun: Option<EngineLayer>,
    /// Samples a second going out, and the volume of all of it.
    rate: f32,
    master: f32,
}

impl Mix {
    /// The next sample of everything, the left ear's and the right's.
    fn next(&mut self) -> (f32, f32) {
        let mut sum = 0.0_f32;
        let ease = 1.0 - (-1.0 / (0.06 * self.rate)).exp();
        let clips = &self.clips;
        let rate = self.rate;
        let mut play = |voice: &mut Voice| -> bool {
            voice.gain += (voice.gain_to - voice.gain) * ease;
            voice.pitch += (voice.pitch_to - voice.pitch) * ease;
            let clip = &clips[voice.clip];
            let count = clip.samples.len();
            if count < 2 {
                return false;
            }
            if voice.gain < 1.0e-4 && voice.gain_to == 0.0 {
                // Silent: a loop holds its place, and a shot that has faded is over.
                return voice.looping;
            }
            let index = voice.at as usize;
            let between = (voice.at - index as f64) as f32;
            let next = if index + 1 < count {
                index + 1
            } else if voice.looping {
                0
            } else {
                index
            };
            sum += (clip.samples[index] + (clip.samples[next] - clip.samples[index]) * between)
                * voice.gain;
            voice.at += f64::from(voice.pitch * clip.rate / rate);
            if voice.at >= count as f64 {
                if !voice.looping {
                    return false;
                }
                voice.at -= count as f64;
            }
            true
        };
        for voice in self.loops.values_mut() {
            play(voice);
        }
        self.shots.retain_mut(&mut play);
        let (left, right) = if self.layers.len() >= 2 {
            let note = self.recorded_engine();
            (note, note)
        } else {
            self.engine.next(rate)
        };
        // Loud moments are rounded off rather than clipped.
        let out = |ear: f32| ((sum + ear) * self.master).tanh();
        (out(left), out(right))
    }
}

/// One of the engine's recorded loops: the engine held at steady revs.
struct EngineLayer {
    clip: usize,
    /// The revs it was held at, as near as its description asked for.
    rpm: f32,
    at: f64,
}

/// Which loops the engine is made of and the revs each is at.
///
/// A loop made from a description is at whatever revs it came out at, not the revs it was
/// asked for, and two loops faded together at the wrong speeds are heard as two engines. So
/// the revs can be given beside the sounds, in `engine.txt`, from each loop's measured note
/// (`tools/audio/engine_pitch.py` measures it): a line for each loop, `NAME RPM`, and
/// `overrun NAME RPM` for the one played on a shut throttle. Without the file they are as
/// the constants above have them.
struct EngineTable {
    layers: Vec<(String, f32)>,
    overrun: Option<(String, f32)>,
}

impl EngineTable {
    fn standard() -> Self {
        Self {
            layers: ENGINE_LAYERS
                .iter()
                .map(|&(name, rpm)| (name.to_owned(), rpm))
                .collect(),
            overrun: Some((ENGINE_OVERRUN.0.to_owned(), ENGINE_OVERRUN.1)),
        }
    }

    fn read(folder: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(folder.join("engine.txt")) else {
            return Self::standard();
        };
        let mut table = Self {
            layers: Vec::new(),
            overrun: None,
        };
        for line in text.lines() {
            let words: Vec<&str> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            match words[..] {
                ["overrun", name, rpm] => {
                    if let Ok(rpm) = rpm.parse::<f32>() {
                        table.overrun = Some((name.to_owned(), rpm));
                    }
                }
                [name, rpm] => {
                    if let Ok(rpm) = rpm.parse::<f32>() {
                        table.layers.push((name.to_owned(), rpm));
                    }
                }
                _ => {}
            }
        }
        table.layers.retain(|(_, rpm)| *rpm > 0.0);
        table.layers.sort_by(|a, b| a.1.total_cmp(&b.1));
        table
    }
}

impl Mix {
    /// The next sample of the engine from its recorded loops.
    ///
    /// The two loops whose revs the engine is between are both played, each at the speed that
    /// brings it to the revs now, and faded one into the other by how far between them the
    /// revs are, at even power, so that the pull through the revs has no step in it. The
    /// throttle sets how loud they are; shut, at speed, the overrun's loop comes up instead.
    fn recorded_engine(&mut self) -> f32 {
        let rate = self.rate;
        let (rpm, load, gain) = self.engine.eased(rate);
        if gain < 1.0e-4 {
            return 0.0;
        }
        let clips = &self.clips;
        let read = |layer: &mut EngineLayer, weight: f32| -> f32 {
            let clip = &clips[layer.clip];
            let count = clip.samples.len();
            if count < 2 {
                return 0.0;
            }
            // Every loop keeps turning whether it is heard or not, so that it comes in
            // already running.
            let pitch = (rpm / layer.rpm).clamp(0.5, 2.4);
            let index = layer.at as usize % count;
            let between = (layer.at - layer.at.floor()) as f32;
            let next = (index + 1) % count;
            layer.at = (layer.at + f64::from(pitch * clip.rate / rate)) % count as f64;
            if weight <= 0.0 {
                return 0.0;
            }
            (clip.samples[index] + (clip.samples[next] - clip.samples[index]) * between) * weight
        };
        // Which two loops the revs are between, and how far.
        let last = self.layers.len() - 1;
        let above = self
            .layers
            .iter()
            .position(|layer| layer.rpm > rpm)
            .unwrap_or(last + 1);
        let (low, high, share) = if above == 0 {
            (0, 0, 0.0)
        } else if above > last {
            (last, last, 0.0)
        } else {
            let (a, b) = (self.layers[above - 1].rpm, self.layers[above].rpm);
            (above - 1, above, (rpm - a) / (b - a))
        };
        let turn = share * std::f32::consts::FRAC_PI_2;
        let mut note = 0.0;
        // On the throttle it is loud; off it, at an idle, it still ticks over.
        let driven = 0.3 + 0.7 * load.powf(0.7);
        for (index, layer) in self.layers.iter_mut().enumerate() {
            let weight = if index == low && index == high {
                1.0
            } else if index == low {
                turn.cos()
            } else if index == high {
                turn.sin()
            } else {
                0.0
            };
            note += read(layer, weight * driven);
        }
        if let Some(overrun) = &mut self.overrun {
            let coasting = (1.0 - load).powi(2) * ((rpm - 1300.0) / 1500.0).clamp(0.0, 1.0);
            note += read(overrun, coasting * 0.7);
        }
        // At the limiter the spark is cut and comes back, fourteen times a second.
        let stutter = if self.engine.limiter {
            self.engine.whine = (self.engine.whine + 14.0 / rate).fract();
            if self.engine.whine < 0.5 {
                0.35
            } else {
                1.0
            }
        } else {
            1.0
        };
        note * gain * stutter
    }
}

/// One bank of four cylinders, and the pipe it breathes out through.
#[derive(Default)]
struct Bank {
    /// The pressure of the firing that is going on, dying away.
    pulse: f32,
    /// The pipe: a line the pulse runs down and comes back up, turned over at its open end.
    pipe: Vec<f32>,
    at: usize,
    /// What the pipe's far end last gave back, dulled, as a long pipe dulls a sound.
    damped: f32,
    /// What is heard of it, with the hiss taken off the top.
    heard: f32,
}

impl Bank {
    fn next(&mut self, tone: f32, decay: f32) -> f32 {
        let back = self.pipe[self.at];
        self.damped += (back - self.damped) * 0.38;
        let into = self.pulse - self.damped * 0.72;
        self.pipe[self.at] = into;
        self.at = (self.at + 1) % self.pipe.len();
        self.pulse *= decay;
        self.heard += (into - self.heard) * tone;
        self.heard
    }
}

/// A supercharged cross-plane V8, synthesised.
///
/// Eight cylinders fire in two turns of the crank, evenly spaced, but in an order
/// (1-5-4-8-6-3-7-2) that gives each bank of four its firings at uneven intervals: two close
/// together, then a gap. Each bank breathes out through its own pipe, so what is heard at the
/// back is two uneven rhythms laid over each other, which is the burble no other engine has.
/// Here each firing is a pulse of pressure into its bank's pipe, a line that the pulse runs down
/// and reflects back up; the throttle sets how hard each cylinder fires and how bright it is;
/// and over it all, under load, is the whine of the blower's rotors.
struct Engine {
    /// Revolutions a minute, and how far the throttle is open, now and asked for.
    rpm: f32,
    rpm_to: f32,
    throttle: f32,
    throttle_to: f32,
    gain: f32,
    gain_to: f32,
    limiter: bool,
    /// How far through its two turns the crank is, from 0 to 1, and the firing it is on.
    cycle: f32,
    firing: usize,
    banks: [Bank; 2],
    whine: f32,
    noise: u32,
    hiss: f32,
    rumble: f32,
}

/// Which bank each firing is on, in the order they fire: cylinders 1 to 4 are one bank.
const FIRING_BANK: [usize; 8] = [0, 1, 0, 1, 1, 0, 1, 0];

impl Engine {
    fn new(rate: f32) -> Self {
        // Pipes of two lengths, so that the two banks are not one sound twice.
        let pipe = |metres: f32| Bank {
            pipe: vec![0.0; ((metres / 343.0 * 2.0 * rate) as usize).max(8)],
            ..Bank::default()
        };
        Self {
            rpm: 700.0,
            rpm_to: 700.0,
            throttle: 0.0,
            throttle_to: 0.0,
            gain: 0.0,
            gain_to: 0.0,
            limiter: false,
            cycle: 0.0,
            firing: 0,
            banks: [pipe(0.92), pipe(1.04)],
            whine: 0.0,
            noise: 0x2545_F491,
            hiss: 0.0,
            rumble: 0.0,
        }
    }

    fn random(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// Moves its revs, its throttle and its volume a sample on towards what was last asked
    /// for, and gives them as they are now.
    fn eased(&mut self, rate: f32) -> (f32, f32, f32) {
        let ease = 1.0 - (-1.0 / (0.05 * rate)).exp();
        self.rpm += (self.rpm_to - self.rpm) * ease;
        self.throttle += (self.throttle_to - self.throttle) * ease;
        self.gain += (self.gain_to - self.gain) * ease * 0.3;
        (
            self.rpm.max(300.0),
            self.throttle.clamp(0.0, 1.0),
            self.gain,
        )
    }

    /// The next sample of it, to the left and to the right: a bank a side, mostly.
    fn next(&mut self, rate: f32) -> (f32, f32) {
        let (rpm, load, gain) = self.eased(rate);
        if gain < 1.0e-4 {
            return (0.0, 0.0);
        }
        // Two turns of the crank to a cycle.
        self.cycle += rpm / 120.0 / rate;
        if self.cycle >= 1.0 {
            self.cycle -= 1.0;
        }
        let firing = ((self.cycle * 8.0) as usize).min(7);
        if firing != self.firing {
            self.firing = firing;
            // At the limiter the spark is cut to every other cylinder, which is its stutter.
            let cut = self.limiter && firing % 2 == 0;
            if !cut {
                // No two firings are alike; and each is harder the wider the throttle. With
                // the throttle shut the cylinders still pump air, quietly.
                let uneven = 1.0 + 0.16 * self.random();
                let push = (0.2 + 0.8 * load) * uneven;
                // Fewer firings a second at low revs are each heard for longer: even it out.
                self.banks[FIRING_BANK[firing]].pulse += push * (1500.0 / rpm).sqrt().min(1.6);
            }
        }
        // Brighter the harder it works and the faster it turns.
        let cutoff = 260.0 + 1500.0 * load + rpm * 0.2;
        let tone = 1.0 - (-std::f32::consts::TAU * cutoff / rate).exp();
        let decay = (-1.0 / (0.0021 * rate)).exp();
        let [left, right] = &mut self.banks;
        let (a, b) = (left.next(tone, decay), right.next(tone, decay));

        // The rush of air in and out, under load.
        let rush = self.random();
        self.hiss += (rush - self.hiss) * 0.2;
        let breath = self.hiss * (0.012 + 0.07 * load) * (rpm / 5000.0).min(1.3);
        // The blower: two rotors of four lobes, geared up from the crank, heard as a whine
        // that climbs with the revs and swells with the load.
        self.whine = (self.whine + rpm / 60.0 * 2.4 * 4.0 / rate).fract();
        let turn = self.whine * std::f32::consts::TAU;
        let blower = (turn.sin() + 0.35 * (turn * 2.0).sin())
            * (0.006 + 0.05 * load * load)
            * (rpm / 6000.0).min(1.0);
        // And a little of the shake of the whole thing, which is what an idle is.
        self.rumble += (self.random() - self.rumble) * 0.01;
        let shake = self.rumble * 0.25 * (1.0 - load) * (1400.0 / rpm).min(1.0);

        let both = breath + blower + shake;
        let level = self.gain * 0.5;
        (
            (a * 0.8 + b * 0.45 + both) * level,
            (b * 0.8 + a * 0.45 + both) * level,
        )
    }
}

/// Reads a WAV file of 16-bit samples: its samples, mixed to one channel, and its rate.
fn read_wav(bytes: &[u8]) -> Option<(Vec<f32>, f32)> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let word = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let half = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let (mut channels, mut rate, mut bits) = (0_u16, 0_u32, 0_u16);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = word(at + 4)? as usize;
        let body = at + 8;
        match &bytes[at..at + 4] {
            b"fmt " => {
                // Plain samples only: a compressed file is not one of these.
                if half(body)? != 1 {
                    return None;
                }
                channels = half(body + 2)?;
                rate = word(body + 4)?;
                bits = half(body + 14)?;
            }
            b"data" => {
                if channels == 0 || bits != 16 || rate == 0 {
                    return None;
                }
                let data = bytes.get(body..(body + size).min(bytes.len()))?;
                let frame = 2 * channels as usize;
                let samples = data
                    .chunks_exact(frame)
                    .map(|frame| {
                        let sum: f32 = frame
                            .chunks_exact(2)
                            .map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0)
                            .sum();
                        sum / f32::from(channels)
                    })
                    .collect();
                return Some((samples, rate as f32));
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        at = body + size + size % 2;
    }
    None
}

/// The game's sound: what plays, and the output it plays through.
pub struct Sounds {
    mix: Arc<Mutex<Mix>>,
    names: HashMap<String, usize>,
    /// Held so that it goes on playing: dropping it stops the sound.
    stream: Option<cpal::Stream>,
    /// Set by the output when it stops: headphones plugged in or pulled out, or another
    /// output chosen, ends the stream that was playing.
    lost: Arc<AtomicBool>,
    /// When opening the output again was last tried, so that it is not tried every frame.
    tried: std::time::Instant,
}

/// Opens the machine's sound output and has it play `mix`, at whatever rate the output runs.
fn open(mix: &Arc<Mutex<Mix>>, lost: &Arc<AtomicBool>) -> Option<cpal::Stream> {
    let device = cpal::default_host().default_output_device().or_else(|| {
        log::warn!("no sound: there is no output to play it through");
        None
    })?;
    let supported = device
        .default_output_config()
        .map_err(|error| log::warn!("no sound: {error}"))
        .ok()?;
    if supported.sample_format() != cpal::SampleFormat::F32 {
        log::warn!(
            "no sound: the output wants {:?} samples",
            supported.sample_format()
        );
        return None;
    }
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate as f32;
    let channels = config.channels as usize;
    if let Ok(mut mix) = mix.lock() {
        if mix.rate != rate {
            mix.rate = rate;
            mix.engine = Engine::new(rate);
        }
    }
    let playing = mix.clone();
    let stopped = lost.clone();
    let stream = device
        .build_output_stream(
            config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let Ok(mut mix) = playing.lock() else {
                    out.fill(0.0);
                    return;
                };
                for frame in out.chunks_mut(channels) {
                    let (left, right) = mix.next();
                    for (channel, sample) in frame.iter_mut().enumerate() {
                        *sample = if channel % 2 == 0 { left } else { right };
                    }
                }
            },
            move |error| {
                log::warn!("sound: {error}");
                stopped.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|error| log::warn!("no sound: {error}"))
        .ok()?;
    stream
        .play()
        .map_err(|error| log::warn!("no sound: {error}"))
        .ok()?;
    log::info!("sound: playing at {rate} Hz");
    Some(stream)
}

/// What the game asks of the engine's note this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineNote {
    pub rpm: f32,
    /// How far the throttle is open, from 0 to 1.
    pub throttle: f32,
    pub limiter: bool,
    /// Its volume, 0 for silent: an engine that is not being driven is not heard.
    pub gain: f32,
}

impl Sounds {
    /// Opens the default sound output, and reads every WAV file in `folder` as a sound named
    /// by its file. `None`, with the reason logged, on a machine that has no output.
    pub fn new(folder: &Path) -> Option<Self> {
        let (clips, names) = read_folder(folder);
        log::info!("sound: {} sounds from {}", clips.len(), folder.display());
        let table = EngineTable::read(folder);
        let mix = Arc::new(Mutex::new(Mix::new(clips, &names, &table, 48_000.0)));
        let lost = Arc::new(AtomicBool::new(false));
        let stream = open(&mix, &lost)?;
        Some(Self {
            mix,
            names,
            stream: Some(stream),
            lost,
            tried: std::time::Instant::now(),
        })
    }

    /// Whether there is a sound of this name.
    pub fn has(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }

    /// What the game asks for this frame, all at once: `ask` is handed something to ask with.
    ///
    /// An output that has stopped is opened again here, on whatever the machine now plays
    /// through, a second after it stopped and each second after that until it opens.
    pub fn frame(&mut self, ask: impl FnOnce(&mut Asking<'_>)) {
        if self.lost.load(Ordering::Relaxed) && self.tried.elapsed().as_secs_f32() > 1.0 {
            self.tried = std::time::Instant::now();
            // The old one is let go of first: two cannot both hold the output.
            self.stream = None;
            self.lost.store(false, Ordering::Relaxed);
            self.stream = open(&self.mix, &self.lost);
            if self.stream.is_none() {
                self.lost.store(true, Ordering::Relaxed);
            }
        }
        if let Ok(mut mix) = self.mix.lock() {
            ask(&mut Asking {
                mix: &mut mix,
                names: &self.names,
            });
        }
    }
}

impl Mix {
    /// A mix of these sounds, with the engine's loops found among them by name.
    fn new(
        clips: Vec<Clip>,
        names: &HashMap<String, usize>,
        table: &EngineTable,
        rate: f32,
    ) -> Self {
        let layer = |(name, rpm): &(String, f32)| {
            names.get(name).map(|&clip| EngineLayer {
                clip,
                rpm: *rpm,
                at: 0.0,
            })
        };
        let layers: Vec<EngineLayer> = table.layers.iter().filter_map(layer).collect();
        if layers.len() >= 2 {
            let revs: Vec<String> = layers.iter().map(|l| format!("{:.0}", l.rpm)).collect();
            log::info!(
                "sound: the engine is {} recorded loops, at {} rpm",
                layers.len(),
                revs.join(", ")
            );
        } else {
            log::info!("sound: the engine is synthesised: its loops are not there");
        }
        Self {
            clips,
            loops: HashMap::new(),
            shots: Vec::new(),
            engine: Engine::new(rate),
            layers,
            overrun: table.overrun.as_ref().and_then(layer),
            rate,
            master: 0.8,
        }
    }
}

/// Reads every WAV file in a folder as a sound named by its file.
fn read_folder(folder: &Path) -> (Vec<Clip>, HashMap<String, usize>) {
    {
        let mut clips = Vec::new();
        let mut names = HashMap::new();
        let mut files: Vec<_> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "wav"))
            // A file whose name starts with `_` is there to be listened to, not played.
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| !name.to_string_lossy().starts_with('_'))
            })
            .collect();
        files.sort();
        for path in files {
            let Some(name) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            match std::fs::read(&path).ok().and_then(|bytes| read_wav(&bytes)) {
                Some((samples, rate)) => {
                    names.insert(name, clips.len());
                    clips.push(Clip {
                        samples: Arc::new(samples),
                        rate,
                    });
                }
                None => log::warn!("{}: not a 16-bit WAV file", path.display()),
            }
        }
        (clips, names)
    }
}

/// What a frame asks of the sound.
pub struct Asking<'a> {
    mix: &'a mut Mix,
    names: &'a HashMap<String, usize>,
}

impl Asking<'_> {
    /// A loop's volume and pitch from now on: it plays as long as its volume is above zero,
    /// and keeps its place while it is not. A name there is no sound for asks for nothing.
    pub fn looping(&mut self, name: &str, gain: f32, pitch: f32) {
        let Some(&clip) = self.names.get(name) else {
            return;
        };
        let voice = self.mix.loops.entry(clip).or_insert(Voice {
            clip,
            // Each starts somewhere of its own, so that loops begun together don't beat.
            at: (clip as f64 * 7919.0) % 40_000.0,
            gain: 0.0,
            gain_to: 0.0,
            pitch: pitch.max(0.05),
            pitch_to: pitch.max(0.05),
            looping: true,
        });
        voice.gain_to = gain.max(0.0);
        voice.pitch_to = pitch.clamp(0.05, 4.0);
    }

    /// Plays a sound once, from now.
    pub fn once(&mut self, name: &str, gain: f32, pitch: f32) {
        let Some(&clip) = self.names.get(name) else {
            return;
        };
        // No more at once than can be told apart.
        if self.mix.shots.len() < 24 {
            self.mix.shots.push(Voice {
                clip,
                at: 0.0,
                gain: gain.max(0.0),
                gain_to: gain.max(0.0),
                pitch: pitch.clamp(0.05, 4.0),
                pitch_to: pitch.clamp(0.05, 4.0),
                looping: false,
            });
        }
    }

    /// The engine's note from now on.
    pub fn engine(&mut self, note: EngineNote) {
        let engine = &mut self.mix.engine;
        engine.rpm_to = note.rpm.clamp(300.0, 12_000.0);
        engine.throttle_to = note.throttle.clamp(0.0, 1.0);
        engine.limiter = note.limiter;
        engine.gain_to = note.gain.max(0.0);
    }
}

/// The engine as the sounds in `folder` make it, its recorded loops if they are there, at
/// these revs and this throttle, for `seconds`: for hearing it without the game. Gives its
/// samples, left and right interleaved.
pub fn engine_preview(
    folder: &Path,
    rate: f32,
    seconds: f32,
    at: impl Fn(f32) -> (f32, f32),
) -> Vec<f32> {
    let (clips, names) = read_folder(folder);
    let mut mix = Mix::new(clips, &names, &EngineTable::read(folder), rate);
    mix.master = 1.0;
    mix.engine.gain = 1.0;
    mix.engine.gain_to = 1.0;
    let count = (seconds * rate) as usize;
    let mut out = Vec::with_capacity(count * 2);
    for index in 0..count {
        let (rpm, throttle) = at(index as f32 / rate);
        mix.engine.rpm_to = rpm;
        mix.engine.throttle_to = throttle;
        let (left, right) = mix.next();
        out.push(left);
        out.push(right);
    }
    out
}

/// The synthesised engine alone, at these revs and this throttle, for `seconds`: for hearing
/// it, and for its tests. Gives its samples, left and right interleaved.
pub fn engine_alone(rate: f32, seconds: f32, at: impl Fn(f32) -> (f32, f32)) -> Vec<f32> {
    let mut engine = Engine::new(rate);
    engine.gain = 1.0;
    engine.gain_to = 1.0;
    let count = (seconds * rate) as usize;
    let mut out = Vec::with_capacity(count * 2);
    for index in 0..count {
        let (rpm, throttle) = at(index as f32 / rate);
        engine.rpm_to = rpm;
        engine.throttle_to = throttle;
        let (left, right) = engine.next(rate);
        out.push(left);
        out.push(right);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV file of these samples.
    fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let data = samples.len() as u32 * 2;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn a_wav_file_is_read_and_mixed_to_one_channel() {
        let (samples, rate) = read_wav(&wav(&[16384, -16384, 0, 32767], 1, 48_000)).unwrap();
        assert_eq!(rate, 48_000.0);
        assert_eq!(samples.len(), 4);
        assert!((samples[0] - 0.5).abs() < 1.0e-4 && (samples[1] + 0.5).abs() < 1.0e-4);
        // Two channels are averaged.
        let (stereo, _) = read_wav(&wav(&[16384, 0, -16384, -16384], 2, 44_100)).unwrap();
        assert_eq!(stereo.len(), 2);
        assert!((stereo[0] - 0.25).abs() < 1.0e-4 && (stereo[1] + 0.5).abs() < 1.0e-4);
        // What is not one is refused.
        assert!(read_wav(b"not a wav file at all").is_none());
        assert!(read_wav(&[]).is_none());
    }

    fn voice(clip: usize, gain: f32, looping: bool) -> Voice {
        Voice {
            clip,
            at: 0.0,
            gain,
            gain_to: gain,
            pitch: 1.0,
            pitch_to: 1.0,
            looping,
        }
    }

    fn mix(samples: Vec<f32>) -> Mix {
        Mix {
            clips: vec![Clip {
                samples: Arc::new(samples),
                rate: 48_000.0,
            }],
            loops: HashMap::new(),
            shots: Vec::new(),
            engine: Engine::new(48_000.0),
            layers: Vec::new(),
            overrun: None,
            rate: 48_000.0,
            master: 1.0,
        }
    }

    #[test]
    fn a_recorded_engine_fades_from_one_loop_to_the_next_as_the_revs_rise() {
        // Two loops that can be told apart: one of 0.2 throughout, as the idle, and one
        // of -0.2, as the middle loop.
        let clip = |value: f32| Clip {
            samples: Arc::new(vec![value; 4800]),
            rate: 48_000.0,
        };
        let names: HashMap<String, usize> = [("engine_idle", 0), ("engine_mid", 1)]
            .into_iter()
            .map(|(name, clip)| (name.to_owned(), clip))
            .collect();
        let table = EngineTable::standard();
        let mut mix = Mix::new(vec![clip(0.2), clip(-0.2)], &names, &table, 48_000.0);
        assert_eq!(mix.layers.len(), 2);
        mix.master = 1.0;
        (mix.engine.gain, mix.engine.gain_to) = (1.0, 1.0);
        (mix.engine.throttle, mix.engine.throttle_to) = (1.0, 1.0);
        let mut heard_at = |rpm: f32| {
            (mix.engine.rpm, mix.engine.rpm_to) = (rpm, rpm);
            // What goes out is rounded off; undo that to see what was mixed.
            mix.next().0.atanh()
        };
        // At and below the lower loop's revs, it alone; at and above the higher's, it alone.
        assert!((heard_at(600.0) - 0.2).abs() < 1.0e-3);
        assert!((heard_at(7000.0) + 0.2).abs() < 1.0e-3);
        // Half-way between (840 and 2235 are their revs), as much of each: they cancel.
        assert!(heard_at(1537.5).abs() < 1.0e-3);
        // And nearer the lower, more of it.
        assert!(heard_at(1200.0) > 0.05);
    }

    #[test]
    fn a_shot_plays_once_and_a_loop_goes_round() {
        let mut once = mix(vec![0.2; 100]);
        once.shots.push(voice(0, 1.0, false));
        let heard: Vec<f32> = (0..150).map(|_| once.next().0).collect();
        assert!(heard[50] > 0.15, "it is heard: {}", heard[50]);
        assert_eq!(heard[120], 0.0, "and then it is over");
        assert!(once.shots.is_empty());

        let mut round = mix(vec![0.2; 100]);
        round.loops.insert(0, voice(0, 1.0, true));
        let heard: Vec<f32> = (0..350).map(|_| round.next().0).collect();
        assert!(heard[340] > 0.15, "still going: {}", heard[340]);
        // Asked to be silent, it fades rather than stops, and keeps its place.
        round.loops.get_mut(&0).unwrap().gain_to = 0.0;
        let after: Vec<f32> = (0..48_000).map(|_| round.next().0).collect();
        assert!(after[10] > 0.1 && after[47_000] == 0.0);
        assert_eq!(round.loops.len(), 1);
    }

    #[test]
    fn played_at_twice_the_pitch_a_sound_takes_half_as_long() {
        let mut fast = mix(vec![0.2; 1000]);
        let mut shot = voice(0, 1.0, false);
        (shot.pitch, shot.pitch_to) = (2.0, 2.0);
        fast.shots.push(shot);
        let lasted = (0..2000).take_while(|_| fast.next().0 != 0.0).count();
        assert!((495..=505).contains(&lasted), "{lasted} samples");
    }

    /// How much of a signal is at a frequency, by its correlation with a sine and a cosine.
    fn at_frequency(samples: &[f32], rate: f32, hertz: f32) -> f32 {
        let (mut sine, mut cosine) = (0.0_f32, 0.0_f32);
        for (index, sample) in samples.iter().enumerate() {
            let turn = std::f32::consts::TAU * hertz * index as f32 / rate;
            sine += sample * turn.sin();
            cosine += sample * turn.cos();
        }
        sine.hypot(cosine) / samples.len() as f32
    }

    #[test]
    fn the_engine_fires_eight_times_in_two_turns_and_burbles() {
        let rate = 48_000.0;
        let rpm = 3000.0;
        let both = engine_alone(rate, 2.0, |_| (rpm, 0.6));
        let left: Vec<f32> = both.chunks(2).skip(24_000).map(|pair| pair[0]).collect();
        assert!(left.iter().all(|sample| sample.is_finite()));
        let loudest = left.iter().fold(0.0_f32, |most, s| most.max(s.abs()));
        assert!((0.02..1.5).contains(&loudest), "peaks at {loudest}");
        // Its note: four firings a turn, 200 Hz at 3000 rpm. And under it what a bank that
        // fires unevenly adds, which an even-firing engine has none of: a bank's four firings
        // fall on the first, third, sixth and eighth of the cycle's eight, which is strongest
        // at three times the cycle's own rate, three eighths of the firing's.
        let firing = rpm / 60.0 * 4.0;
        let note = at_frequency(&left, rate, firing);
        let burble = at_frequency(&left, rate, firing * 0.375);
        let between = at_frequency(&left, rate, firing * 0.4375);
        assert!(note > between * 3.0, "its note {note}, beside it {between}");
        assert!(
            burble > between * 2.0,
            "its burble {burble}, beside it {between}"
        );
    }

    #[test]
    fn the_engine_is_louder_and_brighter_under_load_and_silent_when_not_driven() {
        let rate = 48_000.0;
        let loudness = |throttle: f32| {
            let both = engine_alone(rate, 1.5, |_| (2500.0, throttle));
            let tail = &both[both.len() / 2..];
            (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt()
        };
        let (shut, open) = (loudness(0.0), loudness(1.0));
        assert!(open > shut * 2.0, "open {open}, shut {shut}");
        assert!(shut > 1.0e-4, "it still turns over: {shut}");
        let mut engine = Engine::new(rate);
        for _ in 0..1000 {
            assert_eq!(engine.next(rate), (0.0, 0.0));
        }
    }
}
