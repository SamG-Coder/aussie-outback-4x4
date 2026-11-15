"""Builds the game's own 4x4s, from nothing, in pieces.

  blender -b --python tools/blender/truck/build.py -- NAME OUT.glb [KEEP_FOLDER]
  blender -b --python tools/blender/truck/build.py -- all OUT_FOLDER [KEEP_FOLDER]
  blender -b --python tools/blender/truck/build.py -- kit NAME OUT.glb PICTURES_FOLDER
  blender -b --python tools/blender/truck/build.py -- list

No truck here is a real one. Each is a set of numbers (`TRUCKS`): how long, how wide, how
high, where its cab sits, what its nose and its back are like, what lamps and what grille
it has. The numbers are taken from the kinds of 4x4 there are, and mixed: one has the cab of
one maker's ute and the lamps of another's wagon. Nothing is copied and nothing is badged.

A truck is a **bare base** and a **kit**. The base is the truck as it left the factory with
nothing on it. The kit is everything that can be bolted on, every kind of it, each made to
fit this truck: three bull bars, spot lamps, a snorkel, a rack, a light bar, flares, steps
and sliders, four sets of springs and shocks, what goes on a tub or a tray, three tyres and
six rims. All of it is in the truck's file, each a piece of its own, and the game builds a
truck from them as it is played: it shows the base, and of each group the choice that is
fitted (`apps/viewer/src/kit.rs`).

Its cab is a shell: hollow, with windows cut through it and glass that is seen through, a
dash, a wheel and seats inside. Its doors are cut out of it, and they and a tailgate are
made about their hinges, to swing. The places gold can be put (a seat, a boot, a tub or a
tray) are measured as it is made.

Beside each truck's file are written:

* `NAME.parts.json`: which pieces are the base, which are which choice of which group, what
  it wears as it is sold (`stock`) and in its pictures (`showroom`), its hinges, the places
  gold can be put (`holds`), its numbers;
* `NAME.audit.json`: what a check of every piece found (`audit`): its faults are printed
  too, as `NAME: FAULT piece: what`;
* and, into the folder `TRUCK_SPECS` names (the game's are in `scenes/trucks`), what the
  game reads: `NAME.kit`, the parts list as lines; `NAME.actor`; and `NAME.vehicle`, its
  numbers, a first reckoning from its size, its years and its grade (`game_files`).

Pieces are named for what they are: `cab`, `interior`, `bonnet`, `guard_L`, `door_FL`,
`door_back`, `tailgate`, `grille`, `bumper_front`, `tray` or `tub`, `chassis`, `axle_front`,
and of the kit `acc_bullbar_plate`, `acc_snorkel`, `susp_lift_F`, `tyre_mud_FL`,
`rim_spoke5_FL`... A wheel's origin is its hub, and a door's its hinge; everything else's
is the ground under the middle of the truck, which faces +Y here and -Z in the file.

Materials are named for the game too: `paint` is what the game recolours and rusts, what
begins `inside` takes no dust or mud (`Coat` in solstice-render), and `glass` has an alpha,
which the game draws as glass seen through.

With KEEP_FOLDER, pictures are kept there to look at: the truck as it is shown, bare, and
with its doors open. `kit` makes a picture of every choice of everything, fitted in turn.
"""
import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []

# ============================================================================ The trucks.
# Lengths in metres. `y` runs along the truck, forward; the middle of the wheelbase is 0.
BASE = dict(
    wheelbase=2.85, track=1.42, tyre=0.36, tyre_wide=0.225, rim_radius=0.2,
    front=0.8, rear=1.05,                # how far the body runs past each axle
    wide=1.66,                           # the body, across
    sill=0.42, belt=1.0, roof=1.68,      # heights: its sills, the bottom of its glass, its roof
    cowl=0.5,                            # how far behind the front axle the windscreen starts
    bonnet_front=0.93, bonnet_back=1.02, # the bonnet's height at the nose and at the cowl
    screen=0.4,                          # how far the windscreen leans back for each metre up
    cab_len=1.22,                        # the cab, from the foot of its screen to its back
    back_glass=0.06,                     # how far the cab's back leans forward at the roof
    tumble=0.085,                        # how much narrower the roof is than the belt, a side
    nose_taper=0.05,                     # and the nose than the cab, a side
    doors=2,
    back="tray",                         # tray, tub or wagon
    tray_deck=0.93, tray_wide=1.78, tray_sides=0.24,
    tub_top=1.0,
    round=0.05,                          # how round its edges are: a box is 0.03, a pebble 0.11
    tuck=0.035,                          # how far its sides tuck under, from belt to sill
    crown=0.03,                          # how much higher its roof and bonnet are along the middle
    tail_taper=0.03,
    lamps="round",                       # round, twin_round, square, twin_square, wide, corner, wrap, stacked, slim, c_clamp, halo, pod
    grille="slats",                      # slats, bars, mesh, egg, split, panel, wide, frame, hex, letter, honey
    bumper="blade",                      # blade, channel, tube, moulded, sport, plate
    flares="none",                       # none, bolted, moulded
    guards="joined",                     # joined to the bonnet, or separate: a narrow bonnet between two guards
    arches="round",                      # round or square
    bonnet="flat",                       # flat, bulge, scoop, vents
    rim="slot8",                         # slot8, sunraysia, spoke5, spoke6, mesh, beadlock
    rim_paint="steel",                   # steel, white, black, bronze
    tread="at",                          # road, at, mud
    side="plain",                        # plain, moulding, cladding, stripe, twotone
    mirror="stalk",                      # stalk, arm, pod
    roof_style="plain",                  # plain, ribs, gutter
    bar="tube",                          # with a bull bar: tube, plate, nudge
    spots=False,                         # lamps on the bar
    tray_style="alloy",                  # alloy, timber, boxes
    tub_style="plain",                   # plain, sportsbar, tonneau, canopy
    tail="tall",                         # tail lamps: small, tall, wrap, bar
    aerial=False,
    paint2=(0.9, 0.88, 0.8),             # a second colour, for a stripe or a two-tone
    arch=0.085,                          # the gap round a tyre
    lift=0.0,
    paint=(0.72, 0.68, 0.55),
    fit=("mirrors",),                    # what is bolted on
)

# The shapes trucks come in: each a kind of 4x4 there has been, and no one maker's.
KINDS = {
    # A 1980s farm ute: upright, narrow, leaf-sprung.
    "old_ute": dict(BASE),
    # A tiny short-wheelbase 4x4.
    "tiny": dict(BASE, wheelbase=2.03, track=1.21, tyre=0.335, tyre_wide=0.2, rim_radius=0.19, front=0.6, rear=0.72, wide=1.42,
                 sill=0.4, belt=0.98, roof=1.66, cowl=0.46, bonnet_front=0.92, bonnet_back=1.0, screen=0.3, cab_len=1.1,
                 back_glass=0.04, tumble=0.07, nose_taper=0.03, round=0.04, tray_wide=1.46, tray_deck=0.9),
    # Flat panels, an upright screen, a narrow bonnet between its guards: built to be mended.
    "flat": dict(BASE, wheelbase=3.18, track=1.5, tyre=0.4, tyre_wide=0.25, front=0.84, rear=1.2, wide=1.74,
                 sill=0.5, belt=1.14, roof=1.92, cowl=0.54, bonnet_front=1.06, bonnet_back=1.14, screen=0.2, cab_len=1.2,
                 back_glass=0.02, tumble=0.045, nose_taper=0.1, round=0.03, crown=0.012, tuck=0.02, grille="mesh",
                 flares="bolted", tray_deck=1.02, tray_wide=1.86),
    # A big 1990s wagon on coils.
    "old_wagon": dict(BASE, wheelbase=2.97, track=1.56, tyre=0.4, tyre_wide=0.27, front=0.88, rear=1.12, wide=1.84,
                      sill=0.46, belt=1.1, roof=1.86, cowl=0.56, bonnet_front=1.02, bonnet_back=1.12, screen=0.4, cab_len=1.25,
                      doors=4, back="wagon", tumble=0.1, round=0.055, lamps="square", grille="bars", flares="bolted",
                      tray_deck=1.0, tray_wide=1.9),
    # The 1990s: the same things with their corners rubbed off.
    "nineties": dict(BASE, wheelbase=2.95, track=1.48, tyre=0.375, tyre_wide=0.245, front=0.88, rear=1.15, wide=1.72,
                     sill=0.44, belt=1.05, roof=1.74, cowl=0.56, bonnet_front=0.96, bonnet_back=1.07, screen=0.52, cab_len=1.25,
                     back_glass=0.09, tumble=0.11, nose_taper=0.08, round=0.075, lamps="square", grille="bars",
                     tray_deck=0.96, tub_top=1.05),
    # A modern dual-cab: high belt, raked screen, moulded everything.
    "modern": dict(BASE, wheelbase=3.08, track=1.54, tyre=0.385, tyre_wide=0.265, rim_radius=0.22, front=0.95, rear=1.28, wide=1.85,
                   sill=0.47, belt=1.12, roof=1.8, cowl=0.62, bonnet_front=1.04, bonnet_back=1.16, screen=0.62, cab_len=2.15,
                   doors=4, back="tub", tub_top=1.12, back_glass=0.1, tumble=0.12, nose_taper=0.1, round=0.085,
                   lamps="wide", grille="bars", bumper="moulded", flares="moulded", tray_deck=1.02, tray_wide=1.88),
    # A wide-bodied fast ute.
    "wide": dict(BASE, wheelbase=3.22, track=1.71, tyre=0.42, tyre_wide=0.3, rim_radius=0.22, front=0.98, rear=1.3, wide=2.02,
                 sill=0.52, belt=1.16, roof=1.86, cowl=0.64, bonnet_front=1.08, bonnet_back=1.2, screen=0.64, cab_len=2.15,
                 doors=4, back="tub", tub_top=1.16, back_glass=0.1, tumble=0.14, nose_taper=0.11, round=0.09,
                 lamps="wide", grille="mesh", flares="moulded", arch=0.11, tray_deck=1.08, tray_wide=2.0),
    # A full-size American-style truck.
    "full": dict(BASE, wheelbase=3.69, track=1.88, tyre=0.47, tyre_wide=0.33, rim_radius=0.22, front=1.0, rear=1.3, wide=2.2,
                 sill=0.58, belt=1.3, roof=2.03, cowl=0.7, bonnet_front=1.22, bonnet_back=1.34, screen=0.66, cab_len=2.3,
                 doors=4, back="tub", tub_top=1.3, back_glass=0.08, tumble=0.14, nose_taper=0.1, round=0.075,
                 lamps="wide", grille="bars", flares="moulded", arch=0.12, tray_deck=1.2, tray_wide=2.2),
    # Tall, square and expensive.
    "square": dict(BASE, wheelbase=2.89, track=1.64, tyre=0.43, tyre_wide=0.29, rim_radius=0.25, front=0.82, rear=1.0, wide=1.93,
                   sill=0.54, belt=1.22, roof=1.98, cowl=0.58, bonnet_front=1.16, bonnet_back=1.24, screen=0.26, cab_len=1.3,
                   doors=4, back="wagon", back_glass=0.03, tumble=0.06, nose_taper=0.04, round=0.04, crown=0.015,
                   grille="slats", bumper="moulded", flares="moulded", tray_deck=1.1, tray_wide=1.95),
}

WAGON = dict(back="wagon")
FIT = ("mirrors",)
BUILT = ("mirrors", "bullbar", "snorkel", "rack", "lightbar")

# Fifty of them, ten to a grade: its name, its kind, what it costs second-hand, what it is,
# and how it differs from its kind. Named for places in the high country.
LINE_UP = [
    # ---- Grade 1: paddock bashers. Thirty or forty years old, slow, simple, cheap.
    ("dargo", 1, "Dargo", "old_ute", 6500, "A single-cab tray-back diesel on leaf springs. Slow, and will not die.", dict(paint=(0.75, 0.72, 0.6))),
    ("dargo_tub", 1, "Dargo Styleside", "old_ute", 7500, "The same cab with a steel tub and square lamps.",
     dict(back="tub", tub_top=0.98, lamps="square", grille="bars", paint=(0.5, 0.08, 0.06))),
    ("omeo", 1, "Omeo", "tiny", 8000, "A tiny petrol wagon: floats over mud the big ones sink in, and runs out of breath on a hill.",
     dict(WAGON, grille="bars", paint=(0.78, 0.1, 0.08))),
    ("omeo_tray", 1, "Omeo Tray", "tiny", 7000, "The tiny one with its back cut off and a tray on.", dict(rear=0.95, paint=(0.2, 0.42, 0.62))),
    ("licola", 1, "Licola", "flat", 9500, "A short flat-panelled tray-back from the seventies: nothing on it that a hammer cannot fix.",
     dict(wheelbase=2.3, rear=0.95, tyre=0.38, sill=0.46, belt=1.08, roof=1.84, bonnet_front=1.0, bonnet_back=1.08, flares="none",
          tray_deck=0.98, paint=(0.72, 0.75, 0.68))),
    ("licola_top", 1, "Licola Hardtop", "flat", 11000, "The short one with a steel roof to its tail.",
     dict(WAGON, wheelbase=2.3, rear=0.85, tyre=0.38, sill=0.46, belt=1.08, roof=1.84, bonnet_front=1.0, bonnet_back=1.08, flares="none",
          paint=(0.82, 0.62, 0.2))),
    ("tambo", 1, "Tambo", "old_wagon", 9000, "A boxy eighties family wagon: four doors, vinyl seats, a carburettor.",
     dict(wheelbase=2.65, track=1.4, tyre=0.365, tyre_wide=0.23, wide=1.68, sill=0.42, belt=1.02, roof=1.76, bonnet_front=0.94,
          bonnet_back=1.03, round=0.04, flares="none", fit=FIT, paint=(0.42, 0.3, 0.16))),
    ("buckland", 1, "Buckland", "old_ute", 8500, "An extra-cab ute with a long tub and room behind the seats for a swag.",
     dict(cab_len=1.6, wheelbase=3.05, back="tub", tub_top=0.98, lamps="square", grille="slats", paint=(0.16, 0.3, 0.18))),
    ("bendoc", 1, "Bendoc", "tiny", 6000, "A small round-cornered wagon on coil springs with full-time four-wheel drive.",
     dict(WAGON, wheelbase=2.2, track=1.38, wide=1.6, rear=0.68, round=0.07, screen=0.42, tumble=0.1, paint=(0.85, 0.8, 0.55))),
    ("wonnangatta", 1, "Wonnangatta", "old_ute", 10500, "A one-tonner: long wheelbase, heavy springs, a tray you could camp on.",
     dict(wheelbase=3.25, rear=1.25, tyre=0.38, sill=0.46, belt=1.04, roof=1.72, tray_deck=0.98, tray_wide=1.9, paint=(0.86, 0.86, 0.84))),
    # ---- Grade 2: old tourers. Big, heavy, made to go a long way.
    ("howitt", 2, "Howitt", "old_wagon", 22000, "A big diesel wagon on coils and live axles: the one every grey nomad wanted.",
     dict(fit=("mirrors", "bullbar", "rack"), paint=(0.82, 0.82, 0.8))),
    ("howitt_ute", 2, "Howitt Ute", "old_wagon", 19000, "The big wagon's nose on a cab-chassis with a tray.",
     dict(doors=2, back="tray", wheelbase=3.17, rear=1.2, fit=("mirrors", "bullbar"), paint=(0.68, 0.66, 0.56))),
    ("howitt_short", 2, "Howitt Shorty", "old_wagon", 18000, "Two doors and half a metre shorter: turns inside the long one.",
     dict(doors=2, wheelbase=2.4, rear=0.9, paint=(0.1, 0.24, 0.42))),
    ("jamieson", 2, "Jamieson", "flat", 28000, "A flat-panelled cab-chassis with a steel tray. Farms run on them.",
     dict(fit=("mirrors", "snorkel", "bullbar"), paint=(0.86, 0.84, 0.74))),
    ("jamieson_troop", 2, "Jamieson Troop", "flat", 30000, "The flat cab with a tall wagon body and seats down each side.",
     dict(WAGON, wheelbase=2.98, rear=1.25, roof=2.06, fit=("mirrors", "snorkel"), paint=(0.84, 0.82, 0.76))),
    ("mitta", 2, "Mitta", "nineties", 16000, "A mid-sized nineties wagon with independent front suspension and a soft ride.",
     dict(WAGON, doors=4, wheelbase=2.72, wide=1.78, roof=1.82, belt=1.08, flares="moulded", fit=("mirrors", "rack"), paint=(0.3, 0.32, 0.34))),
    ("mitta_short", 2, "Mitta Shorty", "nineties", 14000, "The same with two doors.",
     dict(WAGON, wheelbase=2.42, rear=0.9, wide=1.78, roof=1.82, belt=1.08, flares="moulded", paint=(0.55, 0.1, 0.1))),
    ("delatite", 2, "Delatite", "nineties", 17000, "A nineties dual-cab ute with a tub: the first of the family utes.",
     dict(doors=4, cab_len=2.05, back="tub", wheelbase=3.0, rear=1.25, paint=(0.78, 0.78, 0.76))),
    ("delatite_tray", 2, "Delatite Tray", "nineties", 15000, "The single cab, with a tray.", dict(wheelbase=3.0, rear=1.2, paint=(0.2, 0.3, 0.5))),
    ("feathertop", 2, "Feathertop", "old_wagon", 26000, "A big rounded wagon with a straight six: quiet, heavy and unstoppable.",
     dict(wheelbase=2.85, wide=1.93, round=0.085, screen=0.5, tumble=0.12, lamps="wide", grille="slats", bumper="moulded",
          flares="moulded", paint=(0.12, 0.25, 0.2))),
    # ---- Grade 3: modern workhorses.
    ("ovens", 3, "Ovens", "modern", 42000, "A dual-cab turbo-diesel ute with a tub: what half the country drives to work.", dict(paint=(0.55, 0.57, 0.6))),
    ("ovens_tray", 3, "Ovens Tray", "modern", 36000, "The single cab with an alloy tray, as a tradesman buys it.",
     dict(doors=2, cab_len=1.3, back="tray", paint=(0.9, 0.9, 0.88))),
    ("ovens_extra", 3, "Ovens Extra", "modern", 39000, "An extra cab: two doors, a long tub.", dict(doors=2, cab_len=1.7, paint=(0.42, 0.16, 0.1))),
    ("kiewa", 3, "Kiewa", "modern", 48000, "A seven-seat wagon on the ute's chassis.",
     dict(WAGON, wheelbase=2.79, rear=1.12, wide=1.88, roof=1.85, grille="slats", fit=("mirrors", "rack"), paint=(0.12, 0.2, 0.16))),
    ("buffalo", 3, "Buffalo", "modern", 45000, "Another maker's dual-cab: a square jaw, a steel bar, a bigger engine.",
     dict(round=0.06, screen=0.55, nose_taper=0.06, grille="mesh", bumper="steel", wheelbase=3.2, paint=(0.08, 0.1, 0.14))),
    ("buffalo_tray", 3, "Buffalo Tray", "modern", 40000, "The square-jawed one as a dual-cab with a tray.",
     dict(round=0.06, screen=0.55, nose_taper=0.06, grille="mesh", bumper="steel", wheelbase=3.2, cab_len=2.05, back="tray", paint=(0.86, 0.52, 0.1))),
    ("hotham", 3, "Hotham", "modern", 58000, "A tall mid-sized wagon: the school run and the long weekend alike.",
     dict(WAGON, wheelbase=2.79, rear=1.1, wide=1.89, sill=0.5, belt=1.16, roof=1.9, round=0.07, paint=(0.8, 0.8, 0.8))),
    ("tumut", 3, "Tumut", "tiny", 34000, "The tiny one, built new: square, light, and good at everything small.",
     dict(WAGON, wheelbase=2.25, track=1.4, wide=1.64, tyre=0.35, roof=1.72, round=0.045, bumper="moulded", flares="moulded",
          grille="bars", paint=(0.55, 0.7, 0.2))),
    ("walhalla", 3, "Walhalla", "modern", 38000, "A soft wagon with all-wheel drive and not much clearance.",
     dict(WAGON, wheelbase=2.7, rear=1.0, tyre=0.35, sill=0.38, belt=1.0, roof=1.66, bonnet_front=0.92, bonnet_back=1.03, round=0.1,
          screen=0.75, tumble=0.15, paint=(0.45, 0.47, 0.5))),
    ("ovens_cab", 3, "Ovens Cab-chassis", "modern", 41000, "The dual cab with a tray and a ladder rack.",
     dict(cab_len=2.05, back="tray", fit=("mirrors", "rack"), paint=(0.9, 0.9, 0.88))),
    # ---- Grade 4: built tourers.
    ("jamieson_v8", 4, "Jamieson V8", "flat", 95000, "The flat cab with a V8 diesel, two rows of seats and everything bolted on.",
     dict(doors=4, cab_len=2.0, track=1.6, tyre=0.43, tyre_wide=0.29, sill=0.56, belt=1.2, roof=1.98, bonnet_front=1.12, bonnet_back=1.2,
          tray_deck=1.1, tray_wide=1.9, lift=0.05, fit=BUILT, paint=(0.62, 0.55, 0.36))),
    ("jamieson_v8_single", 4, "Jamieson V8 Single", "flat", 88000, "The V8 single cab: a long tray, a long-range tank, a winch.",
     dict(track=1.6, tyre=0.43, tyre_wide=0.29, sill=0.56, belt=1.2, roof=1.98, bonnet_front=1.12, bonnet_back=1.2, tray_deck=1.1,
          tray_wide=1.9, lift=0.05, fit=("mirrors", "bullbar", "snorkel"), paint=(0.84, 0.84, 0.8))),
    ("jamieson_troop_v8", 4, "Jamieson Troop V8", "flat", 99000, "The tall wagon, built for the desert: rack, awning, two spares.",
     dict(WAGON, wheelbase=2.98, rear=1.25, roof=2.1, tyre=0.43, tyre_wide=0.29, sill=0.56, belt=1.2, bonnet_front=1.12, bonnet_back=1.2,
          lift=0.05, fit=("mirrors", "bullbar", "snorkel", "rack", "spare"), paint=(0.75, 0.7, 0.55))),
    ("bogong", 4, "Bogong Sport", "wide", 92000, "A wide-bodied ute on long-travel suspension: fast over corrugations, flared and loud.",
     dict(fit=("mirrors", "lightbar"), paint=(0.05, 0.22, 0.5))),
    ("bogong_wagon", 4, "Bogong Wagon", "wide", 96000, "The wide one with a roof to its tail.",
     dict(WAGON, wheelbase=2.9, rear=1.15, roof=1.9, paint=(0.5, 0.5, 0.52))),
    ("ovens_tourer", 4, "Ovens Tourer", "modern", 78000, "The dual-cab ute, built: lift, bar, rack, bigger tyres.",
     dict(tyre=0.42, tyre_wide=0.29, lift=0.06, sill=0.52, arch=0.1, fit=BUILT, paint=(0.3, 0.36, 0.26))),
    ("feathertop_gx", 4, "Feathertop GX", "modern", 105000, "The big wagon's grandchild: a twin-turbo V8 and eight seats.",
     dict(WAGON, wheelbase=2.85, rear=1.2, track=1.65, wide=1.98, tyre=0.42, sill=0.52, belt=1.2, roof=1.94, bonnet_front=1.12,
          bonnet_back=1.24, round=0.07, grille="slats", fit=("mirrors", "rack", "bullbar"), paint=(0.86, 0.86, 0.84))),
    ("howitt_y", 4, "Howitt Y", "modern", 98000, "A big petrol V8 wagon: independent all round and very fast on sand.",
     dict(WAGON, wheelbase=3.07, rear=1.2, track=1.7, wide=2.0, tyre=0.42, sill=0.52, belt=1.22, roof=1.94, bonnet_front=1.14,
          bonnet_back=1.26, round=0.1, screen=0.7, grille="mesh", paint=(0.05, 0.05, 0.06))),
    ("buffalo_chase", 4, "Buffalo Chase", "modern", 85000, "The square-jawed dual-cab with a tray, a canopy frame and a lift.",
     dict(round=0.06, screen=0.55, nose_taper=0.06, grille="mesh", bumper="steel", wheelbase=3.2, cab_len=2.05, back="tray", tyre=0.42,
          tyre_wide=0.29, lift=0.06, sill=0.52, arch=0.1, fit=BUILT, paint=(0.7, 0.2, 0.08))),
    ("tumut_built", 4, "Tumut Trail", "tiny", 52000, "The tiny one on big tyres with a rack: goes where the big ones cannot fit.",
     dict(WAGON, wheelbase=2.25, track=1.44, wide=1.64, tyre=0.39, tyre_wide=0.26, roof=1.72, sill=0.45, round=0.045, bumper="steel",
          flares="bolted", grille="bars", lift=0.05, arch=0.1, fit=("mirrors", "rack", "bullbar", "snorkel"), paint=(0.9, 0.86, 0.6))),
    # ---- Grade 5: the top end.
    ("kestrel", 5, "Kestrel", "full", 165000, "A full-size desert truck: over two metres wide, a bonnet like a billiard table, a supercharged V8.",
     dict(paint=(0.55, 0.03, 0.03))),
    ("kestrel_xl", 5, "Kestrel XL", "full", 150000, "The full-size truck with a long tub and a diesel: tows anything.",
     dict(wheelbase=4.0, rear=1.45, tyre=0.44, arch=0.09, grille="slats", paint=(0.08, 0.1, 0.16))),
    ("kestrel_tray", 5, "Kestrel Tray", "full", 158000, "The full-size cab on a tray: a mine-site chase truck.",
     dict(back="tray", fit=("mirrors", "bullbar", "lightbar"), paint=(0.9, 0.9, 0.88))),
    ("cobbler", 5, "Cobbler", "square", 240000, "A tall, square, very expensive wagon with three locking diffs and a spare on its back door.",
     dict(fit=("mirrors", "rack", "spare"), paint=(0.06, 0.07, 0.07))),
    ("cobbler_ute", 5, "Cobbler Ute", "square", 255000, "The square one as a dual-cab with a tub.",
     dict(back="tub", cab_len=2.05, wheelbase=3.4, rear=1.2, tub_top=1.22, paint=(0.5, 0.52, 0.5))),
    ("stirling", 5, "Stirling", "square", 135000, "A new square wagon with soft corners: air springs, and clever with it.",
     dict(round=0.085, screen=0.36, tumble=0.08, wheelbase=3.02, lamps="square", grille="bars", fit=("mirrors", "rack"), paint=(0.6, 0.64, 0.56))),
    ("stirling_short", 5, "Stirling 90", "square", 120000, "The same with two doors and a short tail.",
     dict(round=0.085, screen=0.36, tumble=0.08, wheelbase=2.59, rear=0.9, doors=2, lamps="square", grille="bars", paint=(0.82, 0.8, 0.72))),
    ("feathertop_300", 5, "Feathertop Sahara", "modern", 150000, "The flagship wagon: everything the big one ever was, with leather.",
     dict(WAGON, wheelbase=2.85, rear=1.22, track=1.67, wide=1.99, tyre=0.43, sill=0.54, belt=1.22, roof=1.96, bonnet_front=1.14,
          bonnet_back=1.27, round=0.08, grille="slats", paint=(0.2, 0.05, 0.06))),
    ("wellington", 5, "Wellington", "full", 185000, "A heavy-duty truck a size up again: a big diesel, and a step to climb into it.",
     dict(tyre=0.48, sill=0.64, belt=1.38, roof=2.14, bonnet_front=1.32, bonnet_back=1.44, tub_top=1.38, round=0.055, screen=0.5,
          grille="slats", fit=("mirrors", "lightbar"), paint=(0.3, 0.3, 0.32))),
    ("kosciuszko", 5, "Kosciuszko", "full", 210000, "The biggest wagon there is: three rows, a V8, and room for a week's water.",
     dict(WAGON, wheelbase=3.4, rear=1.35, round=0.07, grille="bars", fit=("mirrors", "rack"), paint=(0.86, 0.86, 0.86))),
]

