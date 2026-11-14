"""Cuts the game's map files into chunks by what is in them, and says which chunks are which.

    python tools/hosting/chunks.py assets/hub.solstice assets/outback.solstice ...

The maps have a good deal in common: the truck, the ground's and the bark's textures, every
kind of tree. Each map's file holds its own copy, at its own place in the file. Cut into
blocks of a fixed size, two files' copies never line up; cut where the bytes themselves say
to (wherever a running hash of the last twenty bytes comes to zero), the same run of bytes is
cut the same way wherever it lies, and so is the same chunks in both files.

For each FILE this writes FILE.chunks: a line a chunk, `OFFSET LENGTH HASH`, the hash being
the first 24 hex digits of the chunk's SHA-256. The browser's loader
(apps/viewer/web/loader.js) keeps what it downloads by HASH, so a chunk it has from one map is
not downloaded again for another. And it prints how much the files share.

Needs numpy.
"""
import hashlib
import sys
from pathlib import Path

import numpy as np

# A chunk ends where the low BITS bits of the hash are zero: one byte in 2**BITS, so chunks
# average 1 MB; none is under LEAST or over MOST.
BITS = 20
LEAST = 256 * 1024
MOST = 4 * 1024 * 1024
# What each byte adds to the hash: the same numbers every time, so that every file is cut alike.
GEAR = np.random.default_rng(0x4F7574626163).integers(0, 1 << 32, 256, dtype=np.uint32)
WINDOW = 32 * 1024 * 1024


def cuts(path):
    """Where each chunk of the file ends."""
    size = path.stat().st_size
    found = []
    with open(path, "rb") as file:
        start = 0
        while start < size:
            # Each window starts BITS bytes early, so that the hash at its first byte is whole.
            back = min(start, BITS)
            file.seek(start - back)
            data = np.frombuffer(file.read(WINDOW + back), dtype=np.uint8)
            gear = GEAR[data]
            hashed = np.zeros(len(data), dtype=np.uint32)
            for shift in range(BITS):
                hashed[shift:] += gear[: len(data) - shift] << np.uint32(shift)
            zero = np.nonzero((hashed[back:] & np.uint32((1 << BITS) - 1)) == 0)[0]
            found.extend((start + zero + 1).tolist())
            start += WINDOW
    ends, last = [], 0
    for cut in found + [size]:
        while cut - last > MOST:
            last += MOST
            ends.append(last)
        if cut - last >= LEAST or cut == size:
            if cut > last:
                ends.append(cut)
                last = cut
    return ends


def chunks(path):
    """The file's chunks: (offset, length, hash)."""
    out, last = [], 0
    with open(path, "rb") as file:
        for end in cuts(path):
            data = file.read(end - last)
            out.append((last, end - last, hashlib.sha256(data).hexdigest()[:24]))
            last = end
    return out


def main():
    paths = [Path(p) for p in sys.argv[1:]]
    if not paths:
        sys.exit(__doc__)
    seen = {}  # hash -> length, over the files so far
    total = fresh_total = 0
    for path in paths:
        mine = chunks(path)
        path.with_suffix(path.suffix + ".chunks").write_text(
            "".join(f"{offset} {length} {digest}\n" for offset, length, digest in mine), encoding="ascii", newline="\n"
        )
        size = sum(length for _, length, _ in mine)
        fresh = sum(length for _, length, digest in mine if digest not in seen)
        for _, length, digest in mine:
            seen[digest] = length
        total += size
        fresh_total += fresh
        print(f"{path.name}: {len(mine)} chunks, {size / 1e6:.0f} MB, {(size - fresh) / 1e6:.0f} MB of it already in the maps before it")
    print(f"all together: {total / 1e6:.0f} MB, {fresh_total / 1e6:.0f} MB once what is shared is counted once")


main()
