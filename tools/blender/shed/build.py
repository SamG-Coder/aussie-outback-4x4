"""Builds the shed at the Crossroads: a bush machinery shed with a bunk room, as a model.

  blender -b --python tools/blender/shed/build.py -- OUT.glb [KEEP_FOLDER]

A skillion roof of corrugated iron on a frame of bush poles: on the left an open bay to park
the truck in, on the right a slab-fronted room to sleep in behind a verandah, and a rainwater tank
at the end. It is kept bare: two lanterns, and by the wall of the room the chest.

Nothing is drawn by hand. Every piece is put together here from boxes, poles, turned shapes
and corrugated sheets, each sheet a real corrugated surface with its screws; the textures are
made in `textures.py`. KEEP_FOLDER, if given, is where the Blender file and the textures are
kept to look at (`shed.blend`, `textures/`), and two pictures of it rendered (`shed_*.png`).

The model faces Blender's -Y, which is +Z in the file: the open front. Its origin is on the
ground at the middle of the slab. The game is told where its walls are, to stop a truck, and
where to park, by `crates/solstice-bake/src/outback/shed.rs`, which has the same measurements.
"""
import math
import os
import random
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textures  # noqa: E402

args = sys.argv[sys.argv.index("--") + 1:]
OUT = args[0]
KEEP = args[1] if len(args) > 1 else None
rng = random.Random(1851)

# ---- The measurements, in metres.
HALF_W, HALF_D = 5.0, 3.5           # the posts' rectangle: 10 m wide, 7 m deep
SLOPE = 0.9 / 7.0                    # the roof falls 0.9 m from front to back
TILT = math.atan(SLOPE)
ROOM_FRONT = -0.4                    # the room's front wall: a verandah before it
PITCH, WAVE = 0.0762, 0.0095         # corrugated iron: three inches a wave
SHEET = 10 * PITCH                   # a sheet covers ten of them


def beam_top(y):
    """How high the top of the frame's beams is, at a place front to back."""
    return 3.20 - (y + HALF_D) * SLOPE


def roof_at(y):
    """The underside of the roof's iron there: on the battens, which are on the rafters."""
    return beam_top(y) + 0.15 + 0.04


def move(x=0.0, y=0.0, z=0.0):
    return Matrix.Translation((x, y, z))


def turn(axis, degrees):
    return Matrix.Rotation(math.radians(degrees), 4, axis)


def frame(origin, x, y, z):
    """A matrix that carries a piece's own x, y and z to these three directions."""
    m = Matrix.Identity(4)
    for row in range(3):
        m[row][0], m[row][1], m[row][2], m[row][3] = x[row], y[row], z[row], origin[row]
    return m


def between(a, b):
    """A matrix whose z runs from `a` to `b`, and how far that is."""
    a, b = Vector(a), Vector(b)
    z = (b - a).normalized()
    x = z.orthogonal().normalized()
    return frame(a, x, z.cross(x), z), (b - a).length


# ---- The pieces gather here, by what they are made of and how they are finished, and each
# gathering becomes one mesh.
class Pieces:
    def __init__(self):
        self.groups = {}

    def add(self, material, finish, verts, faces, uvs, at):
        """`finish` is `hard` (flat faces), `cut` (flat, its edges eased), `soft` (flat,
        well rounded), `round` (smooth) or `sheet` (smooth, seen from both sides)."""
        group = self.groups.setdefault((material, finish), ([], [], []))
        first = len(group[0])
        group[0].extend(at @ Vector(v) for v in verts)
        mirrored = at.determinant() < 0
        for face, uv in zip(faces, uvs):
            if mirrored:
                face, uv = face[::-1], uv[::-1]
            group[1].append([first + i for i in face])
            group[2].append(uv)


pieces = Pieces()
# The door of the room is a model of its own, about its hinge, which the game swings.
door_pieces = Pieces()
# Where its hinge is, at the foot of the door.
HINGE = (0.785, ROOM_FRONT + 0.03, 0.19)


def box(material, sx, sy, sz, at, grain="x", tile=1.0, finish="cut"):
    """A box about its own middle. Its texture's grain runs along the axis named."""
    hx, hy, hz = sx / 2, sy / 2, sz / 2
    verts = [(-hx, -hy, -hz), (hx, -hy, -hz), (hx, hy, -hz), (-hx, hy, -hz),
             (-hx, -hy, hz), (hx, -hy, hz), (hx, hy, hz), (-hx, hy, hz)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (2, 3, 7, 6), (1, 2, 6, 5), (3, 0, 4, 7)]
    axis = "xyz".index(grain)
    ou, ov = rng.random(), rng.random()
    uvs = []
    for face in faces:
        corners = [verts[i] for i in face]
        flat = next(a for a in range(3) if all(abs(c[a] - corners[0][a]) < 1e-9 for c in corners))
        across = next(a for a in range(3) if a not in (flat, axis)) if flat != axis else (axis + 1) % 3
        along = axis if flat != axis else (axis + 2) % 3
        uvs.append([(c[along] / tile + ou, c[across] / tile + ov) for c in corners])
    pieces.add(material, finish, verts, faces, uvs, at)