KIND_STYLE = {
    "old_ute": dict(grille="slats", lamps="round", bumper="blade", rim="slot8", roof_style="gutter"),
    "tiny": dict(grille="bars", lamps="round", bumper="channel", rim="slot8", roof_style="gutter"),
    "flat": dict(guards="separate", lamps="pod", grille="mesh", bumper="channel", rim="sunraysia", rim_paint="white", roof_style="ribs",
                 mirror="arm", arches="square"),
    "old_wagon": dict(grille="egg", lamps="twin_square", bumper="channel", rim="sunraysia", side="moulding"),
    "nineties": dict(grille="wide", lamps="square", bumper="moulded", rim="spoke6", side="twotone", mirror="pod"),
    "modern": dict(grille="frame", lamps="wrap", bumper="moulded", rim="spoke6", side="cladding", mirror="pod", arches="square", tread="road"),
    "wide": dict(grille="mesh", lamps="wrap", bumper="sport", rim="beadlock", rim_paint="black", bonnet="vents", mirror="pod", arches="square"),
    "full": dict(grille="frame", lamps="stacked", bumper="sport", rim="spoke5", rim_paint="black", bonnet="bulge", mirror="arm", arches="square"),
    "square": dict(grille="slats", lamps="round", bumper="moulded", rim="spoke5", side="moulding", mirror="pod", arches="square"),
}
STYLE = {
    "dargo": dict(aerial=True),
    "dargo_tub": dict(grille="bars", lamps="square", bumper="channel", rim="sunraysia", rim_paint="white", side="stripe", tub_style="plain"),
    "omeo": dict(side="stripe", paint2=(0.95, 0.93, 0.85)),
    "omeo_tray": dict(grille="panel", bumper="tube", rim="sunraysia", rim_paint="white", tray_style="timber"),
    "licola": dict(grille="panel", tray_style="timber", bumper="blade"),
    "licola_top": dict(grille="slats", rim="slot8", rim_paint="steel", side="twotone", paint2=(0.92, 0.9, 0.84)),
    "tambo": dict(grille="egg", lamps="twin_square", bumper="blade", rim="slot8", side="moulding", aerial=True),
    "buckland": dict(grille="split", lamps="twin_round", bumper="channel", rim="spoke5", side="stripe", tub_style="sportsbar"),
    "bendoc": dict(grille="panel", lamps="round", bumper="blade", rim="slot8", side="plain", roof_style="plain"),
    "wonnangatta": dict(grille="wide", lamps="twin_round", bumper="tube", rim="sunraysia", rim_paint="white", tray_style="boxes"),
    "howitt": dict(spots=True, aerial=True),
    "howitt_ute": dict(grille="bars", lamps="square", tray_style="boxes", bar="plate"),
    "howitt_short": dict(grille="wide", lamps="twin_round", rim="spoke5", side="twotone", paint2=(0.75, 0.75, 0.72)),
    "jamieson": dict(tray_style="timber", bar="tube"),
    "jamieson_troop": dict(grille="panel", rim="slot8", rim_paint="steel", bumper="tube"),
    "mitta": dict(grille="split", lamps="twin_square", rim="mesh"),
    "mitta_short": dict(grille="bars", lamps="wide", rim="spoke5", side="stripe"),
    "delatite": dict(grille="egg", lamps="square", rim="slot8", side="moulding", tub_style="tonneau"),
    "delatite_tray": dict(grille="slats", lamps="twin_round", rim="sunraysia", rim_paint="white", side="plain", bumper="channel"),
    "feathertop": dict(grille="wide", lamps="wide", rim="spoke6", side="twotone", paint2=(0.62, 0.6, 0.52), mirror="pod"),
    "ovens": dict(tub_style="sportsbar"),
    "ovens_tray": dict(grille="bars", bumper="channel", rim="slot8", rim_paint="steel", side="plain", tray_style="alloy"),
    "ovens_extra": dict(grille="mesh", rim="spoke5", tub_style="tonneau"),
    "kiewa": dict(grille="slats", lamps="wide", rim="mesh", side="moulding"),
    "buffalo": dict(grille="split", lamps="stacked", bumper="sport", rim="spoke5", rim_paint="black", bonnet="bulge", tub_style="plain"),
    "buffalo_tray": dict(grille="split", lamps="stacked", bumper="plate", rim="slot8", rim_paint="black", bonnet="bulge", tray_style="boxes"),
    "hotham": dict(grille="wide", lamps="wrap", rim="spoke5", side="cladding"),
    "tumut": dict(grille="bars", lamps="round", rim="spoke5", rim_paint="black", side="cladding", roof_style="gutter", arches="square"),
    "walhalla": dict(grille="mesh", lamps="wrap", rim="mesh", side="plain", arches="round", tread="road"),
    "ovens_cab": dict(grille="egg", rim="sunraysia", rim_paint="black", tray_style="boxes", side="plain"),
    "jamieson_v8": dict(bar="plate", spots=True, rim="beadlock", rim_paint="black", tread="mud", tray_style="boxes", bonnet="scoop", aerial=True),
    "jamieson_v8_single": dict(bar="tube", rim="sunraysia", rim_paint="black", tread="mud", tray_style="timber", bonnet="scoop"),
    "jamieson_troop_v8": dict(grille="panel", bar="plate", spots=True, rim="beadlock", rim_paint="bronze", tread="mud", bonnet="scoop"),
    "bogong": dict(tub_style="sportsbar", side="stripe", paint2=(0.02, 0.02, 0.02)),
    "bogong_wagon": dict(grille="frame", rim="mesh", rim_paint="bronze", bonnet="bulge"),
    "ovens_tourer": dict(bar="plate", spots=True, rim="beadlock", rim_paint="black", tread="mud", tub_style="canopy", aerial=True),
    "feathertop_gx": dict(grille="wide", lamps="wrap", bar="nudge", rim="spoke6", rim_paint="black", side="moulding"),
    "howitt_y": dict(grille="egg", lamps="wide", rim="mesh", bonnet="bulge", side="plain"),
    "buffalo_chase": dict(grille="split", lamps="stacked", bar="tube", spots=True, rim="beadlock", rim_paint="bronze", tread="mud",
                          bonnet="bulge", tray_style="boxes", aerial=True),
    "tumut_built": dict(grille="bars", lamps="round", bar="nudge", spots=True, rim="spoke5", rim_paint="white", tread="mud", arches="square"),
    "kestrel": dict(tub_style="sportsbar"),
    "kestrel_xl": dict(grille="slats", lamps="twin_square", bumper="channel", rim="spoke6", rim_paint="steel", bonnet="flat", side="moulding"),
    "kestrel_tray": dict(grille="mesh", bar="plate", spots=True, rim="slot8", rim_paint="white", tread="mud", tray_style="boxes"),
    "cobbler": dict(aerial=False),
    "cobbler_ute": dict(grille="bars", rim="beadlock", rim_paint="black", tub_style="tonneau", tread="mud"),
    "stirling": dict(grille="panel", lamps="square", rim="spoke6", side="cladding"),
    "stirling_short": dict(grille="panel", lamps="twin_round", rim="slot8", rim_paint="white", side="twotone", paint2=(0.92, 0.92, 0.9)),
    "feathertop_300": dict(grille="frame", lamps="wide", rim="mesh", side="moulding", bonnet="bulge"),
    "wellington": dict(grille="egg", lamps="twin_square", bumper="plate", rim="spoke6", bonnet="scoop", tub_style="tonneau"),
    "kosciuszko": dict(grille="wide", lamps="stacked", bumper="moulded", rim="mesh", bonnet="flat", side="moulding"),
}

GRADES = {1: "Paddock bashers", 2: "Old tourers", 3: "Modern workhorses", 4: "Built tourers", 5: "The top end"}

# The year each was made. A truck is of its time: its shape, and every piece on it.
YEARS = dict(
    dargo=1984, dargo_tub=1987, omeo=1988, omeo_tray=1983, licola=1976, licola_top=1979, tambo=1986, buckland=1989, bendoc=1981,
    wonnangatta=1978,
    howitt=1992, howitt_ute=1994, howitt_short=1990, jamieson=1996, jamieson_troop=1999, mitta=1995, mitta_short=1993, delatite=1998,
    delatite_tray=1997, feathertop=1996,
    ovens=2014, ovens_tray=2007, ovens_extra=2016, kiewa=2010, buffalo=2018, buffalo_tray=2017, hotham=2009, tumut=2019, walhalla=2005,
    ovens_cab=2012,
    jamieson_v8=2016, jamieson_v8_single=2012, jamieson_troop_v8=2020, bogong=2021, bogong_wagon=2022, ovens_tourer=2019,
    feathertop_gx=2015, howitt_y=2013, buffalo_chase=2020, tumut_built=2021,
    kestrel=2024, kestrel_xl=2022, kestrel_tray=2023, cobbler=2021, cobbler_ute=2023, stirling=2022, stirling_short=2024,
    feathertop_300=2023, wellington=2024, kosciuszko=2025,
)


def era_of(year):
    return ("seventies" if year < 1980 else "eighties" if year < 1990 else "nineties" if year < 2000 else "noughties" if year < 2010
            else "tens" if year < 2020 else "twenties")


# How a body was drawn in each time. The seventies: flat panels, an upright screen. The
# eighties: a box with its edges eased. The nineties and after: rubbed round, the screen
# laid back. The tens: a high belt and little glass. The twenties: square again, and tall
# in the nose. (A kind that is flat-panelled on purpose keeps its own shape through all of it.)
ERA_SHAPE = {
    "seventies": dict(round=0.032, screen=0.16, tumble=0.04, crown=0.012, tuck=0.015),
    "eighties": dict(round=0.042, screen=0.36, tumble=0.075, crown=0.02, tuck=0.025),
    "nineties": dict(round=0.085, screen=0.52, tumble=0.11, crown=0.035, tuck=0.04),
    "noughties": dict(round=0.105, screen=0.62, tumble=0.13, crown=0.04, tuck=0.045, belt_add=0.02),
    "tens": dict(round=0.08, screen=0.64, tumble=0.13, crown=0.03, tuck=0.035, belt_add=0.05, bonnet_add=0.03),
    "twenties": dict(round=0.06, screen=0.52, tumble=0.1, crown=0.02, tuck=0.025, belt_add=0.07, bonnet_add=0.07),
}
KEEPS_SHAPE = ("flat", "square")

# The pieces each time made, to choose among. Round sealed lamps and chrome blades; then
# square lamps, stripes and white steel wheels; then lamps that wrap a corner and bumpers
# in the body's colour; then big chrome bars; then hexagons and strips of light; then tall
# faces with a slit of a lamp, and wheels in black.
POOLS = {
    "seventies": dict(grille=["slats", "panel", "bars", "split"], lamps=["round", "twin_round"], bumper=["blade", "channel"],
                      rim=["slot8", "sunraysia"], rim_paint=["white", "steel"], side=["plain", "stripe"], mirror=["stalk"],
                      roof_style=["gutter"], tail=["small"]),
    "eighties": dict(grille=["egg", "bars", "slats", "split", "panel"], lamps=["square", "twin_square", "round", "twin_round"],
                     bumper=["blade", "channel", "tube"], rim=["sunraysia", "slot8", "spoke5"], rim_paint=["white", "steel"],
                     side=["stripe", "moulding", "plain", "twotone"], mirror=["stalk", "arm"], roof_style=["gutter", "plain"],
                     tail=["small", "tall"]),
    "nineties": dict(grille=["wide", "egg", "split", "slats"], lamps=["corner", "wide", "twin_square", "square"], bumper=["moulded", "channel"],
                     rim=["spoke6", "spoke5", "sunraysia", "mesh"], rim_paint=["steel", "white"], side=["twotone", "moulding", "cladding"],
                     mirror=["pod", "stalk"], roof_style=["plain"], tail=["tall", "wrap"]),
    "noughties": dict(grille=["wide", "split", "egg", "frame"], lamps=["wrap", "corner", "wide"], bumper=["moulded", "sport"],
                      rim=["spoke6", "spoke5", "mesh"], rim_paint=["steel"], side=["plain", "moulding", "cladding"], mirror=["pod"],
                      roof_style=["plain"], tail=["wrap", "tall"]),
    "tens": dict(grille=["hex", "frame", "mesh", "split"], lamps=["wrap", "slim", "stacked"], bumper=["sport", "moulded", "plate"],
                 rim=["mesh", "spoke5", "spoke6", "beadlock"], rim_paint=["steel", "black"], side=["cladding", "plain"], mirror=["pod", "arm"],
                 roof_style=["plain"], tail=["wrap", "bar"]),
    "twenties": dict(grille=["letter", "honey", "hex", "frame", "mesh"], lamps=["slim", "c_clamp", "halo", "stacked"],
                     bumper=["sport", "plate", "moulded"], rim=["beadlock", "spoke5", "mesh", "spoke6"], rim_paint=["black", "bronze", "steel"],
                     side=["cladding", "plain"], mirror=["pod", "arm"], roof_style=["plain"], tail=["bar", "wrap"]),
}
STEPS = dict(grille=1, lamps=1, bumper=2, rim=3, rim_paint=1, side=2, mirror=1, roof_style=1, tail=1)
# What a truck is known by, whatever its time.
FIXED = {
    "cobbler": dict(lamps="halo", grille="slats"), "cobbler_ute": dict(lamps="halo", grille="bars"),
    "stirling": dict(lamps="halo", grille="panel"), "stirling_short": dict(lamps="halo", grille="panel"),
    "tumut": dict(lamps="round", grille="bars"), "tumut_built": dict(lamps="halo", grille="bars"),
    "kestrel": dict(lamps="c_clamp", grille="letter"), "wellington": dict(lamps="c_clamp", grille="frame"),
}


def line_up():
    made, seen = {}, {}
    for key, grade, called, kind, price, about, changes in LINE_UP:
        year = YEARS[key]
        era = era_of(year)
        truck = dict(KINDS[kind])
        if kind not in KEEPS_SHAPE:
            shape = ERA_SHAPE[era]
            truck.update({name: value for name, value in shape.items() if not name.endswith("_add")})
            truck["belt"] += shape.get("belt_add", 0.0)
            truck["bonnet_front"] += shape.get("bonnet_add", 0.0)
            truck["bonnet_back"] += shape.get("bonnet_add", 0.0)
        truck.update(KIND_STYLE[kind])
        # Each time's pieces go round the trucks of that time, so that no two have the same set.
        turn = seen.setdefault(era, 0)
        seen[era] += 1
        for part, options in POOLS[era].items():
            truck[part] = options[(turn * STEPS[part] + len(part)) % len(options)]
        if kind == "flat":
            truck.update(guards="separate", lamps="pod" if year < 2015 else "halo", roof_style="ribs", mirror="arm", arches="square")
        truck.update({name: value for name, value in changes.items() if name not in POOLS[era]})
        truck.update({name: value for name, value in STYLE.get(key, {}).items() if name not in POOLS[era]})
        truck.update(FIXED.get(key, {}))
        truck.update(grade=grade, called=called, kind=kind, price=price, about=about, year=year, era=era)
        made[key] = truck
    return made


TRUCKS = line_up()

# ============================================================================ Materials.
PAINTS = {
    # name: (colour, roughness, metal, glow)
    "paint": (None, 0.22, 0.25, None),
    "trim": ((0.03, 0.03, 0.032), 0.55, 0.0, None),          # black plastic and rubber
    "tyre": ((0.022, 0.022, 0.023), 0.85, 0.0, None),
    "steel": ((0.5, 0.5, 0.5), 0.35, 0.9, None),            # bare and plated: bumpers, rims
    "alloy": ((0.62, 0.63, 0.64), 0.42, 0.8, None),         # trays and racks
    "iron": ((0.06, 0.06, 0.065), 0.6, 0.6, None),          # chassis, axles, springs
    "glass": ((0.012, 0.016, 0.018), 0.06, 0.0, None),
    "lamp": ((0.86, 0.88, 0.9), 0.1, 0.55, (1.0, 0.96, 0.85)),   # a lamp's glass, with its mirror behind it
    "lens_line": ((0.42, 0.44, 0.47), 0.2, 0.5, None),       # the flutes in a lamp's glass
    "argent": ((0.5, 0.51, 0.52), 0.45, 0.5, None),          # the silver paint of an old grille panel
    "badge": ((0.04, 0.04, 0.045), 0.3, 0.3, None),
    "lamp_tail": ((0.5, 0.02, 0.02), 0.2, 0.0, (1.0, 0.05, 0.03)),
    "lamp_amber": ((0.8, 0.35, 0.02), 0.2, 0.0, None),
    "inside_dark": ((0.02, 0.02, 0.022), 0.8, 0.0, None),    # what shows through an arch
    "inside_trim": ((0.13, 0.13, 0.135), 0.7, 0.0, None),   # a cab's lining, its dash, its door cards
    "inside_seat": ((0.32, 0.3, 0.27), 0.92, 0.0, None),   # its seats
    "plate": ((0.85, 0.82, 0.3), 0.5, 0.0, None),            # a number plate
    "chrome": ((0.78, 0.78, 0.78), 0.12, 1.0, None),
    "rim_white": ((0.82, 0.82, 0.78), 0.45, 0.1, None),
    "rim_black": ((0.03, 0.03, 0.03), 0.4, 0.4, None),
    "rim_bronze": ((0.35, 0.22, 0.1), 0.35, 0.8, None),
    "wood": ((0.36, 0.24, 0.13), 0.8, 0.0, None),
    "stripe": ("second", 0.3, 0.2, None),                   # the truck's second colour
    "hook": ((0.7, 0.05, 0.03), 0.5, 0.2, None),             # a recovery point
    "shock_yellow": ((0.75, 0.55, 0.03), 0.4, 0.2, None),    # a heavier shock absorber
    "shock_red": ((0.6, 0.04, 0.03), 0.4, 0.2, None),        # a lift kit's
    "shock_blue": ((0.03, 0.14, 0.55), 0.4, 0.3, None),      # a long-travel one's
}


SECOND = [(0.9, 0.88, 0.8)]


def material(name, paint):
    made = bpy.data.materials.get(name)
    if made:
        return made
    colour, rough, metal, glow = PAINTS[name]
    if colour == "second":
        colour = SECOND[0]
    made = bpy.data.materials.new(name)
    made.use_nodes = True
    shader = made.node_tree.nodes["Principled BSDF"]
    shader.inputs["Base Color"].default_value = (*(colour or paint), 1.0)
    shader.inputs["Roughness"].default_value = rough
    shader.inputs["Metallic"].default_value = metal
    if glow:
        shader.inputs["Emission Color"].default_value = (*glow, 1.0)
        shader.inputs["Emission Strength"].default_value = 1.0
    if name == "glass":
        # Seen through: a third of what is behind it is the glass, tinted, and the rest shows.
        shader.inputs["Alpha"].default_value = GLASS
        try:
            made.surface_render_method = "BLENDED"
        except AttributeError:
            made.blend_method = "BLEND"
    return made


GLASS = 0.18


def clone(piece, name):
    """A piece just like another, under a name of its own."""
    made = Piece(name)
    made.bm.free()
    made.bm, made.materials = piece.bm.copy(), list(piece.materials)
    return made


def merge(piece, other):
    """Brings all of another piece into this one, each face keeping its material."""
    made = {}
    for vert in other.bm.verts:
        made[vert] = piece.bm.verts.new(vert.co)
    for face in other.bm.faces:
        try:
            new = piece.bm.faces.new([made[vert] for vert in face.verts])
        except ValueError:
            continue
        new.material_index = piece.slot(other.materials[face.material_index])
        new.smooth = True
    other.bm.free()


def carve(piece, cutter, paint, how="DIFFERENCE"):
    """Cuts one piece out of another, or (`INTERSECT`) keeps only what of it is within the
    cutter. What the cutter leaves bare is of the cutter's material. The cutter is used up."""
    def stood(source):
        mesh = bpy.data.meshes.new("carve")
        source.bm.to_mesh(mesh)
        for name in source.materials:
            mesh.materials.append(material(name, paint))
        made = bpy.data.objects.new("carve", mesh)
        bpy.context.scene.collection.objects.link(made)
        return made

    body, tool = stood(piece), stood(cutter)
    cut_out = body.modifiers.new("cut", "BOOLEAN")
    cut_out.object, cut_out.operation, cut_out.solver = tool, how, "EXACT"
    cut_out.material_mode = "TRANSFER"
    bpy.context.view_layer.update()
    mesh = bpy.data.meshes.new_from_object(body.evaluated_get(bpy.context.evaluated_depsgraph_get()))
    piece.bm.clear()
    piece.bm.from_mesh(mesh)
    piece.materials = [made.name for made in mesh.materials]
    for each in (body, tool):
        old = each.data
        bpy.data.objects.remove(each, do_unlink=True)
        bpy.data.meshes.remove(old)
    bpy.data.meshes.remove(mesh)
    cutter.bm.free()


