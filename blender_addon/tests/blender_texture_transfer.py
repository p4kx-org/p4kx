# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks package texture transfer and alpha handling in headless Blender."""

from __future__ import annotations

import math
import sys
import tempfile
from pathlib import Path

import bpy

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]

from p4kx_importer.materials import ImageCache  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402
from synthetic import manifest, write_package  # noqa: E402
from synthetic_materials import texture  # noqa: E402


def check_synthetic(root: Path) -> None:
    entries = [
        {**texture(1, "srgb", "opacity"), "encoding": "linear_colour"},
        {**texture(2, "linear", "smoothness"), "encoding": "data"},
        {**texture(3, "linear", "opacity"), "encoding": "srgb"},
        texture(4, "linear", "smoothness"),
    ]
    path = write_package(root, manifest(version="1.12", textures=entries))
    loaded = load_package(path)
    cache = ImageCache(loaded)
    cases = [
        ("t0001", "Linear Rec.709", "STRAIGHT"),
        ("t0002", "Non-Color", "CHANNEL_PACKED"),
        ("t0003", "sRGB", "STRAIGHT"),
        ("t0004", "Non-Color", "CHANNEL_PACKED"),
    ]
    for identifier, space, alpha in cases:
        image = cache.get(identifier)
        assert image is not None, identifier
        assert image.colorspace_settings.name == space, identifier
        assert image.alpha_mode == alpha, identifier
    assert cache.get("t0001") is not cache.get("t0002")
    assert not loaded.warnings, loaded.warnings


def check_produced(root: Path) -> None:
    loaded = load_package(root / "scene.json")
    assert loaded.version == (1, 12)
    cache = ImageCache(loaded)
    colour_id = next(t.id for t in loaded.textures.values() if t.encoding == "linear_colour")
    image = cache.get(colour_id)
    assert image is not None, colour_id
    assert image.colorspace_settings.name == "Linear Rec.709", colour_id
    assert image.alpha_mode == "STRAIGHT", colour_id
    for actual, encoded in zip(tuple(image.pixels[:4]), (128, 64, 16, 81), strict=True):
        assert math.isclose(actual, encoded / 255.0, abs_tol=0.001), tuple(image.pixels[:4])
    data_id = next(t.id for t in loaded.textures.values() if t.alpha == "smoothness")
    data = cache.get(data_id)
    assert data is not None, data_id
    assert data.colorspace_settings.name == "Non-Color", data_id
    assert data.alpha_mode == "CHANNEL_PACKED", data_id
    assert not loaded.warnings, loaded.warnings


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as folder:
        check_synthetic(Path(folder))
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if args:
        check_produced(Path(args[0]))
    print(f"Blender {bpy.app.version_string} texture transfer verified")
