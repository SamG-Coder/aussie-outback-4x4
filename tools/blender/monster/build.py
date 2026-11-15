"""Builds what a monster truck stands on, from nothing, in pieces.

  blender -b --python tools/blender/monster/build.py -- OUT.glb [KEEP_FOLDER [TRUCK...]]

A monster truck is a truck on a frame, as the first of them were: a pickup, taken off its
axles and stood on a frame of tube over axles and tyres from a much bigger machine. In the
game it is put together as it is played, of any of the game's own trucks
(`kit::assemble_monster` in `apps/viewer/src/kit.rs`): every piece of the truck is kept but
its axles, its springs and its wheels, lifted, and stood on what is made here.

* a **frame** of tube, in three: an end over each axle, where a rail kicks up over the axle
  under a hoop that the shock absorbers hang from, and a middle between them, two rails low
  and two under the truck with uprights and diagonals between and a skid plate under
  (`chassis_frame_F`, `chassis_frame_R`, `chassis_frame_M`). The ends go where the truck's
  axles are and the middle is drawn out to reach them, so one frame fits every wheelbase;
* an engine amidships, its gearbox and transfer case, and eight pipes (`chassis_engine`);
* two **axles**, each a tube with its differential to one side, a planetary hub at each
  end and a truss over it (`axle_front`, `axle_rear`);
* what holds each axle: four **links** to the frame, two **shock absorbers** a wheel, each
  a body on the frame and a rod on the axle, and a **driveshaft** from the transfer case
  (`susp_link_*`, `susp_shock_*`, `susp_rod_*`, `susp_drive_*`);
* four **tyres** 1.68 m high and 1.08 m wide, with chevron lugs, on deep steel **rims**
  with a bead lock (`tyre_mud_*`, `rim_beadlock_*`).

Its axles move. The game draws what is on an axle where the axle's two wheels have it, and
turns what runs between the frame and an axle to go on reaching it (`Fitted::rides`): the
kit says which pieces those are, `axle PIECE front|rear` and `link PIECE front|rear frame
X Y Z axle X Y Z` (`truck` for `frame` is the middle of the truck, which a frame's end is
not: where a driveshaft starts). And `fits WHEELBASE SITS MIDDLE` says what it was made to:
the wheelbase it is made at, how high a truck's own chassis sits on it, and how long its
middle is.

Beside the model it writes what the game reads, into the folder `TRUCK_SPECS` names (the
game's is `scenes/trucks`): `monster.kit`, `monster.actor` and `monster.vehicle`, which
`bake --kit scenes/trucks/monster.actor --output assets/trucks/monster.truck` packs. With
KEEP_FOLDER, pictures are kept there of each TRUCK named stood on it (the Kestrel, if none
is): the trucks are not in the file.
"""
import importlib.util
import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
found = importlib.util.spec_from_file_location("truck_build", os.path.join(HERE, "..", "truck", "build.py"))
truck = importlib.util.module_from_spec(found)
found.loader.exec_module(truck)
Piece = truck.Piece

args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []

NAME = "monster"
# The wheelbase it is made at, which is the longest of the game's trucks'; how far each end
# of the frame reaches in from its axle; and how long the frame's middle then is.
WHEELBASE = 3.69
REACH = 1.0
MIDDLE = WHEELBASE - 2 * REACH
# Its wheels: a tyre's radius and how wide it is, its rim's radius, and how far apart the
# middles of the two on an axle are.
R, TYRE_WIDE, RIM, TRACK = 0.84, 1.08, 0.33, 2.78
HALF = TYRE_WIDE / 2
# How far a wheel goes up from where it rests, and down.
TRAVEL = (0.40, 0.33)
# Heights: the low rails between the axles, where they run over an axle, the rails under
# the truck, and the tops of the hoops the shock absorbers hang from. And how high a
# truck's own chassis sits: on the rails under it.
BELLY, OVER, DECK, HOOP = 1.0, 1.52, 1.70, 2.22
SITS = DECK + 0.045
# What it is painted in where a truck is not there to say: the frame is gold.
PAINT, SECOND = (0.03, 0.3, 0.085), (0.95, 0.7, 0.03)

truck.PAINTS.update({
    "frame": (SECOND, 0.38, 0.35, None),                         # the tube frame
    "shock_body": ((0.55, 0.56, 0.58), 0.3, 0.9, None),          # a shock absorber's body
    "shock_cap": ((0.03, 0.14, 0.55), 0.35, 0.4, None),          # its ends, and its reservoir
    "header": ((0.74, 0.7, 0.62), 0.25, 1.0, None),              # exhaust pipes, a little blued
    "block": ((0.1, 0.1, 0.11), 0.5, 0.7, None),                 # an engine
})