# ============================================================================ Making shapes.
class Piece:
    """One piece of a truck: a mesh of its own, of several materials."""

    plain = False        # made with its edges as they are to be, and not eased

    def __init__(self, name):
        self.name = name
        self.bm = bmesh.new()
        self.materials = []

    def slot(self, name):
        if name not in self.materials:
            self.materials.append(name)
        return self.materials.index(name)

    def take(self, other, name, move=None):
        """Brings a shape made apart into this piece, of one material."""
        slot = self.slot(name)
        made = {}
        for vert in other.verts:
            made[vert] = self.bm.verts.new(move @ vert.co if move else vert.co)
        for face in other.faces:
            try:
                new = self.bm.faces.new([made[vert] for vert in face.verts])
            except ValueError:
                continue
            new.material_index = slot
            new.smooth = True
        other.free()

    def prism(self, name, outline, x0, x1, round=0.0, shape=None, grid=0.0):
        """An outline in y and z, drawn across from x0 to x1, its two ends' edges rounded.
        `shape` then moves each point: a body is narrower at its roof and its nose, and
        its panels are not flat. Cut into a `grid` first, it has points to bend at."""
        def make():
            bm = bmesh.new()
            near = [bm.verts.new((x0, y, z)) for y, z in outline]
            far = [bm.verts.new((x1, y, z)) for y, z in outline]
            count = len(outline)
            ends = []
            try:
                ends.append(bm.faces.new(near))
                ends.append(bm.faces.new(list(reversed(far))))
            except ValueError:
                pass
            for index in range(count):
                after = (index + 1) % count
                bm.faces.new([near[after], near[index], far[index], far[after]])
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            return bm, [edge for face in ends for edge in face.edges]

        bm = eased(make, round, 4)
        if grid > 0.0:
            cut(bm, grid)
        if shape:
            for vert in bm.verts:
                vert.co = shape(vert.co)
        self.take(bm, name)

    def box(self, name, low, high, round=0.0, move=None, steps=2):
        bm = bmesh.new()
        made = bmesh.ops.create_cube(bm, size=1.0)["verts"]
        size = Vector(high) - Vector(low)
        middle = (Vector(high) + Vector(low)) * 0.5
        for vert in made:
            vert.co = Vector((vert.co.x * size.x, vert.co.y * size.y, vert.co.z * size.z)) + middle
        if round > 0.0:
            bmesh.ops.bevel(bm, geom=bm.edges[:], offset=min(round, min(size) * 0.45), segments=steps, profile=0.5, affect="EDGES")
        self.take(bm, name, move)

    def rod(self, name, a, b, radius, around=14):
        """A round bar from a to b."""
        a, b = Vector(a), Vector(b)
        along = b - a
        if along.length < 1e-6:
            return
        bm = bmesh.new()
        bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=around, radius1=radius, radius2=radius, depth=along.length)
        turn = Vector((0, 0, 1)).rotation_difference(along.normalized()).to_matrix().to_4x4()
        self.take(bm, name, Matrix.Translation((a + b) * 0.5) @ turn)

    def tube(self, name, points, radius, around=12, joints=True):
        """A bar bent through points: a bull bar, an exhaust."""
        for a, b in zip(points, points[1:]):
            self.rod(name, a, b, radius, around)
        for point in points[1:-1] if joints else ():
            bm = bmesh.new()
            bmesh.ops.create_uvsphere(bm, u_segments=around, v_segments=8, radius=radius)
            self.take(bm, name, Matrix.Translation(point))

    def turned(self, name, profile, at, axis="x", around=40, ends=True):
        """A profile of (along the axis, radius) turned about the axis: a wheel, a lamp."""
        bm = bmesh.new()
        rings = []
        for along, radius in profile:
            ring = []
            for step in range(around):
                angle = math.tau * step / around
                if axis == "x":
                    ring.append(bm.verts.new((along, radius * math.cos(angle), radius * math.sin(angle))))
                else:
                    ring.append(bm.verts.new((radius * math.cos(angle), along, radius * math.sin(angle))))
            rings.append(ring)
        for a, b in zip(rings, rings[1:]):
            for step in range(around):
                after = (step + 1) % around
                bm.faces.new([a[step], a[after], b[after], b[step]])
        for ring in (rings[0], rings[-1]) if ends else ():
            try:
                bm.faces.new(ring)
            except ValueError:
                pass
        bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-5)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        self.take(bm, name, Matrix.Translation(at))

    def slab(self, name, outline, y0, y1, round=0.0):
        """An outline in x and z, drawn along the truck from y0 to y1, the edges of its far
        end rounded: what is on a truck's face is made of these."""
        def make():
            bm = bmesh.new()
            near = [bm.verts.new((x, y0, z)) for x, z in outline]
            far = [bm.verts.new((x, y1, z)) for x, z in outline]
            count = len(outline)
            ends = []
            try:
                ends.append(bm.faces.new(near))
                ends.append(bm.faces.new(list(reversed(far))))
            except ValueError:
                pass
            for index in range(count):
                after = (index + 1) % count
                try:
                    bm.faces.new([near[after], near[index], far[index], far[after]])
                except ValueError:
                    pass
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            return bm, (list(ends[1].edges) if len(ends) == 2 else [])

        self.take(eased(make, round, 2), name)

    def frame(self, name, outline, y0, y1, thick):
        """A frame round an outline in x and z: a band `thick` wide inside its edge."""
        inner = inset(outline, thick)
        count = len(outline)
        for index in range(count):
            after = (index + 1) % count
            self.slab(name, [outline[index], outline[after], inner[after], inner[index]], y0, y1)

    def hull(self, name, points):
        """The least shape that holds some points: a block through a wall, to cut a window."""
        bm = bmesh.new()
        verts = [bm.verts.new(point) for point in points]
        bmesh.ops.convex_hull(bm, input=verts)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        self.take(bm, name)

    def plate(self, name, corners, thick):
        """A flat piece through corners given in order, `thick` along its own facing: glass,
        a door's skin."""
        bm = bmesh.new()
        front = [bm.verts.new(corner) for corner in corners]
        face = bm.faces.new(front)
        face.normal_update()
        out = bmesh.ops.extrude_face_region(bm, geom=[face])
        moved = [item for item in out["geom"] if isinstance(item, bmesh.types.BMVert)]
        bmesh.ops.translate(bm, verts=moved, vec=face.normal * thick)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        self.take(bm, name)

    def finish(self, paint, origin=(0.0, 0.0, 0.0)):
        """Makes the piece an object. One made about its own middle, as a wheel is about its
        hub, is put at `origin`."""
        mesh = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for name in self.materials:
            mesh.materials.append(material(name, paint))
        made = bpy.data.objects.new(self.name, mesh)
        made.location = origin
        bpy.context.scene.collection.objects.link(made)
        if not self.plain:
            first = made.modifiers.new("slivers", "WELD")
            first.merge_threshold = 0.0006
            first.mode = "CONNECTED"       # within a shell: two shells that touch stay two
            ease = made.modifiers.new("ease", "BEVEL")
            ease.width, ease.segments = 0.004, 2
            ease.limit_method, ease.angle_limit = "ANGLE", math.radians(38)
            ease.harden_normals = True
        weld = made.modifiers.new("weld", "WELD")
        weld.merge_threshold = 0.0002
        weld.mode = "CONNECTED"
        even = made.modifiers.new("even", "WEIGHTED_NORMAL")
        even.keep_sharp = False
        return made


THROWN = [0]        # how many shapes had to be rounded again, with less


def eased(make, offset, segments):
    """Makes a shape, and rounds the edges it names. A round too big for a short edge beside
    it throws points out of the shape, as spikes: such a shape is made again, and rounded
    only as far as its edges allow."""
    bm, edges = make()
    if offset <= 0.0 or not edges:
        return bm
    reach = lambda: [(min(vert.co[axis] for vert in bm.verts), max(vert.co[axis] for vert in bm.verts)) for axis in range(3)]
    before = reach()
    bmesh.ops.bevel(bm, geom=edges, offset=offset, segments=segments, profile=0.5, affect="EDGES")
    if any(now[0] < was[0] - 1e-4 or now[1] > was[1] + 1e-4 for now, was in zip(reach(), before)):
        bm.free()
        bm, edges = make()
        bmesh.ops.bevel(bm, geom=edges, offset=offset, segments=segments, profile=0.5, affect="EDGES", clamp_overlap=True)
        THROWN[0] += 1
    return bm


def rounded(x0, z0, x1, z1, radius, steps=4):
    """A rectangle with its corners rounded, as an outline in x and z."""
    radius = min(radius, (x1 - x0) / 2 - 1e-4, (z1 - z0) / 2 - 1e-4)
    if radius < 5e-4:
        return [(x1, z1), (x0, z1), (x0, z0), (x1, z0)]
    points = []
    for corner, (cx, cz) in enumerate(((x1 - radius, z1 - radius), (x0 + radius, z1 - radius), (x0 + radius, z0 + radius), (x1 - radius, z0 + radius))):
        for step in range(steps + 1):
            angle = math.pi / 2 * (corner + step / steps)
            points.append((cx + radius * math.cos(angle), cz + radius * math.sin(angle)))
    return points


def oval(x, z, wide, tall, steps=24):
    return [(x + wide * math.cos(math.tau * step / steps), z + tall * math.sin(math.tau * step / steps)) for step in range(steps)]


def inset(outline, by):
    """An outline drawn in from its edge, towards its middle."""
    xs, zs = [x for x, _ in outline], [z for _, z in outline]
    cx, cz = (min(xs) + max(xs)) / 2, (min(zs) + max(zs)) / 2
    wide, tall = (max(xs) - min(xs)) / 2, (max(zs) - min(zs)) / 2
    keep_x, keep_z = max(1.0 - by / max(wide, 1e-4), 0.1), max(1.0 - by / max(tall, 1e-4), 0.1)
    return [(cx + (x - cx) * keep_x, cz + (z - cz) * keep_z) for x, z in outline]


def span(outline, at, axis):
    """How far an outline reaches across at a height (axis 1), or up at a place across
    (axis 0): the least and the greatest, or nothing where it is not."""
    found = []
    for index, a in enumerate(outline):
        b = outline[(index + 1) % len(outline)]
        if (a[axis] - at) * (b[axis] - at) <= 0.0 and a[axis] != b[axis]:
            share = (at - a[axis]) / (b[axis] - a[axis])
            found.append(a[1 - axis] + (b[1 - axis] - a[1 - axis]) * share)
    return (min(found), max(found)) if len(found) >= 2 else None


def cut(bm, grid, axes=(0, 1, 2)):
    """Cuts a shape through with planes a `grid` apart, so that it can be bent."""
    for axis in axes:
        low = min(vert.co[axis] for vert in bm.verts)
        high = max(vert.co[axis] for vert in bm.verts)
        count = int((high - low) / grid)
        for step in range(1, count + 1):
            at, way = Vector((0.0, 0.0, 0.0)), Vector((0.0, 0.0, 0.0))
            at[axis] = low + (high - low) * step / (count + 1)
            way[axis] = 1.0
            bmesh.ops.bisect_plane(bm, geom=bm.verts[:] + bm.edges[:] + bm.faces[:], plane_co=at, plane_no=way, dist=1e-5)


def arch(y, z, radius, floor, power=2.0):
    """The points of a wheel arch about (y, z), from its back to its front, above `floor`:
    round, or with `power` above 2 squarer at its shoulders."""
    reach = max(min((floor - z) / radius, 0.9), -0.9)
    low = math.asin(math.copysign(abs(reach) ** (power / 2), reach))
    steps = 22
    points = []
    for step in range(steps + 1):
        angle = math.pi - low - (math.pi - 2 * low) * step / steps
        along, up = math.cos(angle), math.sin(angle)
        points.append((y + radius * math.copysign(abs(along) ** (2 / power), along), z + radius * math.copysign(abs(up) ** (2 / power), up)))
    return points


# How far apart a body is cut to be bent: a hand and a half.
GRID = 0.13


