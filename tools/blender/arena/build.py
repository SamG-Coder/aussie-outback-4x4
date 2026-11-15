"""Builds the set dressing of Bulldust Bowl, the monster-truck showground, as one model of
named pieces.

  blender -b --python tools/blender/arena/build.py -- OUT.glb [KEEP_FOLDER]

The arena is `scenes/arena.txt`, and its land, its jumps, its mud and its loop are made by
the bake (`crates/solstice-bake/src/outback/arena`, `docs/arena.md`). What is made here is
what stands about on it and makes it a place: two stands for a crowd, lamps on towers, the
gate's arch with the ground's name on it, the announcer's box, shipping containers, drums,
tyres in stacks and tyres half buried along the lanes, a row of wrecks to drive over, the
board that says what speed the loop wants, and, outside, a station's windmill and tank.

The bake stands each piece by its name, where its design says, and says what of it stops a
truck in boxes (`solid` in `arena/mod.rs`): the sizes here and there are the same. Every
piece is made about the middle of its foot on the ground, with its front towards +z. Lengths
are in metres, as the game has them: x to the right, y up, z towards whoever looks at its
front. (The bake finds a piece by the end of its name, so no piece's name ends another's.)

It has no textures: iron, steel, timber, rubber and paint, by their colours alone.
KEEP_FOLDER, if given, is where the Blender file is kept.
"""
import math
import os
import sys

import bpy

args = sys.argv[sys.argv.index("--") + 1:]
OUT = args[0]
KEEP = args[1] if len(args) > 1 else None

# name: colour, roughness, metal, glow
PAINTS = {
    "steel": ((0.36, 0.37, 0.39), 0.5, 0.8, 0.0),
    "galv": ((0.62, 0.64, 0.65), 0.45, 0.7, 0.0),
    "iron": ((0.55, 0.56, 0.56), 0.55, 0.6, 0.0),
    "rust": ((0.36, 0.17, 0.09), 0.9, 0.2, 0.0),
    "oxide": ((0.5, 0.12, 0.07), 0.7, 0.2, 0.0),
    "timber": ((0.42, 0.3, 0.2), 0.85, 0.0, 0.0),
    "plank": ((0.55, 0.43, 0.3), 0.85, 0.0, 0.0),
    "rubber": ((0.03, 0.03, 0.032), 0.92, 0.0, 0.0),
    "white": ((0.86, 0.85, 0.81), 0.7, 0.0, 0.0),
    "cream": ((0.8, 0.74, 0.6), 0.75, 0.0, 0.0),
    "yellow": ((0.9, 0.62, 0.04), 0.6, 0.0, 0.0),
    "red": ((0.62, 0.07, 0.05), 0.6, 0.0, 0.0),
    "blue": ((0.1, 0.22, 0.42), 0.6, 0.0, 0.0),
    "green": ((0.12, 0.3, 0.18), 0.65, 0.0, 0.0),
    "black": ((0.02, 0.02, 0.022), 0.7, 0.0, 0.0),
    "glass": ((0.05, 0.07, 0.08), 0.15, 0.0, 0.0),
    "faded": ((0.62, 0.66, 0.68), 0.6, 0.3, 0.0),
    "lamp": ((1.0, 0.95, 0.82), 0.4, 0.0, 6.0),
}

# Each piece: its paints, each with its points and faces, in the game's axes.
pieces = {}
# The longest a face may be: the renderer draws small triangles, and one longer than the
# view may not be drawn at all.
LONGEST = 4.0


def blender(point):
    """A point as the game has it, as Blender has it: z up, and y away from the front. (A
    turn, not a mirroring: a face keeps the way it faces.)"""
    x, y, z = point
    return (x, -z, y)


def add(piece, paint, verts, faces):
    held, made = pieces.setdefault(piece, {}).setdefault(paint, ([], []))
    first = len(held)
    held.extend(blender(v) for v in verts)
    made.extend(tuple(first + i for i in face) for face in faces)


