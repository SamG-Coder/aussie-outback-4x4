"""The shed's textures, made from nothing: weathered corrugated iron, rusting iron, sawn
hardwood gone grey, bush poles, a concrete slab, and worn paint.

Each is three pictures a metre or two across that repeat without a seam: colour, the lie of
the surface (a normal map) and how rough and how much a metal it is. They are worked out with
numpy, which Blender carries, so that the shed is made by its script alone and nothing is
drawn by hand. Noise that repeats comes from filtering random numbers by their frequencies,
which wraps round at the picture's edges of itself.
"""
import numpy as np

SIZE = 1024


def noise(seed, falloff=2.0, stretch=(1.0, 1.0), low=0.0):
    """Noise from 0 to 1 that repeats at the edges. `falloff` is how fast fine detail fades
    (more is smoother); `stretch` draws it out along x and y; `low` cuts the broadest."""
    rng = np.random.default_rng(seed)
    white = np.fft.fft2(rng.standard_normal((SIZE, SIZE)))
    # Drawn out along an axis, it changes that many times more slowly along it.
    fy = np.fft.fftfreq(SIZE)[:, None] * SIZE * stretch[1]
    fx = np.fft.fftfreq(SIZE)[None, :] * SIZE * stretch[0]
    f = np.sqrt(fx * fx + fy * fy)
    f[0, 0] = 1.0
    shaped = white / f ** (falloff / 2.0)
    shaped[f < low] = 0.0
    shaped[0, 0] = 0.0
    out = np.real(np.fft.ifft2(shaped))
    out -= out.min()
    return out / out.max()