def ball(piece, name, at, radius, around=12):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=around, v_segments=8, radius=radius)
    piece.take(bm, name, Matrix.Translation(at))


# ============================================================================ The frame.
def frame_end(name, axle, towards):
    """The frame over an axle at `axle`: `towards` is the way the middle of the truck is from
    it. It reaches `REACH` towards the middle, where the frame's middle meets it."""
    made = Piece(name)
    tube = lambda points, radius=0.04: made.tube("frame", [Vector(p) for p in points], radius, 12)
    inner, kick, outer = axle + towards * REACH, axle + towards * 0.55, axle - towards * 0.75
    far = axle - towards * 0.85
    for s in (-1, 1):
        # The low rail, which kicks up over the axle, and the rail under the truck.
        tube([(s * 0.48, inner, BELLY), (s * 0.48, kick, OVER), (s * 0.48, outer, OVER)], 0.045)
        tube([(s * 0.56, inner, DECK), (s * 0.56, far, DECK)], 0.045)
        tube([(s * 0.48, outer, OVER), (s * 0.56, far, DECK)], 0.035)
        for y, z in ((inner, BELLY), (kick, OVER)):
            tube([(s * 0.48, y, z), (s * 0.56, y, DECK)], 0.032)
        # A hoop over the axle, for the shock absorbers to hang from.
        tube([(s * 0.56, axle + 0.46, DECK), (s * 0.72, axle + 0.3, HOOP), (s * 0.72, axle - 0.3, HOOP), (s * 0.56, axle - 0.46, DECK)], 0.04)
        tube([(s * 0.56, axle, DECK), (s * 0.72, axle, HOOP)], 0.028)
        for along in (-0.16, 0.16):
            made.box("steel", (s * 0.72 - 0.035, axle + along - 0.03, HOOP - 0.11), (s * 0.72 + 0.035, axle + along + 0.03, HOOP - 0.01), 0.006)
        # Where the links are held: a plate on the low rail, and one hung between the rails.
        made.box("steel", (s * 0.48 - 0.05, inner - 0.06, BELLY - 0.09), (s * 0.48 + 0.05, inner + 0.06, BELLY + 0.05), 0.008)
        hung = axle + towards * (REACH + 0.05)
        tube([(s * 0.3, hung, 1.42), (s * 0.52, hung, 1.42)], 0.03)
        tube([(s * 0.52, hung, 1.42), (s * 0.56, inner, DECK)], 0.028)
        tube([(s * 0.52, hung, 1.42), (s * 0.48, inner, BELLY)], 0.028)
    # Across: under the truck, between the hoops, and at the end.
    for y in (far, kick):
        tube([(-0.56, y, DECK), (0.56, y, DECK)], 0.035)
    for along in (0.3, -0.3):
        tube([(-0.72, axle + along, HOOP), (0.72, axle + along, HOOP)], 0.035)
    tube([(-0.72, axle + 0.3, HOOP), (0.72, axle - 0.3, HOOP)], 0.026)
    tube([(-0.48, outer, OVER), (0.48, outer, OVER)], 0.04)
    # A push bar, low, under the truck's own bumper.
    tube([(-0.48, outer, OVER), (-0.62, outer - towards * 0.3, OVER - 0.1), (0.62, outer - towards * 0.3, OVER - 0.1), (0.48, outer, OVER)], 0.04)
    return made


def frame_middle():
    """The frame between its two ends. It is drawn out along the truck to reach them, so
    what is in it runs along the truck or straight across it."""
    made = Piece("chassis_frame_M")
    tube = lambda points, radius=0.04: made.tube("frame", [Vector(p) for p in points], radius, 12)
    half = MIDDLE / 2
    for s in (-1, 1):
        tube([(s * 0.48, -half, BELLY), (s * 0.48, half, BELLY)], 0.045)
        tube([(s * 0.56, -half, DECK), (s * 0.56, half, DECK)], 0.045)
        for y in (-half * 0.4, half * 0.4):
            tube([(s * 0.48, y, BELLY), (s * 0.56, y, DECK)], 0.032)
        tube([(s * 0.48, -half, BELLY), (s * 0.56, -half * 0.4, DECK)], 0.028)
        tube([(s * 0.48, half, BELLY), (s * 0.56, half * 0.4, DECK)], 0.028)
    for y in (-half * 0.4, half * 0.4):
        tube([(-0.56, y, DECK), (0.56, y, DECK)], 0.035)
    # A skid plate under the belly, on three bearers.
    made.box("alloy", (-0.54, -half, BELLY - 0.075), (0.54, half, BELLY - 0.05), 0.01)
    for y in (-half * 0.8, 0.0, half * 0.8):
        made.box("steel", (-0.54, y - 0.03, BELLY - 0.05), (0.54, y + 0.03, BELLY - 0.02), 0.004)
    return made