def turned(material, profile, at, around=24, tile=1.0, finish="round"):
    """A shape turned on a lathe about its own z: `profile` is its outline, (radius, z)."""
    verts, faces, uvs = [], [], []
    lengths = [0.0]
    for (r0, z0), (r1, z1) in zip(profile, profile[1:]):
        lengths.append(lengths[-1] + math.hypot(r1 - r0, z1 - z0))
    girth = max(r for r, _ in profile) * math.tau
    ou = rng.random()
    for r, z in profile:
        for step in range(around):
            angle = step / around * math.tau
            verts.append((r * math.cos(angle), r * math.sin(angle), z))
    for ring in range(len(profile) - 1):
        for step in range(around):
            a, b = ring * around + step, ring * around + (step + 1) % around
            faces.append((a, b, b + around, a + around))
            u0, u1 = step / around * girth / tile + ou, (step + 1) / around * girth / tile + ou
            v0, v1 = lengths[ring] / tile, lengths[ring + 1] / tile
            uvs.append([(v0, u0), (v0, u1), (v1, u1), (v1, u0)])
    pieces.add(material, finish, verts, faces, uvs, at)


def rod(material, a, b, radius, around=10, tile=1.0, finish="round", ends=True):
    """A straight round bar from one place to another."""
    at, length = between(a, b)
    profile = [(radius, 0.0), (radius, length)]
    if ends:
        profile = [(0.0, 0.0)] + profile + [(0.0, length)]
    turned(material, profile, at, around, tile, finish)


def pole(material, a, b, radius, tile=1.2):
    """A bush pole: round, not quite straight, a little thicker at its butt."""
    at, length = between(a, b)
    # A ring to each third of a metre: a short log is two or three, a post is nine.
    rings, around = max(2, min(9, int(length / 0.35))), 12
    verts, faces, uvs = [], [], []
    ou = rng.random()
    for ring in range(rings + 1):
        t = ring / rings
        r = radius * (1.08 - 0.14 * t) * (1.0 + rng.uniform(-0.035, 0.035))
        dx, dy = (rng.uniform(-0.012, 0.012), rng.uniform(-0.012, 0.012)) if 0 < ring < rings else (0.0, 0.0)
        for step in range(around):
            angle = step / around * math.tau
            verts.append((dx + r * math.cos(angle), dy + r * math.sin(angle), t * length))
    for ring in range(rings):
        for step in range(around):
            a0, b0 = ring * around + step, ring * around + (step + 1) % around
            faces.append((a0, b0, b0 + around, a0 + around))
            u0, u1 = step / around * radius * math.tau / tile + ou, (step + 1) / around * radius * math.tau / tile + ou
            v0, v1 = ring / rings * length / tile, (ring + 1) / rings * length / tile
            uvs.append([(v0, u0), (v0, u1), (v1, u1), (v1, u0)])
    # Its sawn ends.
    for ring, order in ((0, -1), (rings, 1)):
        loop = [ring * around + step for step in range(around)][::order]
        faces.append(tuple(loop))
        uvs.append([(verts[i][0] / tile + ou, verts[i][1] / tile) for i in loop])
    pieces.add(material, "round", verts, faces, uvs, at)


def sheet(material, waves, length, at, rows=10, length_at=None, tile=1.2):
    """A sheet of corrugated iron: x across its waves, y along it, z out of its face.
    `length_at` gives it a raked end, by how far across."""
    per = 6
    columns = int(round(waves * per))
    verts, faces, uvs = [], [], []
    ou, ov = rng.random(), rng.random()
    bow, phase = rng.uniform(0.0, 0.004), rng.uniform(0.0, math.tau)
    for row in range(rows + 1):
        for column in range(columns + 1):
            x = column / per * PITCH
            y = row / rows * (length_at(x) if length_at else length)
            z = WAVE * math.sin(column / per * math.tau) + bow * math.sin(y * 1.7 + phase)
            verts.append((x, y, z))
    wide = columns + 1
    for row in range(rows):
        for column in range(columns):
            a = row * wide + column
            face = (a, a + 1, a + wide + 1, a + wide)
            faces.append(face)
            uvs.append([(verts[i][0] / tile + ou, verts[i][1] / tile + ov) for i in face])
    pieces.add(material, "sheet", verts, faces, uvs, at)


def screw(at):
    """A roofing screw's head on its washer, standing on its own z. There are hundreds, each
    seen as a dot: a few faces each is all they are given."""
    turned("dark", [(0.011, 0.0), (0.007, 0.006), (0.0, 0.008)], at, around=5, finish="hard")


def strip(material, outline, x0, x1, at, tile=1.0, finish="sheet"):
    """Folded metal: an outline, (y, z), drawn out along x."""
    verts, faces, uvs = [], [], []
    run = [0.0]
    for (y0, z0), (y1, z1) in zip(outline, outline[1:]):
        run.append(run[-1] + math.hypot(y1 - y0, z1 - z0))
    for y, z in outline:
        verts += [(x0, y, z), (x1, y, z)]
    for i in range(len(outline) - 1):
        faces.append((2 * i, 2 * i + 1, 2 * i + 3, 2 * i + 2))
        uvs.append([(run[i] / tile, x0 / tile), (run[i] / tile, x1 / tile),
                    (run[i + 1] / tile, x1 / tile), (run[i + 1] / tile, x0 / tile)])
    pieces.add(material, finish, verts, faces, uvs, at)


