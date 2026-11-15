"""Builds the town: four buildings along one side of a street, as a model.

  blender -b --python tools/blender/town/build.py -- OUT.glb [KEEP_FOLDER]

A gold town of the Victorian high country, what is left of one: a hotel, a general store, the
gold buyer's office and a hall, side by side behind their verandahs, of weatherboard and
corrugated iron. It is kept bare, as the shed is: the buildings, their signs, and nothing
standing about in front of them.

It is built of the shed's pieces (`tools/blender/shed/build.py`: boxes, poles, turned shapes
and corrugated sheets with their screws) and its textures, with paint over the boards.
KEEP_FOLDER, if given, is where the Blender file is kept to look at (`town.blend`), and
pictures of it rendered (`town_*.png`).

The model faces Blender's -Y, which is +Z in the file: the street. Its origin is on the
ground in the middle of the yard in front of the buildings, whose fronts stand `FRONT` metres
behind it. The game is told where the buildings are, to stop a truck, and where the gold
buyer's is, by `crates/solstice-bake/src/outback/town.rs`, which has the same measurements.
"""
import math
import os
import sys

import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "shed"))
import build as kit  # noqa: E402  (the shed's script: its pieces, not its shed)

args = sys.argv[sys.argv.index("--") + 1:]
OUT = args[0]
KEEP = args[1] if len(args) > 1 else None
rng = kit.rng
move, turn, frame, box, rod, strip, slab = kit.move, kit.turn, kit.frame, kit.box, kit.rod, kit.strip, kit.slab
PITCH, WAVE, SHEET = kit.PITCH, kit.WAVE, kit.SHEET

# ---- The measurements, in metres. (`town.rs` has the same.)
FRONT = 6.0            # the buildings' front walls, behind the model's origin
DECK = 2.6             # how deep a verandah is
FLOOR = 0.32           # how high the floors are above the ground
UP = Vector((0, 0, 1))
# Each building: its name, from and to across the street's front, and how deep it is.
HOTEL = ("hotel", -27.0, -13.0, 9.0)
STORE = ("store", -10.0, 0.0, 9.0)
BUYER = ("buyer", 3.0, 11.0, 7.0)
HALL = ("hall", 14.0, 26.0, 11.0)

# Paint, over the shed's boards; and the letters of the signs, which are flat colour.
kit.PAINTS.update({
    # Paint is on the shed's `worn` surface, which is pale and scuffed: the boards under it
    # are shaped, each lapped over the next, and need no grain of their own.
    "cream": ("worn", (0.90, 0.84, 0.66), 1, 0, False),
    "sage": ("worn", (0.52, 0.63, 0.50), 1, 0, False),
    "oxide": ("worn", (0.55, 0.19, 0.13), 1, 0, False),
    "stone": ("worn", (0.72, 0.70, 0.64), 1, 0, False),
    "white": ("worn", (0.93, 0.92, 0.88), 1, 0, False),
    "door_green": ("worn", (0.11, 0.22, 0.14), 0.8, 0, False),
    "door_red": ("worn", (0.34, 0.08, 0.06), 0.8, 0, False),
    "sign_board": ("worn", (0.95, 0.91, 0.78), 1, 0, False),
    "sign_dark": ("worn", (0.09, 0.16, 0.12), 0.9, 0, False),
    "brick": ("concrete", (0.62, 0.30, 0.22), 1, 0, False),
    "letter": (None, (0.07, 0.06, 0.05), 0.55, 0, True),
    "letter_gold": (None, (0.85, 0.62, 0.14), 0.4, 0.3, True),
})
# What is lettered, once the meshes are made: the words, where, running which way, facing
# which way, how tall, and in what.
signs = []


def vec(*xyz):
    return Vector(xyz)


def boards(origin, along, out, width, height, paint, foot=FLOOR):
    """A weatherboard wall: boards lapped each over the one below, on a wall behind them.
    From `origin` on the ground at its end, running `along`, facing `out`."""
    origin, along, out = Vector(origin), Vector(along), Vector(out)
    show = 0.165
    rows = max(1, round((height - foot) / show))
    show = (height - foot) / rows
    middle = origin + along * (width / 2)
    box(paint, width, 0.09, height - foot, frame(middle - out * 0.045 + UP * ((height + foot) / 2), along, out, UP))
    for row in range(rows):
        at = frame(middle + out * 0.014 + UP * (foot + (row + 0.5) * show), along, out, UP) @ turn("X", 6.5)
        box(paint, width + rng.uniform(-0.004, 0.004), 0.021, show + 0.035, at, tile=1.7)


