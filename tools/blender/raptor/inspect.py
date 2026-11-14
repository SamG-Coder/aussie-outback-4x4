"""Lists the Raptor GLB's objects: hierarchy, bounds and materials. Run headless in Blender."""
import bpy, sys
from mathutils import Vector

path = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=path)

def bounds(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi

lo_all = Vector((1e9,) * 3); hi_all = Vector((-1e9,) * 3)
def walk(obj, depth):
    line = "  " * depth + f"{obj.type[:4]} {obj.name!r}"
    if obj.type == "MESH":
        lo, hi = bounds(obj)
        for i in range(3):
            lo_all[i] = min(lo_all[i], lo[i]); hi_all[i] = max(hi_all[i], hi[i])
        mats = ",".join(m.name for m in obj.data.materials if m)
        line += f" tris={sum(len(p.vertices)-2 for p in obj.data.polygons)} lo=({lo.x:.2f},{lo.y:.2f},{lo.z:.2f}) hi=({hi.x:.2f},{hi.y:.2f},{hi.z:.2f}) mats=[{mats}]"
    else:
        t = obj.matrix_world.translation
        line += f" at=({t.x:.3f},{t.y:.3f},{t.z:.3f}) scale={tuple(round(s,3) for s in obj.matrix_world.to_scale())}"
    print(line)
    for child in obj.children:
        walk(child, depth + 1)

for obj in bpy.context.scene.objects:
    if obj.parent is None:
        walk(obj, 0)
print("ALL lo", tuple(round(v, 3) for v in lo_all), "hi", tuple(round(v, 3) for v in hi_all))
print("MATERIALS")
for m in bpy.data.materials:
    bsdf = m.node_tree.nodes.get("Principled BSDF") if m.use_nodes else None
    info = ""
    if bsdf:
        bc = bsdf.inputs["Base Color"]
        info = f" base={'tex' if bc.is_linked else tuple(round(c,3) for c in bc.default_value)} rough={'tex' if bsdf.inputs['Roughness'].is_linked else round(bsdf.inputs['Roughness'].default_value,2)} metal={'tex' if bsdf.inputs['Metallic'].is_linked else round(bsdf.inputs['Metallic'].default_value,2)} alpha={'tex' if bsdf.inputs['Alpha'].is_linked else round(bsdf.inputs['Alpha'].default_value,2)} blend={m.blend_method if hasattr(m,'blend_method') else ''}"
    print(" ", repr(m.name), info)