def sheet(piece, paint, verts):
    """A thin sheet seen from both sides: two faces, back to back, each with its own
    points (two faces on the same points are not a mesh Blender will have)."""
    add(piece, paint, verts, [tuple(range(len(verts)))])
    add(piece, paint, verts, [tuple(reversed(range(len(verts))))])


def box(piece, paint, low, high):
    """A box, by its least corner and its greatest, in lengths no longer than a face may
    be: each of its sides is as many faces as that takes."""
    (x0, y0, z0), (x1, y1, z1) = low, high

    def cuts(a, b):
        count = max(1, math.ceil((b - a) / LONGEST))
        return [a + (b - a) * k / count for k in range(count + 1)]

    xs, ys, zs = cuts(x0, x1), cuts(y0, y1), cuts(z0, z1)

    def side(us, vs, place, flip):
        for i in range(len(us) - 1):
            for j in range(len(vs) - 1):
                quad = [place(us[i], vs[j]), place(us[i + 1], vs[j]), place(us[i + 1], vs[j + 1]), place(us[i], vs[j + 1])]
                add(piece, paint, quad, [(3, 2, 1, 0) if flip else (0, 1, 2, 3)])

    side(xs, ys, lambda x, y: (x, y, z1), False)
    side(xs, ys, lambda x, y: (x, y, z0), True)
    side(zs, ys, lambda z, y: (x0, y, z), False)
    side(zs, ys, lambda z, y: (x1, y, z), True)
    side(xs, zs, lambda x, z: (x, y1, z), True)
    side(xs, zs, lambda x, z: (x, y0, z), False)


def member(piece, paint, start, end, thick):
    """A square bar from one place to another, in lengths a face may be."""
    sx, sy, sz = start
    ex, ey, ez = end
    dx, dy, dz = ex - sx, ey - sy, ez - sz
    long = math.sqrt(dx * dx + dy * dy + dz * dz)
    if long < 1e-6:
        return
    ax, ay, az = dx / long, dy / long, dz / long
    # Two directions across it.
    if abs(ay) > 0.95:
        ux, uy, uz = 1.0, 0.0, 0.0
    else:
        ux, uy, uz = az, 0.0, -ax
        size = math.hypot(ux, uz)
        ux, uz = ux / size, uz / size
    vx, vy, vz = ay * uz - az * uy, az * ux - ax * uz, ax * uy - ay * ux
    half = thick * 0.5
    count = max(1, math.ceil(long / LONGEST))
    rings = []
    for k in range(count + 1):
        t = long * k / count
        cx, cy, cz = sx + ax * t, sy + ay * t, sz + az * t
        rings.append([(cx + (ux * a + vx * b) * half, cy + (uy * a + vy * b) * half, cz + (uz * a + vz * b) * half)
                      for a, b in ((-1, -1), (1, -1), (1, 1), (-1, 1))])
    for k in range(count):
        verts = rings[k] + rings[k + 1]
        add(piece, paint, verts, [(i, (i + 1) % 4, 4 + (i + 1) % 4, 4 + i) for i in range(4)])
    add(piece, paint, rings[0], [(3, 2, 1, 0)])
    add(piece, paint, rings[-1], [(0, 1, 2, 3)])


def tube(piece, paint, foot, radius, high, sides=14, top=None):
    """An upright round drum or pole: the middle of its foot, its radius, its height.
    `top` is its radius at the top, if not the same."""
    top = radius if top is None else top
    x, y, z = foot
    verts = []
    for ring, (r, h) in enumerate(((radius, 0.0), (top, high))):
        for k in range(sides):
            a = math.tau * k / sides
            verts.append((x + r * math.cos(a), y + h, z + r * math.sin(a)))
    faces = [(k, sides + k, sides + (k + 1) % sides, (k + 1) % sides) for k in range(sides)]
    faces.append(tuple(range(sides)))
    faces.append(tuple(reversed(range(sides, 2 * sides))))
    add(piece, paint, verts, faces)


