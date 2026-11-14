"""Makes the game's sounds from their descriptions, with a text-to-audio model run locally.

    python tools/audio/generate.py [--only NAME ...] [--takes N] [--redo] [--steps 100]

Reads tools/audio/sounds.txt and writes a WAV for each sound into assets/sounds/, which the
game loads. The model is MOSS-SoundEffect v2.0 (OpenMOSS, Apache-2.0): 48 kHz mono, up to 30
seconds a clip. It is not part of the repository; see docs/sound.md for setting it up, and
set MOSS_MODEL to where it is if that is not ../audio/models/MOSS-SoundEffect-v2.0.

What it does to each clip after the model has made it:

* A **loop** is made a little longer than asked, and its tail is faded into its head, so
  that it plays round with no seam. A constant sound, which is what a loop should be, hides
  the join completely.
* A **shot** has the silence trimmed from its ends and a few milliseconds of fade put on
  them, so that it neither starts late nor clicks.
* Both are set to an even loudness, so that the game mixes them by its own numbers rather
  than by how loud the model happened to make each.

`--takes N` makes N different takes of each sound (`name.1.wav`, `name.2.wav`, ...) to choose
between by ear: put the seed of the one you like in sounds.txt.
"""
import argparse
import os
import sys
import time
from pathlib import Path

# The model's own speed-up needs a compiler Windows does not have; without it, it only
# starts sooner.
os.environ.setdefault("TORCHDYNAMO_DISABLE", "1")

RATE = 48_000
ROOT = Path(__file__).resolve().parents[2]
LIST = Path(__file__).with_name("sounds.txt")
OUT = ROOT / "assets" / "sounds"
# How much longer than asked a loop is made, which is how long its ends are faded together.
OVERLAP = 2.0
# How loud each kind is left, as its RMS in decibels below full scale; and no sample above
# the ceiling.
LEVEL = {"loop": -22.0, "shot": -16.0}
CEILING = 0.89
# How many frames to the second the model builds a sound from, which it leaves whistles at
# the multiples of: see `unwhistled`.
FRAME_RATE = 50
# What no sound here should have in it, which the model is told to keep away from.
UNWANTED = "music, speech, voices, electrical hum, buzzing, static, hiss, distortion, low quality"


def read_list(path):
    """The sounds asked for: name, kind, seconds, seed, description."""
    sounds = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = line.split("#")[0].strip()
        if not line:
            continue
        words = line.split(None, 4)
        if len(words) < 5 or words[1] not in LEVEL:
            sys.exit(f"{path.name}:{number}: expected `NAME loop|shot SECONDS SEED DESCRIPTION`")
        name, kind, seconds, seed, prompt = words
        seconds = float(seconds)
        longest = 30.0 - (OVERLAP if kind == "loop" else 0.0)
        if not 0.5 <= seconds <= longest:
            sys.exit(f"{path.name}:{number}: a {kind} is from 0.5 to {longest:g} seconds")
        sounds.append((name, kind, seconds, int(seed), prompt))
    return sounds