# ============================================================================ A truck.
def build(name):
    t = dict(TRUCKS[name])
    lift = t["lift"]
    for height in ("sill", "belt", "roof", "bonnet_front", "bonnet_back", "tray_deck", "tub_top"):
        t[height] += lift
    front_axle, rear_axle = t["wheelbase"] / 2, -t["wheelbase"] / 2
    nose, tail = front_axle + t["front"], rear_axle - t["rear"]
    half = t["wide"] / 2
    hub = t["tyre"]
    gap = t["tyre"] + t["arch"]
    cowl = front_axle - max(t["cowl"], gap + 0.06)
    screen_top = cowl - t["screen"] * (t["roof"] - t["bonnet_back"])
    wagon = t["back"] == "wagon"
    cab_back = tail + 0.02 if wagon else cowl - t["cab_len"]
    roof_back = cab_back + t["back_glass"] + (0.0 if not wagon else 0.02)
    edge = t["round"]
    paint = t["paint"]
    pieces = {}
    hinges = {}         # a piece that swings: where its hinge is, about which way, how far
    holds = []          # a place gold can be put: its name, its two corners, the doors to it

    def piece(called):
        return pieces.setdefault(called, Piece(called))

    def side(y, z):
        """Half the body's width at a place along it and a height: widest at its belt,
        tucked under towards its sills, leaning in above to its roof, drawn in to its nose."""
        wide = half
        if z > t["belt"]:
            wide -= t["tumble"] * min((z - t["belt"]) / (t["roof"] - t["belt"]), 1.0) ** 1.25
        else:
            wide -= t["tuck"] * min((t["belt"] - z) / (t["belt"] - t["sill"]), 1.0) ** 2
        if y > cowl:
            wide -= t["nose_taper"] * ((y - cowl) / (nose - cowl)) ** 2
        if y < rear_axle:
            wide -= t["tail_taper"] * ((rear_axle - y) / (rear_axle - tail)) ** 2
        return wide

    def top(y):
        """How high the body is at a place along it: its bonnet, its screen, its roof."""
        if y >= cowl:
            return t["bonnet_back"] + (t["bonnet_front"] - t["bonnet_back"]) * (y - cowl) / (nose - cowl)
        if y >= screen_top:
            return t["bonnet_back"] + (t["roof"] - t["bonnet_back"]) * (cowl - y) / (cowl - screen_top)
        return t["roof"]

    def shaped(co):
        """Where a point of a body made square is in the body as it is: its sides curved, and
        its bonnet and its roof higher along the middle than at their edges."""
        across = co.x / half
        high = top(co.y)
        near_top = max(0.0, 1.0 - (high - co.z) / 0.35) ** 2
        # The windscreen is flat glass, and the back of the cab is: the crown fades out
        # towards each, along the bonnet and along the roof.
        if co.y >= cowl:
            along = min((co.y - cowl) / 0.2, 1.0)
        elif co.y <= screen_top:
            along = min((screen_top - co.y) / 0.3, 1.0) * min(max(co.y - roof_back, 0.0) / 0.25, 1.0)
        else:
            along = 0.0
        along = along * along * (3.0 - 2.0 * along)
        crown = t["crown"] * (1.0 - across * across) * near_top * along
        return Vector((across * side(co.y, co.z), co.y, co.z + crown))

    def mirrored(make):
        for sign, letter in ((-1.0, "L"), (1.0, "R")):
            make(sign, letter)

    SECOND[0] = t["paint2"]
    squared = 3.4 if t["arches"] == "square" else 2.0
    rim_metal = {"steel": "steel", "white": "rim_white", "black": "rim_black", "bronze": "rim_bronze"}[t["rim_paint"]]
    t["rim_metal"] = rim_metal

    # ---- The nose: its core, with the arches cut in it, and what is fitted to it.
    front_floor = t["sill"] + 0.06
    front_arch = arch(front_axle, hub + 0.02, gap, front_floor, squared)
    separate = t["guards"] == "separate"
    core = half - 0.25 if separate else half
    if separate:
        # A narrow bonnet between two guards, each a piece of its own, lower than it.
        outline = [(cowl, t["sill"] + 0.18), (cowl, t["bonnet_back"]), (nose - 0.04, t["bonnet_front"]), (nose, t["bonnet_front"] - 0.08),
                   (nose, front_floor + 0.14)]
        piece("nose").prism("paint", outline, -core, core, edge, shaped, GRID)
        guard_back, guard_front = t["bonnet_back"] - 0.15, t["bonnet_front"] - 0.17

        def guard(sign, letter):
            shape = [(cowl, t["sill"]), (cowl, guard_back), (nose - 0.2, guard_front), (nose - 0.05, guard_front - 0.1), (nose - 0.03, front_floor)]
            shape += [point for point in reversed(front_arch) if cowl + 0.01 < point[0] < nose - 0.05]
            piece("guard_" + letter).prism("paint", shape, sign * (core - 0.03), sign * half, max(edge, 0.05), shaped, GRID)

        mirrored(guard)
    else:
        outline = [(cowl, t["sill"]), (cowl, t["bonnet_back"]), (nose - 0.04, t["bonnet_front"]), (nose, t["bonnet_front"] - 0.1),
                   (nose, front_floor)]
        outline += [point for point in reversed(front_arch) if cowl + 0.01 < point[0] < nose - 0.02]
        piece("nose").prism("paint", outline, -half, half, edge, shaped, GRID)
    # What shows through an arch is dark.
    piece("nose").box("inside_dark", (-half + 0.14, front_axle - gap, t["sill"]), (half - 0.14, front_axle + gap, hub + gap - 0.02))

    # The bonnet: a lid let into the nose, a finger's width proud of it.
    lid_in = 0.035 if separate else 0.1
    lid_half = core - lid_in
    lid = [(cowl + 0.03, t["bonnet_back"] + 0.002), (nose - 0.09, t["bonnet_front"] + 0.002), (nose - 0.09, t["bonnet_front"] + 0.012),
           (cowl + 0.03, t["bonnet_back"] + 0.012)]

    def lid_shaped(co, raised=0.0):
        across = co.x / lid_half
        inner = side(co.y, co.z) - (half - core) - lid_in
        along = min(max(co.y - cowl, 0.0) / 0.2, 1.0)
        crown = t["crown"] * (1.0 - (across * inner / half) ** 2) * along * along * (3.0 - 2.0 * along)
        return Vector((across * inner, co.y, co.z + crown + raised))

    bonnet = piece("bonnet")
    bonnet.prism("paint", lid, -lid_half, lid_half, 0.008, lid_shaped, GRID)
    lid_long = nose - cowl
    if t["bonnet"] == "bulge":
        hump = [(cowl + 0.05, t["bonnet_back"] + 0.01), (nose - 0.16, t["bonnet_front"] + 0.01), (nose - 0.2, t["bonnet_front"] + 0.05),
                (cowl + 0.05, t["bonnet_back"] + 0.055)]
        bonnet.prism("paint", hump, -lid_half * 0.5, lid_half * 0.5, 0.035, lid_shaped, GRID)
    elif t["bonnet"] == "scoop":
        z = t["bonnet_back"] + t["crown"]
        y = cowl + lid_long * 0.42
        bonnet.box("paint", (-0.2, y - 0.26, z - 0.01), (0.2, y + 0.26, z + 0.07), 0.03)
        bonnet.box("inside_dark", (-0.17, y + 0.245, z + 0.012), (0.17, y + 0.265, z + 0.055))
    elif t["bonnet"] == "vents":
        for sign in (-1, 1):
            y = cowl + lid_long * 0.45
            z = t["bonnet_back"] + (t["bonnet_front"] - t["bonnet_back"]) * 0.45 + t["crown"] * 0.7
            bonnet.box("trim", (sign * lid_half * 0.5 - 0.09, y - 0.16, z + 0.004), (sign * lid_half * 0.5 + 0.09, y + 0.16, z + 0.02), 0.008)
            for row in range(4):
                bonnet.box("inside_dark", (sign * lid_half * 0.5 - 0.07, y - 0.12 + row * 0.07, z + 0.016), (sign * lid_half * 0.5 + 0.07, y - 0.09 + row * 0.07, z + 0.024))

    # ---- The face: the grille, and the lamps beside it.
    face = nose + 0.004
    era = t["era"]
    olden = era in ("seventies", "eighties")
    mask_low, mask_high = front_floor + 0.16, t["bonnet_front"] - 0.13
    if mask_high - mask_low < 0.21:
        mask_low = mask_high - 0.21
    lamp_z = (mask_low + mask_high) / 2 + 0.005
    nose_half = side(nose, lamp_z) - (half - core)
    lamp_room = {"round": 0.25, "twin_round": 0.36, "square": 0.27, "twin_square": 0.24, "wide": 0.36, "corner": 0.37, "wrap": 0.36, "stacked": 0.21,
                 "slim": 0.36, "c_clamp": 0.25, "halo": 0.25, "pod": 0.03}[t["lamps"]]
    grille_half = nose_half - lamp_room - (0.0 if separate else 0.035)
    low, high = mask_low, mask_high
    middle = (low + high) / 2
    made = piece("grille")
    bright = "chrome" if t["grade"] != 4 and era != "twenties" else "trim"
    shell = "paint" if era in ("nineties", "twenties") else bright
    style = t["grille"]

    # An old truck's face is one panel, in silver or in black, with its lamps and its
    # grille both set in it.
    panel = ("argent", "trim", "paint")[t["year"] % 3]
    if olden:
        reach = nose_half - 0.015
        made.slab(panel, rounded(-reach, low - 0.025, reach, high + 0.025, 0.014), face - 0.004, face + 0.007, 0.003)

    # The grille's opening, as each time drew it: a plain rectangle; a low one with soft
    # corners; one wider at its top; a hexagon that reaches down into the bumper; a tall one.
    gh = grille_half
    if era == "noughties":
        opening = [(-gh, high + 0.01), (-gh * 0.84, low - 0.01), (gh * 0.84, low - 0.01), (gh, high + 0.01)]
    elif era == "tens":
        opening = [(-gh * 0.86, high + 0.015), (-gh, middle + 0.02), (-gh * 0.78, low - 0.09), (gh * 0.78, low - 0.09), (gh, middle + 0.02), (gh * 0.86, high + 0.015)]
    elif era == "twenties":
        opening = rounded(-gh, low - 0.1, gh, high + 0.025, 0.02)
    elif era == "nineties":
        opening = rounded(-gh, low + 0.035, gh, high - 0.005, 0.035)
    else:
        opening = rounded(-gh, low, gh, high, 0.012)
    rim_wide = {"seventies": 0.014, "eighties": 0.016, "nineties": 0.02, "noughties": 0.03, "tens": 0.026, "twenties": 0.03}[era]
    inner = inset(opening, rim_wide)
    lowest, highest = min(z for _, z in inner), max(z for _, z in inner)
    made.slab("paint" if style == "panel" else "inside_dark", inset(opening, 0.004), face - 0.004, face + 0.008)
    made.frame(shell, opening, face - 0.002, face + 0.034, rim_wide)

    def across(count, metal, thick, deep, x_from=-9.0, x_to=9.0, edge=0.012):
        """Bars across the opening, each as long as the opening is wide where it lies."""
        for row in range(count):
            z = lowest + (highest - lowest) * (row + 1) / (count + 1)
            reach = span(inner, z, 1)
            if reach:
                x0, x1 = max(reach[0] + 0.004, x_from), min(reach[1] - 0.004, x_to)
                if x1 - x0 > 0.03:
                    fine = thick < 0.004
                    made.slab(metal, rounded(x0, z - thick, x1, z + thick, 0.0 if fine else min(thick * 0.8, edge)), face + 0.006, face + deep, 0.0 if fine else min(thick * 0.6, 0.004))

    def upright(count, metal, thick, deep):
        for column in range(count):
            x = -gh + 2 * gh * (column + 1) / (count + 1)
            reach = span(inner, x, 0)
            if reach and reach[1] - reach[0] > 0.03:
                fine = thick < 0.004
                made.slab(metal, rounded(x - thick, reach[0] + 0.004, x + thick, reach[1] - 0.004, 0.0 if fine else thick * 0.8), face + 0.006, face + deep, 0.0 if fine else min(thick * 0.6, 0.004))

    def mesh(fine=1.0):
        across(max(4, int((highest - lowest) / (0.024 * fine))), "trim", 0.0028, 0.018)
        upright(max(8, int(2 * gh / (0.03 * fine))), "trim", 0.0028, 0.018)

    if style == "slats":
        across(5 if highest - lowest > 0.19 else 4, bright, 0.011, 0.026)
    elif style == "bars":
        upright(9 if gh > 0.4 else 7, bright, 0.012, 0.028)
    elif style in ("mesh", "hex", "honey"):
        mesh(1.25 if style == "honey" else 1.0)
    elif style == "egg":
        across(max(3, int((highest - lowest) / 0.05)), bright, 0.005, 0.026)
        upright(max(6, int(2 * gh / 0.065)), bright, 0.005, 0.026)
    elif style == "split":
        made.slab(shell, rounded(-0.05, lowest - 0.01, 0.05, highest + 0.01, 0.012), face, face + 0.036, 0.006)
        across(4, bright, 0.01, 0.026, x_to=-0.055)
        across(4, bright, 0.01, 0.026, x_from=0.055)
    elif style == "panel":
        # A pressed panel in the truck's own colour, with slots punched in it.
        slots = 9 if gh > 0.4 else 7
        for column in range(slots):
            x = -gh + 2 * gh * (column + 1) / (slots + 1)
            reach = span(inner, x, 0)
            if reach:
                made.slab("inside_dark", rounded(x - 0.02, reach[0] + 0.02, x + 0.02, reach[1] - 0.02, 0.018), face + 0.006, face + 0.011)
    elif style == "wide":
        mesh(1.6)
        reach = span(inner, middle, 1) or (-gh, gh)
        made.slab(bright, rounded(reach[0] + 0.004, middle - 0.03, reach[1] - 0.004, middle + 0.03, 0.02), face + 0.006, face + 0.04, 0.008)
    elif style == "letter":
        # Its name across it, on a broad bar in its own colour.
        mesh(1.4)
        bar_z = middle + 0.02
        reach = span(inner, bar_z, 1) or (-gh, gh)
        made.slab("paint", rounded(reach[0] + 0.004, bar_z - 0.045, reach[1] - 0.004, bar_z + 0.045, 0.012), face + 0.006, face + 0.04, 0.006)
        letters = 7
        for index in range(letters):
            x = (index - (letters - 1) / 2) * 0.064
            made.slab(bright, rounded(x - 0.021, bar_z - 0.026, x + 0.021, bar_z + 0.026, 0.006), face + 0.038, face + 0.047, 0.002)
    else:
        # A heavy frame round a mesh, with bars across it.
        mesh()
        across(2, bright, 0.014, 0.036)
    # Its badge: an oval in the middle of a newer grille, a row of letters over an older one.
    if style not in ("letter", "split"):
        size = 0.75 if olden else 1.0
        at = (lowest + highest) / 2
        maker = {"old_ute": "shield", "nineties": "shield", "modern": "shield", "tiny": "roundel", "flat": "diamond", "old_wagon": "bar",
                 "wide": "hexagon", "full": "hexagon", "square": "roundel"}[t["kind"]]

        def emblem(grow):
            wide, tall = 0.06 * size + grow, 0.05 * size + grow
            if maker == "shield":
                return [(-wide, at + tall), (wide, at + tall), (wide, at - tall * 0.2), (0.0, at - tall * 1.15), (-wide, at - tall * 0.2)]
            if maker == "diamond":
                return [(0.0, at + tall * 1.15), (wide * 1.25, at), (0.0, at - tall * 1.15), (-wide * 1.25, at)]
            if maker == "roundel":
                return oval(0.0, at, tall * 1.05, tall * 1.05)
            if maker == "bar":
                return rounded(-wide * 1.6, at - tall * 0.5, wide * 1.6, at + tall * 0.5, 0.01)
            return [(wide * 1.15 * math.cos(math.tau * step / 6), at + tall * 1.15 * math.sin(math.tau * step / 6)) for step in range(6)]

        made.slab(bright, emblem(0.012), face + 0.02, face + 0.044, 0.004)
        made.slab("badge", emblem(0.0), face + 0.04, face + 0.0475, 0.002)

    # ---- Lamps. Each is made as a lamp is: a bezel, a glass with its flutes, a mirror
    # behind; or, newer, a dark housing with a lens like an eye in it and strips of light.
    def dome(lamp, x, z, radius, ring=None):
        """A round sealed lamp: a bezel, and a domed glass with flutes pressed down it."""
        ring = ring or bright
        lamp.turned(ring, [(0.0, radius + 0.022), (0.016, radius + 0.021), (0.027, radius + 0.011), (0.024, radius + 0.001)], (x, face, z), "y", 36, False)
        lamp.turned("lamp", [(0.022, radius), (0.031, radius * 0.82), (0.037, radius * 0.48), (0.0395, 0.0)], (x, face, z), "y", 36, False)
        for column in range(-3, 4):
            off = column * radius * 0.25
            tall = math.sqrt(max((radius * 0.93) ** 2 - off * off, 0.0))
            for part in range(6):
                z0, z1 = -tall + 2 * tall * part / 6, -tall + 2 * tall * (part + 1) / 6
                where = math.hypot(off, (z0 + z1) / 2) / radius
                out = 0.022 + 0.0175 * (1.0 - where * where)
                lamp.box("lens_line", (x + off - 0.0022, face + out - 0.004, z + z0), (x + off + 0.0022, face + out + 0.0012, z + z1))

    def sealed(lamp, x, z, wide, tall, ring=None):
        """A rectangular sealed lamp: a frame, and a glass with flutes down it."""
        lamp.slab(ring or bright, rounded(x - wide - 0.016, z - tall - 0.016, x + wide + 0.016, z + tall + 0.016, 0.014), face - 0.002, face + 0.024, 0.006)
        lamp.slab("lamp", rounded(x - wide, z - tall, x + wide, z + tall, 0.01), face + 0.012, face + 0.031, 0.007)
        count = max(3, int(wide / 0.022))
        for column in range(-count, count + 1):
            off = column * wide / (count + 0.8)
            lamp.box("lens_line", (x + off - 0.0018, face + 0.029, z - tall + 0.012), (x + off + 0.0018, face + 0.0322, z + tall - 0.012))

    def housing(lamp, outline, called="trim"):
        lamp.slab(called, outline, face - 0.002, face + 0.016, 0.005)

    def eye(lamp, x, z, radius):
        """A projector: a ring, and a lens like an eye."""
        lamp.turned(bright if bright != "trim" else "steel", [(0.012, radius + 0.012), (0.026, radius + 0.01), (0.03, radius + 0.002)], (x, face, z), "y", 24, False)
        lamp.turned("lamp", [(0.022, radius), (0.034, radius * 0.7), (0.038, 0.0)], (x, face, z), "y", 24, False)

    def mirror_box(lamp, x0, z0, x1, z1):
        """A reflector behind flat glass: a silver box with a small lamp in it."""
        lamp.slab("lamp", rounded(x0, z0, x1, z1, 0.008), face + 0.01, face + 0.022, 0.004)
        count = max(2, int((x1 - x0) / 0.03))
        for column in range(1, count):
            x = x0 + (x1 - x0) * column / count
            lamp.box("lens_line", (x - 0.0015, face + 0.02, z0 + 0.008), (x + 0.0015, face + 0.0235, z1 - 0.008))

    def light(lamp, x0, z0, x1, z1):
        """A strip of light."""
        lamp.slab("lamp", rounded(min(x0, x1), min(z0, z1), max(x0, x1), max(z0, z1), 0.004), face + 0.012, face + 0.024, 0.003)

    def flasher(amber, x0, z0, x1, z1, framed=True):
        x0, x1 = min(x0, x1), max(x0, x1)
        if framed:
            amber.slab(bright, rounded(x0 - 0.008, z0 - 0.008, x1 + 0.008, z1 + 0.008, 0.008), face - 0.002, face + 0.016, 0.004)
        amber.slab("lamp_amber", rounded(x0, z0, x1, z1, 0.006), face + 0.008, face + 0.022, 0.005)

    def head_lamp(sign, letter):
        lamp = piece("lamp_head_" + letter)
        amber = piece("indicator_" + letter)
        outer = side(nose, lamp_z)
        x = sign * (grille_half + (nose_half - grille_half) * 0.5)
        inner_x, outer_x = sign * (grille_half + 0.025), sign * (nose_half - 0.02)
        kind = t["lamps"]
        ring = None if era != "twenties" else "trim"
        if kind == "pod" or (kind == "halo" and separate):
            # On the front of each guard, with its flasher on top of the guard.
            px, pz = sign * (core + (half - core) * 0.45), t["bonnet_front"] - 0.33
            shift = Vector((0.0, -0.034, 0.0))
            dome(lamp, px, pz, 0.09, ring)
            if kind == "halo":
                lamp.tube("lamp", [(px + 0.103 * math.cos(math.tau * step / 20), face + 0.026, pz + 0.103 * math.sin(math.tau * step / 20)) for step in range(21)], 0.006, 6)
            bmesh.ops.translate(lamp.bm, verts=lamp.bm.verts[:], vec=shift)
            amber.turned(bright, [(0.0, 0.04), (0.02, 0.04)], (px, nose - 0.2, t["bonnet_front"] - 0.168), "y", 18)
            amber.turned("lamp_amber", [(0.02, 0.034), (0.045, 0.03), (0.055, 0.0)], (px, nose - 0.2, t["bonnet_front"] - 0.168), "y", 18, False)
            return
        if kind == "round":
            dome(lamp, x, lamp_z, 0.09, ring)
            flasher(amber, x - 0.05, lamp_z - 0.165, x + 0.05, lamp_z - 0.125)
        elif kind == "twin_round":
            for step in (-1, 1):
                dome(lamp, x + step * 0.083, lamp_z + 0.008, 0.062, ring)
            flasher(amber, x - 0.11, lamp_z - 0.125, x + 0.11, lamp_z - 0.095)
        elif kind == "square":
            sealed(lamp, x - sign * 0.022, lamp_z + 0.01, 0.092, 0.066)
            flasher(amber, outer_x - sign * 0.045, lamp_z - 0.056, outer_x, lamp_z + 0.076, False)
        elif kind == "twin_square":
            for step in (-1, 1):
                sealed(lamp, x, lamp_z + step * 0.058, 0.082, 0.04)
            flasher(amber, x - 0.08, low - 0.075, x + 0.08, low - 0.04)
        elif kind == "wide":
            housing(lamp, rounded(min(inner_x, outer_x), lamp_z - 0.045, max(inner_x, outer_x), lamp_z + 0.075, 0.012))
            x0, x1 = sorted((inner_x + sign * 0.012, outer_x - sign * 0.075))
            mirror_box(lamp, x0, lamp_z - 0.033, x1, lamp_z + 0.063)
            flasher(amber, outer_x - sign * 0.065, lamp_z - 0.033, outer_x - sign * 0.01, lamp_z + 0.063, False)
        elif kind == "corner":
            # A flush lamp, and an amber one beside it that goes round the corner: the nineties.
            housing(lamp, rounded(min(inner_x, outer_x), lamp_z - 0.04, max(inner_x, outer_x), lamp_z + 0.08, 0.02))
            x0, x1 = sorted((inner_x + sign * 0.012, outer_x - sign * 0.085))
            mirror_box(lamp, x0, lamp_z - 0.028, x1, lamp_z + 0.068)
            flasher(amber, outer_x - sign * 0.075, lamp_z - 0.028, outer_x - sign * 0.008, lamp_z + 0.068, False)
            corner = sign * (outer + 0.004)
            amber.box("lamp_amber", (min(corner, corner - sign * 0.014), nose - 0.15, lamp_z - 0.028), (max(corner, corner - sign * 0.014), nose + 0.004, lamp_z + 0.068), 0.008)
        elif kind == "wrap":
            # Swept back and taller at its outer end, with a lens like an eye, a mirror
            # beside it, its flasher at the corner and a strip of light along its foot.
            housing(lamp, [(inner_x, lamp_z - 0.03), (inner_x, lamp_z + 0.05), (outer_x - sign * 0.03, lamp_z + 0.1), (outer_x, lamp_z + 0.085),
                           (outer_x, lamp_z - 0.015), (outer_x - sign * 0.02, lamp_z - 0.03)][::int(sign)])
            eye(lamp, inner_x + sign * 0.07, lamp_z + 0.022, 0.034)
            x0, x1 = sorted((inner_x + sign * 0.125, outer_x - sign * 0.07))
            if x1 - x0 > 0.03:
                mirror_box(lamp, x0, lamp_z - 0.008, x1, lamp_z + 0.07)
            flasher(amber, outer_x - sign * 0.06, lamp_z, outer_x - sign * 0.012, lamp_z + 0.072, False)
            if era == "tens":
                light(lamp, inner_x + sign * 0.015, lamp_z - 0.024, outer_x - sign * 0.03, lamp_z - 0.014)
            corner = sign * (outer + 0.004)
            lamp.box("trim", (min(corner, corner - sign * 0.016), nose - 0.2, lamp_z - 0.01), (max(corner, corner - sign * 0.016), nose + 0.004, lamp_z + 0.085), 0.01)
            amber.box("lamp_amber", (min(corner, corner + sign * 0.006), nose - 0.17, lamp_z + 0.01), (max(corner, corner + sign * 0.006), nose - 0.06, lamp_z + 0.065), 0.004)
        elif kind == "slim":
            # A slit of light high on the face, and the lamp itself small under it: the twenties.
            housing(lamp, rounded(min(inner_x, outer_x), high - 0.035, max(inner_x, outer_x), high + 0.015, 0.008))
            light(lamp, inner_x + sign * 0.012, high - 0.022, outer_x - sign * 0.012, high + 0.002)
            housing(lamp, rounded(x - 0.095, lamp_z - 0.095, x + 0.095, lamp_z - 0.005, 0.02))
            eye(lamp, x - 0.04, lamp_z - 0.05, 0.027)
            eye(lamp, x + 0.04, lamp_z - 0.05, 0.027)
            flasher(amber, x - 0.14, high - 0.032, x + 0.14, high - 0.026, False)
        elif kind == "c_clamp":
            # Two lamps one over the other, in a bracket of light that opens towards the grille.
            housing(lamp, rounded(x - 0.1, lamp_z - 0.13, x + 0.1, lamp_z + 0.13, 0.018))
            for step in (-1, 1):
                mirror_box(lamp, x - 0.055 - sign * 0.012, lamp_z + step * 0.055 - 0.04, x + 0.055 - sign * 0.012, lamp_z + step * 0.055 + 0.04)
            far = x + sign * 0.085
            light(lamp, far, lamp_z - 0.115, far - sign * 0.014, lamp_z + 0.115)
            for z in (lamp_z + 0.108, lamp_z - 0.108):
                light(lamp, far, z - 0.007, far - sign * 0.16, z + 0.007)
            flasher(amber, x - 0.05, low - 0.07, x + 0.05, low - 0.045, False)
        elif kind == "halo":
            # A round lamp in a ring of light: an old shape made new.
            housing(lamp, oval(x, lamp_z, 0.118, 0.118, 32))
            dome(lamp, x, lamp_z, 0.082, "trim")
            lamp.tube("lamp", [(x + 0.1 * math.cos(math.tau * step / 20), face + 0.024, lamp_z + 0.1 * math.sin(math.tau * step / 20)) for step in range(21)], 0.006, 6)
            flasher(amber, x - 0.05, lamp_z - 0.175, x + 0.05, lamp_z - 0.15, False)
        else:
            # Tall and narrow, one lamp over another.
            housing(lamp, rounded(x - 0.085, lamp_z - 0.125, x + 0.085, lamp_z + 0.125, 0.014))
            for step in (-1, 1):
                mirror_box(lamp, x - 0.068, lamp_z + step * 0.058 - 0.045, x + 0.068, lamp_z + step * 0.058 + 0.045)
            light(lamp, x - 0.07, lamp_z - 0.006, x + 0.07, lamp_z + 0.006)
            flasher(amber, x - 0.06, low - 0.075, x + 0.06, low - 0.048, False)

    mirrored(head_lamp)

    # ---- Bumpers.
    bumper_z = front_floor - 0.02
    bumper = piece("bumper_front")
    out = nose + 0.02
    kind = t["bumper"]
    if kind == "blade":
        bumper.box("chrome", (-half - 0.02, out, bumper_z - 0.05), (half + 0.02, out + 0.07, bumper_z + 0.05), 0.022)
        for sign in (-1, 1):
            bumper.box("trim", (sign * 0.3 - 0.03, out + 0.05, bumper_z - 0.075), (sign * 0.3 + 0.03, out + 0.1, bumper_z + 0.075), 0.015)
            bumper.box("iron", (sign * 0.42 - 0.03, nose - 0.2, bumper_z - 0.03), (sign * 0.42 + 0.03, out + 0.01, bumper_z + 0.03))
    elif kind == "channel":
        bumper.box("steel", (-half - 0.01, out, bumper_z - 0.07), (half + 0.01, out + 0.09, bumper_z + 0.07), 0.012)
        bumper.box("inside_dark", (-half + 0.04, out + 0.085, bumper_z - 0.035), (half - 0.04, out + 0.094, bumper_z + 0.035))
        for sign in (-1, 1):
            bumper.box("iron", (sign * 0.42 - 0.03, nose - 0.2, bumper_z - 0.03), (sign * 0.42 + 0.03, out + 0.01, bumper_z + 0.03))
    elif kind == "tube":
        for z in (bumper_z - 0.045, bumper_z + 0.055):
            bumper.tube("steel", [(-half + 0.02, nose - 0.06, z), (-half + 0.12, out + 0.08, z), (half - 0.12, out + 0.08, z), (half - 0.02, nose - 0.06, z)], 0.03)
        for sign in (-1, 1):
            bumper.rod("steel", (sign * 0.42, out + 0.08, bumper_z - 0.045), (sign * 0.42, out + 0.08, bumper_z + 0.055), 0.024)
            bumper.box("iron", (sign * 0.42 - 0.03, nose - 0.2, bumper_z - 0.03), (sign * 0.42 + 0.03, out + 0.06, bumper_z + 0.03))
    elif kind == "plate":
        bumper.box("iron", (-half + 0.02, out, bumper_z - 0.1), (half - 0.02, out + 0.16, bumper_z + 0.09), 0.02)
        bumper.box("steel", (-0.13, out + 0.155, bumper_z - 0.03), (0.13, out + 0.175, bumper_z + 0.03), 0.006)
        for sign in (-1, 1):
            bumper.box("hook", (sign * 0.5 - 0.025, out + 0.15, bumper_z - 0.05), (sign * 0.5 + 0.025, out + 0.21, bumper_z + 0.02), 0.012)
    else:
        deep = 0.19 if kind == "sport" else 0.16
        bumper.prism("paint", [(nose - 0.02, bumper_z - deep), (nose + 0.09, bumper_z - deep + 0.04), (nose + 0.11, bumper_z + 0.06),
                               (nose + 0.02, bumper_z + 0.12), (nose - 0.02, bumper_z + 0.12)], -half - 0.01, half + 0.01, 0.04)
        bumper.box("inside_dark", (-half * 0.5, nose + 0.1, bumper_z - 0.1), (half * 0.5, nose + 0.125, bumper_z - 0.01), 0.01)
        for sign in (-1, 1):
            x = sign * half * 0.74
            bumper.box("trim", (x - 0.07, nose + 0.095, bumper_z - 0.09), (x + 0.07, nose + 0.118, bumper_z - 0.02), 0.012)
            bumper.turned("lamp", [(0.0, 0.03), (0.01, 0.026)], (x, nose + 0.115, bumper_z - 0.055), "y", 16)
        if kind == "sport":
            bumper.prism("alloy", [(nose - 0.12, bumper_z - deep - 0.06), (nose + 0.08, bumper_z - deep + 0.03), (nose + 0.1, bumper_z - deep + 0.06),
                                   (nose - 0.12, bumper_z - deep - 0.02)], -half * 0.55, half * 0.55, 0.01)
            for sign in (-1, 1):
                bumper.box("hook", (sign * 0.34 - 0.022, nose + 0.1, bumper_z - 0.13), (sign * 0.34 + 0.022, nose + 0.15, bumper_z - 0.08), 0.01)
    rear_z = t["sill"] + 0.02
    rear = piece("bumper_rear")
    if kind in ("blade", "channel"):
        rear.box("chrome" if kind == "blade" else "steel", (-half + 0.03, tail - 0.09, rear_z - 0.05), (half - 0.03, tail - 0.01, rear_z + 0.05), 0.02)
    elif kind in ("tube", "plate"):
        rear.tube("steel" if kind == "tube" else "iron", [(-half + 0.05, tail + 0.05, rear_z), (-half + 0.1, tail - 0.1, rear_z), (half - 0.1, tail - 0.1, rear_z), (half - 0.05, tail + 0.05, rear_z)], 0.035)
    else:
        rear.box("trim", (-half + 0.02, tail - 0.1, rear_z - 0.07), (half - 0.02, tail - 0.01, rear_z + 0.07), 0.03)
        rear.box("paint", (-half + 0.02, tail - 0.07, rear_z + 0.07), (half - 0.02, tail - 0.005, rear_z + 0.1), 0.02)
    # A tow bar.
    rear.box("iron", (-0.04, tail - 0.16, rear_z - 0.1), (0.04, tail - 0.02, rear_z - 0.05))
    rear.turned("chrome", [(0.0, 0.0), (0.012, 0.024), (0.036, 0.024), (0.048, 0.0)], (0.0, tail - 0.14, rear_z - 0.05), "x", 14)

    # ---- The cab.
    rear_arch = arch(rear_axle, hub + 0.02, gap, t["sill"], squared)
    if wagon:
        cab = [(cowl, t["sill"]), (cowl, t["bonnet_back"]), (screen_top, t["roof"]), (roof_back, t["roof"]), (cab_back, t["belt"] + 0.02),
               (cab_back, t["sill"] + 0.05)]
        cab += [point for point in rear_arch if point[0] > cab_back + 0.02]
    else:
        cab = [(cowl, t["sill"]), (cowl, t["bonnet_back"]), (screen_top, t["roof"]), (roof_back, t["roof"]), (cab_back, t["belt"] + 0.02),
               (cab_back, t["sill"])]
    piece("cab").prism("paint", cab, -half, half, edge * 1.3, shaped, GRID)
    if wagon:
        piece("liner").box("inside_dark", (-half + 0.14, rear_axle - gap, t["sill"]), (half - 0.14, rear_axle + gap, hub + gap - 0.02))

    # Glass, and the doors' skins: each lies on the body's side, a little proud of it.
    proud = 0.007
    pillar = 0.085
    glass_low, glass_high = t["belt"] + 0.035, t["roof"] - 0.115
    # The roof: pressed ribs along it, or a gutter down each side.
    if t["roof_style"] == "ribs":
        for column in range(5):
            x = (column - 2) * (side(0.0, t["roof"]) * 0.36)
            piece("roof_trim").box("paint", (x - 0.03, roof_back + 0.12, t["roof"] + t["crown"] * 0.7), (x + 0.03, screen_top - 0.1, t["roof"] + t["crown"] * 0.7 + 0.014), 0.008)
    elif t["roof_style"] == "gutter":
        for sign in (-1, 1):
            x = sign * (side(0.0, t["roof"] - 0.03) + 0.004)
            piece("roof_trim").rod("paint", (x, roof_back + 0.03, t["roof"] - 0.035), (x, screen_top - 0.02, t["roof"] - 0.035), 0.011, 8)

    def on_side(sign, y, z, out):
        return (sign * (side(y, z) + out), y, z)

    def screen_at(z):
        """Where the windscreen is, along y, at a height."""
        return cowl + (screen_top - cowl) * (z - t["bonnet_back"]) / (t["roof"] - t["bonnet_back"])

    def back_at(z):
        """Where the cab's back is, along y, at a height: upright to the belt, leaning above it."""
        return cab_back + (roof_back - cab_back) * max(z - t["belt"], 0.0) / (t["roof"] - t["belt"])

    def skin(made, called, outline, sign, out, thick, hole=None):
        """A piece lying on the body's side, a little proud of it, following its curve: a
        door's panel, a pane of glass. Its outline is in y and z. With a `hole`, it is a
        frame: a door's skin about its window, the rubber about a pane."""
        bm = bmesh.new()
        if hole:
            edges = []
            for loop in (outline, hole):
                verts = [bm.verts.new((0.0, y, z)) for y, z in loop]
                edges += [bm.edges.new((a, b)) for a, b in zip(verts, verts[1:] + verts[:1])]
            bmesh.ops.triangle_fill(bm, edges=edges, use_beauty=True)
        else:
            bm.faces.new([bm.verts.new((0.0, y, z)) for y, z in outline])
        cut(bm, GRID, (1, 2))
        lifted = bmesh.ops.extrude_face_region(bm, geom=bm.faces[:])
        inner = {item for item in lifted["geom"] if isinstance(item, bmesh.types.BMVert)}
        for vert in bm.verts:
            vert.co.x = sign * (side(vert.co.y, vert.co.z) + (out - thick if vert in inner else out))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        made.take(bm, called)

    # The windscreen and the back glass.
    for called, at, way in (("glass_screen", screen_at, 1.0), ("glass_back", back_at, -1.0)):
        if wagon and called == "glass_back":
            low_z, high_z, out = t["belt"] + 0.12, glass_high - 0.03, way * 0.013
        else:
            low_z, high_z, out = glass_low + 0.02, glass_high - 0.02, way * 0.006
        narrow = 0.04 if wagon and called == "glass_back" else 0.0

        def pane_at(inset, grow, lifted):
            inset += narrow
            corners = [(-(side(at(low_z), low_z) - inset), at(low_z - grow) + lifted, low_z - grow), ((side(at(low_z), low_z) - inset), at(low_z - grow) + lifted, low_z - grow),
                       ((side(at(high_z), high_z) - inset), at(high_z + grow) + lifted, high_z + grow), (-(side(at(high_z), high_z) - inset), at(high_z + grow) + lifted, high_z + grow)]
            if way > 0:
                corners.reverse()
            return corners

        # Its rubber, a frame about the opening, and then its glass.
        rubber, opening = pane_at(pillar - 0.014, 0.012, out * 0.5), pane_at(pillar + 0.02, -0.02, out * 0.5)
        for corner in range(4):
            after = (corner + 1) % 4
            piece(called).plate("trim", [rubber[corner], rubber[after], opening[after], opening[corner]], -0.004)
        piece(called).plate("glass", pane_at(pillar, 0.0, out), -0.006)

    # Doors: where each begins and ends along the cab, front edge and back.
    door_front = cowl - 0.04
    if t["doors"] == 2:
        back_edge = max(door_front - 1.0, rear_axle + gap + 0.05) if wagon else cab_back + 0.12
        spans = [("F", door_front, back_edge)]
    else:
        back_edge = (rear_axle + gap + 0.03) if wagon else (cab_back + 0.1)
        middle = door_front - (door_front - back_edge) * 0.53
        spans = [("F", door_front, middle + 0.012), ("R", middle - 0.012, back_edge)]

    def door_shapes(row, start, end):
        """A door's skin and its pane of glass, as outlines along the truck and up."""
        low_z, top = t["sill"] + 0.05, glass_high + 0.045
        if row == "F":
            # Its front edge follows the windscreen's pillar above the bonnet.
            panel = [(start, low_z), (end, low_z), (end, top), (screen_at(top) - 0.06, top), (start, t["bonnet_back"] - 0.02)]
            pane = [(screen_at(glass_low) - 0.1, glass_low), (end + 0.05, glass_low), (end + 0.05, glass_high), (screen_at(glass_high) - 0.12, glass_high)]
        else:
            panel = [(start, low_z), (end, low_z), (end, top), (start, top)]
            pane = [(start - 0.05, glass_low), (end + 0.07, glass_low), (end + 0.07, glass_high), (start - 0.05, glass_high)]
        return panel, pane

    def quarter_pane():
        """A wagon's side glass behind its doors, if there is room for one."""
        start = spans[-1][2] - 0.07
        end = back_at(glass_low) + 0.14
        if not wagon or start - end < 0.25:
            return None
        return [(start, glass_low), (end, glass_low), (back_at(glass_high) + 0.12, glass_high), (start, glass_high)]

    # ---- The cab is a shell. It is hollowed; its windows are cut through it; and its doors
    # are cut out of it, each a piece of its own that swings on its front edge.
    wall = 0.05
    floor, ceiling = t["sill"] + 0.07, t["roof"] - 0.05
    step = rear_axle + gap + 0.06             # where a wagon's floor rises over its back wheels
    boot_floor = hub + gap + 0.07
    inner = lambda y, z: side(y, z) - wall
    back_in = lambda z: cab_back + (roof_back - cab_back) * max(z - t["belt"], 0.0) / (t["roof"] - t["belt"]) + wall + 0.01
    hollow = [(cowl - wall, floor), (cowl - wall, t["bonnet_back"] - 0.02), (screen_at(ceiling) - wall - 0.02, ceiling), (back_in(ceiling), ceiling),
              (back_in(t["belt"]), t["belt"] + 0.02)]
    hollow += [(cab_back + wall, boot_floor), (step, boot_floor), (step, floor)] if wagon else [(cab_back + wall, floor)]
    cutter = Piece("hollow")
    cutter.prism("inside_trim", hollow, -half, half, edge, lambda co: Vector((co.x / half * inner(co.y, co.z), co.y, co.z)), GRID)
    carve(piece("cab"), cutter, paint)

    openings = Piece("openings")
    for at, way in ((screen_at, 1.0), (back_at, -1.0)):
        if wagon and way < 0:
            low_z, high_z = t["belt"] + 0.14, glass_high - 0.05
        else:
            low_z, high_z = glass_low + 0.04, glass_high - 0.04
        narrow = 0.04 if wagon and way < 0 else 0.0
        reach_low, reach_high = side(at(low_z), low_z) - pillar - 0.02 - narrow, side(at(high_z), high_z) - pillar - 0.02 - narrow
        openings.hull("inside_trim", [(sign * reach, at(z) + lifted, z) for reach, z in ((reach_low, low_z), (reach_high, high_z))
                                      for sign in (-1, 1) for lifted in (way * 0.15, -way * 0.3)])
    panes = [door_shapes(*span)[1] for span in spans] + ([quarter_pane()] if quarter_pane() else [])
    for sign in (-1, 1):
        x0, x1 = sorted((sign * (half - 0.36), sign * (half + 0.1)))
        for pane in panes:
            openings.prism("inside_trim", inset(pane, 0.02), x0, x1)
    carve(piece("cab"), openings, paint)

    jamb = (step if wagon else cab_back + wall) + 0.02

    def door_hole(row, start, end):
        """What of the cab's wall a door is: a little less than its skin, which laps its frame."""
        low_z, top = floor + 0.01, glass_high + 0.045
        back = max(end, jamb)
        back_top = max(back, back_in(top) + 0.03)
        if row == "F":
            return [(start - 0.03, low_z), (back, low_z), (back, t["belt"]), (back_top, top), (screen_at(top) - 0.14, top), (start - 0.03, t["bonnet_back"] - 0.02)]
        return [(start, low_z), (back, low_z), (back, t["belt"]), (back_top, top), (start, top)]

    holes = Piece("holes")
    for sign, letter in ((-1.0, "L"), (1.0, "R")):
        x0, x1 = sorted((sign * (half - 0.36), sign * (half + 0.1)))
        for row, start, end in spans:
            door = clone(piece("cab"), f"door_{row}{letter}")
            cutter = Piece("cutter")
            cutter.prism("inside_trim", door_hole(row, start, end), x0, x1)
            carve(door, cutter, paint, "INTERSECT")
            pieces[door.name] = door
            holes.prism("inside_trim", door_hole(row, start, end), x0, x1)
            hinges[door.name] = ((sign * side(start, t["belt"]), start, 0.0), "up", 70.0 * sign)
    if wagon:
        # The back door: it swings on its right edge, and the back glass is in it.
        reach = side(cab_back, t["belt"]) - 0.1
        outline = rounded(-reach, boot_floor + 0.01, reach, glass_high, 0.04)
        door = clone(piece("cab"), "door_back")
        cutter = Piece("cutter")
        cutter.slab("inside_trim", outline, cab_back - 0.1, back_in(glass_high) + 0.1)
        carve(door, cutter, paint, "INTERSECT")
        merge(door, pieces.pop("glass_back"))
        pieces[door.name] = door
        holes.slab("inside_trim", outline, cab_back - 0.1, back_in(glass_high) + 0.1)
        hinges[door.name] = ((reach, cab_back, 0.0), "up", 100.0)
    carve(piece("cab"), holes, paint)

    # ---- Inside it: a dash and a wheel, seats, and what is between them. The wheel is on
    # the right.
    cabin = piece("interior")
    reach = inner(cowl - 0.3, floor + 0.3) - 0.012
    fire = cowl - wall + 0.01
    dash_top = t["bonnet_back"] - 0.04
    cabin.prism("inside_trim", [(fire, floor + 0.4), (fire, dash_top), (fire - 0.3, dash_top - 0.01), (fire - 0.4, dash_top - 0.07), (fire - 0.42, dash_top - 0.2),
                                (fire - 0.3, floor + 0.36)], -reach, reach, 0.03)
    drives = reach * 0.52
    cabin.box("inside_dark", (drives - 0.17, fire - 0.37, dash_top - 0.02), (drives + 0.17, fire - 0.2, dash_top + 0.05), 0.02)
    cabin.box("inside_dark", (-0.12, fire - 0.425, dash_top - 0.19), (0.12, fire - 0.38, dash_top - 0.06), 0.008)      # the radio, the heater
    hub_at, column = Vector((drives, fire - 0.56, dash_top - 0.01)), Vector((drives, fire - 0.36, dash_top - 0.12))
    cabin.rod("inside_dark", column, hub_at, 0.022, 10)
    ring = Piece("ring")
    ring.turned("inside_dark", [(0.016 * math.cos(math.tau * step / 8), 0.185 + 0.016 * math.sin(math.tau * step / 8)) for step in range(9)], (0, 0, 0), "y", 28, False)
    for spoke in range(3):
        angle = math.tau * (spoke / 3 + 0.25)
        ring.rod("inside_dark", (0, 0, 0), (0.18 * math.cos(angle), 0.0, 0.18 * math.sin(angle)), 0.012, 6)
    ring.turned("inside_dark", [(-0.02, 0.0), (-0.02, 0.045), (0.015, 0.05)], (0, 0, 0), "y", 12)
    lean = Vector((0.0, -1.0, 0.0)).rotation_difference((hub_at - column).normalized()).to_matrix().to_4x4()
    cabin.take(ring.bm, "inside_dark", Matrix.Translation(hub_at) @ lean)
    cabin.box("inside_dark", (-0.1, screen_at(glass_high - 0.07) - wall - 0.06, glass_high - 0.11), (0.1, screen_at(glass_high - 0.07) - wall - 0.04, glass_high - 0.04), 0.008)

    front_back = max(cowl - 1.18, (step if wagon else cab_back + wall) + 0.14)
    # A wagon's back seat leans back over where its floor rises. Where there is all but
    # room for one, the front seats are a little further forward, as they are in a short car.
    rear_back = (step - 0.12) if wagon else (cab_back + wall + 0.14)
    short = 0.05 - (front_back - 0.12 - (rear_back + 0.43))
    if 0.0 < short <= 0.14:
        front_back += short + 0.001
    bench = t["era"] in ("seventies", "eighties") and t["doors"] == 2

    def seat(x0, x1, back, headrest=True, deep=0.47):
        """A seat: what it stands on, its cushion, its back leaning, and a rest for a head."""
        cabin.box("inside_dark", (x0 + 0.04, back + 0.08, floor - 0.005), (x1 - 0.04, back + deep - 0.06, floor + 0.2))
        cabin.box("inside_seat", (x0, back + 0.04, floor + 0.2), (x1, back + deep, floor + 0.33), 0.035)
        cabin.prism("inside_seat", [(back + 0.11, floor + 0.3), (back, floor + 0.3), (back - 0.1, floor + 0.86), (back - 0.01, floor + 0.88)], x0, x1, 0.03)
        if headrest:
            for middle in ([(x0 + x1) / 2] if x1 - x0 < 0.7 else [x0 + 0.26, (x0 + x1) / 2, x1 - 0.26]):
                cabin.box("inside_seat", (middle - 0.12, back - 0.13, floor + 0.89), (middle + 0.12, back - 0.04, floor + 1.06), 0.03)
                cabin.rod("steel", (middle, back - 0.07, floor + 0.84), (middle, back - 0.08, floor + 0.92), 0.008, 6)

    seat_reach = inner(front_back + 0.2, floor + 0.3) - 0.03
    deep = min(0.47, fire - 0.44 - front_back)
    if bench:
        seat(-seat_reach, seat_reach, front_back, False, deep)
    else:
        seat(0.09, seat_reach, front_back, t["era"] != "seventies", deep)
        seat(-seat_reach, -0.09, front_back, t["era"] != "seventies", deep)
        cabin.box("inside_trim", (-0.075, front_back + 0.06, floor - 0.005), (0.075, fire - 0.36, floor + 0.27), 0.02)
    cabin.rod("steel", (0.0, front_back + deep + 0.08, floor + 0.2), (0.02, front_back + deep + 0.02, floor + 0.48), 0.009, 8)
    cabin.turned("inside_dark", [(-0.022, 0.0), (-0.012, 0.022), (0.012, 0.022), (0.022, 0.0)], (0.02, front_back + deep + 0.02, floor + 0.5), "x", 10)
    holds.append(("passenger_seat", (-seat_reach, front_back + 0.04, floor + 0.33), (-0.09 if not bench else -0.05, front_back + deep, floor + 0.75), ["door_FL"]))
    if front_back - 0.12 - (rear_back + 0.43) > 0.05:
        rear_reach = inner(rear_back + 0.2, floor + 0.3) - 0.03
        seat(-rear_reach, rear_reach, rear_back, t["era"] not in ("seventies", "eighties"), 0.43)
        behind = ["door_RL", "door_RR"] if len(spans) == 2 else ["door_FL", "door_FR"]
        holds.append(("back_seat", (-rear_reach, rear_back + 0.04, floor + 0.33), (rear_reach, rear_back + 0.43, floor + 0.75), behind))
    if wagon:
        boot_reach = inner(cab_back + 0.3, boot_floor + 0.2) - 0.03
        holds.append(("boot", (-boot_reach, cab_back + wall + 0.05, boot_floor), (boot_reach, min(step, rear_back - 0.14) - 0.03, boot_floor + 0.45), ["door_back"]))

    def doors(sign, letter):
        for row, start, end in spans:
            door = piece(f"door_{row}{letter}")
            panel, pane = door_shapes(row, start, end)
            skin(door, "paint", panel, sign, proud, 0.012, inset(pane, 0.02))
            skin(door, "trim", [(y + (0.018 if index in (0, 3) else -0.018), z + (-0.018 if index < 2 else 0.018)) for index, (y, z) in enumerate(pane)], sign, proud + 0.003, 0.006,
                 inset(pane, 0.02))
            skin(door, "glass", pane, sign, proud + 0.006, 0.006)
            # Its handle: a chrome one to pull on an old truck, a flap in its own colour on a new.
            handle_y, handle_z = end + 0.1, t["belt"] - 0.075
            x = sign * (side(handle_y, handle_z) + proud)
            if t["mirror"] == "pod":
                door.box("paint", (min(x, x + sign * 0.02), handle_y - 0.075, handle_z - 0.02), (max(x, x + sign * 0.02), handle_y + 0.075, handle_z + 0.02), 0.008)
                door.box("inside_dark", (min(x, x + sign * 0.006), handle_y - 0.06, handle_z - 0.034), (max(x, x + sign * 0.006), handle_y + 0.06, handle_z - 0.02))
            else:
                door.box("chrome", (min(x, x + sign * 0.024), handle_y - 0.055, handle_z - 0.014), (max(x, x + sign * 0.024), handle_y + 0.055, handle_z + 0.014), 0.006)
                door.turned("chrome", [(0.0, 0.014), (0.01, 0.014)], (min(x, x + sign * 0.01), handle_y + 0.09, handle_z), "x", 10)

    mirrored(doors)

    # A wagon's side glass behind its doors.
    if wagon:
        def quarter(sign, letter):
            pane = quarter_pane()
            if pane is None:
                return
            start, end = pane[0][0], pane[1][0]
            skin(piece("glass_quarter_" + letter), "trim", [(start + 0.018, glass_low - 0.018), (end - 0.018, glass_low - 0.018),
                                                           (back_at(glass_high) + 0.102, glass_high + 0.018), (start + 0.018, glass_high + 0.018)], sign, 0.003, 0.006, inset(pane, 0.02))
            skin(piece("glass_quarter_" + letter), "glass", pane, sign, 0.006, 0.006)

        mirrored(quarter)

    # ---- What its time put on it.
    era = t["era"]
    old = era in ("seventies", "eighties")
    if old:
        def of_its_time(sign, letter):
            # A bar down the front door's glass, where a small pane turns to let air in.
            start = spans[0][1]
            y = screen_at(glass_low) - 0.34
            x = lambda z: sign * (side(y, z) + proud + 0.012)
            piece(f"door_F{letter}").rod("trim", (x(glass_low), y, glass_low), (x(glass_high), y + 0.015, glass_high), 0.008, 6)
            if era == "seventies":
                # Hinges on the outside of each door, and a rubber catch each side of the bonnet.
                for row, front, back_end in spans:
                    for z in (t["sill"] + 0.22, t["belt"] - 0.13):
                        hinge = sign * (side(front, z) + proud)
                        piece(f"door_{row}{letter}").box("trim", (min(hinge, hinge + sign * 0.016), front - 0.02, z - 0.035), (max(hinge, hinge + sign * 0.016), front + 0.05, z + 0.035), 0.006)
                y = cowl + (nose - cowl) * 0.45
                z = top(y) - (0.2 if separate else 0.05)
                catch = sign * ((core if separate else side(y, z)) + 0.002)
                piece("bonnet").box("trim", (min(catch, catch + sign * 0.02), y - 0.03, z - 0.05), (max(catch, catch + sign * 0.02), y + 0.03, z + 0.03), 0.008)

        mirrored(of_its_time)
        if era == "seventies":
            # A flap in the scuttle, and a windscreen in two halves.
            piece("nose").box("paint", (-0.2, cowl + 0.015, t["bonnet_back"] + 0.012), (0.2, cowl + 0.1, t["bonnet_back"] + 0.026), 0.008)
            low_z, high_z = glass_low + 0.02, glass_high - 0.02
            piece("glass_screen").rod("trim", (0.0, screen_at(low_z) + 0.012, low_z), (0.0, screen_at(high_z) + 0.012, high_z), 0.011, 8)
    else:
        if wagon:
            # Rails along the roof.
            reach = side(0.0, t["roof"]) - 0.11
            z = t["roof"] + t["crown"] * 0.5 + 0.05
            for sign in (-1, 1):
                piece("roof_rails").rod("alloy" if era != "twenties" else "trim", (sign * reach, roof_back + 0.3, z), (sign * reach, screen_top - 0.25, z), 0.014, 8)
                for y in (roof_back + 0.32, (roof_back + screen_top) / 2, screen_top - 0.27):
                    piece("roof_rails").box("trim", (sign * reach - 0.02, y - 0.04, z - 0.05), (sign * reach + 0.02, y + 0.04, z - 0.005), 0.01)
        # A third brake lamp over the back glass.
        piece("lamp_stop").box("lamp_tail", (-0.13, back_at(glass_high + 0.03) - 0.022, glass_high + 0.02), (0.13, back_at(glass_high + 0.03) - 0.004, glass_high + 0.045), 0.006)
        if era in ("tens", "twenties"):
            # A fin on the roof, and the pillar between the doors blacked out.
            y = roof_back + 0.3
            z = t["roof"] + t["crown"] * 0.6
            piece("fin").prism("paint", [(y + 0.2, z - 0.01), (y, z - 0.01), (y + 0.01, z + 0.055), (y + 0.05, z + 0.035)], -0.022, 0.022, 0.008)
            if len(spans) == 2:
                def blacked(sign, letter):
                    middle = (spans[0][2] + spans[1][1]) / 2
                    skin(piece(f"door_F{letter}"), "trim", [(middle + 0.075, glass_low - 0.01), (middle - 0.075, glass_low - 0.01), (middle - 0.075, glass_high + 0.01),
                                                            (middle + 0.075, glass_high + 0.01)], sign, proud + 0.005, 0.005)

                mirrored(blacked)
        elif era == "noughties":
            piece("aerial").rod("trim", (-0.3, roof_back + 0.25, t["roof"] + t["crown"] * 0.5), (-0.3, roof_back + 0.1, t["roof"] + 0.42), 0.005, 6)
    t["hubcap"] = era == "seventies"

    # ---- What runs along its side, between its arches.
    run_front = front_axle - gap - 0.07
    run_back = (rear_axle + gap + 0.07) if t["back"] != "tray" else cab_back + 0.03
    if t["side"] != "plain" and run_front - run_back > 0.4:
        def along(sign, letter):
            made = piece("trim_" + letter)
            band = lambda low_z, high_z: [(run_front, low_z), (run_back, low_z), (run_back, high_z), (run_front, high_z)]
            if t["side"] == "moulding":
                z = t["belt"] - 0.3
                skin(made, "trim", band(z - 0.022, z + 0.022), sign, proud + 0.012, 0.014)
            elif t["side"] == "cladding":
                skin(made, "trim", band(t["sill"] + 0.02, t["sill"] + 0.2), sign, proud + 0.01, 0.012)
            elif t["side"] == "stripe":
                z = t["belt"] - 0.13
                skin(made, "stripe", band(z - 0.035, z + 0.035), sign, proud + 0.002, 0.004)
                skin(made, "stripe", band(z - 0.075, z - 0.055), sign, proud + 0.002, 0.004)
            else:
                skin(made, "stripe", band(t["sill"] + 0.03, t["belt"] - 0.33), sign, proud + 0.002, 0.004)

        mirrored(along)
    if t["aerial"]:
        x = half - 0.12 if not separate else core - 0.06
        piece("aerial").rod("chrome", (x, cowl + 0.12, t["bonnet_back"]), (x + 0.02, cowl - 0.02, t["bonnet_back"] + 0.85), 0.004, 6)

    # ---- The back.
    tail_lamp_z = t["belt"] - 0.16
    # Behind the cab the chassis rises over the back axle: how high its top is there, and
    # where it begins to rise.
    rail_rear = max(t["sill"] - 0.03, hub + 0.27)
    kick = rear_axle + gap + 0.3
    if t["back"] == "tray":
        deck, wide = t["tray_deck"], t["tray_wide"] / 2
        start = cab_back - 0.07
        tray = piece("tray")
        # It stands on the chassis by its mounts.
        for x in (-0.42, 0.42):
            for y in (tail + 0.3, rear_axle - 0.1, min(kick - 0.4, start - 0.2)):
                if deck - 0.14 - rail_rear > 0.01:
                    tray.box("iron", (x - 0.045, y - 0.06, rail_rear - 0.01), (x + 0.045, y + 0.06, deck - 0.13))
        tray.box("alloy", (-wide, tail, deck - 0.05), (wide, start, deck), 0.006)
        t["bed"] = (wide - 0.03, deck, start - 0.03, tail + 0.03)
        # A timber deck to lay on it: boards along it.
        boards = 9
        for board in range(boards):
            x0 = -wide + 0.03 + board * (2 * wide - 0.06) / boards
            piece("acc_tray_timber").box("wood", (x0 + 0.004, tail + 0.03, deck + 0.001), (x0 + (2 * wide - 0.06) / boards - 0.004, start - 0.03, deck + 0.022), 0.003)
        # Tool boxes to hang under it ahead of each wheel, and a water tank behind one.
        boxes = piece("acc_tray_boxes")
        for sign in (-1, 1):
            x0, x1 = sorted((sign * (wide - 0.02), sign * (wide - 0.42)))
            if start - 0.02 - (rear_axle + gap + 0.08) > 0.25:
                boxes.box("alloy", (x0, rear_axle + gap + 0.08, deck - 0.42), (x1, min(rear_axle + gap + 0.7, start - 0.02), deck - 0.06), 0.015)
        boxes.box("trim", (-wide + 0.03, tail + 0.06, deck - 0.36), (-wide + 0.4, rear_axle - gap - 0.08, deck - 0.06), 0.03)
        for x in (-0.42, 0.42):
            tray.box("iron", (x - 0.04, tail + 0.05, deck - 0.14), (x + 0.04, start - 0.05, deck - 0.05))
        for row in range(5):
            y = tail + 0.12 + row * (start - tail - 0.24) / 4
            tray.box("iron", (-wide + 0.02, y - 0.025, deck - 0.1), (wide - 0.02, y + 0.025, deck - 0.05))
        # Drop sides and a tailboard.
        sides = t["tray_sides"]
        for sign in (-1, 1):
            tray.box("alloy", (sign * wide - 0.018, tail + 0.02, deck), (sign * wide + 0.018, start - 0.02, deck + sides), 0.006)
        tray.box("alloy", (-wide, tail - 0.012, deck), (wide, tail + 0.024, deck + sides), 0.006)
        # The headboard: a frame behind the cab, with a mesh in it.
        top = t["roof"] - 0.12
        for sign in (-1, 1):
            tray.rod("iron", (sign * (wide - 0.03), start, deck), (sign * (wide - 0.03), start, top), 0.022)
        tray.rod("iron", (-wide + 0.03, start, top), (wide - 0.03, start, top), 0.022)
        tray.box("alloy", (-wide + 0.03, start - 0.012, deck), (wide - 0.03, start + 0.012, deck + sides + 0.1), 0.004)
        for column in range(9):
            x = -wide + 0.12 + column * (2 * wide - 0.24) / 8
            tray.rod("iron", (x, start, deck + sides + 0.1), (x, start, top), 0.006, 6)
        # Guards over the wheels, under the deck.
        for sign in (-1, 1):
            # A strip bent over the wheel, made a length at a time: one outline that long and
            # that thin is not always filled as it is drawn.
            points = arch(rear_axle, hub + 0.02, gap + 0.02, hub + 0.1)
            x = sign * (t["track"] / 2)
            for (y0, z0), (y1, z1) in zip(points, points[1:]):
                if abs(y1 - y0) + abs(z1 - z0) > 1e-4:
                    tray.prism("trim", [(y0, z0), (y1, z1), (y1, z1 + 0.02), (y0, z0 + 0.02)], x - t["tyre_wide"] / 2 - 0.03, x + t["tyre_wide"] / 2 + 0.03)
        for sign, letter in ((-1, "L"), (1, "R")):
            x = sign * (wide - 0.1)
            lamp = piece("lamp_tail_" + letter)
            if t["tail"] in ("bar", "wrap"):
                # A strip of small lamps in a guard, as a new tray has.
                lamp.box("trim", (x - 0.16, tail - 0.034, deck - 0.14), (x + 0.16, tail - 0.012, deck - 0.06), 0.008)
                for step in range(3):
                    lamp.turned("lamp_tail" if step < 2 else "lamp_amber", [(0.0, 0.03), (0.012, 0.026)], (x + (step - 1) * 0.1, tail - 0.036, deck - 0.1), "y", 14)
            else:
                lamp.box("lamp_tail", (x - 0.07, tail - 0.03, deck - 0.13), (x + 0.07, tail - 0.012, deck - 0.065), 0.006)
                lamp.box("lamp_amber", (x - 0.07, tail - 0.03, deck - 0.165), (x + 0.07, tail - 0.012, deck - 0.135), 0.006)
    elif t["back"] == "tub":
        start = cab_back - 0.03
        top = t["tub_top"]
        t["bed"] = (half - 0.12, top + 0.02, start - 0.1, tail + 0.1)
        tub = [(start, t["sill"] + 0.02), (start, top), (tail, top), (tail, t["sill"] + 0.08)]
        tub += [point for point in rear_arch if tail + 0.04 < point[0] < start - 0.04]
        piece("tub").prism("paint", tub, -half, half, edge, lambda co: Vector((co.x / half * side(co.y, co.z), co.y, co.z)), GRID)
        piece("tub").box("inside_dark", (-half + 0.14, rear_axle - gap, t["sill"]), (half - 0.14, rear_axle + gap, hub + gap - 0.02))
        # The well: dark, sunk a hand into the top.
        piece("tub").box("inside_dark", (-half + 0.09, tail + 0.07, top - 0.02), (half - 0.09, start - 0.07, top + 0.004))
        piece("tub").box("trim", (-half + 0.03, tail + 0.01, top), (half - 0.03, start - 0.01, top + 0.016), 0.006)
        piece("tub").box("inside_dark", (-half + 0.1, tail + 0.08, top + 0.002), (half - 0.1, start - 0.08, top + 0.018))
        if True:
            reach = side(start - 0.2, top) - 0.06
            piece("acc_sportsbar").tube("steel" if t["grade"] < 4 else "iron", [(-reach, start - 0.5, top), (-reach, start - 0.16, t["roof"] - 0.2), (reach, start - 0.16, t["roof"] - 0.2), (reach, start - 0.5, top)], 0.032)
            for sign in (-1, 1):
                piece("acc_sportsbar").rod("steel" if t["grade"] < 4 else "iron", (sign * reach, start - 0.16, top), (sign * reach, start - 0.16, t["roof"] - 0.2), 0.032)
        if True:
            piece("acc_tonneau").box("trim", (-half + 0.05, tail + 0.03, top + 0.012), (half - 0.05, start - 0.03, top + 0.04), 0.012)
        if True:
            lid_top = t["roof"] - 0.03
            shell = [(start - 0.03, top + 0.01), (start - 0.05, lid_top), (tail + 0.12, lid_top), (tail + 0.03, top + 0.01)]
            piece("acc_canopy").prism("paint", shell, -half + 0.02, half - 0.02, edge, lambda co: Vector((co.x / half * side(co.y, min(co.z, t["roof"])), co.y, co.z)), GRID)
            for sign in (-1, 1):
                window = [(start - 0.2, top + 0.1), (tail + 0.3, top + 0.1), (tail + 0.33, lid_top - 0.1), (start - 0.2, lid_top - 0.1)]
                skin(piece("acc_canopy"), "inside_dark", window, sign, -0.014, 0.004)       # a canopy is not hollow: dark behind its glass
                skin(piece("acc_canopy"), "glass", window, sign, -0.012, 0.006)
        gate = [(-(side(tail, top) - 0.09), tail - proud, t["sill"] + 0.16), ((side(tail, top) - 0.09), tail - proud, t["sill"] + 0.16),
                ((side(tail, top) - 0.09), tail - proud, top - 0.03), (-(side(tail, top) - 0.09), tail - proud, top - 0.03)]
        piece("tailgate").plate("paint", gate, -0.012)
        hinges["tailgate"] = ((0.0, tail - proud, t["sill"] + 0.16), "across", 90.0)
        piece("tailgate").box("trim", (-0.1, tail - proud - 0.022, top - 0.14), (0.1, tail - proud, top - 0.1), 0.006)
        for sign, letter in ((-1, "L"), (1, "R")):
            x = sign * (side(tail, tail_lamp_z) - 0.045)
            lamp = piece("lamp_tail_" + letter)
            kind = t["tail"]
            if kind == "small":
                lamp.box("chrome", (x - 0.05, tail - 0.014, top - 0.2), (x + 0.05, tail + 0.008, top - 0.07), 0.006)
                lamp.box("lamp_tail", (x - 0.04, tail - 0.02, top - 0.16), (x + 0.04, tail - 0.01, top - 0.08), 0.004)
                lamp.box("lamp_amber", (x - 0.04, tail - 0.02, top - 0.19), (x + 0.04, tail - 0.01, top - 0.165), 0.004)
            elif kind == "bar":
                lamp.box("trim", (x - 0.045, tail - 0.014, top - 0.4), (x + 0.045, tail + 0.01, top - 0.04), 0.01)
                for z in (top - 0.1, top - 0.19, top - 0.28):
                    lamp.box("lamp_tail", (x - 0.03, tail - 0.02, z - 0.03), (x + 0.03, tail - 0.012, z + 0.03), 0.004)
            else:
                lamp.box("lamp_tail", (x - 0.04, tail - 0.012, top - 0.34), (x + 0.04, tail + 0.01, top - 0.05), 0.008)
                lamp.box("lamp_amber", (x - 0.034, tail - 0.016, top - 0.34), (x + 0.034, tail - 0.01, top - 0.28), 0.004)
                if kind == "wrap":
                    corner = sign * (side(tail, tail_lamp_z) + 0.003)
                    lamp.box("lamp_tail", (min(corner, corner - sign * 0.012), tail - 0.004, top - 0.34), (max(corner, corner - sign * 0.012), tail + 0.1, top - 0.05), 0.008)
    else:
        gate_top = t["belt"] + 0.09
        gate = [(-(side(tail, t["belt"]) - 0.1), back_at(t["sill"] + 0.14) - proud, t["sill"] + 0.14), ((side(tail, t["belt"]) - 0.1), back_at(t["sill"] + 0.14) - proud, t["sill"] + 0.14),
                ((side(tail, gate_top) - 0.1), back_at(gate_top) - proud, gate_top), (-(side(tail, gate_top) - 0.1), back_at(gate_top) - proud, gate_top)]
        piece("door_back").plate("paint", gate, -0.012)
        for sign, letter in ((-1, "L"), (1, "R")):
            x = sign * (side(tail, tail_lamp_z) - 0.05)
            y = back_at(tail_lamp_z)
            lamp = piece("lamp_tail_" + letter)
            kind = t["tail"]
            if kind == "small":
                lamp.box("lamp_tail", (x - 0.045, y - 0.014, tail_lamp_z - 0.05), (x + 0.045, y + 0.008, tail_lamp_z + 0.05), 0.006)
                lamp.box("lamp_amber", (x - 0.045, y - 0.014, tail_lamp_z - 0.11), (x + 0.045, y + 0.008, tail_lamp_z - 0.06), 0.006)
            elif kind == "bar":
                # A bar of light right across the back.
                lamp.box("lamp_tail", (min(0.0, x), y - 0.016, tail_lamp_z + 0.06), (max(0.0, x), y + 0.006, tail_lamp_z + 0.1), 0.008)
                lamp.box("lamp_tail", (x - 0.04, y - 0.016, tail_lamp_z - 0.1), (x + 0.04, y + 0.006, tail_lamp_z + 0.1), 0.008)
            else:
                high = glass_high if kind == "tall" else tail_lamp_z + 0.13
                lamp.box("lamp_tail", (x - 0.04, y - 0.014, tail_lamp_z - 0.13), (x + 0.04, y + 0.008, high), 0.008)
                lamp.box("lamp_amber", (x - 0.034, y - 0.018, tail_lamp_z - 0.13), (x + 0.034, y - 0.012, tail_lamp_z - 0.07), 0.004)
                if kind == "wrap":
                    corner = sign * (side(tail + 0.05, tail_lamp_z) + 0.003)
                    lamp.box("lamp_tail", (min(corner, corner - sign * 0.012), y, tail_lamp_z - 0.13), (max(corner, corner - sign * 0.012), y + 0.12, tail_lamp_z + 0.13), 0.008)

    # ---- The small things: wipers, number plates, a filler cap, mud flaps, a lip round
    # each arch, a step under the doors.
    wipers = piece("wipers")
    for x in (-half * 0.5, half * 0.12):
        base = (x, screen_at(glass_low + 0.02) + 0.02, glass_low + 0.02)
        tip_z = glass_low + 0.13
        wipers.rod("trim", base, (x + half * 0.42, screen_at(tip_z) + 0.014, tip_z), 0.007, 6)
    piece("plate_front").box("plate", (-0.19, nose + (0.11 if t["bumper"] == "steel" else 0.125), bumper_z - 0.055), (0.19, nose + (0.118 if t["bumper"] == "steel" else 0.133), bumper_z + 0.055), 0.004)
    plate_y = (tail - 0.012) if t["back"] != "wagon" else (back_at(t["sill"] + 0.3) - 0.012)
    piece("plate_rear").box("plate", (-0.19, plate_y - 0.01, rear_z + 0.09), (0.19, plate_y, rear_z + 0.2), 0.004)
    # The filler is behind the back wheel where there is body there, and not in a door.
    # Where there is no room behind the wheel it is over it.
    behind = rear_axle - gap - 0.32 > tail
    cap_y = rear_axle - gap - 0.2 if behind else rear_axle - 0.05
    cap_z = (t["belt"] if t["back"] == "wagon" else t["tub_top"]) - 0.2 if behind else min(t["belt"] - 0.08, hub + gap + 0.16)
    if t["back"] != "tray":
        piece("filler").turned("trim", [(0.0, 0.06), (0.012, 0.06), (0.016, 0.05)], (-(side(cap_y, cap_z) + 0.014), cap_y, cap_z), "x", 20)
    for sign, letter in ((-1, "L"), (1, "R")):
        x = sign * t["track"] / 2
        piece("flap_" + letter).box("trim", (x - t["tyre_wide"] / 2 - 0.02, rear_axle - gap - 0.05, hub - 0.2), (x + t["tyre_wide"] / 2 + 0.02, rear_axle - gap - 0.035, hub + 0.2), 0.004)
    def lip(sign, letter):
        for axle, where in ((front_axle, "F"), (rear_axle, "R")):
            if where == "R" and t["back"] == "tray":
                continue
            inner = arch(axle, hub + 0.02, gap - 0.004, t["sill"] + 0.04, squared)
            outer = arch(axle, hub + 0.02, gap + 0.028, t["sill"] + 0.04, squared)
            x = side(axle, hub + gap)
            piece(f"lip_{where}{letter}").prism("paint", inner + list(reversed(outer)), sign * (x - 0.02), sign * (x + 0.016), 0.006)

    mirrored(lip)

    # ---- Flares to go over the arches in place of their lips: bolted ones, and moulded.
    for kind in ("bolted", "moulded"):
        def flare(sign, letter, kind=kind):
            for axle, where in ((front_axle, "F"), (rear_axle, "R")):
                if where == "R" and t["back"] == "tray":
                    continue
                inner = arch(axle, hub + 0.02, gap, t["sill"] + 0.03, squared)
                outer = arch(axle, hub + 0.02, gap + (0.07 if kind == "bolted" else 0.09), t["sill"] + 0.03, squared)
                band = inner + list(reversed(outer))
                x = side(axle, hub + 0.02 + gap)
                out = 0.045 if kind == "bolted" else 0.07
                made = piece(f"acc_flares_{kind}_{where}{letter}")
                made.prism("trim" if kind == "bolted" else "paint", band, sign * (x - 0.01), sign * (x + out), 0.012 if kind == "moulded" else 0.004)
                if kind == "bolted":
                    # The bolts that hold it.
                    for point in outer[2:-2:3]:
                        made.turned("steel", [(0.0, 0.009), (0.006, 0.009)], (sign * (x + out) - (0.006 if sign > 0 else 0.0), point[0] - (point[0] - axle) * 0.06, point[1] - 0.012), "x", 8)

        mirrored(flare)

    # ---- Steps under the doors, or sliders: a rail of tube to take a rock.
    def step(sign, letter):
        x = sign * (half - 0.02)
        piece("acc_steps_" + letter).box("trim", (min(x, x + sign * 0.16), spans[-1][2] - 0.05, t["sill"] - 0.075), (max(x, x + sign * 0.16), door_front + 0.05, t["sill"] - 0.035), 0.012)
        for y in (door_front - 0.2, spans[-1][2] + 0.2):
            x0, x1 = sorted((sign * (half - 0.12), sign * (half + 0.03)))
            piece("acc_steps_" + letter).box("iron", (x0, y - 0.02, t["sill"] - 0.05), (x1, y + 0.02, t["sill"] + 0.012))
        rail, z = sign * (half + 0.07), t["sill"] - 0.05
        slider = piece("acc_sliders_" + letter)
        slider.tube("iron", [(sign * (half - 0.14), door_front + 0.12, z + 0.03), (rail, door_front + 0.02, z), (rail, spans[-1][2] - 0.02, z),
                             (sign * (half - 0.14), spans[-1][2] - 0.12, z + 0.03)], 0.03)
        slider.rod("iron", (sign * (half - 0.14), (door_front + spans[-1][2]) / 2, z + 0.03), (rail, (door_front + spans[-1][2]) / 2, z), 0.022, 8)

    mirrored(step)

    # ---- Under it: the chassis, the axles, the springs, the exhaust.
    rail_z = t["sill"] - 0.02 + 0.0
    chassis = piece("chassis")
    rail_top = lambda y: rail_z - 0.01 if y >= kick else (rail_rear if y <= kick - 0.25 else rail_z - 0.01 + (rail_rear - rail_z + 0.01) * (kick - y) / 0.25)
    along = [nose - 0.12, kick, kick - 0.25, tail + 0.06]
    for sign in (-1, 1):
        chassis.prism("iron", [(y, rail_top(y)) for y in along] + [(y, rail_top(y) - 0.12) for y in reversed(along)], sign * 0.4 - 0.035, sign * 0.4 + 0.035)
    for y in (tail + 0.12, rear_axle - 0.45, rear_axle + 0.5, 0.0, front_axle - 0.5, nose - 0.2):
        chassis.box("iron", (-0.4, y - 0.035, rail_top(y) - 0.11), (0.4, y + 0.035, rail_top(y) - 0.02))
    chassis.box("iron", (-0.36, -0.75, rail_z - 0.26), (0.02, -0.05, rail_z - 0.05), 0.03)     # the tank
    chassis.box("iron", (-0.2, front_axle - 0.9, rail_z - 0.2), (0.2, front_axle - 0.25, rail_z + 0.08), 0.05)   # gearbox
    piece("exhaust").tube("steel", [(0.3, front_axle - 0.5, rail_z - 0.16), (0.3, -0.6, rail_z - 0.18), (0.26, rear_axle + 0.5, rail_z - 0.2),
                                    (0.3, rear_axle - 0.2, rail_z - 0.05), (0.42, tail + 0.05, rail_z - 0.12)], 0.028)
    for called, axle in (("axle_front", front_axle), ("axle_rear", rear_axle)):
        made = piece(called)
        reach = t["track"] / 2 - t["tyre_wide"] / 2
        made.rod("iron", (-reach, axle, hub), (reach, axle, hub), 0.05, 16)
        for sign in (-1, 1):
            # A hub at each end of it, to carry the wheel.
            made.turned("iron", [(0.0, 0.05), (0.0, 0.1), (0.05, 0.1), (0.07, 0.06)], (sign * reach - (0.07 if sign > 0 else 0.0), axle, hub), "x", 16)
        made.turned("iron", [(-0.11, 0.05), (-0.09, 0.12), (0.0, 0.15), (0.09, 0.12), (0.11, 0.05)], (0.12 if called == "axle_front" else 0.0, axle, hub))
        made.rod("iron", (0.12 if called == "axle_front" else 0.0, axle, hub), (0.0, axle + (-0.9 if axle > 0 else 0.9), hub + 0.14), 0.03, 10)
        # Its springs and its shock absorbers, of each kind there is: each lifts the body by
        # so much, and is made to reach it there.
        for kind, lift, leaves, thick, colour in SUSPENSION:
            fitted = piece(f"susp_{kind}_{called[5].upper()}")
            fitted.plain = True
            for sign in (-1, 1):
                x = sign * 0.4
                seat = hub + 0.06 + lift * 0.6
                if lift > 0.03:
                    fitted.box("iron", (x - 0.035, axle - 0.06, hub + 0.04), (x + 0.035, axle + 0.06, seat))    # a block under the spring
                for leaf in range(leaves):
                    length = 0.62 - 0.3 * leaf / max(leaves - 1, 1)
                    fitted.box("iron", (x - 0.03, axle - length, seat + leaf * 0.014), (x + 0.03, axle + length, seat + 0.012 + leaf * 0.014))
                top = seat + leaves * 0.014
                for end in (-0.6, 0.6):
                    if rail_top(axle + end) - 0.12 > top:
                        fitted.box("iron", (x - 0.022, axle + end - 0.025, top - 0.03), (x + 0.022, axle + end + 0.025, rail_top(axle + end) - 0.11 + lift))
                fitted.box("iron", (x - 0.05, axle - 0.055, hub - 0.065), (x + 0.05, axle + 0.055, top + 0.012))      # the plate that clamps it to the axle
                low = Vector((x + sign * 0.1, axle + 0.06, hub + 0.02))
                high = Vector((x + sign * 0.07, axle + 0.1, hub + gap - 0.07 + lift))
                fitted.rod("chrome" if kind != "stock" else "iron", low, high, thick * 0.42, 10)
                fitted.rod(colour, low, low + (high - low) * 0.6, thick, 12)
                if kind in ("lift", "long"):
                    # A reservoir beside it, for the oil.
                    beside = Vector((sign * 0.055, -0.03, 0.0))
                    fitted.rod(colour, low + (high - low) * 0.2 + beside, low + (high - low) * 0.58 + beside, thick * 0.7, 10)
                if kind == "long":
                    # And a coil about it.
                    along = (high - low).normalized()
                    out = along.cross(Vector((0.0, 1.0, 0.0))).normalized()
                    third = along.cross(out)
                    turns, steps = 6, 8
                    coil = []
                    for step in range(turns * steps + 1):
                        angle = math.tau * step / steps
                        coil.append(low + (high - low) * (0.5 + 0.44 * step / (turns * steps)) + (out * math.cos(angle) + third * math.sin(angle)) * (thick + 0.022))
                    fitted.tube("shock_blue", coil, 0.007, 6, False)

    # ---- Mirrors: the truck's own.
    def mirror(sign, letter):
        y, z = screen_at(t["belt"] + 0.1) - 0.02, t["belt"] + 0.13
        x = sign * side(y, z)
        made = piece("mirror_" + letter)
        head = lambda x0, x1, low_z, high_z, called: made.box(called, (min(x0, x1), y - 0.012, low_z), (max(x0, x1), y + 0.05, high_z), 0.02)
        if t["mirror"] == "stalk":
            made.rod("chrome", (x, y, z - 0.04), (x + sign * 0.13, y + 0.02, z), 0.01, 8)
            made.turned("chrome", [(0.0, 0.0), (0.02, 0.07), (0.03, 0.07)], (x + sign * 0.15, y - 0.01, z + 0.02), "y", 18)
        elif t["mirror"] == "arm":
            # Two arms and a tall head: a mirror to see a trailer with.
            for height in (z - 0.08, z + 0.12):
                made.rod("trim", (x, y - 0.02, height), (x + sign * 0.2, y + 0.02, height), 0.011, 8)
            head(x + sign * 0.17, x + sign * 0.24, z - 0.13, z + 0.17, "trim")
        else:
            made.box("trim", (min(x - sign * 0.01, x + sign * 0.06), y - 0.01, z - 0.045), (max(x - sign * 0.01, x + sign * 0.06), y + 0.045, z - 0.005), 0.012)
            head(x + sign * 0.04, x + sign * 0.2, z - 0.05, z + 0.08, "paint")

    mirrored(mirror)

    # ---- The kit: what can be bolted on. Every kind of every thing is made, to fit this
    # truck, each a piece of its own: the game fits one or leaves it off.
    out = nose + 0.2
    low_z, high_z = front_floor - 0.08, t["bonnet_front"] + 0.02
    for kind in ("tube", "plate", "nudge"):
        bar = piece("acc_bullbar_" + kind)
        metal = "steel" if kind == "tube" and t["grade"] < 4 else "iron"
        if kind == "nudge":
            bar.tube(metal, [(-0.36, nose + 0.1, low_z + 0.05), (-0.36, out - 0.04, low_z + 0.12), (-0.34, out - 0.02, high_z - 0.12),
                             (0.34, out - 0.02, high_z - 0.12), (0.36, out - 0.04, low_z + 0.12), (0.36, nose + 0.1, low_z + 0.05)], 0.03)
            bar.box(metal, (-0.34, out - 0.05, low_z + 0.02), (0.34, out - 0.02, low_z + 0.1), 0.012)
        elif kind == "plate":
            # A winch bar: a deep plate with wings, a fairlead, and one hoop over the grille.
            bar.box(metal, (-0.5, nose + 0.08, low_z - 0.04), (0.5, out, low_z + 0.2), 0.02)
            for sign in (-1, 1):
                x0, x1 = sorted((sign * 0.5, sign * (half - 0.01)))
                bar.prism(metal, [(nose - 0.12, low_z + 0.02), (out - 0.03, low_z - 0.02), (out - 0.01, low_z + 0.19), (nose - 0.12, low_z + 0.16)], x0, x1, 0.015)
                bar.box("hook", (sign * 0.42 - 0.025, out - 0.01, low_z - 0.02), (sign * 0.42 + 0.025, out + 0.05, low_z + 0.05), 0.012)
            bar.box("steel", (-0.14, out - 0.004, low_z + 0.05), (0.14, out + 0.016, low_z + 0.12), 0.006)
            bar.tube(metal, [(-0.36, out - 0.04, low_z + 0.2), (-0.34, out - 0.02, high_z), (0.34, out - 0.02, high_z), (0.36, out - 0.04, low_z + 0.2)], 0.028)
        else:
            bar.box(metal, (-half + 0.02, nose + 0.1, low_z - 0.02), (half - 0.02, out, low_z + 0.13), 0.02)
            for sign in (-1, 1):
                bar.tube(metal, [(sign * 0.36, out - 0.03, low_z + 0.12), (sign * 0.36, out - 0.02, high_z), (sign * 0.3, out - 0.02, high_z + 0.02)], 0.028)
                bar.tube(metal, [(sign * 0.36, out - 0.03, low_z + 0.3), (sign * (half - 0.1), out - 0.06, low_z + 0.3), (sign * (half - 0.02), nose - 0.1, low_z + 0.32)], 0.024)
            bar.rod(metal, (-0.36, out - 0.02, high_z), (0.36, out - 0.02, high_z), 0.028)
    # Spot lamps for the bar: for the hoop of a tube or a winch bar, and lower for a nudge bar.
    for called, drop in (("acc_spots", 0.0), ("acc_spots_low", 0.12)):
        spots = piece(called)
        for sign in (-1, 1):
            x, z = sign * 0.2, high_z - drop + 0.1
            spots.turned("trim", [(-0.05, 0.085), (0.0, 0.1), (0.02, 0.1)], (x, out - 0.03, z), "y", 24)
            spots.turned("lamp", [(0.012, 0.0), (0.022, 0.09)], (x, out - 0.03, z), "y", 24)
            spots.rod("trim", (x, out - 0.04, z - 0.1), (x, out - 0.03, high_z - drop), 0.012, 6)
    snorkel = piece("acc_snorkel")
    y = cowl + 0.12
    x = half + 0.035
    snorkel.tube("trim", [(x - 0.04, y + 0.25, t["bonnet_back"] - 0.12), (x, y, t["bonnet_back"] - 0.02), (x - t["tumble"] * 0.6, screen_at(t["roof"] - 0.2) + 0.05, t["roof"] - 0.2),
                          (x - t["tumble"], screen_top + 0.02, t["roof"] + 0.07)], 0.04)
    snorkel.box("trim", (x - t["tumble"] - 0.055, screen_top - 0.01, t["roof"] + 0.05), (x - t["tumble"] + 0.055, screen_top + 0.1, t["roof"] + 0.17), 0.02)
    rack = piece("acc_rack")
    start, end = screen_top - 0.15, (roof_back + 0.2 if wagon else roof_back + 0.1)
    reach = side(0.0, t["roof"]) - 0.08
    z = t["roof"] + 0.09
    for sign in (-1, 1):
        rack.rod("alloy", (sign * reach, start, z), (sign * reach, end, z), 0.018, 8)
        for y in (start - 0.08, (start + end) / 2, end + 0.08):
            rack.rod("iron", (sign * reach, y, z), (sign * (reach - 0.03), y, t["roof"] - 0.01), 0.012, 6)
    count = max(3, int((start - end) / 0.2))
    for row in range(count + 1):
        y = start + (end - start) * row / count
        rack.box("alloy", (-reach, y - 0.02, z - 0.01), (reach, y + 0.02, z + 0.01), 0.004)
    # A light bar: on the roof, or on the rack if there is one.
    for called, up in (("acc_lightbar", 0.06), ("acc_lightbar_high", 0.16)):
        z = t["roof"] + up
        y = screen_top - 0.2
        piece(called).box("trim", (-0.5, y - 0.03, z - 0.035), (0.5, y + 0.04, z + 0.035), 0.012)
        piece(called).box("lamp", (-0.48, y + 0.035, z - 0.025), (0.48, y + 0.046, z + 0.025), 0.004)
        for sign in (-1, 1):
            piece(called).box("trim", (sign * 0.42 - 0.02, y - 0.02, t["roof"] - 0.01), (sign * 0.42 + 0.02, y + 0.03, z - 0.03))
    if wagon:
        # A spare wheel on the back door.
        turn = Matrix.Translation((0.18, back_at(t["belt"] - 0.05) - t["tyre_wide"] / 2 - 0.03, t["belt"] - 0.02)) @ Matrix.Rotation(math.pi / 2, 4, "Z")
        piece("acc_spare").plain = True
        tyre(piece("acc_spare"), t, "at", turn)
        rim(piece("acc_spare"), t, t["rim"], turn)

    made = {called: each.finish(paint) for called, each in pieces.items()}
    # What swings is made about its hinge.
    for called, (at, axis, opens) in hinges.items():
        made[called].data.transform(Matrix.Translation(-Vector(at)))
        made[called].location = at

    # ---- The wheels. Each kind of tyre and each kind of rim is made once, about a hub, and
    # stands at all four: so a file has one of each, however many wheels show it.
    corners = (("FL", -1, front_axle), ("FR", 1, front_axle), ("RL", -1, rear_axle), ("RR", 1, rear_axle))

    def four(called, make):
        first = Piece(called)
        first.plain = True
        make(first)
        one = first.finish(paint)
        bpy.context.view_layer.update()
        mesh = bpy.data.meshes.new_from_object(one.evaluated_get(bpy.context.evaluated_depsgraph_get()))
        mesh.name = called
        bpy.data.objects.remove(one, do_unlink=True)
        names = []
        for letter, x, y in corners:
            wheel = bpy.data.objects.new(f"{called}_{letter}", mesh)
            wheel.location = (x * t["track"] / 2, y, hub)
            wheel.rotation_euler = (0.0, 0.0, math.pi if x > 0 else 0.0)
            bpy.context.scene.collection.objects.link(wheel)
            made[wheel.name] = wheel
            names.append(wheel.name)
        return names

    groups = {
        "bullbar": {kind: ["acc_bullbar_" + kind] for kind in ("tube", "plate", "nudge")},
        "spots": {"high": ["acc_spots"], "low": ["acc_spots_low"]},
        "snorkel": {"fitted": ["acc_snorkel"]},
        "rack": {"fitted": ["acc_rack"]},
        "lightbar": {"roof": ["acc_lightbar"], "rack": ["acc_lightbar_high"]},
        "arches": {"lip": [called for called in made if called.startswith("lip_")],
                   "bolted": [called for called in made if called.startswith("acc_flares_bolted_")],
                   "moulded": [called for called in made if called.startswith("acc_flares_moulded_")]},
        "steps": {"steps": ["acc_steps_L", "acc_steps_R"], "sliders": ["acc_sliders_L", "acc_sliders_R"]},
        "suspension": {kind: [f"susp_{kind}_F", f"susp_{kind}_R"] for kind, *_ in SUSPENSION},
        "tyres": {tread: four("tyre_" + tread, lambda made_piece, tread=tread: tyre(made_piece, t, tread)) for tread in TREADS},
        "rims": {kind: four("rim_" + kind, lambda made_piece, kind=kind: rim(made_piece, t, kind)) for kind in RIMS},
    }
    if t["back"] == "tub":
        groups["tub"] = {"sportsbar": ["acc_sportsbar"], "tonneau": ["acc_tonneau"], "canopy": ["acc_canopy"]}
    elif t["back"] == "tray":
        groups["tray"] = {"timber": ["acc_tray_timber"], "boxes": ["acc_tray_boxes"]}
    else:
        groups["spare"] = {"fitted": ["acc_spare"]}
    # What it wears as it is sold, with nothing added; and what it wears in its pictures.
    modern = t["era"] in ("noughties", "tens", "twenties")
    stock = {group: None for group in groups}
    stock.update(suspension="stock", arches="moulded" if t["flares"] == "moulded" and t["kind"] in ("wide", "full", "square", "modern") else "lip",
                 tyres="road" if modern else "at", rims=t["rim"] if t["rim"] != "beadlock" else ("spoke6" if modern else "slot8"))
    if "spare" in groups and t["kind"] == "square":
        stock["spare"] = "fitted"
    fit = t["fit"]
    showroom = dict(stock)
    showroom.update(
        bullbar=t["bar"] if "bullbar" in fit else None,
        spots=("low" if t["bar"] == "nudge" else "high") if t["spots"] and "bullbar" in fit else None,
        snorkel="fitted" if "snorkel" in fit else None,
        rack="fitted" if "rack" in fit else None,
        lightbar=("rack" if "rack" in fit else "roof") if "lightbar" in fit else None,
        arches={"none": "lip", "bolted": "bolted", "moulded": "moulded"}[t["flares"]],
        steps="sliders" if t["grade"] == 4 else ("steps" if modern else None),
        suspension={1: "stock", 2: "stock", 3: "heavy", 4: "lift", 5: "long" if t["kind"] in ("wide", "full") else "lift"}[t["grade"]],
        tyres=t["tread"] if t["tread"] != "at" or not modern else stock["tyres"], rims=t["rim"],
    )
    if "tub" in groups:
        showroom["tub"] = t["tub_style"] if t["tub_style"] != "plain" else None
    if "tray" in groups:
        showroom["tray"] = t["tray_style"] if t["tray_style"] in ("timber", "boxes") else None
    if "spare" in groups and "spare" in fit:
        showroom["spare"] = "fitted"
    fitted = {called for options in groups.values() for names in options.values() for called in names}
    numbers = dict(front_axle=front_axle, rear_axle=rear_axle, track=t["track"], tyre=t["tyre"], nose=nose, tail=tail, wide=t["wide"], roof=t["roof"],
                   hub=hub, sill=t["sill"], wheelbase=t["wheelbase"], arch=gap, tyre_wide=t["tyre_wide"], rim=wheel_size(t)[2])
    parts = dict(name=name, called=t["called"], year=t["year"], grade=t["grade"], numbers={key: round(value, 4) for key, value in numbers.items()},
                 base=sorted(called for called in made if called not in fitted), groups=groups, stock=stock, showroom=showroom,
                 # Fitting one thing takes another off: a winch bar is the bumper, and flares cover the lips.
                 replaces={"bullbar.plate": ["bumper_front", "plate_front"]},
                 # What stays with the axles when the body is lifted, and how far each kind of spring lifts it.
                 unsprung=sorted(called for called in made if called.startswith(("axle_", "susp_", "tyre_", "rim_"))),
                 lift={kind: lift for kind, lift, *_ in SUSPENSION},
                 # What swings open, about its own origin; where gold can be put, and behind which doors; and what rides on a door.
                 hinges={called: dict(at=[round(v, 4) for v in at], axis=axis, opens=opens) for called, (at, axis, opens) in hinges.items()},
                 holds=[dict(name=called, low=[round(v, 3) for v in low], high=[round(v, 3) for v in high], doors=behind) for called, low, high, behind in holds]
                 + ([dict(name="bed", low=[round(-t["bed"][0], 3), round(t["bed"][3], 3), round(t["bed"][1], 3)],
                          high=[round(t["bed"][0], 3), round(t["bed"][2], 3), round(t["bed"][1] + 0.35, 3)], doors=[])] if "bed" in t else []),
                 rides={"acc_spare": "door_back"} if wagon else {})
    return list(made.values()), numbers, t, parts


