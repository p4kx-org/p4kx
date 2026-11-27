# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verify the rigid mount and evaluated pose of synthetic Character packages."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import sys

import addon_utils
import bpy
from mathutils import Matrix, Quaternion, Vector

POSITION_TOLERANCE = 1e-4
SPINE = "spine"


def require(condition: bool, label: str) -> None:
    """Reject a missing or incorrect pose observation."""
    if not condition:
        raise RuntimeError(label)


def require_position(actual: Vector, expected: Vector, label: str) -> None:
    """Check positions in Blender world metres."""
    require(
        (actual - expected).length <= POSITION_TOLERANCE,
        f"{label}: expected {tuple(expected)}, got {tuple(actual)}",
    )


def evaluated_world(obj: bpy.types.Object) -> Matrix:
    """Return the object's current dependency-graph world matrix."""
    graph = bpy.context.evaluated_depsgraph_get()
    return obj.evaluated_get(graph).matrix_world.copy()


def evaluated_bone(armature: bpy.types.Object, name: str) -> Matrix:
    """Return a pose bone's current dependency-graph world matrix."""
    graph = bpy.context.evaluated_depsgraph_get()
    current = armature.evaluated_get(graph)
    return current.matrix_world @ current.pose.bones[name].matrix


def require_matrix(actual: Matrix, expected: Matrix, label: str) -> None:
    """Check all components of the evaluated transform."""
    difference = max(
        abs(value - target)
        for actual_row, expected_row in zip(actual, expected)
        for value, target in zip(actual_row, expected_row)
    )
    require(difference <= POSITION_TOLERANCE, f"{label}: matrix error {difference:.6g}")


def move_spine(armature: bpy.types.Object) -> None:
    """Change the authored anchor without moving the root pelvis."""
    spine = armature.pose.bones[SPINE]
    spine.rotation_mode = "QUATERNION"
    spine.rotation_quaternion = Quaternion(Vector((0.3, 0.7, 0.5)).normalized(), math.radians(41.0))
    spine.location = Vector((0.2, -0.1, 0.6))
    bpy.context.view_layer.update()


def verify_pose(armature: bpy.types.Object, satchel: bpy.types.Object, helper: bpy.types.Object) -> float:
    """Verify that the rigid mesh and its locator follow the selected bone."""
    pelvis = evaluated_bone(armature, "pelvis")
    spine = evaluated_bone(armature, SPINE)
    satchel_before = evaluated_world(satchel)
    helper_before = evaluated_world(helper)
    (drawable,) = (
        obj for obj in (satchel, *satchel.children_recursive)
        if obj.type == "MESH" and len(obj.data.polygons) > 0
    )
    vertex = drawable.data.vertices[0].co.copy()
    drawn_before = evaluated_world(drawable) @ vertex
    move_spine(armature)

    require_matrix(evaluated_bone(armature, "pelvis"), pelvis, "root pelvis moved")
    delta = evaluated_bone(armature, SPINE) @ spine.inverted()
    satchel_after = evaluated_world(satchel)
    require_matrix(satchel_after, delta @ satchel_before, "rigid mesh did not follow the spine matrix")
    require_matrix(evaluated_world(helper), delta @ helper_before, "child locator did not follow the spine matrix")
    drawn_after = evaluated_world(drawable) @ vertex
    require_position(drawn_after, delta @ drawn_before, "drawn vertex pose")
    displacement = (drawn_after - drawn_before).length
    require(displacement > POSITION_TOLERANCE, "pose control did not move the drawn satchel")
    return round(displacement, 6)


def verify_package(folder: Path, *, carrier: str, offset: float) -> dict[str, object]:
    """Import one normal Character package and check its authored mount and evaluated pose."""
    from p4kx_importer import importer, package
    from p4kx_importer.validate_package import validate_package

    scene_file = folder / "scene.json"
    report = validate_package(scene_file)
    structure, digests = report["structure"], report["digests"]
    require(structure["status"] == "valid", f"{folder.name}: invalid package structure: {structure}")
    require(digests["status"] == "verified", f"{folder.name}: unverified package digests: {digests}")
    loaded = package.load_package(scene_file)
    require(loaded.version == (1, 21), f"{folder.name}: expected package format 1.21")
    (rig,) = (mesh.rig for mesh in loaded.meshes.values() if mesh.rig is not None)
    require(rig.worn_mount_provenance is not None, f"{folder.name}: missing worn mount provenance")
    pieces = {part.name for part in rig.parts if part.origin == "piece"}
    require(pieces == {carrier, "pack/Mount_Satchel"}, f"{folder.name}: incorrect worn source pieces")
    require(rig.body == "male" and rig.bones == 3 and not rig.animations, "fixture rig or pose controls differ")

    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    result = importer.import_package(bpy.context, loaded)
    require(result.meshes == 1 and result.armatures == 1, f"{folder.name}: rigged package import failed")
    require(not result.warnings, f"{folder.name}: unexpected import warnings: {result.warnings}")
    bpy.context.view_layer.update()

    (armature,) = (obj for obj in bpy.data.objects if obj.type == "ARMATURE")
    satchel = bpy.data.objects.get("satchel")
    helper = bpy.data.objects.get("satchel_loc")
    require(satchel is not None and helper is not None, "missing rigid model or original locator")
    require(satchel.parent == armature and satchel.parent_type == "BONE", "rigid mesh lacks a bone parent")
    require(satchel.parent_bone == SPINE, "rigid mesh is attached to the wrong skeleton bone")
    require(helper.parent == satchel, "original locator is outside its rigid mesh hierarchy")

    expected_locator = Vector((0.0, 0.1, 1.5 + offset))
    expected_center = expected_locator - Vector((0.0, 0.0, 0.5))
    center = evaluated_world(satchel).translation.copy()
    locator = evaluated_world(helper).translation.copy()
    require_position(center, expected_center, "rest rigid mesh")
    require_position(locator, expected_locator, "rest original locator")
    return {
        "case": folder.name,
        "bone": SPINE,
        "restMetres": [round(value, 6) for value in center],
        "locatorRestMetres": [round(value, 6) for value in locator],
        "posedVertexDisplacementMetres": verify_pose(armature, satchel, helper),
    }


def main() -> None:
    """Verify the original and replacement holder in separate empty Blender scenes."""
    parser = argparse.ArgumentParser()
    parser.add_argument("addon", type=Path)
    parser.add_argument("packages", type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    require(bpy.app.version[:2] == (5, 3), "this witness requires Blender 5.3")
    require(args.addon.is_absolute() and args.packages.is_absolute(), "select absolute input paths")
    require((args.addon / "p4kx_importer" / "__init__.py").is_file(), "missing normal P4KX add-on")
    sys.path.insert(0, str(args.addon))
    cases = [
        verify_package(args.packages / "original", carrier="torso/Mount_Carrier", offset=0.2),
        verify_package(args.packages / "replacement", carrier="Mount_New_Carrier", offset=0.8),
    ]
    print(json.dumps({"blender": bpy.app.version_string, "build": bpy.app.build_hash.decode("ascii"), "cases": cases}))


if __name__ == "__main__":
    main()