def engine():
    """Amidships: a V8, its gearbox and its transfer case, and four pipes a side that sweep
    out under the truck's sills."""
    made = Piece("chassis_engine")
    made.box("block", (-0.27, -1.0, 1.1), (0.27, -0.18, 1.5), 0.03)
    for s in (-1, 1):
        # A bank of cylinders, leant out, and its rocker cover.
        lean = Matrix.Translation((s * 0.2, -0.59, 1.47)) @ Matrix.Rotation(s * -0.72, 4, "Y")
        made.box("block", (-0.12, -0.38, -0.04), (0.12, 0.38, 0.2), 0.02, lean)
        made.box("chrome", (-0.1, -0.36, 0.2), (0.1, 0.36, 0.27), 0.03, lean)
        for index in range(4):
            y = -0.92 + index * 0.22
            made.tube("header", [Vector((s * 0.36, y, 1.5)), Vector((s * 0.6, y - 0.03, 1.44)), Vector((s * 0.78, y - 0.12, 1.47)), Vector((s * 0.93, y - 0.3, 1.58))], 0.034, 12)
    # The blower's drive at its front, the sump, the gearbox and the transfer case.
    made.turned("alloy", [(-1.07, 0.0), (-1.07, 0.1), (-1.0, 0.1)], (0.0, 0.0, 1.28), "y", 20)
    made.box("block", (-0.2, -0.95, 1.02), (0.2, -0.25, 1.1), 0.02)
    made.turned("alloy", [(-0.18, 0.21), (-0.05, 0.17), (0.3, 0.13), (0.34, 0.1)], (0.0, 0.0, 1.27), "y", 20)
    made.box("alloy", (-0.34, 0.3, 0.99), (0.34, 0.62, 1.36), 0.03)
    for x, y in ((0.25, 0.62), (-0.25, 0.3)):
        made.turned("steel", [(-0.06, 0.07), (0.06, 0.07)], (x, y, 1.08), "y", 14)
    # What it stands on.
    for s in (-1, 1):
        for y in (-0.85, -0.3, 0.45):
            made.tube("frame", [Vector((s * 0.25, y, 1.06)), Vector((s * 0.48, y, BELLY))], 0.022, 8)
    return made