# Springs and shocks: what it is called, how far it lifts the body, how many leaves are in
# its springs, how thick its shock absorbers are, and what colour.
SUSPENSION = (("stock", 0.0, 3, 0.022, "iron"), ("heavy", 0.025, 5, 0.027, "shock_yellow"), ("lift", 0.05, 5, 0.031, "shock_red"), ("long", 0.1, 4, 0.036, "shock_blue"))
TREADS = ("road", "at", "mud")
RIMS = ("slot8", "sunraysia", "spoke5", "spoke6", "mesh", "beadlock")


def wheel_size(t):
    """A wheel's radius, how wide it is, and its rim's radius. A new truck's wheels are
    bigger in their tyres than an old one's. Every tyre fits every rim."""
    radius, rim = t["tyre"], t["rim_radius"]
    return radius, t["tyre_wide"], max(rim, radius * {"tens": 0.57, "twenties": 0.6, "noughties": 0.54}.get(t.get("era"), 0.0))


def tyre(piece, t, tread, turn=None):
    """A tyre about the origin, its axle along x: road, all-terrain or mud."""
    turn = turn or Matrix.Identity(4)
    radius, wide, rim = wheel_size(t)
    half = wide / 2
    carcass = Piece("carcass")
    carcass.turned("tyre", [(-half * 0.55, rim), (-half * 0.96, rim + (radius - rim) * 0.3), (-half * 1.02, rim + (radius - rim) * 0.62), (-half * 0.93, radius - 0.022),
                            (-half * 0.78, radius - 0.012), (half * 0.78, radius - 0.012), (half * 0.93, radius - 0.022), (half * 1.02, rim + (radius - rim) * 0.62),
                            (half * 0.96, rim + (radius - rim) * 0.3), (half * 0.55, rim)], (0, 0, 0), "x", 56, False)
    piece.take(carcass.bm, "tyre", turn)
    tread_blocks = Piece("tread")
    if tread == "road":
        # Ribs round it, with fine cuts across them.
        for x0, x1 in ((-half * 0.8, -half * 0.5), (-half * 0.4, -half * 0.08), (half * 0.08, half * 0.4), (half * 0.5, half * 0.8)):
            count = 64
            for step in range(count):
                block = Matrix.Rotation(math.tau * step / count, 4, "X") @ Matrix.Translation((0, 0, radius - 0.012))
                tread_blocks.box("tyre", (x0, -math.tau * radius / count * 0.44, -0.004), (x1, math.tau * radius / count * 0.44, 0.008), 0.0, block)
    else:
        # Blocks in two rows, staggered, and lugs on the shoulders: bigger and fewer for mud.
        mud = tread == "mud"
        blocks = 20 if mud else 30
        high = 0.02 if mud else 0.012
        for step in range(blocks):
            for x0, x1, shift, down in ((-half * 0.84, -half * 0.08, 0.0, 0.0), (half * 0.08, half * 0.84, 0.5, 0.0),
                                        (-half * 1.06, -half * 0.78, 0.25, -0.022), (half * 0.78, half * 1.06, 0.75, -0.022)):
                angle = math.tau * (step + shift) / blocks
                block = Matrix.Rotation(angle, 4, "X") @ Matrix.Translation((0, 0, radius - 0.012 + down))
                arc = math.tau * radius / blocks * (0.52 if mud else 0.6)
                tread_blocks.box("tyre", (x0, -arc / 2, -0.004), (x1, arc / 2, high), 0.003, block, 1)
    piece.take(tread_blocks.bm, "tyre", turn)
    return piece