def slab(material, outline, thick, at, tile=1.0, finish="cut"):
    """A flat piece cut to an outline, (x, y), and as thick as given, about its own z = 0."""
    count = len(outline)
    verts = [(x, y, -thick / 2) for x, y in outline] + [(x, y, thick / 2) for x, y in outline]
    faces = [tuple(range(count - 1, -1, -1)), tuple(range(count, 2 * count))]
    faces += [(i, (i + 1) % count, (i + 1) % count + count, i + count) for i in range(count)]
    ou = rng.random()
    uvs = [[(verts[i][0] / tile + ou, verts[i][1] / tile) for i in face] for face in faces[:2]]
    uvs += [[(0, 0), (0.1, 0), (0.1, thick / tile), (0, thick / tile)] for _ in range(count)]
    pieces.add(material, finish, verts, faces, uvs, at)


IRONS = ["iron"] * 9 + ["iron_dull"] * 5 + ["iron_warm"] * 3 + ["rust"] * 3


def iron():
    """Which iron a sheet is: no two came off the same truck."""
    return rng.choice(IRONS)


# =============================================================== The frame.
def build_frame():
    for x in (-HALF_W, 0.0, HALF_W):
        for y in (-HALF_D, 0.0, HALF_D):
            pole("pole", (x, y, -0.15), (x, y, beam_top(y) - 0.17), 0.115)
    for x in (-2.5, 2.5):
        pole("pole", (x, HALF_D, -0.15), (x, HALF_D, beam_top(HALF_D) - 0.17), 0.10)
    # The beams: poles along the tops of the posts.
    for y in (-HALF_D, 0.0, HALF_D):
        z = beam_top(y) - 0.09
        pole("pole", (-HALF_W - 0.35, y, z), (HALF_W + 0.35, y, z), 0.095)
    # Knee braces, front and middle: what keeps a pole shed square.
    for y in (-HALF_D, 0.0):
        for x, sides in ((-HALF_W, (1,)), (0.0, (-1, 1)), (HALF_W, (-1,))):
            for side in sides:
                if y == 0.0 and x >= 0.0 and side > 0 or (y == 0.0 and x == HALF_W):
                    continue  # inside the room
                top = beam_top(y) - 0.2
                pole("pole", (x + side * 0.06, y, top - 0.75), (x + side * 0.8, y, top + 0.02), 0.05)
    # Rafters, sawn, on edge, down the slope; and battens across them.
    for index in range(11):
        x = -5.2 + index * 1.04
        y = -0.175
        box("timber", 0.05, 8.1, 0.15, move(x, y, beam_top(y) + 0.076) @ turn("X", -math.degrees(TILT)), grain="y")
    for y in np.arange(-4.1, 3.95, 0.98):
        box("timber", 10.9, 0.075, 0.04, move(0.0, y, beam_top(y) + 0.171) @ turn("X", -math.degrees(TILT)))
    # Wall girts: rails the wall's iron is screwed to.
    for z in (0.75, 1.75):
        box("timber", 10.2, 0.05, 0.075, move(0.0, HALF_D + 0.035, z))
        box("timber", 0.05, 7.1, 0.075, move(-HALF_W - 0.035, 0.0, z), grain="y")
        box("timber", 0.05, 3.95, 0.075, move(HALF_W + 0.035, 1.55, z), grain="y")
        box("timber", 0.05, 3.9, 0.075, move(-0.035, 1.55, z), grain="y")


# =============================================================== The iron.
def build_roof():
    start, length = -4.32, 8.3
    for index in range(15):
        x0 = -5.72 + index * SHEET + rng.uniform(-0.003, 0.003)
        y0 = start + rng.uniform(-0.03, 0.03)
        # Each laps half a wave over the next, and so rides a little above it.
        z0 = roof_at(y0) + WAVE + 0.004 + (index % 2) * 0.004
        at = move(x0, y0, z0) @ turn("X", -math.degrees(TILT))
        sheet(iron(), 10.5, length + rng.uniform(-0.03, 0.03), at, rows=14)
        # Its screws, into each batten, on every third crest.
        for y in np.arange(-4.1, 3.95, 0.98):
            along = (y - y0) / math.cos(TILT)
            for crest in range(1, 10, 3):
                screw(at @ move((crest + 0.25) * PITCH, along, WAVE))
    # Capping along the high edge and down the two sides, and a gutter along the low one.
    high, low = start - 0.02, start + length + 0.02
    cap = [(0.0, -0.11), (0.0, 0.0), (0.14, 0.0)]
    strip("iron_dull", cap, -5.76, 5.76, move(0.0, high, roof_at(high) + 2 * WAVE + 0.012) @ turn("X", -math.degrees(TILT)))
    for x, side in ((-5.74, -1), (5.74, 1)):
        barge = [(0.0, -0.12), (0.0, 0.0), (-side * 0.13, 0.0)]
        at = frame((x, high, roof_at(high) + 2 * WAVE + 0.012), (0, math.cos(TILT), -math.sin(TILT)), (1, 0, 0),
                   (0, math.sin(TILT), math.cos(TILT)))
        strip("iron_dull", barge, 0.0, length + 0.04, at)
    gutter = [(0.065 * math.cos(a), 0.065 * math.sin(a)) for a in np.linspace(math.pi * 1.05, math.tau - 0.05, 12)]
    gz = roof_at(low) - 0.02
    strip("iron_dull", gutter, -5.8, 5.9, move(0.0, low + 0.05, gz))
    for x in np.arange(-5.4, 5.5, 1.2):
        box("dark", 0.02, 0.15, 0.004, move(x, low + 0.03, gz + 0.04) @ turn("X", -20), finish="hard")
    # The downpipe, from the gutter's end across to the tank.
    rod("iron_dull", (5.86, low + 0.05, gz - 0.04), (6.35, 2.4, 2.72), 0.04, ends=False)


