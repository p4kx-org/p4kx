# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Measures the current imported LayerBlend_V2 palette colours for authored TintMode tokens."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from blender_signal_probe import principled, render_linear_signals  # noqa: E402
from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402

CASES = ("Zero", "One", "Two", "Three", "Absent", "Malformed", "Nonfinite", "Unknown", "Repeated", "Unselected")
PALETTE_DIFFUSE = (0.527115126, 0.215860500, 0.051269458)
PALETTE_METAL = (0.630757136, 0.871367119, 0.527115126)
AUTHORED_METAL = (0.7, 0.6, 0.9)
PALETTE_DIELECTRIC_IOR_LEVEL = 0.413522254
VALUE_TOLERANCE = 1e-6
RASTER_TOLERANCE = 1.0 / 255.0


def verify(location: Path) -> None:
    package = load_package(location)
    result = import_package(bpy.context, package)
    assert result.meshes > 0 and not result.warnings, result.warnings
    observations = {}
    for case in CASES:
        observations[case] = _verify_current_materials(case)
    levels = _measure_current_ior_levels(location)
    print(json.dumps({
        "blender": bpy.app.version_string,
        "blender_build": bpy.app.build_hash.decode("ascii"),
        "base_colour_and_metallic": observations,
        "dielectric_ior_level": levels,
        "raster_tolerance": RASTER_TOLERANCE,
    }))


def _verify_current_materials(case: str) -> dict[str, object]:
    painted = case != "Unselected"
    expected = {
        "Dielectric": (PALETTE_DIFFUSE if painted else (1.0, 1.0, 1.0), 0.0),
        "Metal": (PALETTE_METAL if painted else AUTHORED_METAL, 1.0),
    }
    facts = {}
    for lane, (colour, metal) in expected.items():
        name = f"TintMode_{case}_{lane}"
        shader = principled(bpy.data.materials[name])
        base = shader.inputs["Base Color"]
        metallic = shader.inputs["Metallic"]
        assert not base.is_linked and not metallic.is_linked, name
        observed = tuple(base.default_value[:3])
        assert all(math.isclose(a, e, abs_tol=VALUE_TOLERANCE)
                   for a, e in zip(observed, colour, strict=True)), (name, observed, colour)
        assert metallic.default_value == metal, (name, metallic.default_value)
        facts[lane] = {"base_colour": observed, "metallic": metallic.default_value}
        if painted and lane == "Dielectric":
            _verify_dielectric_source(shader, name)
    return facts


def _verify_dielectric_source(shader: bpy.types.Node, name: str) -> None:
    socket = shader.inputs["Specular IOR Level"]
    link, = socket.links
    scale = link.from_node
    assert scale.bl_idname == "ShaderNodeMath" and scale.operation == "MULTIPLY", name
    assert math.isclose(scale.inputs[1].default_value, PALETTE_DIELECTRIC_IOR_LEVEL,
                        abs_tol=VALUE_TOLERANCE), name
    grey_link, = scale.inputs[0].links
    assert grey_link.from_node.bl_idname == "ShaderNodeRGBToBW", name
    image_link, = grey_link.from_node.inputs[0].links
    image = image_link.from_node.image
    assert image is not None and tuple(image.size) == (2, 2), name
    assert all(math.isclose(value, 1.0, abs_tol=VALUE_TOLERANCE)
               for value in image.pixels), name


def _measure_current_ior_levels(location: Path) -> dict[str, float]:
    width, height = 64, 24
    one = bpy.data.materials["TintMode_One_Dielectric"]
    two = bpy.data.materials["TintMode_Two_Dielectric"]
    probes = ((one, "Specular IOR Level", 0.1875), (two, "Specular IOR Level", -0.1875))
    target = location.parent.with_name(f"{location.parent.name}-ior-level.exr")
    pixels = render_linear_signals(probes, target, width, height)
    observed = {}
    for label, row in (("authored_1", 3 * height // 4), ("authored_2", height // 4)):
        start = (row * width + width // 2) * 4
        rgb = pixels[start:start + 3]
        assert all(math.isclose(value, PALETTE_DIELECTRIC_IOR_LEVEL, abs_tol=RASTER_TOLERANCE)
                   for value in rgb), (label, rgb)
        observed[label] = rgb[0]
    return observed


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