# ============================================================================ The axles.
def axle(name, at, towards):
    """An axle across at `at`: `towards` is the way the middle of the truck is from it, along
    the truck, which is the way its differential's nose points."""
    made = Piece(name)
    reach = TRACK / 2 - 0.4
    offset = -0.25 * towards        # its differential is to one side: the front's and the rear's to opposite sides
    made.rod("iron", (-reach, at, R), (reach, at, R), 0.085, 18)
    # The differential, its cover behind it, and its nose, where the driveshaft comes in.
    nose = [(-0.2, 0.0), (-0.19, 0.16), (-0.1, 0.25), (0.06, 0.26), (0.16, 0.18), (0.3, 0.085), (0.33, 0.0)]
    cover = [(-0.045, 0.0), (-0.03, 0.15), (0.0, 0.17), (0.0, 0.0)]
    if towards < 0:
        nose = [(-along, radius) for along, radius in reversed(nose)]
        cover = [(-along, radius) for along, radius in reversed(cover)]
    made.turned("iron", nose, (offset, at, R), "y", 24)
    made.turned("steel", cover, (offset, at - towards * 0.2, R), "y", 20)
    for s in (-1, 1):
        # A planetary hub at each end, in the middle of its wheel.
        made.turned("iron", [(s * reach, 0.085), (s * (reach + 0.04), 0.2), (s * (TRACK / 2 - 0.1), 0.2), (s * (TRACK / 2 - 0.06), 0.14)], (0, at, R), "x", 24, False)
        for step in range(10):
            angle = math.tau * step / 10
            made.rod("steel", (s * (reach + 0.02), at + 0.17 * math.cos(angle), R + 0.17 * math.sin(angle)),
                     (s * (reach + 0.06), at + 0.17 * math.cos(angle), R + 0.17 * math.sin(angle)), 0.012, 6)
        # Where its links and its shock absorbers are held.
        made.box("steel", (s * 0.78 - 0.05, at - 0.05, R - 0.13), (s * 0.78 + 0.05, at + 0.05, R + 0.02), 0.008)
        for along in (-0.16, 0.16):
            made.box("steel", (s * 0.95 - 0.04, at + along - 0.03, R - 0.02), (s * 0.95 + 0.04, at + along + 0.03, R + 0.15), 0.006)
        made.tube("steel", [Vector((s * 0.95, at - 0.16, R + 0.06)), Vector((s * 0.95, at + 0.16, R + 0.06))], 0.02, 8, False)
    # A truss over it.
    made.tube("steel", [Vector((-0.98, at, R + 0.08)), Vector((-0.38, at, R + 0.27)), Vector((0.38, at, R + 0.27)), Vector((0.98, at, R + 0.08))], 0.03, 10)
    for x in (-0.38, 0.38):
        made.tube("steel", [Vector((x, at, R + 0.27)), Vector((x, at, R + 0.07))], 0.024, 8, False)
        made.box("steel", (x - 0.09, at - 0.04, R + 0.22), (x + 0.04, at + 0.04, R + 0.32), 0.006)
    if name == "axle_front":
        # What steers it: a rod behind the axle from one wheel to the other, and a ram.
        behind = at + towards * 0.24
        made.tube("chrome", [Vector((-reach - 0.1, behind, R - 0.03)), Vector((reach + 0.1, behind, R - 0.03))], 0.024, 10, False)
        made.rod("alloy", (-0.55, behind - towards * 0.07, R + 0.05), (0.15, behind - towards * 0.07, R + 0.05), 0.04, 12)
        made.rod("chrome", (0.15, behind - towards * 0.07, R + 0.05), (0.7, behind - towards * 0.07, R + 0.05), 0.018, 8)
        for s in (-1, 1):
            made.tube("steel", [Vector((s * (reach + 0.1), at, R - 0.03)), Vector((s * (reach + 0.1), behind, R - 0.03))], 0.026, 8)
    return made


def link(name, a, b, radius=0.032, paint="steel"):
    """A bar from a to b with a joint at each end."""
    made = Piece(name)
    a, b = Vector(a), Vector(b)
    made.rod(paint, a, b, radius, 12)
    for end, other in ((a, b), (b, a)):
        ball(made, "chrome", end, radius * 1.55)
        made.rod("chrome", end, end + (other - end).normalized() * 0.09, radius * 1.2, 10)
    return made


def shock(name, top, bottom):
    """A shock absorber's body, hung at `top` and reaching towards `bottom`: a tube, a cap at
    each end, and beside it a reservoir on a hose."""
    made = Piece(name)
    top, bottom = Vector(top), Vector(bottom)
    along = (bottom - top).normalized()
    made.rod("shock_body", top + along * 0.06, top + along * 0.86, 0.058, 16)
    made.rod("shock_cap", top, top + along * 0.1, 0.064, 16)
    made.rod("shock_cap", top + along * 0.8, top + along * 0.88, 0.064, 16)
    ball(made, "chrome", top, 0.05)
    out = Vector((1.0 if top.x > 0 else -1.0, 0.0, 0.0))
    beside = out.cross(along).normalized() * 0.13
    made.rod("shock_cap", top + along * 0.14 + beside, top + along * 0.5 + beside, 0.04, 12)
    made.tube("trim", [top + along * 0.14 + beside, top + along * 0.08 + beside * 0.6, top + along * 0.07], 0.012, 6)
    for at in (0.2, 0.44):
        made.rod("steel", top + along * at, top + along * at + beside, 0.014, 6)
    return made


def rod(name, top, bottom):
    """A shock absorber's rod, held at `bottom` and reaching up towards `top`, into its body."""
    made = Piece(name)
    top, bottom = Vector(top), Vector(bottom)
    along = (top - bottom).normalized()
    made.rod("chrome", bottom, bottom + along * 0.84, 0.024, 10)
    made.rod("shock_cap", bottom, bottom + along * 0.1, 0.04, 12)
    ball(made, "chrome", bottom, 0.046)
    return made


def drive(name, a, b):
    """A driveshaft from the transfer case to an axle's differential, a yoke at each end."""
    made = Piece(name)
    a, b = Vector(a), Vector(b)
    along = (b - a).normalized()
    made.rod("steel", a + along * 0.1, b - along * 0.1, 0.045, 14)
    for end, way in ((a, along), (b, -along)):
        made.rod("iron", end, end + way * 0.12, 0.06, 12)
        ball(made, "iron", end + way * 0.06, 0.07)
    return made