def wall(origin, along, out, width, top, bottom=-0.02):
    """A wall of upright sheets from `origin`, running `along`, facing `out`: `top` gives
    its height by how far along it is."""
    done = 0.0
    while done < width - 0.01:
        waves = min(10, max(1, round((width - done) / PITCH)))
        x0 = done
        # Lapped as the roof's are: half a wave over the next, and that much proud of it.
        proud = (round(done / SHEET) % 2) * 0.004
        start = (origin[0] + along[0] * done + out[0] * proud, origin[1] + along[1] * done + out[1] * proud, bottom)
        at = frame(start, along, (0, 0, 1), out)
        sheet(iron(), waves + 0.5, 0.0, at, rows=5, length_at=lambda x: top(x0 + x) - bottom)
        for z in (0.75, 1.75):
            for crest in range(1, waves, 3):
                screw(at @ move((crest + 0.25) * PITCH, z - bottom, WAVE))
        done += waves * PITCH


def build_walls():
    flat = lambda z: (lambda along: z)
    wall((-HALF_W - 0.05, HALF_D + 0.07, 0), (1, 0, 0), (0, 1, 0), 10.1, flat(roof_at(HALF_D) - 0.01))
    wall((-HALF_W - 0.07, -HALF_D - 0.05, 0), (0, 1, 0), (-1, 0, 0), 7.1, lambda a: roof_at(-HALF_D - 0.05 + a) - 0.01)
    wall((HALF_W + 0.07, ROOM_FRONT, 0), (0, 1, 0), (1, 0, 0), 3.95, lambda a: roof_at(ROOM_FRONT + a) - 0.01)
    wall((-0.07, ROOM_FRONT, 0), (0, 1, 0), (-1, 0, 0), 3.95, lambda a: roof_at(ROOM_FRONT + a) - 0.01)


