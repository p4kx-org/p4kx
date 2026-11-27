# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Compare ordinary imported roughness at deterministic original-subset UV probes."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from blender_foreground_roughness_probe import render_roughness_pairs  # noqa: E402
from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402

QUANTIZATION_TOLERANCE = 2.0 / 255.0


def _one_drawn_material(objects: tuple[bpy.types.Object, ...]) -> bpy.types.Material:
    materials = {
        slot.material for obj in objects if obj.type == "MESH" and len(obj.data.polygons)
        for slot in obj.material_slots if slot.material is not None
    }
    assert len(materials) == 1, tuple(material.name for material in materials)
    return materials.pop()


def _uv_probes(objects: tuple[bpy.types.Object, ...], triangles: int) -> tuple[tuple[float, float], ...]:
    meshes = tuple(obj.data for obj in objects if obj.type == "MESH" and len(obj.data.polygons))
    assert len(meshes) == 1, len(meshes)
    mesh = meshes[0]
    assert len(mesh.polygons) == triangles, (len(mesh.polygons), triangles)
    uv = mesh.uv_layers.active
    assert uv is not None
    result = []
    for ordinal in (0, triangles // 3, 2 * triangles // 3, triangles - 1):
        polygon = mesh.polygons[ordinal]
        assert len(polygon.loop_indices) == 3
        point = tuple(sum(uv.data[loop].uv[axis] for loop in polygon.loop_indices) / 3.0
                      for axis in (0, 1))
        assert all(math.isfinite(value) for value in point)
        result.append(point)
    return tuple(result)


def _new_meshes(before: set[bpy.types.Object]) -> tuple[bpy.types.Object, ...]:
    return tuple(obj for obj in bpy.data.objects if obj not in before and obj.type == "MESH")


def _import_pair(root: Path, triangles: int):
    before = set(bpy.data.objects)
    imported = import_package(bpy.context, load_package(root / "package" / "scene.json"))
    assert imported.meshes == 1, imported.warnings
    package_objects = _new_meshes(before)
    package_material = _one_drawn_material(package_objects)
    package_uvs = _uv_probes(package_objects, triangles)
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(root / "textured.glb"))
    glb_objects = _new_meshes(before)
    glb_material = _one_drawn_material(glb_objects)
    glb_uvs = _uv_probes(glb_objects, triangles)
    for package, glb in zip(package_uvs, glb_uvs, strict=True):
        assert all(math.isclose(left, right, rel_tol=0.0, abs_tol=1e-6)
                   for left, right in zip(package, glb, strict=True)), (package, glb)
    return package_material, glb_material, package_uvs


def verify(root: Path) -> None:
    witness = json.loads((root / "witness.private.json").read_text(encoding="utf-8"))
    assert witness["scope"] == "one-original-root-subset-consumer-witness"
    assert not witness["nativeBlenderReadbackExecuted"] and not witness["gameAppearanceProven"]
    assert not witness["originalFirstUseVertexOrderPreserved"]
    assert not witness["firstUseVertexMapRequested"]
    triangles = witness["triangles"]
    assert triangles > 0
    package, glb, uvs = _import_pair(root, triangles)
    readback = render_roughness_pairs(package, glb, uvs, root / "roughness-linear.exr")
    comparisons = []
    for ordinal, (uv, (addon, gltf)) in enumerate(zip(uvs, readback.pairs, strict=True)):
        error = abs(addon - gltf)
        assert error <= QUANTIZATION_TOLERANCE, (ordinal, uv, addon, gltf, error)
        comparisons.append({"ordinal": ordinal, "uv": uv, "addon": (addon,) * 3, "gltf": (gltf,) * 3,
                            "maxAbsoluteError": error})
    receipt = {"schema": 1, "blender": bpy.app.version_string,
               "blenderBuild": bpy.app.build_hash.decode("ascii"), "mip": witness["mip"],
               "triangles": triangles, "sourcePins": witness["inputPins"],
               "scope": "linear-emission-roughness-at-original-subset-uvs",
               "absoluteTolerance": QUANTIZATION_TOLERANCE, "comparisons": comparisons,
               "foregroundControls": {"foregroundPixels": readback.foreground_pixels,
                                      "scalarControls": readback.scalar_controls,
                                      "backgroundPixels": readback.background_pixels},
               "gameAppearanceProven": False, "runtimePermutation": None}
    (root / "blender-readback.private.json").write_text(json.dumps(receipt, indent=2) + "\n",
                                                       encoding="utf-8")
    print(json.dumps({"scope": receipt["scope"], "passed": len(comparisons),
                      "blender": receipt["blender"], "gameAppearanceProven": False}))


if __name__ == "__main__":
    verify(Path(sys.argv[-1]))