# ============================================================================ The wheels.
def tyre(piece):
    """A tyre about the origin, its axle along x: a balloon of a carcass, sixteen lugs a side
    set in chevrons, and paddles down its shoulders."""
    flank = [(HALF * 0.72, RIM), (HALF * 0.9, RIM + 0.07), (HALF * 0.99, RIM + 0.2), (HALF * 1.0, RIM + 0.33),
             (HALF * 0.94, R - 0.105), (HALF * 0.82, R - 0.06), (HALF * 0.55, R - 0.04), (HALF * 0.2, R - 0.034)]
    carcass = Piece("carcass")
    carcass.turned("tyre", [(-x, r) for x, r in flank] + [(x, r) for x, r in reversed(flank)], (0, 0, 0), "x", 72, False)
    piece.take(carcass.bm, "tyre")
    lugs = Piece("lugs")
    count = 16
    for step in range(count):
        for s in (-1, 1):
            angle = math.tau * (step + (0.5 if s > 0 else 0.0)) / count
            # A lug from the middle out to the shoulder, swept back: two of them make a chevron.
            for x0, x1, sweep, drop, high in ((0.02, HALF * 0.56, 0.5, 0.0, 0.05), (HALF * 0.5, HALF * 0.98, 0.5, -0.045, 0.06)):
                place = (Matrix.Rotation(angle, 4, "X") @ Matrix.Translation((0, 0, R - 0.04 + drop))
                         @ Matrix.Rotation(s * sweep, 4, "Z"))
                lugs.box("tyre", (min(s * x0, s * x1), -0.058, -0.07), (max(s * x0, s * x1), 0.058, high), 0.012, place, 1)
            # A paddle down the shoulder, and a rib on the wall under it.
            wall = Matrix.Rotation(angle + math.tau * 0.25 / count, 4, "X")
            lugs.box("tyre", (s * HALF * 0.9 - 0.05, -0.05, R - 0.2), (s * HALF * 0.9 + 0.05, 0.05, R - 0.07), 0.012, wall, 1)
            lugs.box("tyre", (s * HALF * 0.97 - 0.03, -0.022, RIM + 0.1), (s * HALF * 0.97 + 0.03, 0.022, RIM + 0.36), 0.008, wall, 1)
    piece.take(lugs.bm, "tyre")
    return piece


def rim(piece):
    """A rim about the origin, its face towards -x: a wide steel barrel, a dish set deep in
    it with eight holes, the hub's cap standing out of the middle, and a bead lock."""
    steel = Piece("rim")
    steel.turned("rim_black", [(-HALF * 0.74, RIM), (-HALF * 0.78, RIM + 0.022), (-HALF * 0.7, RIM - 0.014), (HALF * 0.7, RIM - 0.014),
                               (HALF * 0.76, RIM + 0.022), (HALF * 0.72, RIM)], (0, 0, 0), "x", 56, False)
    face = -HALF * 0.3
    steel.turned("rim_black", [(-HALF * 0.7, RIM - 0.014), (face - 0.05, RIM * 0.8), (face, RIM * 0.55), (face - 0.01, 0.2)], (0, 0, 0), "x", 56, False)
    # The hub's cap, and the nuts round it.
    steel.turned("steel", [(face - 0.01, 0.2), (face - 0.03, 0.2), (face - 0.035, 0.125), (face - 0.1, 0.115), (face - 0.13, 0.085), (face - 0.14, 0.0)], (0, 0, 0), "x", 32, False)
    for step in range(10):
        angle = math.tau * step / 10
        steel.rod("chrome", (face - 0.055, 0.162 * math.cos(angle), 0.162 * math.sin(angle)), (face - 0.025, 0.162 * math.cos(angle), 0.162 * math.sin(angle)), 0.016, 6)
    dark = Piece("dark")
    for step in range(8):
        at = Matrix.Rotation(math.tau * (step + 0.5) / 8, 4, "X")
        dark.rod("inside_dark", at @ Vector((face - 0.026, 0.0, RIM * 0.69)), at @ Vector((face + 0.03, 0.0, RIM * 0.69)), 0.04, 14)
    dark.turned("inside_dark", [(HALF * 0.2, 0.2), (HALF * 0.2, RIM - 0.02)], (0, 0, 0), "x", 32, False)
    # The bead lock: a ring bolted round the rim's outer edge, in the frame's colour.
    steel.turned("frame", [(-HALF * 0.79, RIM + 0.04), (-HALF * 0.83, RIM + 0.03), (-HALF * 0.83, RIM - 0.03), (-HALF * 0.79, RIM - 0.035), (-HALF * 0.76, RIM - 0.02)],
                 (0, 0, 0), "x", 56, False)
    for step in range(24):
        angle = math.tau * step / 24
        steel.rod("chrome", (-HALF * 0.855, (RIM + 0.004) * math.cos(angle), (RIM + 0.004) * math.sin(angle)),
                  (-HALF * 0.82, (RIM + 0.004) * math.cos(angle), (RIM + 0.004) * math.sin(angle)), 0.011, 6)
    piece.take(dark.bm, "inside_dark")
    truck.move_rim(piece, steel, Matrix.Identity(4))
    return piece