# =============================================================== The floor and the room.
def build_room():
    box("concrete", 10.5, 7.6, 0.14, move(0.0, 0.0, -0.04), tile=2.6)
    # The room's own floor, a step up, of boards.
    for index in range(26):
        box("board", 0.185, 3.85, 0.035, move(0.12 + index * 0.19, 1.55, 0.16), grain="y")
    box("timber", 0.95, 0.34, 0.15, move(1.2, ROOM_FRONT - 0.22, 0.105))
    # Its front: upright slabs, with a door and a window left out of them.
    door, window = (0.74, 1.66, 2.04), (2.7, 3.7, 1.0, 1.9)
    x = 0.13
    while x < 4.92:
        width = rng.uniform(0.17, 0.22)
        centre = x + width / 2
        top = roof_at(ROOM_FRONT) - 0.02 - rng.uniform(0.0, 0.03)
        spans = [(0.03, top)]
        if door[0] - 0.03 < centre < door[1] + 0.03:
            spans = [(door[2] + 0.08, top)]
        elif window[0] - 0.03 < centre < window[1] + 0.03:
            spans = [(0.03, window[2] - 0.06), (window[3] + 0.06, top)]
        for low, high in spans:
            box("timber", width - 0.012, 0.032, high - low,
                move(centre, ROOM_FRONT + rng.uniform(-0.004, 0.004), (low + high) / 2), grain="z")
        x += width
    # The door's frame, and the door: boards on ledges with a brace, standing ajar.
    for x in (door[0], door[1]):
        box("timber", 0.07, 0.09, door[2] + 0.06, move(x, ROOM_FRONT, door[2] / 2 + 0.03), grain="z")
    box("timber", door[1] - door[0] + 0.14, 0.09, 0.07, move((door[0] + door[1]) / 2, ROOM_FRONT, door[2] + 0.065))
    # The door itself is gathered apart, about its hinge: x along it, z up.
    global pieces
    shed_pieces, pieces = pieces, door_pieces
    hung = Matrix.Identity(4)
    for index in range(5):
        box("board", 0.162, 0.028, 1.84, hung @ move(0.085 + index * 0.166, 0.0, 0.92), grain="z")
    for z in (0.22, 0.92, 1.62):
        box("board", 0.8, 0.026, 0.11, hung @ move(0.42, -0.027, z))
    for z in (0.57, 1.27):
        box("board", 0.86, 0.024, 0.09, hung @ move(0.42, -0.027, z) @ turn("Y", 38))
    for z in (0.22, 1.62):
        box("dark", 0.5, 0.006, 0.035, hung @ move(0.26, 0.018, z), finish="hard")
    rod("dark", hung @ Vector((0.76, 0.014, 0.95)), hung @ Vector((0.76, 0.06, 0.95)), 0.016)
    rod("dark", hung @ Vector((0.76, -0.014, 0.95)), hung @ Vector((0.76, -0.07, 0.95)), 0.016)
    pieces = shed_pieces
    # The window: a frame, four panes.
    wx, wz = (window[0] + window[1]) / 2, (window[2] + window[3]) / 2
    for x in (window[0], window[1]):
        box("timber", 0.06, 0.08, 1.0, move(x, ROOM_FRONT, wz), grain="z")
    for z in (window[2], window[3]):
        box("timber", 1.12, 0.08, 0.06, move(wx, ROOM_FRONT, z))
    box("timber", 0.03, 0.04, 0.86, move(wx, ROOM_FRONT, wz), grain="z")
    box("timber", 0.96, 0.04, 0.03, move(wx, ROOM_FRONT, wz))
    box("glass", 0.96, 0.006, 0.86, move(wx, ROOM_FRONT + 0.01, wz), finish="hard")
    # Seen through the door: a bunk against the back wall.
    box("board", 0.95, 2.0, 0.06, move(0.75, 2.4, 0.5), grain="y")
    for x, y in ((0.32, 1.45), (1.18, 1.45), (0.32, 3.35), (1.18, 3.35)):
        box("timber", 0.07, 0.07, 0.5, move(x, y, 0.4), grain="z")
    box("canvas", 0.9, 1.9, 0.12, move(0.75, 2.4, 0.59), finish="soft")
    box("blanket", 0.92, 1.2, 0.05, move(0.75, 2.1, 0.67), finish="soft")
    # A stove's flue, out through the roof, with its collar and its hat.
    fx, fy = 4.1, 2.7
    top = roof_at(fy) + 1.15
    rod("iron_dull", (fx, fy, 0.9), (fx, fy, top), 0.07, around=16, ends=False)
    turned("iron_dull", [(0.075, 0.0), (0.2, -0.07), (0.21, -0.075)], move(fx, fy, roof_at(fy) + 0.16), around=16, finish="sheet")
    turned("iron_dull", [(0.0, 0.16), (0.17, 0.05), (0.175, 0.045)], move(fx, fy, top), around=16, finish="sheet")
    for angle in (0, 120, 240):
        a = math.radians(angle)
        rod("dark", (fx + 0.07 * math.cos(a), fy + 0.07 * math.sin(a), top - 0.02),
            (fx + 0.12 * math.cos(a), fy + 0.12 * math.sin(a), top + 0.08), 0.005, around=5)


# =============================================================== The tank.
def build_tank():
    cx, cy, r, foot, height = 6.35, 2.4, 1.05, 0.38, 2.1
    for y in (-0.7, 0.0, 0.7):
        box("timber", 2.3, 0.22, 0.2, move(cx, cy + y, 0.1))
    for x in np.arange(-0.95, 1.0, 0.27):
        box("timber", 0.24, 2.3, 0.06, move(cx + x, cy, 0.23 + 0.03), grain="y")
    # The corrugations run round it.
    rings = int(height / PITCH * 4)
    profile = [(r + 0.012 * math.sin(i / 4 * math.tau), i / rings * height) for i in range(rings + 1)]
    turned("iron", [(r - 0.01, 0.0)] + profile, move(cx, cy, foot - 0.09), around=56, tile=1.2)
    turned("iron_dull", [(r + 0.03, 0.0), (r + 0.03, 0.03), (0.3, 0.3), (0.3, 0.34), (0.0, 0.36)],
           move(cx, cy, foot - 0.09 + height), around=56, finish="sheet")
    # A tap low on the front, and an overflow near the top.
    rod("brass", (cx - 0.5, cy - r + 0.05, 0.62), (cx - 0.5, cy - r - 0.12, 0.62), 0.02)
    rod("brass", (cx - 0.5, cy - r - 0.1, 0.62), (cx - 0.5, cy - r - 0.1, 0.52), 0.016)
    box("brass", 0.09, 0.014, 0.014, move(cx - 0.5, cy - r - 0.07, 0.66), finish="hard")
    rod("iron_dull", (cx + r - 0.05, cy - 0.4, 2.2), (cx + r + 0.18, cy - 0.4, 2.2), 0.03, ends=False)