def tyre(piece, paint, centre, radius, wide, axis="y", around=18, half=False):
    """A tyre: a ring about `axis` through `centre`, as wide as said, its hole 0.55 of its
    radius. With `half`, only the half of it above its centre: one half buried, standing."""
    cx, cy, cz = centre
    inner = radius * 0.55
    # Its section: tread, shoulders, walls, and the bead round the hole.
    section = [(inner, -wide * 0.38), (radius * 0.86, -wide * 0.5), (radius, -wide * 0.36),
               (radius, wide * 0.36), (radius * 0.86, wide * 0.5), (inner, wide * 0.38)]
    steps = around // 2 if half else around
    verts = []
    for k in range(steps + (1 if half else 0)):
        a = math.pi * k / steps if half else math.tau * k / around
        for r, w in section:
            if axis == "y":
                verts.append((cx + r * math.cos(a), cy + w, cz + r * math.sin(a)))
            else:
                # About z: it stands, and the half that is made is the upper one.
                verts.append((cx + r * math.cos(a), cy + r * math.sin(a), cz + w))
    count = len(section)
    rings = steps if half else around
    faces = []
    for k in range(rings):
        nxt = k + 1 if half else (k + 1) % around
        for s in range(count):
            t = (s + 1) % count
            quad = (k * count + s, k * count + t, nxt * count + t, nxt * count + s)
            faces.append(quad if axis == "y" else tuple(reversed(quad)))
    add(piece, paint, verts, faces)


def words(piece, paint, text, at, tall, proud=0.02, back=False):
    """Lettering standing with its middle at `at`: to be read from +z, or, with `back`,
    from -z."""
    curve = bpy.data.curves.new("words", "FONT")
    curve.body = text
    curve.size = tall
    curve.align_x = "CENTER"
    curve.align_y = "CENTER"
    curve.resolution_u = 3
    curve.extrude = proud * 0.5
    held = bpy.data.objects.new("words", curve)
    bpy.context.scene.collection.objects.link(held)
    mesh = bpy.data.meshes.new_from_object(held.evaluated_get(bpy.context.evaluated_depsgraph_get()))
    turn = -1.0 if back else 1.0
    verts = [(at[0] + turn * v.co.x, at[1] + v.co.y, at[2] + turn * (v.co.z + proud)) for v in mesh.vertices]
    faces = [tuple(polygon.vertices) for polygon in mesh.polygons]
    add(piece, paint, verts, faces)
    bpy.data.objects.remove(held, do_unlink=True)
    bpy.data.meshes.remove(mesh)
    bpy.data.curves.remove(curve)


def iron(piece, paint, low, high, ribs="y"):
    """A sheet of corrugated iron, as a thin box with ribs standing off its +z face (or,
    for a roof, its top): each rib a low bar."""
    box(piece, paint, low, high)
    (x0, y0, z0), (x1, y1, z1) = low, high
    count = max(2, round((x1 - x0) / 0.38))
    for k in range(count):
        x = x0 + (x1 - x0) * (k + 0.5) / count
        if ribs == "y":
            box(piece, paint, (x - 0.05, y0, z1), (x + 0.05, y1, z1 + 0.03))
            box(piece, paint, (x - 0.05, y0, z0 - 0.03), (x + 0.05, y1, z0))
        else:
            box(piece, paint, (x - 0.05, y1, z0), (x + 0.05, y1 + 0.03, z1))


