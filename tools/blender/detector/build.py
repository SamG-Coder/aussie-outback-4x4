"""Builds the metal detector the driver carries on foot, as a model.

  blender -b --python tools/blender/detector/build.py -- OUT.glb [KEEP_FOLDER]

A gold detector of the usual kind: a search coil on the ground ahead, a shaft in three
sections up to a handgrip, the control box under the grip, and a cuff for the forearm. It is
made as it is carried, by somebody facing Blender's +Y (the file's -Z) with it in their right
hand and its coil a stride ahead on the ground; its origin is its grip, where it is held.
It is a thing of its own in the game: held, the game puts the grip in the driver's hand and
tips the shaft until the coil is over the ground; put down, it lies along its shaft.

It has no textures: plain painted plastic and bare aluminium, by their colours alone.
KEEP_FOLDER, if given, is where the Blender file and a picture of it are kept.
"""
import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index("--") + 1:]
OUT = args[0]
KEEP = args[1] if len(args) > 1 else None

# name: colour, roughness, metal
PAINTS = {
    "black": ((0.02, 0.02, 0.022), 0.55, 0.0),
    "grip": ((0.035, 0.035, 0.04), 0.95, 0.0),
    "shaft": ((0.62, 0.63, 0.65), 0.35, 0.4),
    "yellow": ((0.85, 0.55, 0.04), 0.5, 0.0),
    "screen": ((0.05, 0.16, 0.12), 0.15, 0.0),
}
groups = {name: ([], []) for name in PAINTS}


# Where the grip is held, from the feet of whoever holds it: the model's origin.
HAND = Vector((0.27, 0.30, 0.98))


def add(paint, verts, faces, at=Matrix.Identity(4)):
    held, made = groups[paint]
    first = len(held)
    held.extend(tuple(at @ Vector(v) - HAND) for v in verts)
    made.extend(tuple(first + i for i in face) for face in faces)


def between(a, b):
    a, b = Vector(a), Vector(b)
    z = (b - a).normalized()
    x = z.orthogonal().normalized()
    y = z.cross(x)
    m = Matrix.Identity(4)
    for row in range(3):
        m[row][0], m[row][1], m[row][2], m[row][3] = x[row], y[row], z[row], a[row]
    return m, (b - a).length


def rod(paint, a, b, radius, around=12, radius_b=None):
    """A round bar from one place to another, with its ends closed."""
    at, length = between(a, b)
    radius_b = radius if radius_b is None else radius_b
    verts, faces = [], []
    for z, r in ((0.0, radius), (length, radius_b)):
        verts += [(r * math.cos(i / around * math.tau), r * math.sin(i / around * math.tau), z) for i in range(around)]
    for i in range(around):
        j = (i + 1) % around
        faces.append((i, j, j + around, i + around))
    faces.append(tuple(range(around - 1, -1, -1)))
    faces.append(tuple(range(around, 2 * around)))
    add(paint, verts, faces, at)


def box(paint, size, at):
    hx, hy, hz = (s / 2 for s in size)
    verts = [(-hx, -hy, -hz), (hx, -hy, -hz), (hx, hy, -hz), (-hx, hy, -hz),
             (-hx, -hy, hz), (hx, -hy, hz), (hx, hy, hz), (-hx, hy, hz)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (2, 3, 7, 6), (1, 2, 6, 5), (3, 0, 4, 7)]
    add(paint, verts, faces, at)


def ring(paint, outer, inner, thick, at, around=40):
    """A flat ring lying in its own xy: the search coil's rim."""
    verts, faces = [], []
    for z in (-thick / 2, thick / 2):
        for r in (outer, inner):
            verts += [(r * math.cos(i / around * math.tau), r * math.sin(i / around * math.tau), z) for i in range(around)]
    for i in range(around):
        j = (i + 1) % around
        low_out, low_in, top_out, top_in = 0, around, 2 * around, 3 * around
        faces += [(low_out + i, low_in + i, low_in + j, low_out + j), (top_out + i, top_out + j, top_in + j, top_in + i),
                  (low_out + i, low_out + j, top_out + j, top_out + i), (low_in + i, top_in + i, top_in + j, low_in + j)]
    add(paint, verts, faces, at)