# =============================================================== What is kept in it.
def lantern(at):
    """A hurricane lamp, hung by its bail from the hook above `at`."""
    turned("worn_red", [(0.0, -0.3), (0.07, -0.3), (0.075, -0.285), (0.075, -0.25), (0.045, -0.235), (0.035, -0.22)], at, around=14)
    turned("glow", [(0.035, -0.22), (0.058, -0.18), (0.058, -0.13), (0.03, -0.09)], at, around=14)
    turned("worn_red", [(0.03, -0.09), (0.05, -0.075), (0.04, -0.05), (0.02, -0.04), (0.0, -0.04)], at, around=14)
    for side in (-1, 1):
        rod("dark", at @ Vector((side * 0.075, 0, -0.26)), at @ Vector((side * 0.08, 0, -0.07)), 0.004, around=5)
        rod("dark", at @ Vector((side * 0.08, 0, -0.07)), at @ Vector((0, 0, 0.0)), 0.004, around=5)


def chest(at):
    """The chest: boards bound in iron, with a round lid, which stands open: what is put in
    it lies in it, to be seen. Its length is along its own y, and its front faces its -x."""
    long, deep, tall, thick = 1.15, 0.56, 0.42, 0.028
    for x in (-0.2, 0.2):
        box("timber", 0.09, long + 0.06, 0.07, at @ move(x, 0, 0.035), grain="y")
    base = 0.07
    box("board", deep, long, thick, at @ move(0, 0, base + thick / 2), grain="y")
    for row in range(3):
        z = base + thick + (row + 0.5) * tall / 3
        high = tall / 3 - 0.004
        for side in (-1, 1):
            box("board", thick, long, high, at @ move(side * (deep - thick) / 2, 0, z), grain="y")
            box("board", deep - 2 * thick, thick, high, at @ move(0, side * (long - thick) / 2, z))
    # The lid: slats on an arc, between two ends cut to it. It is hinged along the back of
    # the chest, and stands up from there.
    top = base + thick + tall + 0.006
    shut = at
    at = shut @ move(deep / 2, 0, top) @ turn("Y", 90) @ move(-deep / 2, 0, -top)
    radius = 0.44
    half = math.asin(deep / 2 / radius)
    drop = radius * math.cos(half)
    slats = 9
    for index in range(slats):
        a = -half + (index + 0.5) / slats * 2 * half
        width = 2 * half * radius / slats
        lie = at @ move(radius * math.sin(a), 0, top - drop + radius * math.cos(a)) @ turn("Y", math.degrees(a))
        box("board", width - 0.003, long, 0.022, lie, grain="y")
    arc = [(radius * math.sin(a), radius * math.cos(a) - drop) for a in np.linspace(-half, half, 11)]
    outline = [(-deep / 2 + 0.004, 0.0)] + [(x, z - 0.012) for x, z in arc] + [(deep / 2 - 0.004, 0.0)]
    for side in (-1, 1):
        slab("board", outline, thick, at @ frame((0, side * (long / 2 - thick / 2 - 0.004), top), (1, 0, 0), (0, 0, 1), (0, -1, 0)))
    # Iron: three bands over the lid and down the front and back, and its corners.
    for y in (-0.42, 0.0, 0.42):
        for index in range(slats):
            a = -half + (index + 0.5) / slats * 2 * half
            lie = at @ move((radius + 0.013) * math.sin(a), y, top - drop + (radius + 0.013) * math.cos(a)) @ turn("Y", math.degrees(a))
            box("dark", 2 * half * radius / slats + 0.004, 0.05, 0.004, lie, finish="hard")
    # The rest is the chest's own, which stays where it is.
    at = shut
    for y in (-0.42, 0.0, 0.42):
        for side in (-1, 1):
            box("dark", 0.004, 0.05, tall + 0.03, at @ move(side * (deep / 2 + 0.002), y, base + thick + tall / 2), finish="hard")
    for sx in (-1, 1):
        for sy in (-1, 1):
            for z in (base + 0.05, base + thick + tall - 0.04):
                box("dark", 0.07, 0.07, 0.07, at @ move(sx * (deep / 2 - 0.03), sy * (long / 2 - 0.03), z) @ Matrix.Scale(1.0, 4), finish="hard")
    # A hasp on the front, hanging open, and a rope handle at each end.
    front = -(deep / 2 + 0.006)
    box("dark", 0.006, 0.06, 0.13, at @ move(front, 0, base + thick + tall - 0.085), finish="hard")
    for side in (-1, 1):
        y = side * (long / 2 + 0.012)
        ring = [(0.085 + 0.016 * math.cos(a), 0.016 * math.sin(a)) for a in np.linspace(0, math.tau, 9)]
        turned("rope", ring, at @ frame((0, y, base + thick + tall * 0.62), (1, 0, 0), (0, 0, 1), (0, -side, 0)) @ turn("X", 20), around=18)


def build_things():
    """What is in it: a lantern in the bay and one on the verandah, and the chest by the wall
    of the room, facing the bay. It is kept bare: a place to park and to sleep."""
    lantern(move(-2.5, 0.0, beam_top(0.0) - 0.2))
    lantern(move(2.5, -HALF_D, beam_top(-HALF_D) - 0.2))
    chest(move(-0.62, 1.55, 0.03))