def grandstand():
    """A stand for a crowd: eight rows of plank seats rising to the back on a frame of
    scaffold, a wall of iron behind, rails at its ends and an iron roof over it. 24 m wide,
    6.4 m deep, its seats 4.4 m high at the back. Its front is +z."""
    p = "grandstand"
    wide, deep, rows = 24.0, 6.4, 8
    rise, tread = 0.5, deep / rows
    for row in range(rows):
        z = deep * 0.5 - (row + 1) * tread
        y = 0.45 + row * rise
        box(p, "plank", (-wide * 0.5, y, z + 0.1), (wide * 0.5, y + 0.06, z + 0.5))
        box(p, "timber", (-wide * 0.5, y - 0.42, z + tread - 0.04), (wide * 0.5, y - 0.36, z + tread + 0.3))
    # The frame: a post under the front and the back of every other row, every 3 m.
    for k in range(9):
        x = -wide * 0.5 + 0.15 + k * (wide - 0.3) / 8
        for row in range(0, rows, 2):
            z = deep * 0.5 - (row + 1) * tread + 0.3
            member(p, "galv", (x, 0.0, z), (x, 0.45 + row * rise, z), 0.07)
        member(p, "galv", (x, 0.1, deep * 0.5 - 0.3), (x, 0.45 + (rows - 1) * rise, -deep * 0.5 + 0.3), 0.07)
        member(p, "galv", (x, 0.0, -deep * 0.5 + 0.1), (x, 7.0, -deep * 0.5 + 0.1), 0.1)
        member(p, "galv", (x, 0.0, deep * 0.5 - 0.1), (x, 6.3, deep * 0.5 - 0.1), 0.1)
        member(p, "galv", (x, 7.0, -deep * 0.5 + 0.1), (x, 6.3, deep * 0.5 - 0.1), 0.08)
    # The wall behind, the roof, and a rail at each end.
    iron(p, "cream", (-wide * 0.5, 0.3, -deep * 0.5 - 0.02), (wide * 0.5, 5.4, -deep * 0.5 + 0.02))
    for k in range(6):
        x0 = -wide * 0.5 - 0.3 + k * (wide + 0.6) / 6
        x1 = x0 + (wide + 0.6) / 6
        for j in range(2):
            z0, z1 = -deep * 0.5 - 0.4 + j * (deep + 0.9) / 2, -deep * 0.5 - 0.4 + (j + 1) * (deep + 0.9) / 2
            y0, y1 = 7.12 - 0.8 * j * 0.5 * (deep + 0.9) / deep, 7.12 - 0.8 * (j + 1) * 0.5 * (deep + 0.9) / deep
            sheet(p, "iron", [(x0, y0, z0), (x0, y1, z1), (x1, y1, z1), (x1, y0, z0)])
    for x in (-wide * 0.5, wide * 0.5):
        member(p, "galv", (x, 1.4, deep * 0.5 - 0.1), (x, 5.2, -deep * 0.5 + 0.1), 0.06)
    # A board along the front of the roof.
    box(p, "red", (-wide * 0.5, 5.7, deep * 0.5 - 0.14), (wide * 0.5, 6.3, deep * 0.5 - 0.1))
    words(p, "white", "BULLDUST BOWL", (0.0, 6.0, deep * 0.5 - 0.1), 0.42)


def tower():
    """A lamp tower: a steel mast 14 m high on a block, with a bank of six lamps at its
    head, tipped to look down into the arena, which is toward -z."""
    p = "tower"
    box(p, "white", (-0.5, 0.0, -0.5), (0.5, 0.5, 0.5))
    tube(p, "galv", (0.0, 0.5, 0.0), 0.2, 12.9, sides=10, top=0.11)
    for y in (4.0, 8.0, 11.5):
        tube(p, "steel", (0.0, y, 0.0), 0.22, 0.12, sides=10)
    member(p, "steel", (-1.6, 13.4, -0.15), (1.6, 13.4, -0.15), 0.12)
    for row, y in enumerate((13.05, 13.75)):
        for k in range(3):
            x = -1.05 + k * 1.05
            box(p, "steel", (x - 0.42, y - 0.28, -0.42), (x + 0.42, y + 0.28, -0.16))
            box(p, "lamp", (x - 0.36, y - 0.22, -0.46), (x + 0.36, y + 0.22, -0.42))