def step(x, lo, hi):
    t = np.clip((x - lo) / (hi - lo), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def mix(a, b, t):
    """Some of `a` and some of `b`: all `b` where `t` is 1."""
    if np.ndim(t) == 2:
        t = t[..., None]
    return a * (1.0 - t) + b * t


def colour(r, g, b):
    return np.array([r, g, b], dtype=np.float64)[None, None, :] * np.ones((SIZE, SIZE, 1))


def streaks(seed, fine=3.0, long=60.0):
    """Runs down the picture, as rain carries dirt and rust: narrow across, long down."""
    return noise(seed, falloff=fine, stretch=(1.0, long))


def normal_from(height, strength):
    """A normal map of a height picture, repeating as it does."""
    dx = (np.roll(height, -1, axis=1) - np.roll(height, 1, axis=1)) * 0.5 * strength * SIZE / 64.0
    dy = (np.roll(height, -1, axis=0) - np.roll(height, 1, axis=0)) * 0.5 * strength * SIZE / 64.0
    n = np.stack([-dx, -dy, np.ones_like(height)], axis=-1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    return n * 0.5 + 0.5


def orm(rough, metal):
    return np.stack([np.ones_like(rough), np.clip(rough, 0.04, 1.0), np.clip(metal, 0.0, 1.0)], axis=-1)


def iron(rust_amount, seed):
    """Galvanised corrugated iron some decades out in the weather. `rust_amount` from 0, a
    dulled sheet with rust only starting in streaks, to 1, mostly rusted through its zinc."""
    spangle = noise(seed, 1.2, low=18.0)             # the zinc's fine crystal
    cloud = noise(seed + 1, 2.6)                      # broad dulling
    grime = streaks(seed + 2, 2.4, 40.0)
    run = streaks(seed + 3, 2.0, 90.0)
    blotch = noise(seed + 4, 2.2, low=2.0)
    fine = noise(seed + 5, 1.0, low=60.0)

    zinc = colour(0.60, 0.62, 0.63) * (0.86 + 0.2 * cloud[..., None]) * (0.95 + 0.1 * spangle[..., None])
    # White rust and chalk where water sits, grey dirt run down the sheet.
    chalk = step(blotch * 0.6 + cloud * 0.4, 0.55, 0.8)
    zinc = mix(zinc, colour(0.74, 0.75, 0.73), chalk * 0.5)
    zinc = mix(zinc, colour(0.33, 0.31, 0.28), step(grime, 0.5, 0.85) * 0.55)

    # Rust: it starts where the runs are and spreads in blotches.
    start = 0.78 - 0.5 * rust_amount
    rusted = step(blotch * 0.55 + run * 0.45 + fine * 0.06, start, start + 0.16)
    deep = step(blotch * 0.5 + cloud * 0.5 + fine * 0.05, start + 0.12, start + 0.3) * rusted
    rust = mix(colour(0.50, 0.25, 0.11), colour(0.27, 0.12, 0.06), deep)
    rust = rust * (0.8 + 0.4 * fine[..., None])
    # A stain of it runs below each patch.
    stain = step(np.roll(rusted, SIZE // 14, axis=0) * 0.6 + run * 0.5, 0.55, 0.95) * (1.0 - rusted)
    base = mix(zinc, colour(0.52, 0.33, 0.18), stain * 0.45)
    base = mix(base, rust, rusted)

    rough = 0.42 + 0.2 * cloud + 0.16 * chalk + 0.38 * rusted + 0.1 * step(grime, 0.5, 0.85)
    metal = 0.92 - 0.82 * rusted - 0.3 * chalk - 0.25 * step(grime, 0.5, 0.85)
    height = spangle * 0.04 + fine * 0.05 + rusted * (0.25 + 0.5 * fine) + deep * 0.2
    return base, normal_from(height, 2.2), orm(rough, metal)


def timber(seed, warm=0.0, cracks=1.0):
    """Sawn hardwood left in the sun: silver-grey, its grain along the picture's width, with
    checks opened along it. `warm` keeps some of its brown, as under a roof."""
    grain = noise(seed, 1.6, stretch=(28.0, 1.0))
    fibre = noise(seed + 1, 1.0, stretch=(50.0, 1.0), low=40.0)
    cloud = noise(seed + 2, 2.6, stretch=(3.0, 1.0))
    band = noise(seed + 3, 3.0, stretch=(20.0, 1.0))
    check = noise(seed + 4, 1.9, stretch=(80.0, 1.0), low=6.0)
    grey = mix(colour(0.36, 0.34, 0.31), colour(0.62, 0.60, 0.56), grain * 0.6 + band * 0.4)
    brown = mix(colour(0.24, 0.15, 0.09), colour(0.50, 0.34, 0.20), grain * 0.5 + band * 0.5)
    base = mix(grey, brown, np.clip(warm + (cloud - 0.5) * 0.5, 0.0, 1.0))
    base = base * (0.9 + 0.2 * fibre[..., None])
    open_ = step(check, 0.80 - 0.04 * cracks, 0.9) * cracks
    base = mix(base, colour(0.07, 0.06, 0.05), np.clip(open_, 0.0, 1.0) * 0.9)
    rough = 0.82 + 0.12 * fibre - 0.1 * warm
    height = grain * 0.5 + fibre * 0.25 - open_ * 1.4
    return base, normal_from(height, 3.2), orm(rough, np.zeros_like(rough))


def concrete(seed):
    """A slab poured long ago: mottled, pitted, with a hairline crack or two and oil on it."""
    cloud = noise(seed, 2.4)
    mottle = noise(seed + 1, 1.8, low=4.0)
    fine = noise(seed + 2, 0.8, low=50.0)
    pits = step(noise(seed + 3, 1.0, low=30.0), 0.74, 0.8)
    wander = noise(seed + 4, 2.6)
    crack = 1.0 - step(np.abs(wander - 0.5), 0.0, 0.006)
    oil = step(noise(seed + 5, 3.2), 0.68, 0.86)
    base = colour(0.50, 0.49, 0.46) * (0.78 + 0.3 * cloud[..., None]) * (0.92 + 0.16 * mottle[..., None])
    base = mix(base, colour(0.42, 0.33, 0.24), step(mottle * 0.5 + cloud * 0.5, 0.5, 0.9) * 0.35)  # red dust
    base = mix(base, colour(0.10, 0.09, 0.08), oil * 0.7)
    base = mix(base, colour(0.16, 0.15, 0.14), np.clip(pits * 0.6 + crack * 0.85, 0.0, 1.0))
    rough = 0.9 - 0.5 * oil + 0.05 * fine
    height = fine * 0.2 + mottle * 0.3 - pits * 0.8 - crack * 1.2
    return base, normal_from(height, 2.0), orm(rough, np.zeros_like(rough))


def worn(seed):
    """Paint and plain metal that have been used: near white, to be tinted by what wears it,
    with scuffs, scratches and dirt in the corners of its grain."""
    cloud = noise(seed, 2.4)
    scratch = noise(seed + 1, 1.2, stretch=(30.0, 1.0), low=30.0)
    scratch2 = noise(seed + 2, 1.2, stretch=(1.0, 30.0), low=30.0)
    fine = noise(seed + 3, 0.9, low=40.0)
    chips = step(noise(seed + 4, 1.6, low=8.0), 0.7, 0.76)
    scuff = np.maximum(step(scratch, 0.72, 0.8), step(scratch2, 0.74, 0.82))
    base = colour(0.92, 0.92, 0.92) * (0.8 + 0.25 * cloud[..., None])
    base = mix(base, colour(1.0, 1.0, 1.0), scuff * 0.5)
    base = mix(base, colour(0.36, 0.30, 0.25), chips * 0.8)
    base = mix(base, colour(0.55, 0.48, 0.40), step(cloud, 0.6, 0.9) * 0.35)
    rough = 0.5 + 0.2 * cloud + 0.2 * chips + 0.1 * scuff
    height = fine * 0.15 - chips * 0.5 - scuff * 0.3
    return base, normal_from(height, 1.6), orm(rough, 0.15 * (1.0 - chips))


def everything():
    """Every texture the shed wears: its name, then colour, normal, and roughness with metal."""
    return {
        "iron": iron(0.25, 11),
        "rust": iron(0.95, 23),
        "timber": timber(31, warm=0.1),
        "pole": timber(47, warm=0.45, cracks=1.6),
        "board": timber(59, warm=0.75, cracks=0.4),
        "concrete": concrete(61),
        "worn": worn(73),
    }
