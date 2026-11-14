# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the optional authored SOC hierarchy in native Blender."""

from __future__ import annotations

import json
import os
import sys
import tempfile
from importlib import import_module
from pathlib import Path

import addon_utils
import bpy

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
INSTALLED_ROOT = os.environ.get("P4KX_TEST_INSTALLED_BLENDER_ROOT")
ADDON = "bl_ext.user_default.p4kx_importer" if INSTALLED_ROOT else "p4kx_importer"
if not INSTALLED_ROOT:
    sys.path.insert(1, str(HERE.parent))

importer = import_module(f"{ADDON}.importer")
package = import_module(f"{ADDON}.package")
soc_scene = import_module(f"{ADDON}.soc_scene")
from synthetic import interior_manifest, write_package  # noqa: E402
from synthetic_soc_scene import scene  # noqa: E402


def binary_scene():
    """Adds one physical binary table and model lookup to the authored hierarchy."""
    document = scene()
    stream = document["occurrences"][0]["streams"][0]
    stream["member"]["source"] = "synthetic-stream"
    stream["includedObjects"] = [0]
    stream["geometry"] = [{
        "chunkOrdinal": 2,
        "modelOrdinal": 0,
        "target": {"state": "present", "value": {"type": "missing"}},
    }]
    document["includedObjects"] = [{
        "ordinal": 2,
        "source": "synthetic-stream",
        "byteLength": "4",
        "raw": {"state": "present", "value": "AQAAAA=="},
        "tables": {"state": "present", "value": {"models": [{"ordinal": 0}]}},
    }]
    return document


def native_import(folder: Path, authored: bool):
    """Imports a fixture with the same visible scene in either mode."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    manifest = interior_manifest()
    manifest["version"] = "1.13"
    if authored:
        manifest["authored_soc"] = {"file": soc_scene.SOC_SCENE_FILE, "schema_version": 1}
        target = folder / soc_scene.SOC_SCENE_FILE
        target.parent.mkdir(parents=True)
        target.write_text(json.dumps(binary_scene()), encoding="utf-8")
    loaded = package.load_package(write_package(folder, manifest))
    result = importer.import_package(bpy.context, loaded)
    bpy.context.view_layer.update()
    stool = bpy.data.objects["stool"]
    lamp = bpy.data.objects["lamp"]
    return result, tuple(stool.matrix_world.translation), tuple(lamp.matrix_world.translation)


def run() -> None:
    """Compares visible scene parity and verifies authored hierarchy and transforms."""
    with tempfile.TemporaryDirectory() as folder:
        folder = Path(folder)
        baseline, stool, lamp = native_import(folder / "plain", False)
        baseline_counts = (baseline.objects, baseline.meshes, baseline.lights, baseline.instances)
        assert bpy.data.collections.get("Authored SOC") is None
        authored, stool_authored, lamp_authored = native_import(folder / "authored", True)
        assert (authored.objects, authored.meshes, authored.lights, authored.instances) == baseline_counts
        assert (stool_authored, lamp_authored) == (stool, lamp)
        metadata = bpy.data.collections.get("Authored SOC")
        assert metadata is not None and len(metadata.objects) == 7
        door = metadata.objects["Door 4"]
        assert door.parent is metadata.objects["Container 0"]
        assert tuple(door.matrix_world.translation) == (4.0, 2.0, 3.0)
        assert tuple(metadata.objects["Room 0"].matrix_world.translation) == (5.0, 6.0, 7.0)
        assert tuple(metadata.objects["Sensor 5"].matrix_world.translation) == (1.0, 2.0, 3.0)
        child = metadata.objects["Container 1"]
        assert child.parent is metadata.objects["Container 0"]
        assert tuple(child.matrix_world.translation) == (8.0, 3.0, 2.0)
        segment = metadata.objects["Segment 1.2"]
        assert segment.parent is metadata.objects["Area entity 4"]
        assert tuple(segment.matrix_world.translation) == (13.0, 10.0, 10.0)
        assert segment["p4kx_contains_encrypted_source"]
        assert all(obj.type == "EMPTY" and obj.hide_render for obj in metadata.objects)
    print("Authored SOC Blender smoke passed")


if __name__ == "__main__":
    run()