def rim(piece, t, kind, turn=None):
    """A rim about the origin, its face towards -x: its barrel and its lip, and then its face."""
    turn = turn or Matrix.Identity(4)
    radius, wide, rim = wheel_size(t)
    half = wide / 2
    metal = t.get("rim_metal", "steel")
    steel = Piece("rim")
    steel.turned(metal, [(-half * 0.5, rim), (-half * 0.58, rim + 0.012), (-half * 0.5, rim - 0.01), (-half * 0.36, rim - 0.026),
                         (half * 0.5, rim - 0.02), (half * 0.56, rim)], (0, 0, 0), "x", 44, False)
    dark = Piece("dark")
    dark.turned("inside_dark", [(-half * 0.05, 0.0), (-half * 0.05, rim - 0.03)], (0, 0, 0), "x", 32, False)
    face = -half * 0.34
    if kind in ("slot8", "sunraysia", "beadlock"):
        # A pressed steel dish, with slots or round holes in it.
        steel.turned(metal, [(face, rim - 0.026), (face + half * 0.06, rim * 0.5), (-half * 0.52, rim * 0.36), (-half * 0.54, 0.0)], (0, 0, 0), "x", 44, False)
        for step in range(8):
            angle = math.tau * (step + 0.5) / 8
            at = Matrix.Rotation(angle, 4, "X") @ Matrix.Translation((0, 0, rim * 0.74))
            if kind == "sunraysia":
                dark.rod("inside_dark", at @ Vector((face - 0.006, 0.0, 0.0)), at @ Vector((face + 0.02, 0.0, 0.0)), 0.027, 14)
            else:
                dark.box("inside_dark", (face - 0.006, -0.024, -0.032), (face + 0.03, 0.024, 0.032), 0.01, at)
        if kind == "beadlock":
            steel.turned("steel", [(-half * 0.6, rim + 0.016), (-half * 0.62, rim + 0.004), (-half * 0.62, rim - 0.022), (-half * 0.56, rim - 0.024)], (0, 0, 0), "x", 44, False)
            for step in range(16):
                angle = math.tau * step / 16
                steel.rod("steel", (-half * 0.66, (rim - 0.008) * math.cos(angle), (rim - 0.008) * math.sin(angle)),
                          (-half * 0.6, (rim - 0.008) * math.cos(angle), (rim - 0.008) * math.sin(angle)), 0.007, 6)
    else:
        # An alloy: spokes from its hub to its rim.
        count, thick = {"spoke5": (5, 0.062), "spoke6": (6, 0.05), "mesh": (14, 0.02)}[kind]
        for step in range(count):
            at = Matrix.Rotation(math.tau * step / count, 4, "X")
            steel.box(metal, (face - 0.012, -thick / 2, 0.05), (face + 0.03, thick / 2, rim - 0.018), 0.008, at)
            if kind == "mesh":
                lean = Matrix.Rotation(math.tau * (step + 0.5) / count, 4, "X") @ Matrix.Rotation(0.22, 4, "X")
                steel.box(metal, (face - 0.006, -thick / 2, 0.07), (face + 0.024, thick / 2, rim - 0.02), 0.006, lean)
        steel.turned(metal, [(face - 0.02, 0.0), (face - 0.02, 0.07), (face + 0.03, 0.085)], (0, 0, 0), "x", 28, False)
    for step in range(5 if kind != "spoke6" else 6):
        angle = math.tau * step / (5 if kind != "spoke6" else 6)
        steel.rod("chrome", (face - 0.03, 0.045 * math.cos(angle), 0.045 * math.sin(angle)), (face - 0.016, 0.045 * math.cos(angle), 0.045 * math.sin(angle)), 0.009, 6)
    steel.turned("chrome" if metal == "steel" else metal, [(face - 0.034, 0.0), (face - 0.034, 0.026), (face - 0.016, 0.032)], (0, 0, 0), "x", 18, False)
    if t.get("hubcap") and kind in ("slot8", "sunraysia"):
        # A chrome cap over the hub and its nuts: the seventies.
        steel.turned("chrome", [(face - 0.06, 0.0), (face - 0.055, 0.05), (face - 0.035, 0.085), (face - 0.01, 0.095)], (0, 0, 0), "x", 28, False)
    piece.take(dark.bm, "inside_dark", turn)
    move_rim(piece, steel, turn)
    return piece


