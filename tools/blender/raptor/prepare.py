"""Prepares the Raptor GLB for Solstice: metres, -Z forward, origin on the ground under the
car, body as one piece, each wheel a piece with its origin at its hub, each caliper a piece
with its origin on that wheel's upright. Prints the measurements the .vehicle file needs.

  blender -b --python prepare_raptor.py -- IN.glb OUT.glb
"""
import bpy, sys, math, json
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index("--") + 1:]
src, dst = args[0], args[1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)

# The model is in centimetres under two 0.01 scales and faces Blender's -Y: turn it to face +Y,
# which the glTF exporter writes as -Z.
# It also comes out a hundredth of its size: the truck is 5.9 m long.
turn = Matrix.Rotation(math.pi, 4, "Z") @ Matrix.Scale(100.0, 4)

WHEELS = {"Ani_Wheel_FL": "FL", "Ani_Disc_Scale_FR": "FR", "Ani_Wheel_BL": "RL", "Ani_Wheel_BR": "RR"}
CALIPERS = {"Caliper_FL": "FL", "Caliper_FR": "FR", "Caliper_BL": "RL", "Caliper_BR": "RR"}

groups = {"body": []}
doomed = []
for obj in list(bpy.context.scene.objects):
    if obj.type != "MESH":
        continue
    mats = [m.name for m in obj.data.materials if m]
    # The lamps' covers are black at half alpha, and would be drawn solid black.
    if "Light_glass" in mats:
        doomed.append(obj)
        continue
    parent = obj.parent.name if obj.parent else ""
    group = "body"
    for key, corner in CALIPERS.items():
        if key in parent:
            group = "caliper_" + corner
    if group == "body":
        for key, corner in WHEELS.items():
            if key in parent:
                group = "wheel_" + corner
    groups.setdefault(group, []).append(obj)

for obj in doomed:
    bpy.data.objects.remove(obj, do_unlink=True)

# Bake every mesh into world space, in metres, turned.
for members in groups.values():
    for obj in members:
        if obj.data.users > 1:
            obj.data = obj.data.copy()
        world = turn @ obj.matrix_world
        obj.parent = None
        obj.data.transform(world)
        if world.determinant() < 0:
            obj.data.flip_normals()
        obj.matrix_world = Matrix.Identity(4)

def bounds(objs):
    lo = Vector((1e9,) * 3); hi = Vector((-1e9,) * 3)
    for obj in objs:
        for v in obj.data.vertices:
            for i in range(3):
                lo[i] = min(lo[i], v.co[i]); hi[i] = max(hi[i], v.co[i])
    return lo, hi

# The ground is where the tyres' lowest points are; the middle is between the wheels.
tyres = {c: [o for o in groups["wheel_" + c] if "tire" in o.name] for c in ("FL", "FR", "RL", "RR")}
lows = [bounds(t)[0].z for t in tyres.values()]
ground = sum(lows) / 4
mid_x = sum((bounds(t)[0].x + bounds(t)[1].x) * 0.5 for t in tyres.values()) / 4
shift = Matrix.Translation(Vector((-mid_x, 0.0, -ground)))
for members in groups.values():
    for obj in members:
        obj.data.transform(shift)

report = {}
joined = {}
for name, members in groups.items():
    if not members:
        continue
    with bpy.context.temp_override(active_object=members[0], selected_editable_objects=members, selected_objects=members):
        if len(members) > 1:
            bpy.ops.object.join()
    obj = members[0]
    obj.name = name
    obj.data.name = name
    joined[name] = obj

hubs = {}
for corner in ("FL", "FR", "RL", "RR"):
    wheel = joined["wheel_" + corner]
    # The hub is the middle of the tyre: find it from the tyre's own vertices.
    tyre_index = [i for i, m in enumerate(wheel.data.materials) if m and m.name == "tire"]
    verts = set()
    for poly in wheel.data.polygons:
        if poly.material_index in tyre_index:
            verts.update(poly.vertices)
    cos = [wheel.data.vertices[i].co for i in verts]
    lo = Vector((min(c.x for c in cos), min(c.y for c in cos), min(c.z for c in cos)))
    hi = Vector((max(c.x for c in cos), max(c.y for c in cos), max(c.z for c in cos)))
    hub = (lo + hi) * 0.5
    hubs[corner] = hub
    report[corner] = {
        "hub": [round(v, 4) for v in hub],
        "radius": round((hi.z - lo.z) * 0.5, 4),
        "radius_fore_aft": round((hi.y - lo.y) * 0.5, 4),
        "width": round(hi.x - lo.x, 4),
    }

# Left and right the same, so the car's numbers are symmetric: one hub for each axle.
def axle(left, right):
    l, r = hubs[left], hubs[right]
    half = (abs(l.x) + abs(r.x)) * 0.5
    y = (l.y + r.y) * 0.5
    z = (l.z + r.z) * 0.5
    return {left: Vector((math.copysign(half, l.x), y, z)), right: Vector((math.copysign(half, r.x), y, z))}
even = {}
even.update(axle("FL", "FR")); even.update(axle("RL", "RR"))

for corner, hub in even.items():
    wheel = joined["wheel_" + corner]
    # Move the wheel's own hub onto the even one, then put its origin there.
    wheel.data.transform(Matrix.Translation(hub - hubs[corner] - hub))
    wheel.location = hub
    caliper = joined.get("caliper_" + corner)
    if caliper:
        # On the upright: 3 cm above the hub, so that it rides and steers but does not spin.
        origin = hub + Vector((0.0, 0.0, 0.03))  # metres
        caliper.data.transform(Matrix.Translation(hub - hubs[corner] - origin))
        caliper.location = origin

lo, hi = bounds([joined["body"]])
report["body"] = {"lo": [round(v, 3) for v in lo], "hi": [round(v, 3) for v in hi]}
report["even_hubs"] = {c: [round(v, 4) for v in h] for c, h in even.items()}
report["tris"] = {n: sum(len(p.vertices) - 2 for p in o.data.polygons) for n, o in joined.items()}

# Glass is drawn solid: make it what dark tinted glass looks like from outside.
for m in bpy.data.materials:
    bsdf = m.node_tree.nodes.get("Principled BSDF") if m.node_tree else None
    if not bsdf:
        continue
    if m.name == "Window":
        bsdf.inputs["Base Color"].default_value = (0.012, 0.016, 0.018, 1.0)
        bsdf.inputs["Roughness"].default_value = 0.06
        bsdf.inputs["Metallic"].default_value = 0.0
        bsdf.inputs["Alpha"].default_value = 1.0
        m.blend_method = "OPAQUE"
    if m.name == "Raptordummy_material_3_024":
        bsdf.inputs["Alpha"].default_value = 1.0
        m.blend_method = "OPAQUE"

# Drop the empties.
for obj in list(bpy.context.scene.objects):
    if obj.type != "MESH":
        bpy.data.objects.remove(obj, do_unlink=True)

bpy.ops.export_scene.gltf(filepath=dst, export_format="GLB", export_yup=True, export_apply=False,
                          export_animations=False, export_skins=False, export_cameras=False, export_lights=False)
print("REPORT " + json.dumps(report))
