# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks native collision import and preservation of render objects."""

from __future__ import annotations

import json
from importlib import import_module

from pathlib import Path

import addon_utils
import bpy

import synthetic


def cell(value: object, encrypted: bool = False) -> dict[str, object]:
    """Returns one present synthetic data value."""
    return {
        "state": "present",
        "value": value,
        "containsEncryptedSource": encrypted,
    }


def helper_document() -> dict[str, object]:
    """Returns one collision mesh shared between two owner nodes."""
    model = {
        "source": "synthetic",
        "slots": [{
            "chunk": 2,
            "slot": 4,
            "packed_key": cell(5),
            "frame": cell([[0.0, -3.0, 0.0, 10.0], [2.0, 0.0, 0.0, 28.0], [0.0, 0.0, 4.0, 30.0]]),
            "geometry": cell({
                "positions_z_up_metres": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
                "indices": [[0, 1, 2]],
            }, True),
            "disposition": "ready",
        }],
    }
    return {
        "schema_version": 1,
        "models": [model],
        "instances": [{"node": node, "model": 0, "cdf": None} for node in (1, 2)],
    }


def imported(folder: Path, enabled: bool, addon: str) -> tuple[tuple[int, int, int], int]:
    """Imports one fixture in an empty native Blender scene."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    package = import_module(f"{addon}.package")
    importer = import_module(f"{addon}.importer")
    parsed = package.load_package(folder)
    result = importer.import_package(bpy.context, parsed, collision_helpers=enabled)
    objects = [item for item in bpy.data.objects if item.get("p4kx_role") == "collision_helper"]
    if not enabled:
        assert not objects, "the default never imports helper objects"
    else:
        first, second = objects
        assert first.data == second.data, "two owner instances share one source mesh"
        assert first.parent != second.parent, "physical owners stay distinct"
        assert first.hide_render, "helper objects stay out of render"
        assert first.get("p4kx_contains_encrypted_source"), "provenance follows the geometry"
        expected = [(10, 28, 30), (10, 30, 30), (7, 28, 30)]
        assert [tuple(vertex.co) for vertex in first.data.vertices] == expected, "source affine and metres"
    return (result.objects, result.meshes, result.instances), result.collision_helpers


def run(folder: Path, addon: str) -> None:
    """Checks both import modes and rejects malformed geometry before scene mutation."""
    package = import_module(f"{addon}.package")
    importer = import_module(f"{addon}.importer")
    collision_helpers = import_module(f"{addon}.collision_helpers")
    reference = {"file": collision_helpers.FILE, "schema_version": 1}
    manifest = synthetic.manifest(version="1.18", collision_helpers=reference)
    synthetic.write_package(folder, manifest)
    path = folder / collision_helpers.FILE
    path.write_text(json.dumps(helper_document()), encoding="utf-8")
    base, absent = imported(folder, False, addon)
    same, count = imported(folder, True, addon)
    assert base == same and absent == 0 and count == 2, "opt-in preserves the default render topology"
    document = helper_document()
    document["models"][0]["slots"][0]["geometry"]["value"]["indices"] = [[0, 65535, 2]]
    path.write_text(json.dumps(document), encoding="utf-8")
    before = len(bpy.data.objects)
    try:
        importer.import_package(bpy.context, package.load_package(folder), collision_helpers=True)
    except package.PackageError:
        assert len(bpy.data.objects) == before, "preflight leaves the original scene intact"
    else:
        raise AssertionError("an invalid triangle index was accepted")