def container():
    """A shipping container: 6.06 m long, 2.44 m wide, 2.6 m high, its doors at its +x
    end. It lies along x."""
    p = "container"
    long, wide, high = 6.06, 2.44, 2.6
    box(p, "oxide", (-long * 0.5, 0.16, -wide * 0.5), (long * 0.5, high, wide * 0.5))
    count = 22
    for k in range(count):
        x = -long * 0.5 + 0.2 + (long - 0.4) * (k + 0.5) / count
        for z0, z1 in ((wide * 0.5, wide * 0.5 + 0.035), (-wide * 0.5 - 0.035, -wide * 0.5)):
            box(p, "oxide", (x - 0.07, 0.3, z0), (x + 0.07, high - 0.14, z1))
    for x in (-long * 0.5, long * 0.5 - 0.12):
        for z in (-wide * 0.5, wide * 0.5 - 0.12):
            box(p, "rust", (x - 0.01, 0.0, z - 0.01), (x + 0.13, high + 0.01, z + 0.13))
    # Its doors: two leaves, each with two bars.
    for z0 in (-wide * 0.5 + 0.14, 0.02):
        box(p, "rust", (long * 0.5, 0.3, z0), (long * 0.5 + 0.03, high - 0.12, z0 + wide * 0.5 - 0.16))
        for off in (0.3, 0.75):
            tube(p, "galv", (long * 0.5 + 0.06, 0.25, z0 + off), 0.02, high - 0.3, sides=6)
    words(p, "white", "BULLDUST", (0.0, 1.5, wide * 0.5 + 0.035), 0.5)


def stack():
    """Five truck tyres, one on another, not quite square: 1.4 m across, 1.9 m high."""
    for k in range(5):
        lean = (0.05, -0.04, 0.03, -0.05, 0.02)[k]
        tyre("stack", "white" if k % 2 == 0 else "rubber", (lean, 0.19 + k * 0.38, -lean), 0.68, 0.37)


def buried():
    """A tractor's tyre stood on edge and buried to its middle, painted: 1.8 m across,
    0.9 m high, 0.6 m thick. It is looked at along z."""
    tyre("tyre", "white", (0.0, 0.0, 0.0), 0.92, 0.6, axis="z", around=20, half=True)


def drum():
    """A 44 gallon drum: 0.58 m across, 0.88 m high, with its two ribs."""
    tube("drum", "red", (0.0, 0.0, 0.0), 0.29, 0.88, sides=14)
    for y in (0.28, 0.58):
        tube("drum", "red", (0.0, y, 0.0), 0.305, 0.03, sides=14)
    tube("drum", "black", (0.0, 0.88, 0.0), 0.27, 0.012, sides=14)


def wreck(p, paint):
    """A sedan that has been driven over more than once: 1.75 m wide, 4.3 m long, its roof
    pushed down to 1.3 m, its tyres flat. It lies along z, its nose at -z."""
    # Sills and floor, the bonnet, the boot, and the doors' line, each a little out of
    # true; the cabin, low and leaning.
    def slab(x0, x1, y0, y1, z0, z1, lean=0.0, sag=0.0, colour=paint):
        verts = [(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1),
                 (x0 + lean, y1, z0 + 0.04), (x1 + lean, y1 - sag, z0 + 0.04),
                 (x1 + lean, y1 - sag * 0.5, z1 - 0.04), (x0 + lean, y1 + sag * 0.3, z1 - 0.04)]
        faces = [(0, 1, 2, 3), (7, 6, 5, 4), (0, 4, 5, 1), (2, 6, 7, 3), (1, 5, 6, 2), (3, 7, 4, 0)]
        add(p, colour, verts, faces)

    slab(-0.875, 0.875, 0.22, 0.62, -2.15, 2.15)
    slab(-0.84, 0.84, 0.62, 0.86, -2.1, -0.75, sag=0.1)
    slab(-0.84, 0.84, 0.62, 0.9, 1.25, 2.1, sag=-0.05)
    slab(-0.78, 0.78, 0.62, 1.02, -0.75, 1.25, lean=0.03)
    slab(-0.7, 0.7, 1.02, 1.3, -0.45, 1.05, lean=0.07, sag=0.12)
    # What is left of its glass: dark, all round the cabin.
    slab(-0.73, 0.73, 0.98, 1.06, -0.62, 1.16, lean=0.04, colour="glass")
    # Bumpers, and four flat tyres under its arches.
    box(p, "galv", (-0.86, 0.3, -2.22), (0.86, 0.46, -2.14))
    box(p, "galv", (-0.86, 0.32, 2.14), (0.86, 0.48, 2.22))
    for x in (-0.8, 0.8):
        for z in (-1.4, 1.35):
            tyre(p, "rubber", (x, 0.27, z), 0.3, 0.2, axis="y", around=12)