def build():
    hand = HAND.copy()                      # where the grip is held
    elbow = Vector((0.29, 0.02, 1.10))      # the top of the shaft, by the forearm
    coil = Vector((0.20, 1.55, 0.07))       # the middle of the search coil, on the ground ahead
    knee = hand.lerp(coil, 0.5)
    # The shaft: three sections, each a little thinner, with a collar where they join.
    rod("shaft", elbow, hand, 0.013)
    rod("shaft", hand, knee, 0.0125)
    rod("black", knee, coil + Vector((0, -0.06, 0.05)), 0.011)
    rod("black", hand.lerp(coil, 0.47), hand.lerp(coil, 0.53), 0.017)
    # The cuff the forearm lies in, and its stand.
    slope = (hand - elbow).normalized()
    for side in (-1, 1):
        rod("black", elbow + Vector((side * 0.05, 0, 0.0)), elbow + Vector((side * 0.055, 0, 0.09)), 0.008)
    box("black", (0.11, 0.12, 0.012), Matrix.Translation(elbow + Vector((0, 0, -0.012))))
    rod("black", elbow + Vector((0, 0, -0.012)), elbow + Vector((0, -0.02, -0.16)), 0.009)
    # The grip, standing up off the shaft; and the control box, clamped to the shaft just
    # ahead of it, lying along it with its screen up to be read.
    grip_top = hand + Vector((0, 0.03, 0.13))
    rod("grip", hand, grip_top, 0.019, around=14)
    rod("grip", grip_top, grip_top + Vector((0, 0.035, 0.012)), 0.019, around=14)
    along = (coil - hand).normalized()
    out = Vector((1, 0, 0)).cross(along).normalized()
    across = along.cross(out).normalized()
    lie = Matrix.Identity(4)
    centre = hand + along * 0.19 + out * 0.052
    for row in range(3):
        lie[row][0], lie[row][1], lie[row][2], lie[row][3] = across[row], along[row], out[row], centre[row]
    box("black", (0.09, 0.17, 0.05), lie)
    box("yellow", (0.094, 0.03, 0.054), lie @ Matrix.Translation((0, 0.06, 0)))
    box("screen", (0.062, 0.075, 0.004), lie @ Matrix.Translation((0, -0.02, 0.026)))
    # Its clamp: two blocks that take the shaft between the box and themselves.
    for step in (-0.05, 0.05):
        box("black", (0.045, 0.03, 0.05), lie @ Matrix.Translation((0, step, -0.04)))
    # The battery, behind the cuff.
    box("black", (0.07, 0.11, 0.05), Matrix.Translation(elbow + Vector((0, -0.05, -0.06))))
    # The coil: an open ring with two spokes to a hub, where the shaft's foot pivots.
    flat = Matrix.Translation(coil)
    ring("black", 0.15, 0.118, 0.022, flat)
    for turn in (40, 140):
        a = math.radians(turn)
        reach = Vector((math.cos(a), math.sin(a), 0)) * 0.125
        rod("black", coil - reach, coil + reach, 0.011, around=8)
    rod("black", coil + Vector((0, -0.02, 0)), coil + Vector((0, -0.06, 0.05)), 0.014, around=10)
    ring("yellow", 0.152, 0.148, 0.006, flat @ Matrix.Translation((0, 0, 0.006)))
    # The lead, from the coil up the shaft to the box: a few turns round the lower section.
    turns, steps = 7, 84
    last = None
    for step in range(steps + 1):
        t = step / steps
        along = coil.lerp(hand, 0.06 + 0.9 * t)
        angle = t * turns * math.tau
        out = Vector((math.cos(angle), 0, math.sin(angle))) * 0.019
        here = along + out
        if last is not None:
            rod("black", last, here, 0.0035, around=5)
        last = here


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    build()
    made = []
    for name, (verts, faces) in groups.items():
        if not faces:
            continue
        colour, rough, metal = PAINTS[name]
        mesh = bpy.data.meshes.new("detector_" + name)
        mesh.from_pydata(verts, [], faces)
        mesh.polygons.foreach_set("use_smooth", [True] * len(mesh.polygons))
        mesh.update()
        material = bpy.data.materials.new("detector_" + name)
        material.use_nodes = True
        material.use_backface_culling = True
        shader = material.node_tree.nodes["Principled BSDF"]
        shader.inputs["Base Color"].default_value = (*colour, 1.0)
        shader.inputs["Roughness"].default_value = rough
        shader.inputs["Metallic"].default_value = metal
        mesh.materials.append(material)
        piece = bpy.data.objects.new(mesh.name, mesh)
        bpy.context.scene.collection.objects.link(piece)
        # Faces at more than 35 degrees to each other keep a hard edge between them.
        smooth = piece.modifiers.new("edges", "EDGE_SPLIT")
        smooth.split_angle = math.radians(35)
        made.append(piece)
    triangles = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in made)
    print(f"detector: {len(made)} meshes, {triangles} triangles")
    if KEEP:
        os.makedirs(KEEP, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(KEEP, "detector.blend"))
        scene = bpy.context.scene
        scene.render.engine = "BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items] else "BLENDER_EEVEE"
        scene.render.resolution_x, scene.render.resolution_y = 1280, 720
        world = bpy.data.worlds.new("sky")
        world.use_nodes = True
        world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.7, 0.8, 1.0, 1.0)
        scene.world = world
        sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
        sun.data.energy = 4.0
        sun.rotation_euler = (math.radians(50), 0.0, math.radians(-40))
        scene.collection.objects.link(sun)
        ground = bpy.data.meshes.new("ground")
        low = -HAND.z
        ground.from_pydata([(-5, -5, low), (5, -5, low), (5, 5, low), (-5, 5, low)], [], [(0, 1, 2, 3)])
        dirt = bpy.data.materials.new("dirt")
        dirt.use_nodes = True
        dirt.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.42, 0.25, 0.15, 1.0)
        ground.materials.append(dirt)
        scene.collection.objects.link(bpy.data.objects.new("ground", ground))
        camera = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
        scene.collection.objects.link(camera)
        scene.camera = camera
        camera.location = Vector((1.5, -0.35, 1.45)) - HAND
        camera.data.lens = 40
        camera.rotation_euler = (Vector((0.24, 0.75, 0.6)) - HAND - Vector(camera.location)).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(KEEP, "detector.png")
        bpy.ops.render.render(write_still=True)
        for name in ("sun", "ground", "camera"):
            bpy.data.objects.remove(bpy.data.objects[name], do_unlink=True)
    bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_apply=True, export_yup=True)
    print(f"detector: wrote {OUT}, {os.path.getsize(OUT) / 1e3:.0f} KB")


main()
