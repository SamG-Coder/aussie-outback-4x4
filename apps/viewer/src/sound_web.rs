//! Sound in a browser: what the game asks to be heard, kept for the page to play.
//!
//! On the desktop the game's sounds are mixed here, sample by sample (`sound.rs`). In a
//! browser the mixing is the browser's own, done away from the page, which is busy drawing:
//! the page keeps a loop of each sound that loops and sets how loud and how fast each plays
//! (`web/sound.js`). So this is the same thing to ask of as the desktop's mixer, [`Sounds`],
//! and what it does with what is asked is write it down, for the page to take each frame.
//!
//! The engine is its recorded loops (`sound_table.rs`), as on the desktop: the two nearest
//! the revs, each played at the speed that brings it to them, and the overrun's.
use crate::sound_table::{coasting, crossfade, driven, pitch, ENGINE_LAYERS, ENGINE_OVERRUN};

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

/// What is to be heard: every loop's volume and pitch as last asked for, and the sounds to
/// play once that the page has not yet been told of.
#[derive(Default)]
pub struct Sounds {
    loops: Vec<(String, f32, f32)>,
    shots: Vec<(String, f32, f32)>,
}

/// What a frame asks of the sound.
pub struct Asking<'a> {
    sounds: &'a mut Sounds,
}

impl Sounds {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the game asks for this frame, all at once: `ask` is handed something to ask with.
    pub fn frame(&mut self, ask: impl FnOnce(&mut Asking<'_>)) {
        ask(&mut Asking { sounds: self });
    }

    /// What the page is to play, as JSON: `loops`, each a name, a volume and a pitch, which
    /// stand until they are asked for otherwise, and `shots`, the same, each to be played
    /// once, now.
    pub fn take(&mut self) -> String {
        let round = |value: f32| (f64::from(value) * 1000.0).round() / 1000.0;
        let list = |sounds: &[(String, f32, f32)]| -> Vec<serde_json::Value> {
            sounds
                .iter()
                .map(|(name, gain, pitch)| serde_json::json!([name, round(*gain), round(*pitch)]))
                .collect()
        };
        let told = serde_json::json!({ "loops": list(&self.loops), "shots": list(&self.shots) });
        self.shots.clear();
        told.to_string()
    }
}

impl Asking<'_> {
    /// A loop's volume and pitch from now on.
    pub fn looping(&mut self, name: &str, gain: f32, pitch: f32) {
        let (gain, pitch) = (gain.max(0.0), pitch.clamp(0.05, 4.0));
        match self.sounds.loops.iter_mut().find(|l| l.0 == name) {
            Some(asked) => (asked.1, asked.2) = (gain, pitch),
            None => self.sounds.loops.push((name.to_owned(), gain, pitch)),
        }
    }

    /// Plays a sound once, from now.
    pub fn once(&mut self, name: &str, gain: f32, pitch: f32) {
        // No more at once than can be told apart.
        if self.sounds.shots.len() < 24 {
            self.sounds
                .shots
                .push((name.to_owned(), gain.max(0.0), pitch.clamp(0.05, 4.0)));
        }
    }

    /// The engine's note from now on: its loops' volumes and pitches.
    pub fn engine(&mut self, note: EngineNote) {
        let rpm = note.rpm.clamp(300.0, 12_000.0);
        // At the limiter the spark is cut and comes back, too fast to be asked for a frame
        // at a time: it is heard as the engine held back.
        let gain = note.gain.max(0.0) * if note.limiter { 0.7 } else { 1.0 };
        let loud = driven(note.throttle) * gain;
        // Which two loops the revs are between, and how far.
        let last = ENGINE_LAYERS.len() - 1;
        let above = ENGINE_LAYERS
            .iter()
            .position(|layer| layer.1 > rpm)
            .unwrap_or(last + 1);
        for (index, (name, own)) in ENGINE_LAYERS.iter().enumerate() {
            let weight = if above == 0 {
                f32::from(u8::from(index == 0))
            } else if above > last {
                f32::from(u8::from(index == last))
            } else {
                let (low, high) = (ENGINE_LAYERS[above - 1].1, ENGINE_LAYERS[above].1);
                let (lower, higher) = crossfade((rpm - low) / (high - low));
                if index == above - 1 {
                    lower
                } else if index == above {
                    higher
                } else {
                    0.0
                }
            };
            self.looping(name, weight * loud, pitch(rpm, *own));
        }
        let (name, own) = ENGINE_OVERRUN;
        self.looping(name, coasting(note.throttle, rpm) * gain, pitch(rpm, own));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(sounds: &Sounds, name: &str) -> (f32, f32) {
        let found = sounds
            .loops
            .iter()
            .find(|l| l.0 == name)
            .expect("it was asked for");
        (found.1, found.2)
    }

    #[test]
    fn the_engine_is_the_two_loops_its_revs_are_between() {
        let mut sounds = Sounds::new();
        let note = |rpm| EngineNote {
            rpm,
            throttle: 1.0,
            limiter: false,
            gain: 1.0,
        };
        // At a loop's own revs, it alone, at its own speed.
        sounds.frame(|ask| ask.engine(note(1380.0)));
        assert_eq!(asked(&sounds, "engine_low"), (1.0, 1.0));
        assert_eq!(asked(&sounds, "engine_idle").0, 0.0);
        assert_eq!(asked(&sounds, "engine_mid").0, 0.0);
        // Half-way to the next, as much of each, the lower sped up and the higher slowed.
        sounds.frame(|ask| ask.engine(note(1807.5)));
        let (low, mid) = (asked(&sounds, "engine_low"), asked(&sounds, "engine_mid"));
        assert!((low.0 - mid.0).abs() < 1.0e-4 && low.0 > 0.7);
        assert!(low.1 > 1.0 && mid.1 < 1.0);
        // Above the highest, it alone, sped up; and with the throttle open, no overrun.
        sounds.frame(|ask| ask.engine(note(6500.0)));
        assert!(asked(&sounds, "engine_high") == (1.0, 6500.0 / 4740.0));
        assert_eq!(asked(&sounds, "engine_off").0, 0.0);
        // Off the throttle at speed, the overrun is heard, and the rest only ticks over.
        sounds.frame(|ask| {
            ask.engine(EngineNote {
                rpm: 3500.0,
                throttle: 0.0,
                limiter: false,
                gain: 1.0,
            })
        });
        assert!(asked(&sounds, "engine_off").0 > 0.6);
        assert!(asked(&sounds, "engine_mid").0 < 0.3);
    }

    #[test]
    fn a_shot_is_told_once_and_a_loop_stands() {
        let mut sounds = Sounds::new();
        sounds.frame(|ask| {
            ask.looping("creek", 0.5, 1.0);
            ask.once("splash_big", 0.9, 1.1);
        });
        let first: serde_json::Value = serde_json::from_str(&sounds.take()).expect("it is JSON");
        assert_eq!(first["loops"], serde_json::json!([["creek", 0.5, 1.0]]));
        assert_eq!(
            first["shots"],
            serde_json::json!([["splash_big", 0.9, 1.1]])
        );
        let second: serde_json::Value = serde_json::from_str(&sounds.take()).expect("it is JSON");
        assert_eq!(second["loops"], serde_json::json!([["creek", 0.5, 1.0]]));
        assert_eq!(second["shots"], serde_json::json!([]));
    }
}
