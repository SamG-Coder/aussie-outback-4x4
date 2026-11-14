"""Measures the engine's loops: what note each sits on, and whether it holds it.

    python tools/audio/engine_pitch.py            # every assets/sounds/engine_*.wav

A loop made from a description comes out at whatever revs it comes out at. The game fades
between loops by the revs each is at (assets/sounds/engine.txt), so those have to be the revs
heard, not the revs asked for. For each loop this prints:

- its strongest low peaks, with how far below the strongest each is. A V8 fires four times a
  turn, so a loop whose firing note is at F Hz is at F x 15 rpm; the peak at a quarter of that
  is the crankshaft turning, and the half is between;
- how its sound is shared between low and high, which is how bright it is;
- how far its loudness and its brightness wander through the loop: a steady one barely moves.
"""
import sys
from pathlib import Path

import numpy as np
import soundfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "sounds"
BANDS = [(20, 80), (80, 250), (250, 1000), (1000, 4000), (4000, 12000)]


def measure(path):
    wave, rate = soundfile.read(path, dtype="float32")
    if wave.ndim > 1:
        wave = wave.mean(axis=1)
    spectrum = np.abs(np.fft.rfft(wave * np.hanning(len(wave))))
    freqs = np.fft.rfftfreq(len(wave), 1 / rate)
    # Smoothed over about 2 Hz, so that a note that wavers a little is still one peak.
    width = max(int(2.0 * len(wave) / rate), 1)
    smooth = np.convolve(spectrum**2, np.ones(width) / width, mode="same") ** 0.5
    low = (freqs > 18) & (freqs < 900)
    fs, ss = freqs[low], smooth[low]
    reach = width * 3
    peaks = [i for i in range(reach, len(ss) - reach) if ss[i] == ss[i - reach : i + reach + 1].max()]
    peaks = sorted(sorted(peaks, key=lambda i: -ss[i])[:7])
    top = max(ss[i] for i in peaks)
    notes = "  ".join(f"{fs[i]:.0f} Hz ({20 * np.log10(ss[i] / top):+.0f})" for i in peaks)
    shares = [(spectrum[(freqs >= a) & (freqs < b)] ** 2).sum() for a, b in BANDS]
    shares = "  ".join(f"{10 * np.log10(x / sum(shares)):.0f}" for x in shares)
    # Second by second: how loud, and how bright.
    louds, brights = [], []
    for start in range(0, len(wave) - rate + 1, rate // 2):
        piece = wave[start : start + rate]
        part = np.abs(np.fft.rfft(piece * np.hanning(rate)))
        f = np.fft.rfftfreq(rate, 1 / rate)
        louds.append(np.sqrt((piece**2).mean()))
        brights.append((f * part).sum() / part.sum())
    wander = 20 * np.log10(max(louds) / min(louds))
    print(f"{path.stem}: {len(wave) / rate:.1f} s")
    print(f"    peaks: {notes}")
    print(f"    share in 20-80, 80-250, 250-1k, 1k-4k, 4k-12k Hz: {shares} dB")
    print(f"    wanders {wander:.1f} dB in loudness, {min(brights):.0f} to {max(brights):.0f} Hz in brightness")


paths = [Path(p) for p in sys.argv[1:]] or sorted(OUT.glob("engine_*.wav"))
for path in paths:
    measure(path)
