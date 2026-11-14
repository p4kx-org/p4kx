# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies native Blender sampling of the exported mixed-alpha fixture."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer.materials import ImageCache  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402


def verify(location: Path) -> None:
    package = load_package(location)
    assert package.version == (1, 15)
    cache = ImageCache(package)
    manifest = json.loads((location / "scene.json").read_text(encoding="utf-8"))
    for suffix, expected_format in (("base_diff.dds", "png"), ("wear_diff.dds", "dds")):
        identifier = next(row["id"] for row in manifest["textures"] if row["source"].endswith(suffix))
        row = package.textures[identifier]
        assert row.format == expected_format
        image = cache.get(row.id)
        assert image is not None, package.warnings
        assert image.alpha_mode == "STRAIGHT"
        if expected_format == "png":
            assert image.colorspace_settings.name == "Linear Rec.709"
            alphas = list(image.pixels)[3::4]
            assert sum(value == 0.0 for value in alphas) == 1
            assert all(value == 0 or math.isclose(value, 128 / 255.0, abs_tol=0.001) for value in alphas)
        else:
            assert tuple(image.size) == (3, 2)
            assert sorted(list(image.pixels)[3::4]) == [0.0, 1.0, 1.0, 1.0, 1.0, 1.0]
    assert not package.warnings, package.warnings


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
    print(f"Blender {bpy.app.version_string} mixed alpha package verified")
