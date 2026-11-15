"""Builds the showroom the game stands a truck in to be looked at: a studio.

  blender -b --python tools/blender/showroom/build.py -- OUT.glb

A photographer's studio, big enough to walk round a truck in: a grey floor that curves up
into a grey wall all the way round, and a dark roof over it: it is lit by its own lamps (the scene's
`light` lines), as a studio is. In the middle is a low
round stand for the truck. Nothing in it is anything but grey: the truck is what is looked at.

Its origin is the middle of its floor, which is the ground; the truck stands at the origin.
"""
import math
import sys

import bmesh
import bpy

OUT = sys.argv[sys.argv.index("--") + 1]
REACH, HIGH, COVE = 22.0, 18.0, 5.0      # how far off its wall is, how high, and how wide the curve between
AROUND = 96
# Whether it has a roof, and is lit by lamps; or is open to the sky, and lit by the day.
ROOFED = False

bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, colour, rough, metal=0.0):
    made = bpy.data.materials.new(name)
    made.use_nodes = True
    shader = made.node_tree.nodes["Principled BSDF"]
    shader.inputs["Base Color"].default_value = (*colour, 1.0)
    shader.inputs["Roughness"].default_value = rough
    shader.inputs["Metallic"].default_value = metal
    return made


def turned(name, profile, paint, inward=False):
    """A profile of (how far out, how high) turned about the upright through the middle."""
    bm = bmesh.new()
    rings = []
    for out, high in profile:
        ring = []
        for step in range(AROUND):
            angle = math.tau * step / AROUND
            ring.append(bm.verts.new((out * math.cos(angle), out * math.sin(angle), high)))
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for step in range(AROUND):
            after = (step + 1) % AROUND
            face = [a[step], a[after], b[after], b[step]]
            bm.faces.new(face if inward else list(reversed(face)))
    if profile[0][0] > 0.0:
        middle = bm.verts.new((0.0, 0.0, profile[0][1]))
        for step in range(AROUND):
            face = [middle, rings[0][(step + 1) % AROUND], rings[0][step]]
            bm.faces.new(face if not inward else list(reversed(face)))
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(paint)
    for poly in mesh.polygons:
        poly.use_smooth = True
    made = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(made)
    return made


grey = material("studio", (0.2, 0.2, 0.205), 0.55)
floor = material("studio_floor", (0.085, 0.085, 0.09), 0.5)
stand = material("studio_stand", (0.13, 0.13, 0.135), 0.42)
edge = material("studio_edge", (0.75, 0.75, 0.76), 0.3, 0.9)
roof = material("studio_roof", (0.03, 0.03, 0.032), 0.9)

# The floor, out to where it begins to curve up.
# (In rings a stride or two wide: a floor of a few great triangles is lit unevenly.)
rings = [4.4 + (REACH - COVE - 4.4) * step / 10 for step in range(11)]
turned("floor", [(out, 0.0) for out in rings], floor, inward=True)
# The curve, a quarter of a circle, and the wall above it.
cove = [(REACH - COVE + COVE * math.sin(math.pi / 2 * step / 12), COVE - COVE * math.cos(math.pi / 2 * step / 12)) for step in range(13)]
turned("wall", cove + [(REACH, HIGH)], grey, inward=True)
# And a roof over it, dark: the lamps that light it hang under that.
if ROOFED:
    turned("roof", [(REACH, HIGH), (12.0, HIGH + 0.6), (0.01, HIGH + 0.8)], roof, inward=True)
# The stand: a low round step, with a bright edge to it.
turned("stand", [(4.0, 0.035), (4.38, 0.035)], stand, inward=True)
turned("stand_top", [(0.01, 0.035), (1.0, 0.035), (2.0, 0.035), (3.0, 0.035), (4.0, 0.035)], stand, inward=True)
turned("stand_edge", [(4.38, 0.035), (4.4, 0.03), (4.4, 0.0)], edge, inward=True)

for each in bpy.context.scene.objects:
    each.select_set(True)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_apply=True, export_yup=True, use_selection=True)
print("wrote", OUT)
