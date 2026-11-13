# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Imports a sourced Entity classification package in native Blender."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import addon_utils
import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer import importer, package  # noqa: E402


def verify(folder: Path) -> None:
    manifest = json.loads((folder / "scene.json").read_text(encoding="utf-8"))
    root = next(node for node in manifest["nodes"] if node["path"] == "")
    child = next(node for node in manifest["nodes"] if node["path"] == "child")
    assert root["entity"]["classification"]["item_type"]["value"] == "RootCase"
    assert child["entity"]["classification"]["item_type"]["value"] == "ChildCase"
    assert child["entity"]["classification"]["subtype"]["value"] == ""
    assert root["mesh"] == child["mesh"]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    loaded = package.load_package(folder)
    result = importer.import_package(bpy.context, loaded)
    assert not result.warnings, result.warnings
    assert result.objects == 4
    assert result.meshes == 1 and result.instances == 2
    print(f"Blender {bpy.app.version_string} sourced entity package verified")


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))