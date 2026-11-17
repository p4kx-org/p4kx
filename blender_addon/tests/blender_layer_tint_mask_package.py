# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Measures authored TintColor and Diffuse divergence in a synthetic package."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from blender_signal_probe import render_linear_signals  # noqa: E402
from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402

MATERIAL = "MaskedLayerTintInputs"
GLB_BASELINE = (0.0618753, 0.1655129, 0.0844690)
EXPECTED_BASE = (0.1184226, 0.2439359, 0.3746392)
EXPECTED_IOR = (0.6982082,) * 3
TOLERANCE = 1.0 / 255.0
SIZE = 64


def verify(location: Path) -> None:
    package = load_package(location)
    result = import_package(bpy.context, package)
    assert result.meshes > 0 and not result.warnings, result.warnings
    material = bpy.data.materials[MATERIAL]
    probes = (
        (material, "Base Color", 0.375),
        (material, "Specular IOR Level", 0.0),
        (material, "Metallic", -0.375),
    )
    target = location.parent.with_name(f"{location.parent.name}-tint-effective.exr")
    pixels = render_linear_signals(probes, target, SIZE, SIZE)
    observations = {}
    for label, row, expected in (
        ("base_colour", 44, EXPECTED_BASE),
        ("specular_ior_level", 32, EXPECTED_IOR),
        ("metallic", 20, (0.0, 0.0, 0.0)),
    ):
        offset = (row * SIZE + SIZE // 2) * 4
        observed = pixels[offset:offset + 3]
        assert all(math.isclose(a, e, abs_tol=TOLERANCE)
                   for a, e in zip(observed, expected, strict=True)), (label, observed, expected)
        observations[label] = observed
    assert any(abs(a - e) > 0.05 for a, e in zip(observations["base_colour"], GLB_BASELINE, strict=True))
    print(json.dumps({
        "blender": bpy.app.version_string,
        "blender_build": bpy.app.build_hash.decode("ascii"),
        "observations": observations,
        "absolute_tolerance": TOLERANCE,
    }))


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