def unwhistled(wave, np):
    """The sound without the model's whistles.

    The model builds a sound fifty frames to the second, and leaves a trace of that in it: a
    faint steady tone at multiples of 50 Hz, from about 4 kHz up. Under a creek they are not
    heard. In a quiet sound, brought up to its loudness, they are a buzz. Each is a line a
    few hertz wide (wider the shorter the sound) that stands well clear of what is on either
    side of it, so it is found in the spectrum of the whole sound and brought down to its
    surroundings; whatever else is at that pitch, at that level, is left.
    """
    count = len(wave)
    spectrum = np.fft.rfft(wave)
    power = np.abs(spectrum) ** 2
    per_hz = count / RATE
    width = max(2.5, 3.0 * RATE / count)
    half = max(int(width * per_hz), 1)
    near, far = int((width + 2.0) * per_hz), int(24 * per_hz)
    for line in range(FRAME_RATE * 40, RATE // 2 - FRAME_RATE, FRAME_RATE):
        centre = int(round(line * per_hz))
        if centre + far >= len(power):
            break
        inside = power[centre - half : centre + half + 1].mean()
        outside = np.concatenate(
            [power[centre - far : centre - near], power[centre + near : centre + far]]
        ).mean()
        if inside > outside * 1.6:
            spectrum[centre - half : centre + half + 1] *= (outside / inside) ** 0.5
    return np.fft.irfft(spectrum, count).astype(np.float32)


def looped(wave, np):
    """A loop with no seam: its last `OVERLAP` seconds faded into its first, at even power."""
    overlap = int(OVERLAP * RATE)
    body, tail = wave[:-overlap], wave[-overlap:]
    t = np.linspace(0.0, 1.0, overlap, dtype=np.float32)
    head = body[:overlap] * np.sin(t * np.pi / 2) + tail * np.cos(t * np.pi / 2)
    return np.concatenate([head, body[overlap:]])


def trimmed(wave, np):
    """A shot without the quiet before it and after it, and with its ends eased."""
    window = RATE // 100
    frames = len(wave) // window
    loud = np.sqrt((wave[: frames * window].reshape(frames, window) ** 2).mean(axis=1))
    floor = max(loud.max() * 10 ** (-42 / 20), 1.0e-5)
    heard = np.nonzero(loud > floor)[0]
    if len(heard):
        start = max(heard[0] - 1, 0) * window
        end = min(heard[-1] + 12, frames) * window
        wave = wave[start:end]
    fade_in, fade_out = min(RATE // 500, len(wave) // 2), min(RATE // 12, len(wave) // 2)
    wave = wave.copy()
    wave[:fade_in] *= np.linspace(0.0, 1.0, fade_in, dtype=np.float32)
    wave[-fade_out:] *= np.linspace(1.0, 0.0, fade_out, dtype=np.float32)
    return wave


def levelled(wave, kind, np):
    """Set to its kind's loudness, and no louder anywhere than the ceiling."""
    wave = wave - wave.mean()
    rms = float(np.sqrt((wave**2).mean())) or 1.0
    wave = wave * (10 ** (LEVEL[kind] / 20) / rms)
    peak = float(np.abs(wave).max())
    if peak > CEILING:
        wave = wave * (CEILING / peak)
    return wave.astype(np.float32)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--only", nargs="*", default=None, help="just these sounds, by name")
    parser.add_argument("--takes", type=int, default=1, help="takes of each, to choose between")
    parser.add_argument("--redo", action="store_true", help="make again what is already made")
    parser.add_argument("--steps", type=int, default=100, help="the model's solver steps")
    parser.add_argument("--cfg", type=float, default=4.0, help="how closely it keeps to the words")
    args = parser.parse_args()

    sounds = read_list(LIST)
    if args.only is not None:
        missing = set(args.only) - {sound[0] for sound in sounds}
        if missing:
            sys.exit(f"not in {LIST.name}: {', '.join(sorted(missing))}")
        sounds = [sound for sound in sounds if sound[0] in args.only]
    OUT.mkdir(parents=True, exist_ok=True)

    def path_of(name, take):
        return OUT / (f"{name}.wav" if args.takes == 1 else f"{name}.{take + 1}.wav")

    wanted = [
        (sound, take)
        for sound in sounds
        for take in range(args.takes)
        if args.redo or not path_of(sound[0], take).exists()
    ]
    if not wanted:
        print("every sound is made already (--redo makes them again)")
        return

    import numpy as np
    import soundfile
    import torch
    from moss_soundeffect_v2 import MossSoundEffectPipeline

    model = os.environ.get("MOSS_MODEL") or str(
        ROOT.parent / "audio" / "models" / "MOSS-SoundEffect-v2.0"
    )
    if not Path(model).exists():
        sys.exit(f"the model is not at {model}: see docs/sound.md, or set MOSS_MODEL")
    if not torch.cuda.is_available():
        sys.exit("no CUDA GPU is seen: the model needs one")
    print(f"loading {model}...")
    pipe = MossSoundEffectPipeline.from_pretrained(model, torch_dtype=torch.bfloat16, device="cuda")

    for (name, kind, seconds, seed, prompt), take in wanted:
        made = seconds + (OVERLAP if kind == "loop" else 0.0)
        print(f"{name} ({kind}, {seconds:g} s, seed {seed + take})", flush=True)
        began = time.monotonic()
        audio = pipe(
            prompt=prompt,
            seconds=made,
            num_inference_steps=args.steps,
            cfg_scale=args.cfg,
            seed=seed + take,
            negative_prompt=UNWANTED,
            # Its own progress bar is a line a step: say when each sound is done instead.
            progress_bar_cmd=lambda steps: steps,
        )
        print(f"    made in {time.monotonic() - began:.0f} s", flush=True)
        # What one sound used is given back before the next: the model all but fills a
        # 16 GB card, and what spills over into the machine's own memory runs twenty times
        # slower.
        torch.cuda.empty_cache()
        wave = audio[0].float().mean(dim=0).cpu().numpy().astype(np.float32)
        wave = unwhistled(wave, np)
        wave = looped(wave, np) if kind == "loop" else trimmed(wave, np)
        wave = levelled(wave, kind, np)
        soundfile.write(path_of(name, take), wave, RATE, subtype="PCM_16")
    print(f"{len(wanted)} sounds written to {OUT}")


if __name__ == "__main__":
    main()