def arch():
    """The gate's arch: two posts 17.6 m apart and 7 m high, a truss between their heads,
    and the ground's name on a board, to be read from outside (+z) and from inside."""
    p = "arch"
    half = 8.4
    for x in (-half, half):
        for dx in (-0.25, 0.25):
            for dz in (-0.25, 0.25):
                member(p, "oxide", (x + dx, 0.0, dz), (x + dx, 7.2, dz), 0.12)
        for y in (1.2, 2.4, 3.6, 4.8, 6.0, 7.2):
            for a, b in (((-0.25, -0.25), (0.25, -0.25)), ((0.25, -0.25), (0.25, 0.25)),
                         ((0.25, 0.25), (-0.25, 0.25)), ((-0.25, 0.25), (-0.25, -0.25))):
                member(p, "oxide", (x + a[0], y, a[1]), (x + b[0], y, b[1]), 0.06)
        box(p, "white", (x - 0.5, 0.0, -0.5), (x + 0.5, 0.35, 0.5))
    for y in (5.9, 7.2):
        for z in (-0.25, 0.25):
            member(p, "oxide", (-half, y, z), (half, y, z), 0.12)
    for k in range(12):
        x0, x1 = -half + k * 2 * half / 12, -half + (k + 1) * 2 * half / 12
        for z in (-0.25, 0.25):
            member(p, "oxide", (x0, 5.9 if k % 2 else 7.2, z), (x1, 7.2 if k % 2 else 5.9, z), 0.06)
    for z, back in ((0.33, False), (-0.33, True)):
        low, high = (-6.2, 5.95, min(z, z - 0.03 if back else z)), (6.2, 7.15, max(z, z - 0.03 if back else z + 0.03))
        box(p, "cream", low, high)
        words(p, "red", "BULLDUST BOWL", (0.0, 6.62, z + (-0.03 if back else 0.03)), 0.82, back=back)
        words(p, "black", "MUD  ·  JUMPS  ·  THE LOOP", (0.0, 6.12, z + (-0.03 if back else 0.03)), 0.2, back=back)


