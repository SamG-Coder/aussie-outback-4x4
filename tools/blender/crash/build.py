"""Builds the crash-test pad's obstacles, signs and paint, as one model of named pieces.

  blender -b --python tools/blender/crash/build.py -- OUT.glb [KEEP_FOLDER]

The pad is `scenes/damage.txt`: a flat pad with a row of things to drive a truck into, each
a test of the game's damage (`docs/damage.md`). The scene places each piece of this model by
name (`part crash.glb wall X Y Z YAW 1 collide=hull`), so where a test stands is the scene
file's to say, and what it is made of is this script's.

The pad is a slab of concrete, made about the middle of its top. Every obstacle is a convex
shape, made about the middle of its foot on the pad, with the
face a truck meets towards +Z in the file (a truck drives at it along -Z): so the hull the
bake gives it to collide with is the shape that is seen. Signs and paint collide with
nothing. Lengths are in metres, written here as the game has them: x to the right, y up, z
towards whoever drives at it.

It has no textures: concrete, steel and paint, by their colours alone.
KEEP_FOLDER, if given, is where the Blender file is kept.
"""
import math
import os
import sys

import bpy

args = sys.argv[sys.argv.index("--") + 1:]
OUT = args[0]
KEEP = args[1] if len(args) > 1 else None

# name: colour, roughness, metal
PAINTS = {
    "concrete": ((0.42, 0.41, 0.39), 0.9, 0.0),
    "slab": ((0.3, 0.3, 0.29), 0.92, 0.0),
    "rock": ((0.31, 0.25, 0.2), 0.95, 0.0),
    "steel": ((0.33, 0.34, 0.36), 0.45, 0.8),
    "yellow": ((0.9, 0.62, 0.02), 0.6, 0.0),
    "black": ((0.02, 0.02, 0.022), 0.7, 0.0),
    "white": ((0.86, 0.86, 0.84), 0.7, 0.0),
    "red": ((0.62, 0.05, 0.04), 0.6, 0.0),
    "green": ((0.06, 0.38, 0.14), 0.6, 0.0),
    "board": ((0.9, 0.88, 0.8), 0.75, 0.0),
}

# Each piece: its paints, each with its points and faces, in the game's axes.
pieces = {}

# How far paint on the pad stands above it. (It has no depth below it: a line that is
# deeper than it is wide is drawn, from far off, as its dark sides.)
PROUD = 0.02
UNDER = 0.0


def blender(point):
    """A point as the game has it, as Blender has it: z up, and y away from the driver. (A
    turn, not a mirroring: a face keeps the way it faces.)"""
    x, y, z = point
    return (x, -z, y)


def add(piece, paint, verts, faces):
    held, made = pieces.setdefault(piece, {}).setdefault(paint, ([], []))
    first = len(held)
    held.extend(blender(v) for v in verts)
    made.extend(tuple(first + i for i in face) for face in faces)


def box(piece, paint, low, high):
    """A box, by its least corner and its greatest."""
    (x0, y0, z0), (x1, y1, z1) = low, high
    verts = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
             (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (2, 3, 7, 6), (1, 2, 6, 5), (3, 0, 4, 7)]
    add(piece, paint, verts, faces)


def wedge(piece, paint, wide, long, high, lean=0.0):
    """A ramp: from the ground at its near end (+z) up to `high` at its far end, where it
    ends in a wall. With `lean`, its top is that much narrower than its foot each side."""
    w, top = wide * 0.5, wide * 0.5 - lean
    z0, z1 = long * 0.5, -long * 0.5
    verts = [(-w, 0, z0), (w, 0, z0), (w, 0, z1), (-w, 0, z1), (top, high, z1), (-top, high, z1)]
    faces = [(0, 1, 4, 5), (0, 3, 2, 1), (2, 3, 5, 4), (1, 2, 4), (3, 0, 5)]
    add(piece, paint, verts, faces)


def stripes(piece, x0, x1, y0, y1, z, wide=0.5, proud=0.012):
    """Hazard stripes on a face that looks towards +z: yellow and black, upright bars."""
    count = max(2, round((x1 - x0) / wide))
    step = (x1 - x0) / count
    for index in range(count):
        paint = "yellow" if index % 2 == 0 else "black"
        box(piece, paint, (x0 + index * step, y0, z), (x0 + (index + 1) * step, y1, z + proud))