# =============================================================== Into Blender.
PAINTS = {
    # name: (texture, tint, rough, metal, seen from both sides)
    # Iron is told to the game as a third of the metal it is: the game shows a pure metal
    # by what it mirrors, which under a roof is dark.
    "iron": ("iron", (1, 1, 1), 1, 0.35, True),
    "iron_dull": ("iron", (0.78, 0.78, 0.76), 1, 0.35, True),
    "iron_warm": ("iron", (0.95, 0.88, 0.8), 1, 0.35, True),
    "rust": ("rust", (1, 1, 1), 1, 0.35, True),
    "timber": ("timber", (1, 1, 1), 1, 0, False),
    "pole": ("pole", (1, 1, 1), 1, 0, False),
    "board": ("board", (1, 1, 1), 1, 0, False),
    "concrete": ("concrete", (1, 1, 1), 1, 0, False),
    "dark": ("worn", (0.09, 0.085, 0.08), 0.9, 0.4, True),
    "brass": ("worn", (0.72, 0.52, 0.2), 0.7, 0.6, False),
    "worn_red": ("worn", (0.55, 0.07, 0.05), 1, 0.3, False),
    "worn_green": ("worn", (0.16, 0.27, 0.12), 1, 0.3, False),
    "worn_blue": ("worn", (0.1, 0.2, 0.36), 1, 0.3, False),
    "rubber": ("worn", (0.045, 0.045, 0.045), 1.4, 0, False),
    "canvas": ("worn", (0.36, 0.34, 0.22), 1.6, 0, False),
    "blanket": ("worn", (0.5, 0.16, 0.12), 1.7, 0, False),
    "rope": ("worn", (0.6, 0.5, 0.33), 1.7, 0, False),
    "enamel": ("worn", (0.85, 0.87, 0.9), 0.6, 0, True),
    "glass": (None, (0.03, 0.04, 0.045), 0.08, 0, True),
    "glow": (None, (1.0, 0.75, 0.4), 0.4, 0, True),
}


def make_images(folder):
    images = {}
    for name, maps in textures.everything().items():
        for kind, pixels in zip(("colour", "normal", "rough_metal"), maps):
            image = bpy.data.images.new(f"shed_{name}_{kind}", textures.SIZE, textures.SIZE, alpha=False)
            if kind != "colour":
                image.colorspace_settings.name = "Non-Color"
            rgba = np.ones((textures.SIZE, textures.SIZE, 4), dtype=np.float32)
            rgba[..., :3] = np.clip(pixels, 0.0, 1.0)
            image.pixels.foreach_set(rgba.ravel())
            image.filepath_raw = os.path.join(folder, f"{name}_{kind}.png")
            image.file_format = "PNG"
            image.save()
            images[(name, kind)] = image
    return images


def make_material(name, images):
    texture, tint, rough, metal, both = PAINTS[name]
    material = bpy.data.materials.new("shed_" + name)
    material.use_nodes = True
    material.use_backface_culling = not both
    nodes, links = material.node_tree.nodes, material.node_tree.links
    shader = nodes["Principled BSDF"]
    if texture is None:
        shader.inputs["Base Color"].default_value = (*tint, 1.0)
        shader.inputs["Roughness"].default_value = rough
        shader.inputs["Metallic"].default_value = metal
        if name == "glow":
            shader.inputs["Emission Color"].default_value = (1.0, 0.55, 0.16, 1.0)
            shader.inputs["Emission Strength"].default_value = 1.0
        return material
    colour = nodes.new("ShaderNodeTexImage")
    colour.image = images[(texture, "colour")]
    mixed = nodes.new("ShaderNodeMix")
    mixed.data_type, mixed.blend_type = "RGBA", "MULTIPLY"
    mixed.inputs["Factor"].default_value = 1.0
    links.new(colour.outputs["Color"], mixed.inputs["A"])
    mixed.inputs["B"].default_value = (*tint, 1.0)
    links.new(mixed.outputs["Result"], shader.inputs["Base Color"])
    lie = nodes.new("ShaderNodeTexImage")
    lie.image = images[(texture, "normal")]
    bump = nodes.new("ShaderNodeNormalMap")
    links.new(lie.outputs["Color"], bump.inputs["Color"])
    links.new(bump.outputs["Normal"], shader.inputs["Normal"])
    packed = nodes.new("ShaderNodeTexImage")
    packed.image = images[(texture, "rough_metal")]
    apart = nodes.new("ShaderNodeSeparateColor")
    links.new(packed.outputs["Color"], apart.inputs["Color"])
    for channel, socket, factor in (("Green", "Roughness", rough), ("Blue", "Metallic", metal)):
        scaled = nodes.new("ShaderNodeMath")
        scaled.operation = "MULTIPLY"
        scaled.use_clamp = True
        links.new(apart.outputs[channel], scaled.inputs[0])
        scaled.inputs[1].default_value = factor
        links.new(scaled.outputs["Value"], shader.inputs[socket])
    return material