def announcer():
    """The announcer's box: an iron hut on a stand of posts, its window toward +z, with a
    stair up its side. 5 m wide, 4 m deep, 6.4 m high."""
    p = "box"
    for x in (-2.4, 2.4):
        for z in (-1.9, 1.9):
            member(p, "timber", (x, 0.0, z), (x, 3.3, z), 0.2)
    for z in (-1.9, 1.9):
        member(p, "timber", (-2.4, 0.3, z), (2.4, 3.0, z), 0.1)
    box(p, "plank", (-2.6, 3.3, -2.1), (2.6, 3.42, 2.1))
    iron(p, "cream", (-2.5, 3.42, -2.0), (2.5, 6.0, -1.96))
    iron(p, "cream", (-2.5, 3.42, 1.96), (2.5, 4.3, 2.0))
    box(p, "cream", (-2.5, 5.5, 1.96), (2.5, 6.0, 2.0))
    box(p, "glass", (-2.3, 4.3, 1.9), (2.3, 5.5, 1.94))
    for x in (-2.5, 2.46):
        box(p, "cream", (x, 3.42, -2.0), (x + 0.04, 6.0, 2.0))
    iron(p, "iron", (-2.8, 6.0, -2.3), (2.8, 6.06, 2.4), ribs="top")
    box(p, "red", (-2.5, 5.55, 2.0), (2.5, 5.95, 2.03))
    words(p, "white", "ANNOUNCER", (0.0, 5.75, 2.03), 0.26)
    # The stair, up its left side from the front.
    for k in range(11):
        y, z = 0.3 + k * 0.3, 3.2 - k * 0.3
        box(p, "plank", (-3.5, y - 0.04, z - 0.15), (-2.6, y, z + 0.15))
    member(p, "timber", (-3.5, 0.0, 3.3), (-3.5, 3.3, 0.1), 0.1)
    member(p, "timber", (-2.65, 0.0, 3.3), (-2.65, 3.3, 0.1), 0.1)
    # Two horns on its roof.
    for x in (-1.6, 1.6):
        tube(p, "galv", (x, 6.06, 1.8), 0.05, 0.7, sides=6)
        tube(p, "galv", (x, 6.6, 1.95), 0.08, 0.3, sides=8, top=0.24)


def windmill():
    """A station's windmill: a tower of four legs 9.5 m high, a wheel of eighteen sails
    3 m across at its head, facing +z, and its tail behind."""
    p = "windmill"
    foot, head, high = 0.8, 0.16, 9.5
    corners = ((-1, -1), (1, -1), (1, 1), (-1, 1))
    for a, b in corners:
        member(p, "galv", (a * foot, 0.0, b * foot), (a * head, high, b * head), 0.07)
    for k in range(1, 6):
        t = k / 6
        r = foot + (head - foot) * t
        for i in range(4):
            (a, b), (c, d) = corners[i], corners[(i + 1) % 4]
            member(p, "galv", (a * r, high * t, b * r), (c * r, high * t, d * r), 0.04)
            r2 = foot + (head - foot) * (t - 1 / 6)
            member(p, "galv", (a * r2, high * (t - 1 / 6), b * r2), (c * r, high * t, d * r), 0.03)
    box(p, "plank", (-0.5, 8.2, -0.5), (0.5, 8.26, 0.5))
    # The wheel: its hub, its rim, and its sails, each tipped a little to the wind.
    member(p, "steel", (0.0, high + 0.1, -0.4), (0.0, high + 0.1, 0.55), 0.14)
    sails, rim = 18, 1.5
    for k in range(sails):
        a0, a1 = math.tau * k / sails, math.tau * (k + 0.62) / sails
        cy = high + 0.1
        verts = [(0.42 * math.cos(a0), cy + 0.42 * math.sin(a0), 0.56),
                 (rim * math.cos(a0), cy + rim * math.sin(a0), 0.56),
                 (rim * math.cos(a1), cy + rim * math.sin(a1), 0.46),
                 (0.42 * math.cos(a1), cy + 0.42 * math.sin(a1), 0.5)]
        sheet(p, "galv", verts)
        an = math.tau * (k + 1) / sails
        member(p, "steel", (rim * math.cos(a0), cy + rim * math.sin(a0), 0.52),
               (rim * math.cos(an), cy + rim * math.sin(an), 0.52), 0.03)
    # The tail: a vane on a boom.
    member(p, "steel", (0.0, high + 0.1, -0.4), (0.0, high + 0.1, -2.3), 0.05)
    sheet(p, "galv", [(0.0, high - 0.45, -1.5), (0.0, high - 0.7, -2.9), (0.0, high + 0.9, -2.9), (0.0, high + 0.65, -1.5)])
    # The pump rod down its middle.
    member(p, "steel", (0.0, 0.3, 0.0), (0.0, high, 0.0), 0.03)