def words(piece, paint, text, at, tall, flat=False, proud=0.012):
    if flat:
        proud = PROUD
    """Lettering, its middle at `at`: standing, to be read from +z, or flat on the ground,
    to be read by whoever drives over it along -z."""
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
    verts = []
    for vertex in mesh.vertices:
        # The text lies in Blender's x and y, read from above: x across, y up the line.
        across, up, out = vertex.co.x, vertex.co.y, vertex.co.z
        if flat:
            verts.append((at[0] + across, at[1] + out + proud, at[2] - up))
        else:
            verts.append((at[0] + across, at[1] + up, at[2] + out + proud))
    faces = [tuple(polygon.vertices) for polygon in mesh.polygons]
    held_points, made = pieces.setdefault(piece, {}).setdefault(paint, ([], []))
    first = len(held_points)
    held_points.extend(blender(v) for v in verts)
    made.extend(tuple(first + i for i in face) for face in faces)
    bpy.data.objects.remove(held, do_unlink=True)
    bpy.data.meshes.remove(mesh)
    bpy.data.curves.remove(curve)


def sign(piece, heading, name, hint):
    """A board on two posts beside a lane, facing whoever drives down it: its heading (the
    test's number), what it is, and what to do with it."""
    for x in (-1.05, 1.05):
        box(piece, "steel", (x - 0.04, 0.0, -0.04), (x + 0.04, 3.0, 0.04))
    box(piece, "board", (-1.4, 1.5, 0.04), (1.4, 3.1, 0.07))
    box(piece, "red", (-1.4, 2.62, 0.07), (1.4, 3.1, 0.075))
    words(piece, "white", heading, (0.0, 2.86, 0.075), 0.34)
    words(piece, "black", name, (0.0, 2.27, 0.07), 0.42)
    words(piece, "black", hint, (0.0, 1.76, 0.07), 0.2)