def trim(origin, along, out, width, height, paint="white", foot=FLOOR):
    """The boards at a wall's two ends, and the plate under its eave."""
    origin, along, out = Vector(origin), Vector(along), Vector(out)
    for x in (0.045, width - 0.045):
        box(paint, 0.09, 0.03, height - foot, frame(origin + along * x + out * 0.04 + UP * ((height + foot) / 2), along, out, UP), grain="z")
    box(paint, width, 0.03, 0.14, frame(origin + along * (width / 2) + out * 0.04 + UP * (height - 0.07), along, out, UP))


def door(at, along, out, paint, wide=0.95, tall=2.08, glazed=False):
    """A door in its frame, standing on the floor at `at`: the middle of its foot."""
    at, along, out = Vector(at), Vector(along), Vector(out)
    place = lambda x, z, y=0.0: frame(at + along * x + UP * z + out * (0.05 + y), along, out, UP)
    for x in (-wide / 2 - 0.05, wide / 2 + 0.05):
        box("white", 0.1, 0.05, tall + 0.1, place(x, tall / 2 + 0.05), grain="z")
    box("white", wide + 0.2, 0.05, 0.1, place(0, tall + 0.05))
    box(paint, wide, 0.04, tall, place(0, tall / 2), grain="z")
    # Its rails and stiles, proud of its panels; or glass above its lock rail.
    for x in (-wide / 2 + 0.06, wide / 2 - 0.06):
        box(paint, 0.12, 0.02, tall, place(x, tall / 2, 0.03), grain="z")
    for z in (0.1, 0.95, tall - 0.08):
        box(paint, wide, 0.02, 0.16, place(0, z, 0.03))
    if glazed:
        box("glass", wide - 0.26, 0.006, tall - 1.2, place(0, (tall + 0.95) / 2, 0.035), finish="hard")
    rod("brass", at + along * (wide / 2 - 0.1) + UP * 1.0 + out * 0.09, at + along * (wide / 2 - 0.1) + UP * 1.0 + out * 0.15, 0.022)


