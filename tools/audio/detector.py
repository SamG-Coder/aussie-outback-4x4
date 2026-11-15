"""Makes the gold detector's sound: its threshold, the one steady hum a gold detector makes.

    python tools/audio/detector.py [assets/sounds]

A gold detector does not beep. It hums, quietly and all the time (its *threshold*), and
gold is heard as that hum changing: as the coil passes over a target the hum rises in pitch
and swells, and falls back as the coil goes on. So there is one sound here, the hum, made
by arithmetic and not from a description as the rest are (generate.py): a steady reedy note
that comes round with no seam. The game plays it higher and louder as the coil passes, by
how near the gold is (`Soundscape::frame` in apps/viewer/src/play.rs).

Needs numpy. Writes detector_tone.wav: one channel, 16 bits, 48 kHz, two seconds.
"""
import pathlib
import sys
import wave

import numpy as np

RATE = 48_000
SECONDS = 2.0
# The note: low, as a detector's threshold is set, with room above it to rise into. A whole
# number of its waves fits the loop, and of everything else that moves in it.
NOTE = 310.0


def main():
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "assets/sounds")
    out.mkdir(parents=True, exist_ok=True)
    count = int(RATE * SECONDS)
    t = np.arange(count) / RATE
    wave_of = lambda cycles_a_second: 2.0 * np.pi * cycles_a_second * t
    # A reedy hum: the note and its overtones, the odd ones stronger, each fainter than the
    # last, and none above what a small speaker in a control box gives.
    hum = np.zeros(count)
    for overtone in range(1, 13):
        strength = (1.0 if overtone % 2 else 0.45) / overtone**1.45
        hum += strength * np.sin(wave_of(NOTE * overtone) + 0.7 * overtone)
    # It is never quite still: it wavers a little, slowly, and a little faster.
    hum *= 1.0 + 0.035 * np.sin(wave_of(3.0)) + 0.02 * np.sin(wave_of(7.5) + 1.0)
    # And a breath of hiss under it, which comes round with the loop as the rest does.
    rng = np.random.default_rng(1851)
    spectrum = np.fft.rfft(rng.standard_normal(count))
    cycles = np.fft.rfftfreq(count, 1.0 / RATE)
    spectrum *= np.exp(-(((cycles - 2400.0) / 1500.0) ** 2))
    hiss = np.fft.irfft(spectrum, count)
    hum = hum / np.abs(hum).max() + 0.035 * hiss / np.abs(hiss).max()
    samples = np.round(hum / np.abs(hum).max() * 0.82 * 32767).astype("<i2")
    path = out / "detector_tone.wav"
    with wave.open(str(path), "wb") as file:
        file.setnchannels(1)
        file.setsampwidth(2)
        file.setframerate(RATE)
        file.writeframes(samples.tobytes())
    print(f"wrote {path}: {SECONDS:.0f} s, {NOTE:.0f} Hz")


if __name__ == "__main__":
    main()