def build():
    # The pad itself: a slab of concrete, dead flat, that everything stands on. The land
    # under it is not quite flat, and a test wants to be the same every time. Its top is
    # its origin, and it is 120 m wide and 170 m long.
    # (Laid in squares of five metres, each its own: the renderer draws small triangles,
    # and a slab of twelve that are each longer than the view is not drawn at all. They
    # are its top alone, with a skirt round its edge: squares that were each a whole box
    # were drawn, from far off, with slivers of their hidden sides showing.)
    for column in range(24):
        for row in range(34):
            x, z = -60.0 + column * 5.0, -85.0 + row * 5.0
            top = [(x, 0.0, z), (x, 0.0, z + 5.0), (x + 5.0, 0.0, z + 5.0), (x + 5.0, 0.0, z)]
            add("pad", "slab", top, [(0, 1, 2, 3)])
    for column in range(24):
        x = -60.0 + column * 5.0
        for z, turn in ((-85.0, (0, 1, 2, 3)), (85.0, (3, 2, 1, 0))):
            side = [(x, -0.5, z), (x, 0.0, z), (x + 5.0, 0.0, z), (x + 5.0, -0.5, z)]
            add("pad", "slab", side, [turn])
    for row in range(34):
        z = -85.0 + row * 5.0
        for x, turn in ((-60.0, (3, 2, 1, 0)), (60.0, (0, 1, 2, 3))):
            side = [(x, -0.5, z), (x, 0.0, z), (x, 0.0, z + 5.0), (x, -0.5, z + 5.0)]
            add("pad", "slab", side, [turn])
    # 1. A solid wall: the whole nose meets it.
    box("wall", "concrete", (-4.0, 0.0, -0.5), (4.0, 3.0, 0.5))
    stripes("wall", -4.0, 4.0, 0.25, 1.05, 0.5)
    # 2. A post: one narrow part of the nose meets it.
    box("post", "steel", (-0.2, 0.0, -0.2), (0.2, 2.6, 0.2))
    stripes("post", -0.2, 0.2, 0.3, 0.7, 0.2, wide=0.2)
    stripes("post", -0.2, 0.2, 1.3, 1.7, 0.2, wide=0.2)
    # 3. An offset barrier: a block that one side of the nose meets.
    box("barrier", "concrete", (-1.7, 0.0, -0.45), (1.7, 1.4, 0.45))
    stripes("barrier", -1.7, 1.7, 0.2, 0.9, 0.45, wide=0.425)
    # 4. An angled wall is the wall again, turned by the scene.
    # 5. Ledges of rock across the lane: what comes up through the wheels.
    box("ledge_low", "rock", (-3.0, 0.0, -0.6), (3.0, 0.2, 0.6))
    box("ledge_high", "rock", (-3.0, 0.0, -0.6), (3.0, 0.4, 0.6))
    for piece, high in (("ledge_low", 0.2), ("ledge_high", 0.4)):
        box(piece, "yellow", (-3.0, high - 0.05, 0.6), (3.0, high, 0.612))
    # 6. A ramp to a drop: it is driven up and off the end.
    wedge("ramp", "concrete", 4.2, 16.0, 2.2)
    for x in (-2.1, 2.05):
        box("ramp", "yellow", (x, 0.0, 7.95), (x + 0.05, 0.02, 8.0))
    # 7. A rollover bank: a narrow ramp under one side's wheels.
    wedge("bank", "concrete", 1.5, 9.0, 1.6, lean=0.1)

    # The signs, one a test.
    # (Named by their numbers: the bake finds a piece by the end of its name, and a sign
    # called for its wall would be found for the wall.)
    sign("sign_1", "TEST 1", "WALL", "square on, any speed")
    sign("sign_2", "TEST 2", "POST", "middle of the nose")
    sign("sign_3", "TEST 3", "OFFSET", "left of the nose only")
    sign("sign_4", "TEST 4", "ANGLED", "a glancing blow")
    sign("sign_5", "TEST 5", "LEDGES", "20 cm, then 40 cm")
    sign("sign_6", "TEST 6", "DROP", "off the end: 2.2 m")
    sign("sign_7", "TEST 7", "ROLLOVER", "left wheels up the bank")
    sign("sign_0", "START AGAIN", "REPAIR BAY", "stop in the box to mend")

    # Paint on the ground, a lane's worth: the scene lays one in each lane. Its edges, a
    # bar across it every twenty metres with how far it is to the test, and its number at
    # the start. A lane is 8 m wide and runs 90 m from the start line, at its +z end.
    for lane in range(1, 8):
        piece = f"lane_{lane}"
        for x in (-4.0, 3.84):
            box(piece, "white", (x, UNDER, -45.0), (x + 0.16, PROUD, 45.0))
        box(piece, "white", (-4.0, UNDER, 44.7), (4.0, PROUD, 45.0))
        words(piece, "white", str(lane), (0.0, 0.0, 42.0), 2.6, flat=True)
        for metres in (20, 40, 60):
            z = -45.0 + metres
            box(piece, "yellow", (-4.0, UNDER, z - 0.1), (-2.6, PROUD, z + 0.1))
            box(piece, "yellow", (2.6, UNDER, z - 0.1), (4.0, PROUD, z + 0.1))
            words(piece, "yellow", f"{metres} m", (-3.1, 0.0, z + 0.9), 0.7, flat=True)
    # The repair bay: a box on the ground, to stop in.
    for x0, z0, x1, z1 in ((-3.5, -3.5, 3.5, -3.3), (-3.5, 3.3, 3.5, 3.5), (-3.5, -3.5, -3.3, 3.5), (3.3, -3.5, 3.5, 3.5)):
        box("bay", "green", (x0, UNDER, z0), (x1, PROUD, z1))
    words("bay", "green", "REPAIR", (0.0, 0.0, 0.0), 1.3, flat=True)


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build()
    materials = {}
    for name, (colour, rough, metal) in PAINTS.items():
        material = bpy.data.materials.new("crash_" + name)
        material.use_nodes = True
        material.use_backface_culling = True
        shader = material.node_tree.nodes["Principled BSDF"]
        shader.inputs["Base Color"].default_value = (*colour, 1.0)
        shader.inputs["Roughness"].default_value = rough
        shader.inputs["Metallic"].default_value = metal
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
    print(f"crash pad: {len(pieces)} pieces, {triangles} triangles")
    if KEEP:
        os.makedirs(KEEP, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(KEEP, "crash.blend"))
    bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_apply=True, export_yup=True)
    print(f"crash pad: wrote {OUT}, {os.path.getsize(OUT) / 1e3:.0f} KB")


main()
