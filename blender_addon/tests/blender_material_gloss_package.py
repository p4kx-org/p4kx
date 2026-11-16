# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the imported roughness graph against the authored palette and DDNA fixture."""

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


FINISH_PALETTE_BYTE = 184
NORMAL_ALPHA = 196
EXACT_SAMPLE_TOLERANCE = 1.0 / 255.0


def verify(location: Path) -> None:
    package = load_package(location)
    result = import_package(bpy.context, package)
    assert result.meshes > 0, result.warnings
    for byte in (0, 1, 128, 255, FINISH_PALETTE_BYTE):
        multipliers = (("Half", 0.5), ("Zero", 0.0))
        if byte == FINISH_PALETTE_BYTE:
            multipliers += (("Neutral", 1.0),)
        for label, multiplier in multipliers:
            name = f"Gloss_{byte}_{label}"
            verify_constant(_roughness(f"{name}_Constant"), byte, multiplier)
            verify_sample(_roughness(f"{name}_Sample"), multiplier)
    _measure_exact_finish(location)


def _roughness(name: str) -> bpy.types.NodeSocket:
    material = bpy.data.materials[name]
    bsdf = principled(material)
    colour = bsdf.inputs["Base Color"]
    assert not colour.is_linked, name
    assert tuple(colour.default_value) == (1.0, 0.0, 0.0, 1.0), name
    return bsdf.inputs["Roughness"]


def verify_constant(socket: bpy.types.NodeSocket, byte: int, multiplier: float) -> None:
    assert not socket.is_linked
    assert math.isclose(socket.default_value, 1.0 - (byte / 255.0) * multiplier, abs_tol=1e-6)


def verify_sample(socket: bpy.types.NodeSocket, multiplier: float) -> None:
    subtract = _input(socket).from_node
    assert subtract.bl_idname == "ShaderNodeMath" and subtract.operation == "SUBTRACT"
    assert subtract.use_clamp
    assert subtract.inputs[0].default_value == 1.0
    alpha = _input(subtract.inputs[1])
    if multiplier != 1.0:
        scale = alpha.from_node
        assert scale.bl_idname == "ShaderNodeMath" and scale.operation == "MULTIPLY"
        assert not scale.inputs[1].is_linked
        assert scale.inputs[1].default_value == multiplier
        alpha = _input(scale.inputs[0])
    assert alpha.from_socket.name == "Alpha"
    assert alpha.from_node.bl_idname == "ShaderNodeTexImage"
    image = alpha.from_node.image
    assert image is not None
    _verify_pixels(image)
    assert image.has_data
    assert tuple(image.size) == (2, 2)
    assert image.alpha_mode == "CHANNEL_PACKED"
    assert image.colorspace_settings.name == "Non-Color"


def _input(socket: bpy.types.NodeSocket) -> bpy.types.NodeLink:
    link, = socket.links
    return link


def _verify_pixels(image: bpy.types.Image) -> None:
    expected = tuple(value / 255.0 for value in (128, 127, 255, NORMAL_ALPHA))
    pixels = tuple(image.pixels)
    assert len(pixels) == len(expected) * 4
    assert all(math.isclose(actual, authored, abs_tol=1e-6)
               for actual, authored in zip(pixels, expected * 4, strict=True)), pixels


def _measure_exact_finish(location: Path) -> None:
    width, height = 64, 24
    neutral = bpy.data.materials[f"Gloss_{FINISH_PALETTE_BYTE}_Neutral_Sample"]
    half = bpy.data.materials[f"Gloss_{FINISH_PALETTE_BYTE}_Half_Sample"]
    probes = ((neutral, "Roughness", 0.1875), (half, "Roughness", -0.1875))
    target = location.parent.with_name(f"{location.parent.name}-roughness.exr")
    pixels = render_linear_signals(probes, target, width, height)
    observed = {}
    for label, row, expected in (
        ("neutral", 3 * height // 4, 59 / 255),
        ("authored_half", height // 4, 157 / 255),
    ):
        start = (row * width + width // 2) * 4
        sample = pixels[start : start + 3]
        assert all(math.isclose(value, expected, abs_tol=EXACT_SAMPLE_TOLERANCE)
                   for value in sample), (label, sample, expected)
        observed[label] = sample
    print(json.dumps({
        "blender": bpy.app.version_string,
        "blender_build": bpy.app.build_hash.decode("ascii"),
        "palette_byte": FINISH_PALETTE_BYTE,
        "ddna_alpha_byte": NORMAL_ALPHA,
        "abs_tolerance": EXACT_SAMPLE_TOLERANCE,
        "roughness": observed,
    }))


if __name__ == "__main__":
    verify(Path(sys.argv[-1]))
    print(f"Blender {bpy.app.version_string} verified packed-alpha and palette roughness inputs.")