# ============================================================================ The kit.
def game(point):
    """A point here, as the game has it: x across, y up, and -z the way the truck faces."""
    return f"{point[0]:.3f} {point[2]:.3f} {-point[1]:.3f}"


def build():
    """Every piece of it, as objects; and the lines of its kit."""
    front, rear = WHEELBASE / 2, -WHEELBASE / 2
    made = {}
    base = []
    for piece in (frame_end("chassis_frame_F", front, -1), frame_end("chassis_frame_R", rear, 1), frame_middle(), engine(),
                  axle("axle_front", front, -1), axle("axle_rear", rear, 1)):
        made[piece.name] = piece.finish(PAINT)
        base.append(piece.name)

    # ---- What holds the axles, and the lines of the kit that say how each moves.
    moves = ["axle axle_front front", "axle axle_rear rear"]
    held = []
    for end, at, towards in (("front", front, -1), ("rear", rear, 1)):
        letter = end[0].upper()
        for s, which in ((-1, "L"), (1, "R")):
            pairs = (("low", (s * 0.48, at + towards * REACH, BELLY - 0.03), (s * 0.78, at, R - 0.07)),
                     ("top", (s * 0.3, at + towards * (REACH + 0.05), 1.42), (s * 0.34, at, R + 0.27)))
            for level, on_frame, on_axle in pairs:
                name = f"susp_link_{level}_{letter}{which}"
                made[name] = link(name, on_frame, on_axle).finish(PAINT)
                moves.append(f"link {name} {end} frame {game(on_frame)} axle {game(on_axle)}")
                held.append(name)
            for number, along in ((1, -0.16), (2, 0.16)):
                top, bottom = (s * 0.72, at + along, HOOP - 0.06), (s * 0.95, at + along, R + 0.06)
                body, shaft = f"susp_shock_{letter}{which}{number}", f"susp_rod_{letter}{which}{number}"
                made[body] = shock(body, top, bottom).finish(PAINT)
                made[shaft] = rod(shaft, top, bottom).finish(PAINT)
                moves.append(f"link {body} {end} frame {game(top)} axle {game(bottom)}")
                moves.append(f"link {shaft} {end} axle {game(bottom)} frame {game(top)}")
                held += [body, shaft]
        # The driveshaft: from the transfer case, which is in the middle of the truck and
        # not on this end of the frame, on the side this axle's differential is.
        out, nose = drive_ends(end)
        name = f"susp_drive_{letter}"
        made[name] = drive(name, out, nose).finish(PAINT)
        moves.append(f"link {name} {end} truck {game(out)} axle {game(nose)}")
        held.append(name)

    # ---- The wheels: one tyre and one rim, each standing at all four hubs.
    corners = (("FL", -1, front), ("FR", 1, front), ("RL", -1, rear), ("RR", 1, rear))

    def four(called, make):
        first = Piece(called)
        first.plain = True
        make(first)
        one = first.finish(PAINT)
        bpy.context.view_layer.update()
        mesh = bpy.data.meshes.new_from_object(one.evaluated_get(bpy.context.evaluated_depsgraph_get()))
        mesh.name = called
        bpy.data.objects.remove(one, do_unlink=True)
        names = []
        for letter, x, y in corners:
            wheel = bpy.data.objects.new(f"{called}_{letter}", mesh)
            wheel.location = (x * TRACK / 2, y, R)
            wheel.rotation_euler = (0.0, 0.0, math.pi if x > 0 else 0.0)
            bpy.context.scene.collection.objects.link(wheel)
            made[wheel.name] = wheel
            names.append(wheel.name)
        return names

    tyres, rims = four("tyre_mud", tyre), four("rim_beadlock", rim)
    lines = ["# What a monster truck stands on. Written by tools/blender/monster/build.py."]
    lines += [f"base {name}" for name in sorted(base)]
    lines += [f"choice suspension stock {' '.join(held)}", f"choice tyres mud {' '.join(tyres)}", f"choice rims beadlock {' '.join(rims)}"]
    for what in ("stock", "showroom"):
        lines += [f"{what} suspension stock", f"{what} tyres mud", f"{what} rims beadlock"]
    lines += ["lift stock 0", "paint " + " ".join(f"{channel:.4f}" for channel in PAINT), f"fits {WHEELBASE} {SITS:.3f} {MIDDLE:.3f}"]
    return made, lines + moves


