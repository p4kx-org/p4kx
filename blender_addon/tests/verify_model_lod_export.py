# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the P4KX glTF consumer against a synthetic independently verified model."""

from collections import Counter
from math import pi
from pathlib import Path
import sys

import bpy
from mathutils import Quaternion, Vector


def assert_vector(actual, expected):
    assert (actual - Vector(expected)).length < 1e-5, (tuple(actual), expected)


def expected_pose(level):
    if level == 1:
        # Each step is one metre in the local X direction, followed by a 30° rotation.
        displacement = Vector((1.0, 0.0, 0.0))
        positions = [Vector()]
        for index in range(1, 6):
            rotation = Quaternion((0.0, 0.0, 1.0), index * pi / 6.0)
            positions.append(positions[-1] + rotation @ displacement)
        return dict(zip((f"bone_{index}" for index in range(6)), positions, strict=True))
    return {"root": Vector(), "tip": Vector((0.0, 0.0, 2.0))}


def check_blender_result(level):
    objects = list(bpy.context.scene.objects)
    helper = bpy.context.scene.objects["Mount_Helper"]
    assert helper.type == "EMPTY", helper.type
    assert_vector(helper.matrix_world.translation, (1.0, 2.0, 3.0))
    assert_vector(helper.matrix_world.to_3x3() @ Vector((1.0, 0.0, 0.0)), (0.0, 1.0, 0.0))
    mesh, = [
        obj for obj in objects
        if obj.type == "MESH" and any(mod.type == "ARMATURE" for mod in obj.modifiers)
    ]
    armature, = [obj for obj in objects if obj.type == "ARMATURE"]
    modifier, = [modifier for modifier in mesh.modifiers if modifier.type == "ARMATURE"]
    assert modifier.object == armature, "mesh is bound to the imported armature"
    expected = expected_pose(level)
    assert set(bone.name for bone in armature.data.bones) == set(expected), "exact selected skeleton"
    parent = None
    for name, head in expected.items():
        bone = armature.data.bones[name]
        assert bone.parent == parent, "exact imported bone hierarchy"
        world_head = armature.matrix_world @ bone.head_local
        assert_vector(world_head, head)
        parent = bone
    materials = Counter(mesh.data.materials[face.material_index].name for face in mesh.data.polygons)
    assert materials == {"caps_fixture": 4, "sides_fixture": 8}, materials
    positions = [vertex.co for vertex in mesh.data.vertices]
    assert len(positions) == 24, len(positions)
    extent = tuple(
        max(position[axis] for position in positions) - min(position[axis] for position in positions)
        for axis in range(3)
    )
    assert_vector(Vector(extent), (2.0,) * 3 if level == 1 else (4.0,) * 3)


def main():
    artifacts = Path(sys.argv[sys.argv.index("--") + 1])
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from p4kx_importer.importer import _run_gltf

    assert bpy.app.version[:2] == (5, 3), bpy.app.version_string
    for requested, physical in ((0, 0), (1, 1), (3, 0)):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        result = _run_gltf(bpy.context, artifacts / f"lod{requested}.glb")
        assert result == {"FINISHED"}, result
        check_blender_result(physical)
        print(f"Verified Blender {bpy.app.version_string} requested LOD {requested}, physical LOD {physical}.")


if __name__ == "__main__":
    main()