def move_rim(piece, steel, turn):
    """Brings a rim made apart into a wheel, each of its faces keeping its own material."""
    made = {}
    for vert in steel.bm.verts:
        made[vert] = piece.bm.verts.new(turn @ vert.co)
    for face in steel.bm.faces:
        try:
            new = piece.bm.faces.new([made[vert] for vert in face.verts])
        except ValueError:
            continue
        new.material_index = piece.slot(steel.materials[face.material_index])
        new.smooth = True
    steel.bm.free()


# ============================================================================ Pictures.
def dress(parts, build, lift=True):
    """Shows the pieces of a build and hides the rest: the base, and of each group the
    choice the build names. The body is lifted as its springs lift it."""
    shown = set(parts["base"])
    for group, choice in build.items():
        if choice:
            shown.update(parts["groups"][group][choice])
    for group, choice in build.items():
        if choice:
            shown.difference_update(parts["replaces"].get(f"{group}.{choice}", ()))
    rise = parts["lift"].get(build.get("suspension") or "stock", 0.0) if lift else 0.0
    for each in bpy.context.scene.objects:
        if each.type != "MESH" or each.name in ("ground",):
            continue
        each.hide_render = each.name not in shown
        if each.name not in parts["unsprung"]:
            # From where it rests: a door's own place is at its hinge, not at the ground.
            if "rest" not in each:
                each["rest"] = each.location.z
            each.location.z = each["rest"] + rise
    return shown


def swing(parts, share):
    """Opens what swings, so far of all the way: doors, a tailgate."""
    for called, hinge in parts["hinges"].items():
        angle = math.radians(hinge["opens"]) * share
        bpy.data.objects[called].rotation_euler = (0.0, 0.0, angle) if hinge["axis"] == "up" else (angle, 0.0, 0.0)


def pictures(folder, name, numbers, views=("front", "back", "side"), size=(1280, 720), close=None):
    """`close` is a view of its own: where the camera is, what it looks at, and what the
    picture is called."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 64
    scene.cycles.use_denoising = True
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "OPTIX"
        prefs.get_devices()
        for device in prefs.devices:
            device.use = True
        scene.cycles.device = "GPU"
    except Exception as error:
        print("rendering on the processor:", error)
    scene.render.resolution_x, scene.render.resolution_y = size
    scene.view_settings.view_transform = "AgX"
    world = bpy.data.worlds.new("sky")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.6, 0.72, 1.0, 1.0)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 1.0
    scene.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy, sun.data.angle = 4.0, math.radians(2.0)
    sun.rotation_euler = (math.radians(50), 0.0, math.radians(-140))
    scene.collection.objects.link(sun)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-80, -80, 0), (80, -80, 0), (80, 80, 0), (-80, 80, 0)], [], [(0, 1, 2, 3)])
    dirt = bpy.data.materials.new("dirt")
    dirt.use_nodes = True
    dirt.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.5, 0.3, 0.19, 1.0)
    dirt.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    ground.materials.append(dirt)
    scene.collection.objects.link(bpy.data.objects.new("ground", ground))
    camera = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    scene.collection.objects.link(camera)
    scene.camera = camera
    # In a picture the lamps are off: the game lights them.
    lit = []
    for made in bpy.data.materials:
        shader = made.node_tree.nodes.get("Principled BSDF") if made.use_nodes else None
        if shader and made.name.startswith("lamp"):
            lit.append((shader, shader.inputs["Emission Strength"].default_value))
            shader.inputs["Emission Strength"].default_value = 0.04 if made.name == "lamp" else 0.25
    long = numbers["nose"] - numbers["tail"]
    middle = Vector((0.0, (numbers["nose"] + numbers["tail"]) / 2, numbers["roof"] * 0.48))
    angles = [("front", (-long * 0.85, long * 1.25, numbers["roof"] * 0.95)), ("back", (long * 0.95, -long * 1.2, numbers["roof"] * 1.05)),
              ("side", (-long * 1.6, middle.y, numbers["roof"] * 0.6)),
              ("face", (-numbers["wide"] * 0.55, numbers["nose"] + numbers["wide"] * 1.45, numbers["roof"] * 0.62))]
    for view, at, towards in ([(view, at, None) for view, at in angles if view in views] + list(close or ())):
        if towards is not None:
            middle = Vector(towards)
        elif view == "face":
            middle = Vector((0.0, numbers["nose"], numbers["roof"] * 0.5))
        else:
            middle = Vector((0.0, (numbers["nose"] + numbers["tail"]) / 2, numbers["roof"] * 0.48))
        camera.location = at
        camera.data.lens = 52
        camera.rotation_euler = (middle - Vector(at)).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.abspath(os.path.join(folder, f"{name}_{view}.png"))
        bpy.ops.render.render(write_still=True)
    for shader, strength in lit:
        shader.inputs["Emission Strength"].default_value = strength
    for called in ("sun", "ground", "camera"):
        bpy.data.objects.remove(bpy.data.objects[called], do_unlink=True)


def hold_lines(parts):
    """The places in a truck's cab that things can be put, as lines of its numbers: each a
    box shut in, its size across, high and long, and the middle of its floor, in the game's
    directions. The body is hollow there (`solstice-physics`, `car.rs`). A tub or a tray is
    not one of them: it is open, and is the truck's `tray`."""
    lines = ""
    for hold in parts["holds"]:
        if not hold["doors"]:
            continue
        low, high = hold["low"], hold["high"]
        lines += (f"hold {high[0] - low[0]:.3f} {high[2] - low[2]:.3f} {high[1] - low[1]:.3f} "
                  f"at {(low[0] + high[0]) / 2:.3f} {low[2]:.3f} {-(low[1] + high[1]) / 2:.3f}\n")
    return lines