def drive_ends(end):
    """Where a driveshaft starts, on the transfer case, and where it ends, at the nose of
    its axle's differential: the front's or the rear's."""
    towards = -1 if end == "front" else 1
    offset = -0.25 * towards
    at = -towards * WHEELBASE / 2
    return (offset, 0.62 if end == "front" else 0.3, 1.08), (offset, at + towards * 0.33, R)


def vehicle():
    """Its numbers, with a full-size truck on it: four and a half tonnes on tyres that are
    most of its springing, with fifteen hundred horsepower through two gears. The game
    makes them the numbers of whichever truck is stood on it (`kit::assemble_monster`): its
    wheelbase, its body, and a weight and springs for it."""
    return f"""# What a monster truck stands on: its numbers, with a full-size truck on it.
# Written by tools/blender/monster/build.py: to be driven, and put right.

mass 4600
inertia 11000 12500 5200
centre 0 1.25 0.00
body 2.20 2.13 5.89 at 0 2.06 0.15
driver right

front at={WHEELBASE / 2:.3f} track={TRACK:.3f} radius={R:.4f} inertia=95 driven=yes
front spring=62000 bump=5200 rebound=10500 bar=9000 travel={TRAVEL[0]:.2f},{TRAVEL[1]:.2f} roll_centre=0.75
front brake=13000 grip=1
rear at={WHEELBASE / 2:.3f} track={TRACK:.3f} radius={R:.4f} inertia=105 driven=yes
rear spring=58000 bump=4900 rebound=9800 bar=5000 travel={TRAVEL[0]:.2f},{TRAVEL[1]:.2f} roll_centre=0.80
rear brake=8000 grip=1

tyre grip=1.05,0.95 peak=0.15,9 along=1.5,0.3 across=1.3,0.1
tyre load=11300 sensitivity=0.1 relaxation=0.9

engine idle=1100 limit=8200 inertia=0.6 friction=60,0.012
torque 2000 1250
torque 3000 1500
torque 4000 1750
torque 5000 1900
torque 6000 1880
torque 7000 1750
torque 8000 1500
torque 8200 1300

gears 1.8 1
final 22
reverse 2
gearbox shift=0.2 efficiency=0.88 clutch=4000 cruise=0.42
diff preload=400 power=0.6 coast=0.4
front_diff preload=400 power=0.6 coast=0.4
transfer low=1.6 front=0.5 lock=60000 locker=60000

aero drag=2.9 downforce=0 balance=0.5
steering lock=28 ackermann=0.6 quick=0.6
geometry anti_dive=0.2 anti_squat=0.25
assists abs=no traction=no stability=no descent=no
"""


# ============================================================================ Pictures.
def stood(name):
    """One of the game's trucks, stood on it as the game stands it, to be looked at: all of
    it but its axles, its springs and its wheels, as it is in its own pictures, lifted so
    that its own chassis sits on the frame. Gives its pieces, its numbers as it now stands,
    and how far each of its axles is beyond where the frame's ends were made."""
    shell, numbers, t, parts = truck.build(name)
    bpy.context.view_layer.update()
    low = min((each.matrix_world @ Vector(corner)).z for each in shell if each.name == "chassis" for corner in each.bound_box)
    lift = SITS - low
    shown = set(parts["base"])
    for group, choice in parts["showroom"].items():
        if choice:
            shown.update(parts["groups"][group][choice])
    for group, choice in parts["showroom"].items():
        if choice:
            shown.difference_update(parts["replaces"].get(f"{group}.{choice}", ()))
    kept = []
    for each in shell:
        if each.name.startswith(("axle_", "susp_", "tyre_", "rim_")):
            bpy.data.objects.remove(each, do_unlink=True)
            continue
        each.location.z += lift
        each.hide_render = each.name not in shown
        kept.append(each)
    numbers = dict(numbers, roof=numbers["roof"] + lift, wide=TRACK + TYRE_WIDE)
    return kept, numbers, (numbers["wheelbase"] - WHEELBASE) / 2