def make_objects(gathered, called, images, materials):
    for (name, finish), (verts, faces, uvs) in sorted(gathered.groups.items()):
        mesh = bpy.data.meshes.new(f"{called}_{name}_{finish}")
        mesh.from_pydata([tuple(v) for v in verts], [], faces)
        layer = mesh.uv_layers.new(name="UVMap")
        flat = [value for face in uvs for corner in face for value in corner]
        layer.data.foreach_set("uv", flat)
        mesh.polygons.foreach_set("use_smooth", [finish != "hard"] * len(mesh.polygons))
        mesh.update()
        if name not in materials:
            materials[name] = make_material(name, images)
        mesh.materials.append(materials[name])
        made = bpy.data.objects.new(mesh.name, mesh)
        bpy.context.scene.collection.objects.link(made)
        if finish in ("cut", "soft"):
            ease = made.modifiers.new("ease", "BEVEL")
            ease.width, ease.segments = (0.004, 1) if finish == "cut" else (0.018, 2)
            ease.limit_method, ease.angle_limit = "ANGLE", math.radians(40)
            ease.harden_normals = True
        if finish in ("cut", "soft", "round", "sheet"):
            even = made.modifiers.new("even", "WEIGHTED_NORMAL")
            even.keep_sharp = False
        yield made


def pictures(folder):
    """Two renders of it, to look at: from the road, and into the bay."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 96
    scene.cycles.use_denoising = True
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "OPTIX"
        prefs.get_devices()
        for device in prefs.devices:
            device.use = True
        scene.cycles.device = "GPU"
    except Exception as error:  # A machine with no such card renders on its processor.
        print("rendering on the processor:", error)
    scene.render.resolution_x, scene.render.resolution_y = 1600, 900
    scene.view_settings.view_transform = "AgX"
    world = bpy.data.worlds.new("sky")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.55, 0.7, 1.0, 1.0)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.9
    scene.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy, sun.data.angle = 4.5, math.radians(1.5)
    sun.rotation_euler = (math.radians(52), 0.0, math.radians(-32))
    scene.collection.objects.link(sun)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-60, -60, -0.02), (60, -60, -0.02), (60, 60, -0.02), (-60, 60, -0.02)], [], [(0, 1, 2, 3)])
    dirt = bpy.data.materials.new("dirt")
    dirt.use_nodes = True
    dirt.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.42, 0.25, 0.15, 1.0)
    dirt.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    ground.materials.append(dirt)
    scene.collection.objects.link(bpy.data.objects.new("ground", ground))
    camera = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    scene.collection.objects.link(camera)
    scene.camera = camera
    # A lamp inside, for the pictures: in the game the sky's light gets in by itself.
    lamp = bpy.data.objects.new("lamp", bpy.data.lights.new("lamp", "POINT"))
    lamp.data.energy, lamp.data.shadow_soft_size = 260, 0.6
    lamp.location = (-2.6, -0.4, 2.3)
    scene.collection.objects.link(lamp)
    for name, at, to, lens in (("road", (-9.5, -13.5, 2.3), (0.6, 0.0, 1.5), 30), ("bay", (-3.4, -7.4, 1.7), (-1.6, 3.0, 1.2), 24),
                               ("chest", (-3.0, -0.6, 1.5), (-0.6, 1.7, 0.4), 35), ("tank", (13.5, -6.5, 2.6), (4.5, 1.0, 1.4), 35)):
        camera.location = at
        camera.data.lens = lens
        look = Vector(to) - Vector(at)
        camera.rotation_euler = look.to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(folder, f"shed_{name}.png")
        bpy.ops.render.render(write_still=True)
    for name in ("sun", "ground", "camera", "lamp"):
        bpy.data.objects.remove(bpy.data.objects[name], do_unlink=True)


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build_frame()
    build_roof()
    build_walls()
    build_room()
    build_tank()
    build_things()
    folder = os.path.join(KEEP, "textures") if KEEP else os.path.join(os.path.dirname(os.path.abspath(OUT)), "shed_textures")
    os.makedirs(folder, exist_ok=True)
    images = make_images(folder)
    materials = {}
    made = list(make_objects(pieces, "shed", images, materials))
    door = list(make_objects(door_pieces, "door", images, materials))
    triangles = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in made + door)
    print(f"shed: {len(made) + len(door)} meshes, {triangles} triangles before their edges are eased")
    if KEEP:
        # To look at, the door hangs in its doorway, ajar.
        for piece in door:
            piece.matrix_world = move(*HINGE) @ turn("Z", 24)
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(KEEP, "shed.blend"))
        pictures(KEEP)
        for piece in door:
            piece.matrix_world = Matrix.Identity(4)
    # Two models: the shed, and its door about its hinge.
    for path, chosen in ((OUT, made), (OUT[:-4] + "_door.glb", door)):
        for piece in bpy.context.scene.objects:
            piece.select_set(piece in chosen)
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", export_apply=True, export_yup=True, use_selection=True)
        print(f"shed: wrote {path}, {os.path.getsize(path) / 1e6:.1f} MB")


# The town's script (tools/blender/town/build.py) is built of the same pieces: it reads this
# file for them, and is not after a shed.
if __name__ == "__main__":
    main()