def game_files(name, t, numbers, parts, out, folder):
    """Writes what the game reads of a truck, into `folder`: the list of its kit
    (`NAME.kit`), its actor (`NAME.actor`) and its numbers (`NAME.vehicle`).

    The kit is the parts list as lines: `base PIECE`, `choice GROUP CHOICE PIECE...`,
    `stock GROUP CHOICE` and `showroom GROUP CHOICE` for what it wears bare and in its
    pictures, `replaces GROUP CHOICE PIECE...` for what a choice takes off, and
    `lift CHOICE METRES` for how far each kind of spring lifts it.

    The numbers are a first reckoning from its size, its years and its grade, to be driven
    and put right: its weight from its bulk, its springs for its weight, an engine of its
    grade, and the aids its years had."""
    os.makedirs(folder, exist_ok=True)
    lines = [f"# {t['called']} ({t['year']}): what it is made of. Written by tools/blender/truck/build.py."]
    lines += [f"base {called}" for called in parts["base"]]
    for group, options in parts["groups"].items():
        lines += [f"choice {group} {choice} {' '.join(names)}" for choice, names in options.items()]
    for what in ("stock", "showroom"):
        lines += [f"{what} {group} {choice}" for group, choice in parts[what].items() if choice]
    lines += [f"replaces {key.replace('.', ' ')} {' '.join(names)}" for key, names in parts["replaces"].items()]
    lines += [f"lift {kind} {lift}" for kind, lift in parts["lift"].items()]
    # The colour its paint was made in, as light: what the game tells a respray from.
    lines.append("paint " + " ".join(f"{channel:.4f}" for channel in t["paint"]))
    turned = lambda at: f"{at[0]:.3f} {at[2]:.3f} {-at[1]:.3f}"
    lines += [f"hinge {called} {turned(hinge['at'])} {hinge['axis']} {hinge['opens']:.0f}" for called, hinge in parts["hinges"].items()]
    for hold in parts["holds"]:
        low, high = hold["low"], hold["high"]
        size = f"{high[0] - low[0]:.3f} {high[2] - low[2]:.3f} {high[1] - low[1]:.3f}"
        middle = turned([(low[axis] + high[axis]) / 2 for axis in range(3)])
        lines.append(f"hold {hold['name']} {size} at {middle}" + "".join(f" door {door}" for door in hold["doors"]))
    lines += [f"rides {called} {on}" for called, on in parts["rides"].items()]
    with open(os.path.join(folder, name + ".kit"), "w", encoding="utf-8", newline="\n") as file:
        file.write("\n".join(lines) + "\n")

    long = numbers["nose"] - numbers["tail"]
    wide, roof, sill, radius = numbers["wide"], numbers["roof"], numbers["sill"], numbers["tyre"]
    with open(os.path.join(folder, name + ".actor"), "w", encoding="utf-8", newline="\n") as file:
        file.write(f"# {t['called']} ({t['year']}): {t['about']}\n# Written by tools/blender/truck/build.py, with its model.\n"
                   f"name {name}\nmesh {os.path.relpath(out, folder).replace(os.sep, '/')} 1\nvehicle {name}.vehicle\nkit {name}.kit\n"
                   f"camera 0 {roof - 0.32:.2f} 0\n")

    modern = t["era"] in ("noughties", "tens", "twenties")
    mass = round(118 * long * wide * roof * (0.94 if t["back"] == "tray" else 1.0), -1)
    share = mass / 2800.0       # of the Raptor's weight, whose springs these are scaled from
    stiff = 1.25 if not modern else 1.0     # leaf springs all round
    travel = {1: 0.09, 2: 0.11, 3: 0.12, 4: 0.14, 5: 0.16}[t["grade"]]
    kilowatts = {1: 58, 2: 95, 3: 140, 4: 165, 5: 290}[t["grade"]] * (mass / 2000.0) ** 0.5
    petrol = t["kind"] == "tiny" or (t["grade"] == 5 and t["kind"] in ("wide", "full"))
    if petrol:
        peak = kilowatts * 1000 / (0.95 * 5000 * math.tau / 60)
        curve, idle, limit = [(1000, 0.6), (2000, 0.8), (3000, 0.92), (4000, 1.0), (5000, 0.95), (5800, 0.85), (6200, 0.7)], 750, 6200
    else:
        peak = kilowatts * 1000 / (0.85 * 3400 * math.tau / 60)
        curve, idle, limit = [(1000, 0.6), (1400, 0.9), (1800, 1.0), (2600, 1.0), (3400, 0.85), (4000, 0.65), (4400, 0.4)], 750, 4400
    if t["grade"] == 5:
        gears, final = "4.696 2.985 2.146 1.769 1.52 1.275 1 0.854 0.689 0.636", 3.9
    elif modern:
        gears, final = "4.17 2.34 1.52 1.14 0.87 0.69", 3.9
    else:
        gears, final = "3.93 2.33 1.45 1 0.82", 4.3 if t["grade"] == 1 else 4.1
    half_base = numbers["wheelbase"] / 2
    wheel = 3.6 * (radius / 0.4365) ** 3
    text = f"""# {t['called']} ({t['year']}): {t['about']}
# A first reckoning from its size, its years and its grade (tools/blender/truck/build.py):
# to be driven, and put right.

mass {mass:.0f}
inertia {7600 * share * (long / 5.9) ** 2:.0f} {8300 * share * (long / 5.9) ** 2:.0f} {1800 * share * (wide / 2.2) ** 2:.0f}
centre 0 {sill + 0.3:.2f} {-0.06 * numbers['wheelbase']:.2f}
body {wide:.2f} {roof - sill - 0.1:.2f} {long - 0.1:.2f} at 0 {(roof + sill) / 2 + 0.05:.2f} {-(numbers['nose'] + numbers['tail']) / 2:.2f}
"""
    if "bed" in t:
        reach, floor, front, back = t["bed"]
        text += f"tray {2 * reach:.2f} 0.35 {front - back:.2f} at 0 {floor + 0.175:.2f} {-(front + back) / 2:.2f}\n"
    text += "driver right\n" + hold_lines(parts)
    text += f"""
front at={half_base:.3f} track={numbers['track']:.3f} radius={radius:.4f} inertia={wheel:.2f} driven=yes
front spring={52000 * share * stiff:.0f} bump={5400 * share * stiff:.0f} rebound={10200 * share * stiff:.0f} bar={11000 * share:.0f} travel={travel:.2f},{travel - 0.01:.2f} roll_centre=0.15
front brake={3900 * share:.0f} grip=1
rear at={half_base:.3f} track={numbers['track']:.3f} radius={radius:.4f} inertia={wheel * 1.15:.2f} driven=yes
rear spring={44000 * share * stiff:.0f} bump={4700 * share * stiff:.0f} rebound={8900 * share * stiff:.0f} bar={3000 * share:.0f} travel={travel + 0.02:.2f},{travel:.2f} roll_centre=0.35
rear brake={2300 * share:.0f} grip=1

tyre grip=1.0,0.95 peak=0.13,9 along=1.5,0.3 across=1.35,0.1
tyre load={7000 * share:.0f} sensitivity=0.1 relaxation=0.5

engine idle={idle} limit={limit} inertia={0.5 * (kilowatts / 537) ** 0.5 + 0.12:.2f} friction={45 * (kilowatts / 537) ** 0.5 + 8:.0f},0.012
""" + "".join(f"torque {revs} {peak * part:.0f}\n" for revs, part in curve) + f"""
gears {gears}
final {final}
reverse 4.2
gearbox shift={0.18 if t['grade'] == 5 else 0.35} efficiency=0.88 clutch={max(600, 1500 * share):.0f} cruise=0.42
diff preload=40 power=0 coast=0
front_diff preload=20 power=0 coast=0
transfer low={2.64 if t['grade'] == 5 else 2.3} front=0.5 lock=30000 locker=30000

aero drag={1.5 * wide * roof / 4.4:.2f} downforce=0 balance=0.5
steering lock={36 if t['kind'] == 'tiny' else 33} ackermann=0.7 quick=0.55
geometry anti_dive=0.2 anti_squat=0.25
assists abs={'yes' if modern else 'no'} traction={'yes' if t['era'] in ('tens', 'twenties') else 'no'} stability=no descent={'yes' if t['era'] in ('tens', 'twenties') else 'no'}
"""
    with open(os.path.join(folder, name + ".vehicle"), "w", encoding="utf-8", newline="\n") as file:
        file.write(text)


MOST = 60000        # triangles in one piece: more is a piece made too finely


def audit(parts, numbers):
    """Looks at every piece as it will be written, and says what is wrong with it. A
    **fault** is a thing that will show or will break the game: a piece with nothing in it,
    a face of no size, a face turned inside out, a piece out of place, one side not the
    mirror of the other, a wheel not about its hub. A **note** is a thing to know: an edge
    left open, a piece made finely. Returns what it found of each piece."""
    graph = bpy.context.evaluated_depsgraph_get()
    found = {}
    group_of = {called: f"{group}.{choice}" for group, options in parts["groups"].items() for choice, names in options.items() for called in names}
    boxes = {}
    for each in bpy.context.scene.objects:
        if each.type != "MESH":
            continue
        faults, notes = [], []
        bm = bmesh.new()
        bm.from_object(each, graph)
        bm.transform(each.matrix_world)
        bmesh.ops.triangulate(bm, faces=bm.faces)
        triangles = len(bm.faces)
        if not triangles:
            faults.append("nothing in it")
            found[each.name] = dict(part=group_of.get(each.name, "base"), triangles=0, faults=faults, notes=notes)
            bm.free()
            continue
        low = Vector((min(v.co.x for v in bm.verts), min(v.co.y for v in bm.verts), min(v.co.z for v in bm.verts)))
        high = Vector((max(v.co.x for v in bm.verts), max(v.co.y for v in bm.verts), max(v.co.z for v in bm.verts)))
        boxes[each.name] = (low, high)
        made_low = [min((each.matrix_world @ vert.co)[axis] for vert in each.data.vertices) for axis in range(3)]
        made_high = [max((each.matrix_world @ vert.co)[axis] for vert in each.data.vertices) for axis in range(3)]
        thrown = max(max(made_low[axis] - low[axis], high[axis] - made_high[axis]) for axis in range(3))
        if thrown > 0.008:
            faults.append(f"a point thrown {thrown * 1000:.0f} mm out of it as its edges were eased")
        needles = 0
        for vert in bm.verts:
            if vert.link_edges and min(edge.calc_length() for edge in vert.link_edges) > 0.08:
                if sum(loop.calc_angle() for loop in vert.link_loops) < math.radians(20):
                    needles += 1
        if needles:
            faults.append(f"{needles} needles: points that stick out of it on long thin faces")
        # Faces of no size, and verts that hold nothing.
        thin = sum(1 for face in bm.faces if face.calc_area() < 1e-9)
        if thin:
            (faults if thin > triangles * 0.01 else notes).append(f"{thin} faces of no size")
        loose = sum(1 for vert in bm.verts if not vert.link_faces)
        if loose:
            faults.append(f"{loose} loose points")
        # Edges: open, or shared by more than two faces, or with their two faces wound against each other.
        open_edges = sum(1 for edge in bm.edges if len(edge.link_faces) == 1)
        crowded = sum(1 for edge in bm.edges if len(edge.link_faces) > 2)
        against = sum(1 for edge in bm.edges if len(edge.link_faces) == 2 and not edge.is_contiguous)
        if open_edges:
            notes.append(f"{open_edges} open edges")
        if crowded:
            notes.append(f"{crowded} edges shared by three faces or more")
        if against:
            faults.append(f"{against} edges with faces wound against each other")
        # Each closed shell faces out: its volume is not less than nothing.
        seen, inside_out, shells = set(), 0, 0
        for start in bm.faces:
            if start.index in seen:
                continue
            shell, stack = [], [start]
            seen.add(start.index)
            while stack:
                face = stack.pop()
                shell.append(face)
                for edge in face.edges:
                    for other in edge.link_faces:
                        if other.index not in seen:
                            seen.add(other.index)
                            stack.append(other)
            shells += 1
            if any(len(edge.link_faces) != 2 for face in shell for edge in face.edges):
                continue
            volume = sum(face.verts[0].co.dot(face.verts[1].co.cross(face.verts[2].co)) for face in shell) / 6.0
            if volume < -1e-7:      # less than nothing by more than a flat shell's rounding
                inside_out += 1
        if inside_out:
            faults.append(f"{inside_out} of {shells} shells inside out")
        # What it is made of.
        used = {face.material_index for face in bm.faces}
        names = [slot.material.name if slot.material else None for slot in each.material_slots]
        for index, called in enumerate(names):
            if called is None or called not in PAINTS:
                faults.append(f"a material the game does not know: {called}")
            elif index not in used:
                notes.append(f"material {called} is on it and not used")
        if triangles > MOST:
            notes.append(f"{triangles} triangles")
        # Where it is: within the truck's reach, and off the ground.
        wheel = each.name.startswith(("tyre_", "rim_"))
        reach = numbers["wide"] / 2 + 0.3
        if high.y > numbers["nose"] + 0.45 or low.y < numbers["tail"] - 0.5 or high.x > reach or low.x < -reach or high.z > numbers["roof"] + 0.6:
            faults.append("reaches out of the truck")
        if low.z < (-0.012 if wheel else 0.12):
            (faults if low.z < -0.012 else notes).append(f"hangs to {low.z:.2f} m from the ground")
        if wheel:
            # A wheel is about its hub: and its tyre is the size the truck says, and fits its arch.
            at = each.matrix_world.translation
            if abs(at.z - numbers["hub"]) > 1e-3 or abs(abs(at.x) - numbers["track"] / 2) > 1e-3:
                faults.append("not at its hub")
            radius = max(math.hypot(v.co.y - at.y, v.co.z - at.z) for v in bm.verts)
            inner = min(math.hypot(v.co.y - at.y, v.co.z - at.z) for v in bm.verts)
            if each.name.startswith("tyre_"):
                if abs(radius - numbers["tyre"]) > 0.02:
                    faults.append(f"is {radius:.3f} m to its tread, and the truck says {numbers['tyre']:.3f}")
                if radius > numbers["arch"] - 0.02:
                    faults.append("does not clear its arch")
                if abs(inner - numbers["rim"]) > 0.02:
                    faults.append(f"its bead is at {inner:.3f} m and the rim's edge at {numbers['rim']:.3f}")
            elif radius > numbers["rim"] + 0.03:
                faults.append(f"the rim reaches {radius:.3f} m, past its tyre's bead at {numbers['rim']:.3f}")
        found[each.name] = dict(part=group_of.get(each.name, "base"), triangles=triangles, shells=shells, faults=faults, notes=notes,
                                low=[round(v, 3) for v in low], high=[round(v, 3) for v in high])
        bm.free()
    # One side is the mirror of the other.
    for called, (low, high) in boxes.items():
        if len(called) > 2 and called[-1] == "L" and called[-2] in "_FR":
            other = called[:-1] + "R"
            if other not in boxes:
                found[called]["faults"].append(f"has no {other} across from it")
                continue
            far_low, far_high = boxes[other]
            apart = max(abs(low.x + far_high.x), abs(high.x + far_low.x), abs(low.y - far_low.y), abs(high.y - far_high.y), abs(low.z - far_low.z), abs(high.z - far_high.z))
            if apart > 0.004:
                found[called]["faults"].append(f"is not the mirror of {other}, by {apart * 1000:.0f} mm")
            elif found[called]["triangles"] != found[other]["triangles"]:
                found[called]["notes"].append(f"has {found[called]['triangles']} triangles and {other} has {found[other]['triangles']}")
    # A thing bolted on touches the truck: its box meets the box of something in the base.
    for called, (low, high) in boxes.items():
        if called in parts["base"] or called.startswith(("tyre_", "rim_")):
            continue
        mine = found[called]["part"].split(".")[0]
        near = any(all(low[axis] <= boxes[other][1][axis] + 0.02 and high[axis] >= boxes[other][0][axis] - 0.02 for axis in range(3))
                   for other in boxes if found[other]["part"].split(".")[0] != mine and not other.startswith(("tyre_", "rim_")))
        if not near:
            found[called]["faults"].append("touches nothing of the truck")
    # And the list is whole: every piece is in the base or in a group, and each choice's pieces are there.
    listed = set(parts["base"]) | set(group_of)
    for called in found:
        if called not in listed:
            found[called]["faults"].append("is in no list")
    missing = [f"{part}: {called}" for called, part in group_of.items() if called not in found]
    missing += [f"base: {called}" for called in parts["base"] if called not in found]
    missing += [f"{group} has no choice {choice}" for build in (parts["stock"], parts["showroom"]) for group, choice in build.items()
                if choice and choice not in parts["groups"][group]]
    return found, missing


def one(name, out, keep, views=("front", "back", "side", "face"), size=(1280, 720), kit=None):
    """Makes a truck: its file, the list of its parts, an audit of its pieces, and pictures
    of it bare and as it is shown. With `kit`, a folder, a picture of every choice."""
    import json
    bpy.ops.wm.read_factory_settings(use_empty=True)
    made, numbers, truck, parts = build(name)
    for each in made:
        for poly in each.data.polygons:
            poly.use_smooth = True
    bpy.context.view_layer.update()
    found, missing = audit(parts, numbers)
    triangles = sum(each["triangles"] for called, each in found.items() if not called.startswith(("tyre_", "rim_")) or called.endswith("_FL"))
    bare = sum(found[called]["triangles"] for called in dress(parts, parts["stock"], lift=False) if called in found)
    faults = sum(len(each["faults"]) for each in found.values()) + len(missing)
    notes = sum(len(each["notes"]) for each in found.values())
    parts["triangles"] = dict(file=triangles, bare=bare)
    stem = os.path.splitext(out)[0]
    with open(stem + ".parts.json", "w", encoding="utf-8") as file:
        json.dump(parts, file, indent=1)
    with open(stem + ".audit.json", "w", encoding="utf-8") as file:
        json.dump(dict(name=name, kind=truck["kind"], grade=truck["grade"], faults=faults, notes=notes, missing=missing, pieces=found), file, indent=1)
    print(f"{name}: {len(made)} pieces, {triangles} triangles in the file, {bare} on the truck bare; {faults} faults, {notes} notes; {THROWN[0]} shapes rounded again")
    THROWN[0] = 0
    for called, each in sorted(found.items()):
        for fault in each["faults"]:
            print(f"{name}: FAULT {called}: {fault}")
    for each in missing:
        print(f"{name}: FAULT missing {each}")
    if keep:
        os.makedirs(keep, exist_ok=True)
        dress(parts, parts["showroom"])
        pictures(keep, name, numbers, views, size)
        dress(parts, parts["stock"])
        pictures(keep, name, numbers, (), size, close=[("bare", (-(numbers["nose"] - numbers["tail"]) * 0.85, (numbers["nose"] - numbers["tail"]) * 1.25, numbers["roof"] * 0.95),
                                                       (0.0, (numbers["nose"] + numbers["tail"]) / 2, numbers["roof"] * 0.48))])
        # And with everything that opens open, from the side its passenger gets in at.
        swing(parts, 1.0)
        pictures(keep, name, numbers, (), size, close=[("open", (-(numbers["nose"] - numbers["tail"]) * 0.95, -(numbers["nose"] - numbers["tail"]) * 0.35, numbers["roof"] * 1.25),
                                                       (0.0, (numbers["nose"] + numbers["tail"]) / 2 - 0.3, numbers["roof"] * 0.5))])
        swing(parts, 0.0)
    if kit:
        os.makedirs(kit, exist_ok=True)
        long, wide, roof = numbers["nose"] - numbers["tail"], numbers["wide"], numbers["roof"]
        middle = (0.0, (numbers["nose"] + numbers["tail"]) / 2, roof * 0.48)
        front = ((-long * 0.85, long * 1.25, roof * 0.95), middle)
        back = ((long * 0.95, -long * 1.2, roof * 1.05), middle)
        face = ((-wide * 0.75, numbers["nose"] + wide * 1.5, roof * 0.7), (0.0, numbers["nose"], roof * 0.45))
        wheel = ((-wide * 0.5 - 1.5, numbers["front_axle"] + 0.9, numbers["hub"] + 0.25), (-wide * 0.5, numbers["front_axle"], numbers["hub"]))
        under = ((-wide * 0.5 - 1.5, numbers["front_axle"] + 0.25, numbers["hub"] * 0.9), (-wide * 0.3, numbers["front_axle"], numbers["hub"] + 0.15))
        angle = dict(bullbar=face, spots=face, snorkel=front, rack=front, lightbar=front, arches=front, steps=front, suspension=under, tyres=wheel, rims=wheel,
                     tub=back, tray=back, spare=back)
        for group, options in parts["groups"].items():
            for choice in options:
                worn = dict(parts["stock"])
                worn[group] = choice
                if group == "spots":
                    worn["bullbar"] = "nudge" if choice == "low" else "tube"
                if group == "lightbar" and choice == "rack":
                    worn["rack"] = "fitted"
                if group == "suspension":
                    worn["tyres"], worn["rims"] = None, None
                dress(parts, worn)
                pictures(kit, name, numbers, (), (640, 400), close=[(f"{group}_{choice}", *angle[group])])
    dress(parts, parts["stock"], lift=False)
    for each in bpy.context.scene.objects:
        each.hide_render = False
        each.select_set(True)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_apply=True, export_yup=True, use_selection=True)
    print(f"{name}: wrote {out}, {os.path.getsize(out) / 1e6:.1f} MB")
    game_files(name, truck, numbers, parts, out, os.environ.get("TRUCK_SPECS") or os.path.dirname(out))
    return len(made), triangles, faults, parts


def main():
    if not args or args[0] == "list":
        for name, truck in TRUCKS.items():
            print(f"{name}: grade {truck['grade']}, {truck['called']}: {truck['about']}")
        return
    if args[0] == "all":
        # Every truck, each to its own file in a folder, with one picture of each.
        import json
        folder, keep = args[1], (args[2] if len(args) > 2 else None)
        os.makedirs(folder, exist_ok=True)
        told = []
        for name, truck in TRUCKS.items():
            try:
                count, triangles, faults, parts = one(name, os.path.join(folder, f"{name}.glb"), keep, ("front", "face"), (960, 540))
            except Exception as error:  # One that will not build does not stop the rest.
                print(f"{name}: FAILED: {error!r}")
                continue
            told.append(dict(name=name, called=truck["called"], grade=truck["grade"], kind=truck["kind"], price=truck["price"], year=truck["year"],
                             era=truck["era"], grille=truck["grille"], lamps=truck["lamps"], bumper=truck["bumper"], rim=truck["rim"],
                             about=truck["about"], back=truck["back"], doors=truck["doors"], pieces=count, triangles=triangles,
                             long=round(truck["wheelbase"] + truck["front"] + truck["rear"], 2), wide=truck["wide"],
                             wheelbase=truck["wheelbase"], tyre=round(truck["tyre"] * 2, 2), fit=list(truck["fit"]), faults=faults,
                             bare=parts["triangles"]["bare"], stock=parts["stock"], showroom=parts["showroom"],
                             kit={group: list(options) for group, options in parts["groups"].items()}))
        with open(os.path.join(folder, "trucks.json"), "w", encoding="utf-8") as file:
            json.dump(told, file, indent=1)
        print(f"built {len(told)} of {len(TRUCKS)}")
        return
    if args[0] == "kit":
        # One truck, and a picture of every choice of every thing that can be fitted to it.
        one(args[1], args[2], None, kit=args[3])
        return
    one(args[0], args[1], args[2] if len(args) > 2 else None)


if __name__ == "__main__":
    main()