def window(at, along, out, wide=1.1, tall=1.5, sill=1.0, across=2, down=2):
    """A sash window, its panes in their bars: `at` is on the floor under its middle."""
    at, along, out = Vector(at), Vector(along), Vector(out)
    place = lambda x, z, y=0.0: frame(at + along * x + UP * (sill + z) + out * (0.05 + y), along, out, UP)
    for x in (-wide / 2 - 0.045, wide / 2 + 0.045):
        box("white", 0.09, 0.05, tall + 0.18, place(x, tall / 2), grain="z")
    box("white", wide + 0.18, 0.05, 0.09, place(0, tall + 0.045))
    box("white", wide + 0.3, 0.1, 0.06, place(0, -0.03, 0.025))
    box("glass", wide, 0.006, tall, place(0, tall / 2, 0.0), finish="hard")
    for i in range(1, across):
        box("white", 0.035, 0.03, tall, place(-wide / 2 + i * wide / across, tall / 2, 0.01), grain="z")
    for i in range(1, down):
        box("white", wide, 0.03, 0.05 if i == down // 2 else 0.035, place(0, i * tall / down, 0.01))


def roof(origin, across, down, width, length):
    """Corrugated iron down a slope: from `origin` at the top corner, sheets side by side
    `across`, each running `down`."""
    origin, across, down = Vector(origin), Vector(across).normalized(), Vector(down).normalized()
    normal = across.cross(down)
    if normal.z < 0:
        origin, across, normal = origin + across * width, -across, -normal
    done, index = 0.0, 0
    while done < width - 0.02:
        waves = min(10.5, (width - done) / PITCH)
        at = frame(origin + across * done + normal * (WAVE + 0.004 + (index % 2) * 0.004), across, down, normal)
        long = length + rng.uniform(-0.02, 0.02)
        kit.sheet(kit.iron(), waves, long, at, rows=max(4, int(long / 0.55)))
        # A screw to every third crest, in rows a metre apart.
        for along in np.arange(0.15, long, 0.95):
            for crest in range(1, int(waves), 3):
                kit.screw(at @ move((crest + 0.25) * PITCH, along, WAVE))
        done += SHEET
        index += 1


def capping(a, b, wide=0.16):
    """Folded iron along a ridge, from one end to the other."""
    a, b = Vector(a), Vector(b)
    along = (b - a).normalized()
    side = along.cross(UP).normalized()
    strip("iron_dull", [(-wide, -0.05), (0.0, 0.03), (wide, -0.05)], 0.0, (b - a).length, frame(a, along, side, UP))


def verandah(x0, x1, paint, posts, height=2.75, lean=0.42):
    """A verandah along a building's front: a deck, its posts and their beam, and a skillion
    of iron down from the wall."""
    wide, front = x1 - x0, FRONT - DECK
    middle = (x0 + x1) / 2
    for y in (front + 0.1, FRONT - DECK / 2, FRONT - 0.1):
        box("timber", wide, 0.1, 0.2, move(middle, y, FLOOR - 0.15))
    for index in range(int(DECK / 0.14)):
        y = front + 0.07 + index * 0.14
        box("board", wide + 0.06, 0.132, 0.035, move(middle, y, FLOOR - 0.0175), tile=1.5)
    for x in np.linspace(x0 + 0.5, x1 - 0.5, 4):
        box("concrete", 0.3, 0.3, FLOOR - 0.25, move(x, front + 0.1, (FLOOR - 0.25) / 2))
    # A step up to it, the building's length.
    box("timber", wide - 0.4, 0.3, 0.05, move(middle, front - 0.17, FLOOR / 2 + 0.01))
    for x in (x0 + 0.4, middle, x1 - 0.4):
        box("timber", 0.06, 0.3, FLOOR / 2, move(x, front - 0.17, FLOOR / 4))
    xs = np.linspace(x0 + 0.12, x1 - 0.12, posts)
    for x in xs:
        box(paint, 0.115, 0.115, height - FLOOR, move(x, front + 0.1, (height + FLOOR) / 2), grain="z")
        box(paint, 0.17, 0.17, 0.07, move(x, front + 0.1, FLOOR + 0.035))
        box(paint, 0.17, 0.17, 0.05, move(x, front + 0.1, height - 0.12))
    box(paint, wide, 0.07, 0.19, move(middle, front + 0.1, height - 0.0))
    # Rafters from a plate on the wall down to the beam, and the iron on battens over them.
    top = height + lean
    slope = Vector((0, front - 0.25 - FRONT, height - top + 0.02)).normalized()
    box(paint, wide, 0.05, 0.14, move(middle, FRONT - 0.03, top - 0.02))
    for x in np.arange(x0 + 0.3, x1 - 0.2, 0.9):
        a, b = Vector((x, FRONT - 0.04, top + 0.03)), Vector((x, front - 0.2, height + 0.115))
        at, long = kit.between(a, b)
        box("timber", 0.09, 0.045, long, at @ move(0, 0, long / 2), grain="z")
    roof((x0 - 0.15, FRONT - 0.02, top + 0.11), (1, 0, 0), slope, wide + 0.3, DECK + 0.42)
    low = Vector((0, front - 0.3, height + 0.03))
    gutter = [(0.06 * math.cos(a), 0.06 * math.sin(a)) for a in np.linspace(math.pi * 1.05, math.tau - 0.05, 10)]
    strip("iron_dull", gutter, x0 - 0.2, x1 + 0.2, move(0.0, low.y - 0.04, low.z))
    return top


def gable_end(a, b, eave, rise, out, paint):
    """The triangle of boards over a wall, from one of its ends to the other, under a roof."""
    a, b, out = Vector(a), Vector(b), Vector(out)
    along = (b - a).normalized()
    wide = (b - a).length
    slab(paint, [(0.0, 0.0), (wide, 0.0), (wide / 2, rise)], 0.03, frame(a + UP * eave + out * 0.02, along, UP, along.cross(UP) * -1), tile=1.7)


def side_iron(origin, along, out, width, top):
    """A wall of upright corrugated iron, standing on the building's plinth."""
    kit.wall(origin, along, out, width, top, bottom=FLOOR - 0.05)


def plinth(x0, x1, depth):
    box("concrete", x1 - x0 + 0.1, depth + 0.1, FLOOR, move((x0 + x1) / 2, FRONT + depth / 2, FLOOR / 2 - 0.01), tile=2.4)


def letters(words, middle, along, out, tall, paint="letter"):
    signs.append((words, Vector(middle), Vector(along), Vector(out), tall, paint))


def sign(words, middle, wide, tall, paint="letter"):
    """A painted board facing the street, with what it says on it: dark letters on a pale
    board, or gold ones on a dark."""
    board = "sign_dark" if paint == "letter_gold" else "sign_board"
    box(board, wide, 0.03, tall, frame(Vector(middle), (1, 0, 0), (0, -1, 0), UP))
    for z in (-tall / 2, tall / 2):
        box("white", wide + 0.06, 0.045, 0.05, frame(Vector(middle) + UP * z, (1, 0, 0), (0, -1, 0), UP))
    letters(words, Vector(middle) + Vector((0, -0.022, 0)), (1, 0, 0), (0, -1, 0), tall * 0.56, paint)


# =============================================================== The hotel.
def build_hotel():
    """Weatherboard all round, under a gable that runs along the street: two doors, four
    windows, a chimney at one end, and its name on the verandah's edge."""
    _, x0, x1, depth = HOTEL
    wall_h, pitch = 3.5, math.radians(27)
    back, ridge = FRONT + depth, FRONT + depth / 2
    rise = depth / 2 * math.tan(pitch)
    plinth(x0, x1, depth)
    boards((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), x1 - x0, wall_h, "cream")
    boards((x1, back, 0), (-1, 0, 0), (0, 1, 0), x1 - x0, wall_h, "cream")
    boards((x0, back, 0), (0, -1, 0), (-1, 0, 0), depth, wall_h, "cream")
    boards((x1, FRONT, 0), (0, 1, 0), (1, 0, 0), depth, wall_h, "cream")
    for origin, along, out, wide in (((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), x1 - x0), ((x1, back, 0), (-1, 0, 0), (0, 1, 0), x1 - x0),
                                     ((x0, back, 0), (0, -1, 0), (-1, 0, 0), depth), ((x1, FRONT, 0), (0, 1, 0), (1, 0, 0), depth)):
        trim(origin, along, out, wide, wall_h)
    gable_end((x0, back, 0), (x0, FRONT, 0), wall_h, rise, (-1, 0, 0), "cream")
    gable_end((x1, FRONT, 0), (x1, back, 0), wall_h, rise, (1, 0, 0), "cream")
    for x in (x0 + 3.6, x1 - 3.6):
        door((x, FRONT, FLOOR), (1, 0, 0), (0, -1, 0), "door_green", glazed=True)
    for x in (x0 + 1.5, x0 + 5.9, x1 - 5.9, x1 - 1.5):
        window((x, FRONT, FLOOR), (1, 0, 0), (0, -1, 0))
    for y in (FRONT + 2.5, FRONT + 6.5):
        window((x0, y, FLOOR), (0, -1, 0), (-1, 0, 0), wide=0.9)
        window((x1, y, FLOOR), (0, 1, 0), (1, 0, 0), wide=0.9)
    top = verandah(x0, x1, "white", 7)
    # The roof: two slopes off the ridge, and the capping along it.
    over, reach = 0.35, (depth / 2 + 0.4) / math.cos(pitch)
    for side in (-1, 1):
        down = Vector((0, side * math.cos(pitch), -math.sin(pitch)))
        roof((x0 - over, ridge, wall_h + rise + 0.06), (1, 0, 0), down, x1 - x0 + 2 * over, reach)
    capping((x0 - over, ridge, wall_h + rise + 0.1), (x1 + over, ridge, wall_h + rise + 0.1))
    for x in (x0 - over + 0.02, x1 + over - 0.02):
        for side in (-1, 1):
            a = Vector((x, ridge, wall_h + rise + 0.02))
            b = a + Vector((0, side * math.cos(pitch), -math.sin(pitch))) * reach
            at, long = kit.between(a, b)
            box("white", 0.16, 0.025, long, at @ move(0, 0, long / 2), grain="z")
    # The chimney, of brick, up the outside of one end.
    cx, cy = x0 - 0.42, FRONT + depth * 0.55
    box("brick", 0.8, 1.3, wall_h + 1.2, move(cx, cy, (wall_h + 1.2) / 2), tile=1.1)
    box("brick", 0.6, 0.8, rise + 0.4, move(cx + 0.08, cy, wall_h + 1.2 + (rise + 0.4) / 2), tile=1.1)
    box("brick", 0.72, 0.92, 0.12, move(cx + 0.08, cy, wall_h + 1.2 + rise + 0.46), tile=1.1)
    sign("HOTEL", ((x0 + x1) / 2, FRONT - DECK - 0.36, top - 0.1), 4.6, 0.62)
    # The publican deals in trucks: a board by the first door says so.
    sign("USED TRUCKS  BOUGHT  &  SOLD", (x0 + 3.6, FRONT - 0.05, FLOOR + 2.42), 3.0, 0.3)


# =============================================================== The store and the buyer's.
def false_front(building, paint, words, tall, doors, gold=False):
    """A shop: a weatherboard front carried up past its roof, with its name across the top;
    iron down its sides and across its back, under a gable that runs from front to back."""
    _, x0, x1, depth = building
    wall_h, pitch = 3.2, math.radians(22)
    wide, back, middle = x1 - x0, FRONT + depth, (x0 + x1) / 2
    rise = wide / 2 * math.tan(pitch)
    plinth(x0, x1, depth)
    boards((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), wide, tall, paint)
    trim((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), wide, tall)
    # Its cornice: a board on edge along the top of the front, and a cap on it.
    box("white", wide + 0.3, 0.12, 0.2, move(middle, FRONT - 0.07, tall - 0.02))
    box("white", wide + 0.44, 0.24, 0.05, move(middle, FRONT - 0.09, tall + 0.1))
    # The back of the front, which is seen from behind: boards as well.
    box(paint, wide, 0.03, tall - wall_h, move(middle, FRONT + 0.06, (tall + wall_h) / 2))
    flat = lambda along: wall_h
    side_iron((x1 + 0.02, FRONT + 0.05, 0), (0, 1, 0), (1, 0, 0), depth - 0.05, flat)
    side_iron((x0 - 0.02, back, 0), (0, -1, 0), (-1, 0, 0), depth - 0.05, flat)
    side_iron((x1, back + 0.02, 0), (-1, 0, 0), (0, 1, 0), wide, lambda along: wall_h + rise * (1 - abs(along - wide / 2) / (wide / 2)))
    for at, along, out in (((x0, FRONT, 0), (0, 1, 0), (-1, 0, 0)), ((x1, FRONT, 0), (0, 1, 0), (1, 0, 0))):
        for y in (0.06, depth - 0.06):
            box("white", 0.11, 0.11, wall_h - FLOOR, move(at[0], FRONT + y, (wall_h + FLOOR) / 2), grain="z")
    for x in doors:
        door((middle + x, FRONT, FLOOR), (1, 0, 0), (0, -1, 0), "door_red" if gold else "door_green", glazed=not gold)
    # Shop windows, wide and low, either side of the door.
    span = wide / 2 - 0.9
    for side in (-1, 1):
        window((middle + side * (0.9 + span / 2 - 0.1), FRONT, FLOOR), (1, 0, 0), (0, -1, 0), wide=min(2.4, span - 0.7), tall=1.55, sill=0.7, across=3, down=2)
    top = verandah(x0, x1, "white", 5 if wide > 9 else 4)
    over, reach = 0.25, (wide / 2 + 0.3) / math.cos(pitch)
    for side in (-1, 1):
        down = Vector((side * math.cos(pitch), 0, -math.sin(pitch)))
        roof((middle, FRONT + 0.1, wall_h + rise + 0.05), (0, 1, 0), down, depth + over, reach)
    capping((middle, FRONT + 0.1, wall_h + rise + 0.09), (middle, back + over, wall_h + rise + 0.09))
    sign(words, (middle, FRONT - 0.07, (tall + top) / 2 + 0.28), wide - 1.4, 0.72, "letter_gold" if gold else "letter")
    return top


def build_store():
    top = false_front(STORE, "sage", "MOTOR SPARES", 5.5, [0.0])
    _, x0, x1, _ = STORE
    sign("PARTS   PAINT   OIL", ((x0 + x1) / 2, FRONT - DECK - 0.36, top - 0.12), 4.4, 0.34)


def build_buyer():
    top = false_front(BUYER, "stone", "GOLD BOUGHT", 5.3, [0.0], gold=True)
    _, x0, x1, _ = BUYER
    # What he pays is by weight: a small board under the verandah says so.
    sign("ASSAY  &  CASH BY WEIGHT", ((x0 + x1) / 2, FRONT - DECK - 0.36, top - 0.12), 4.4, 0.34, "letter_gold")


# =============================================================== The hall.
def build_hall():
    """A hall with its gable to the street: weatherboard in front, iron down its sides, a
    porch over its double doors, and a tall window either side."""
    _, x0, x1, depth = HALL
    wall_h, pitch = 4.1, math.radians(31)
    wide, back, middle = x1 - x0, FRONT + depth, (x0 + x1) / 2
    rise = wide / 2 * math.tan(pitch)
    plinth(x0, x1, depth)
    boards((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), wide, wall_h, "oxide")
    trim((x0, FRONT, 0), (1, 0, 0), (0, -1, 0), wide, wall_h)
    gable_end((x0, FRONT, 0), (x1, FRONT, 0), wall_h, rise, (0, -1, 0), "oxide")
    flat = lambda along: wall_h
    side_iron((x1 + 0.02, FRONT + 0.05, 0), (0, 1, 0), (1, 0, 0), depth - 0.05, flat)
    side_iron((x0 - 0.02, back, 0), (0, -1, 0), (-1, 0, 0), depth - 0.05, flat)
    side_iron((x1, back + 0.02, 0), (-1, 0, 0), (0, 1, 0), wide, lambda along: wall_h + rise * (1 - abs(along - wide / 2) / (wide / 2)))
    for x in (-0.5, 0.5):
        door((middle + x, FRONT, FLOOR), (1, 0, 0), (0, -1, 0), "door_green", wide=0.98, tall=2.3)
    for x in (-3.6, 3.6):
        window((middle + x, FRONT, FLOOR), (1, 0, 0), (0, -1, 0), wide=1.0, tall=2.3, sill=0.9, across=2, down=4)
    # A round vent high in the gable, and the hall's name under it.
    kit.turned("white", [(0.0, 0.0), (0.42, 0.0), (0.42, 0.05), (0.3, 0.05), (0.3, 0.02), (0.0, 0.02)],
               frame((middle, FRONT - 0.04, wall_h + rise * 0.52), (1, 0, 0), UP, (0, -1, 0)), around=20)
    sign("BANK", (middle, FRONT - 0.07, wall_h + 0.2), 3.0, 0.6)
    # The porch: a little gable of its own on two posts, over a landing and its steps.
    px, py, ph = 1.7, FRONT - 1.9, 2.75
    box("board", 2 * px, 1.9, 0.05, move(middle, FRONT - 0.95, FLOOR - 0.025), grain="y")
    box("timber", 2 * px - 0.3, 0.32, 0.05, move(middle, py - 0.18, FLOOR / 2))
    for side in (-1, 1):
        box("white", 0.12, 0.12, ph - FLOOR, move(middle + side * (px - 0.1), py + 0.1, (ph + FLOOR) / 2), grain="z")
        down = Vector((side * math.cos(pitch), 0, -math.sin(pitch)))
        roof((middle, py - 0.12, ph + px * math.tan(pitch) + 0.1), (0, 1, 0), down, 2.1, (px + 0.25) / math.cos(pitch))
    slab("oxide", [(-px, 0.0), (px, 0.0), (0.0, px * math.tan(pitch))], 0.03, frame((middle, py + 0.08, ph + 0.05), (1, 0, 0), UP, (0, -1, 0)))
    box("white", 2 * px, 0.1, 0.14, move(middle, py + 0.1, ph - 0.02))
    capping((middle, py - 0.12, ph + px * math.tan(pitch) + 0.14), (middle, FRONT, ph + px * math.tan(pitch) + 0.14), wide=0.12)
    # The roof: two slopes off a ridge that runs from the street to the back.
    over, reach = 0.4, (wide / 2 + 0.45) / math.cos(pitch)
    for side in (-1, 1):
        down = Vector((side * math.cos(pitch), 0, -math.sin(pitch)))
        roof((middle, FRONT - over, wall_h + rise + 0.07), (0, 1, 0), down, depth + 2 * over, reach)
    capping((middle, FRONT - over, wall_h + rise + 0.11), (middle, back + over, wall_h + rise + 0.11))
    for side in (-1, 1):
        a = Vector((middle, FRONT - over + 0.02, wall_h + rise + 0.03))
        b = a + Vector((side * math.cos(pitch), 0, -math.sin(pitch))) * reach
        at, long = kit.between(a, b)
        box("white", 0.18, 0.025, long, at @ move(0, 0, long / 2), grain="z")


# =============================================================== Into Blender.
def make_letters(materials, images):
    """The signs' words, as flat letters standing a little off their boards."""
    made = []
    for words, middle, along, out, tall, paint in signs:
        curve = bpy.data.curves.new("words", "FONT")
        curve.body, curve.size, curve.extrude = words, tall, 0.006
        curve.align_x, curve.align_y = "CENTER", "CENTER"
        curve.space_character = 1.08
        lettered = bpy.data.objects.new("sign_" + words.split()[0].lower(), curve)
        bpy.context.scene.collection.objects.link(lettered)
        lettered.matrix_world = frame(middle, along, UP, out)
        if paint not in materials:
            materials[paint] = kit.make_material(paint, images)
        curve.materials.append(materials[paint])
        made.append(lettered)
    bpy.context.view_layer.update()
    for piece in bpy.context.scene.objects:
        piece.select_set(piece in made)
    bpy.context.view_layer.objects.active = made[0]
    bpy.ops.object.convert(target="MESH")
    return made


def pictures(folder):
    """Renders of it, to look at: along the street, the gold buyer's, and from above."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 80
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
    sun.rotation_euler = (math.radians(48), 0.0, math.radians(-150))
    scene.collection.objects.link(sun)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-150, -150, -0.02), (150, -150, -0.02), (150, 150, -0.02), (-150, 150, -0.02)], [], [(0, 1, 2, 3)])
    dirt = bpy.data.materials.new("dirt")
    dirt.use_nodes = True
    dirt.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.42, 0.25, 0.15, 1.0)
    dirt.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    ground.materials.append(dirt)
    scene.collection.objects.link(bpy.data.objects.new("ground", ground))
    camera = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    scene.collection.objects.link(camera)
    scene.camera = camera
    for name, at, to, lens in (("street", (-36.0, -9.0, 2.0), (4.0, 7.0, 2.6), 28), ("buyer", (2.0, -7.5, 1.7), (7.5, 6.0, 2.4), 28),
                               ("hotel", (-31.0, -6.0, 1.8), (-19.0, 7.0, 2.4), 30), ("above", (8.0, -42.0, 30.0), (0.0, 9.0, 1.0), 35)):
        camera.location = at
        camera.data.lens = lens
        look = Vector(to) - Vector(at)
        camera.rotation_euler = look.to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(folder, f"town_{name}.png")
        bpy.ops.render.render(write_still=True)
    for name in ("sun", "ground", "camera"):
        bpy.data.objects.remove(bpy.data.objects[name], do_unlink=True)


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    kit.pieces = kit.Pieces()
    build_hotel()
    build_store()
    build_buyer()
    build_hall()
    folder = os.path.join(KEEP, "textures") if KEEP else os.path.join(os.path.dirname(os.path.abspath(OUT)), "shed_textures")
    os.makedirs(folder, exist_ok=True)
    images = kit.make_images(folder)
    materials = {}
    made = list(kit.make_objects(kit.pieces, "town", images, materials))
    made += make_letters(materials, images)
    triangles = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in made)
    print(f"town: {len(made)} meshes, {triangles} triangles before their edges are eased")
    if KEEP:
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(KEEP, "town.blend"))
        pictures(KEEP)
    for piece in bpy.context.scene.objects:
        piece.select_set(piece in made)
    bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_apply=True, export_yup=True, use_selection=True)
    print(f"town: wrote {OUT}, {os.path.getsize(OUT) / 1e6:.1f} MB")


main()