def tank():
    """A water tank of corrugated iron on a stand: 3.2 m across, its stand 2.6 m high and
    the tank 2.4 m."""
    p = "tank"
    for x in (-1.3, 1.3):
        for z in (-1.3, 1.3):
            member(p, "timber", (x, 0.0, z), (x, 2.5, z), 0.2)
    for x in (-1.3, 1.3):
        member(p, "timber", (x, 0.2, -1.3), (x, 2.3, 1.3), 0.09)
    for z in (-1.3, 1.3):
        member(p, "timber", (-1.3, 0.2, z), (1.3, 2.3, z), 0.09)
    box(p, "plank", (-1.7, 2.5, -1.7), (1.7, 2.6, 1.7))
    tube(p, "iron", (0.0, 2.6, 0.0), 1.6, 2.4, sides=28)
    for k in range(7):
        tube(p, "iron", (0.0, 2.7 + k * 0.33, 0.0), 1.625, 0.12, sides=28)
    tube(p, "iron", (0.0, 5.0, 0.0), 1.66, 0.25, sides=28, top=0.2)


def board():
    """The loop's board, on two posts: what it is, and what speed to bring to it. To be
    read from +z."""
    p = "sign"
    for x in (-1.5, 1.5):
        member(p, "galv", (x, 0.0, 0.0), (x, 3.6, 0.0), 0.09)
    box(p, "cream", (-1.9, 1.6, 0.05), (1.9, 3.7, 0.09))
    box(p, "red", (-1.9, 3.1, 0.09), (1.9, 3.7, 0.095))
    words(p, "white", "THE LOOP", (0.0, 3.4, 0.095), 0.4)
    words(p, "black", "70 km/h", (0.0, 2.62, 0.09), 0.62)
    words(p, "black", "hold it straight, foot down", (0.0, 1.93, 0.09), 0.2)


def build():
    grandstand()
    tower()
    container()
    stack()
    buried()
    drum()
    wreck("wreck", "faded")
    wreck("wreck_b", "blue")
    wreck("wreck_r", "rust")
    arch()
    announcer()
    windmill()
    tank()
    board()


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build()
    materials = {}
    for name, (colour, rough, metal, glow) in PAINTS.items():
        material = bpy.data.materials.new("arena_" + name)
        material.use_nodes = True
        material.use_backface_culling = True
        shader = material.node_tree.nodes["Principled BSDF"]
        shader.inputs["Base Color"].default_value = (*colour, 1.0)
        shader.inputs["Roughness"].default_value = rough
        shader.inputs["Metallic"].default_value = metal
        if glow:
            shader.inputs["Emission Color"].default_value = (*colour, 1.0)
            shader.inputs["Emission Strength"].default_value = glow
        materials[name] = material
    triangles = 0
    for name, paints in pieces.items():
        verts, faces, painted = [], [], []
        mesh = bpy.data.meshes.new(name)
        for slot, (paint, (held, made)) in enumerate(paints.items()):
            first = len(verts)
            verts.extend(held)
            faces.extend(tuple(first + i for i in face) for face in made)
            painted.extend([slot] * len(made))
            mesh.materials.append(materials[paint])
        mesh.from_pydata(verts, [], faces)
        mesh.polygons.foreach_set("material_index", painted)
        mesh.update()
        piece = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(piece)
        triangles += sum(len(p.vertices) - 2 for p in mesh.polygons)
    print(f"arena: {len(pieces)} pieces, {triangles} triangles")
    if KEEP:
        os.makedirs(KEEP, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(KEEP, "arena.blend"))
    bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_apply=True, export_yup=True)
    print(f"arena: wrote {OUT}, {os.path.getsize(OUT) / 1e3:.0f} KB")


main()
