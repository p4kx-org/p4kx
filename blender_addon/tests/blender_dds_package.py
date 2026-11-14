# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies the DDS/PNG package emitted by the synthetic Rust writer."""

from __future__ import annotations

import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer.materials import ImageCache  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402


def verify(location: Path) -> None:
    package = load_package(location)
    assert package.version >= (1, 14)
    cache = ImageCache(package)
    dds = next(texture for texture in package.textures.values() if texture.format == "dds")
    image = cache.get(dds.id)
    assert image is not None, package.warnings
    assert image.colorspace_settings.name == "Linear Rec.709"
    assert image.alpha_mode == "STRAIGHT"
    assert tuple(image.size) == (8, 8)
    for sample, expected in zip(image.pixels[:4], (128, 64, 16, 81), strict=True):
        assert math.isclose(sample, expected / 255.0, abs_tol=0.001)
    normal = next(texture for texture in package.textures.values() if texture.alpha == "smoothness")
    assert normal.format == "png"
    normal_image = cache.get(normal.id)
    assert normal_image is not None, package.warnings
    assert normal_image.colorspace_settings.name == "Non-Color"
    assert normal_image.alpha_mode == "CHANNEL_PACKED"
    assert not package.warnings, package.warnings


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
    print(f"Blender {bpy.app.version_string} DDS package verified")