def fit(made, shift):
    """Moves the kit to a wheelbase `2 * shift` longer than it was made at, as the game does:
    its ends with the axles, its middle drawn out, and its driveshafts turned and drawn out
    to reach. Gives what to call to put it back."""
    was = [(each, each.matrix_world.copy()) for each in made.values()]
    for name, each in made.items():
        end = name.rsplit("_", 1)[-1]
        if name.startswith("susp_drive_"):
            # Held on the transfer case, which stays, and reaching an axle, which moves.
            a, b = (Vector(point) for point in drive_ends("front" if end == "F" else "rear"))
            b2 = b + Vector((0.0, shift if end == "F" else -shift, 0.0))
            turn = (b - a).rotation_difference(b2 - a).to_matrix().to_4x4()
            along = (b - a).normalized()
            stretch = Matrix.Identity(4)
            grow = (b2 - a).length / (b - a).length - 1.0
            for row in range(3):
                for column in range(3):
                    stretch[row][column] += grow * along[row] * along[column]
            each.matrix_world = Matrix.Translation(a) @ turn @ stretch @ Matrix.Translation(-a) @ each.matrix_world
        elif end.startswith("F") or end == "front":
            each.matrix_world = Matrix.Translation((0.0, shift, 0.0)) @ each.matrix_world
        elif end.startswith("R") or end == "rear":
            each.matrix_world = Matrix.Translation((0.0, -shift, 0.0)) @ each.matrix_world
        elif end == "M":
            each.matrix_world = Matrix.Diagonal((1.0, max((MIDDLE + 2 * shift) / MIDDLE, 0.05), 1.0, 1.0)) @ each.matrix_world

    def back():
        for each, where in was:
            each.matrix_world = where
    return back


def main():
    if not args:
        print(__doc__)
        return
    out, keep, donors = args[0], (args[1] if len(args) > 1 else None), (args[2:] or ["kestrel"])
    bpy.ops.wm.read_factory_settings(use_empty=True)
    made, lines = build()
    for each in made.values():
        for poly in each.data.polygons:
            poly.use_smooth = True
    bpy.context.view_layer.update()
    triangles, seen = 0, set()
    for each in made.values():
        if each.data.name in seen:
            continue
        seen.add(each.data.name)
        mesh = each.evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh()
        triangles += sum(len(poly.vertices) - 2 for poly in mesh.polygons)
    print(f"{NAME}: {len(made)} pieces, {triangles} triangles")
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    for each in bpy.context.scene.objects:
        each.select_set(each.name in made)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_apply=True, export_yup=True, use_selection=True)
    print(f"{NAME}: wrote {out}, {os.path.getsize(out) / 1e6:.1f} MB")
    folder = os.environ.get("TRUCK_SPECS") or os.path.dirname(out)
    os.makedirs(folder, exist_ok=True)
    texts = {
        ".kit": "\n".join(lines) + "\n",
        ".actor": ("# What a monster truck stands on (tools/blender/monster/build.py). It is not driven as it is:\n"
                   "# the game stands one of its trucks on it (`kit::assemble_monster`).\n"
                   f"name {NAME}\nmesh {os.path.relpath(out, folder).replace(os.sep, '/')} 1\nvehicle {NAME}.vehicle\nkit {NAME}.kit\ncamera 0 2.4 0\n"),
        ".vehicle": vehicle(),
    }
    for ending, text in texts.items():
        with open(os.path.join(folder, NAME + ending), "w", encoding="utf-8", newline="\n") as file:
            file.write(text)
    print(f"{NAME}: wrote its kit, its actor and its numbers into {folder}")
    if keep:
        os.makedirs(keep, exist_ok=True)
        for donor in donors:
            kept, numbers, shift = stood(donor)
            back = fit(made, shift)
            long, roof = max(numbers["nose"] - numbers["tail"], 4.6), numbers["roof"]
            middle = (0.0, (numbers["nose"] + numbers["tail"]) / 2, roof * 0.45)
            truck.pictures(keep, f"{NAME}_{donor}", numbers, (), (1280, 720), close=[
                ("front", (-long * 0.95, long * 1.3, roof * 0.75), middle),
                ("side", (-long * 1.75, middle[1], roof * 0.5), middle),
            ])
            back()
            for each in kept:
                bpy.data.objects.remove(each, do_unlink=True)


if __name__ == "__main__":
    main()
