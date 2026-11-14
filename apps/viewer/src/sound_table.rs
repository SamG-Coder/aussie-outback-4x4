//! The engine's loops, which the desktop's mixer (`sound.rs`) and the browser's (`sound_web.rs`)
//! both play.

/// The loops the engine is made of, by name, and the revs each is at. A set with any two of
/// them is an engine; the more there are, the less each is stretched.
///
/// The revs are the ones heard in the loops `tools/audio/sounds.txt` makes, not the ones
/// their descriptions ask for: a V8 fires four times a turn, so a loop whose firing note is
/// at F Hz is at F x 15 rpm. Their notes are at 56, 92, 149 and 316 Hz.
pub const ENGINE_LAYERS: [(&str, f32); 4] = [
    ("engine_idle", 840.0),
    ("engine_low", 1380.0),
    ("engine_mid", 2235.0),
    ("engine_high", 4740.0),
];
/// And the loop for a shut throttle at speed: the overrun.
pub const ENGINE_OVERRUN: (&str, f32) = ("engine_off", 2250.0);

/// How much of each of two loops is heard, the lower and the higher, with the revs this far
/// between theirs, from 0 to 1: faded one into the other at even power, so that a pull through
/// the revs has no step in it.
pub fn crossfade(share: f32) -> (f32, f32) {
    let turn = share.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (turn.cos(), turn.sin())
}

/// How loud the engine's driven loops are for a throttle: on it, loud; off it, it still ticks
/// over.
pub fn driven(throttle: f32) -> f32 {
    0.3 + 0.7 * throttle.clamp(0.0, 1.0).powf(0.7)
}

/// How loud the overrun's loop is: with the throttle shut, at speed.
pub fn coasting(throttle: f32, rpm: f32) -> f32 {
    (1.0 - throttle.clamp(0.0, 1.0)).powi(2) * ((rpm - 1300.0) / 1500.0).clamp(0.0, 1.0) * 0.7
}

/// How fast a loop held at `own` revs is played to be at `rpm`.
pub fn pitch(rpm: f32, own: f32) -> f32 {
    (rpm / own).clamp(0.5, 2.4)
}
