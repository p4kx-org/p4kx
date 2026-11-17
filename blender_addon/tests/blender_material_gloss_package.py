# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks imported roughness against authored palette, DDNA and RGB Gloss inputs."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from blender_signal_probe import principled, render_linear_signals  # noqa: E402
from p4kx_importer.importer import import_package
from p4kx_importer.material_finish import GlossChannel  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402


FINISH_PALETTE_BYTE = 184
NORMAL_ALPHA = 196
EXACT_SAMPLE_TOLERANCE = 1.0 / 255.0
GLOSS_QUANTIZATION_TOLERANCE = 0.5 / 255.0
EXPLICIT_GLOSS_RGBA = (32, 96, 224, 17)
EXPLICIT_GLOSS_GREEN = 163


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
            verify_sample(_roughness(f"{name}_Sample"), multiplier, byte / 255.0)
    for name in ("ExplicitGloss", "ExplicitGlossAlphaFree"):
        inputs = principled(bpy.data.materials[name]).inputs
        assert inputs["Normal"].is_linked == (name == "ExplicitGlossAlphaFree"), name
        verify_explicit_gloss(inputs["Roughness"])
    packed = principled(bpy.data.materials["ExplicitGlossPackedDdna"]).inputs
    assert packed["Normal"].is_linked
    verify_sample(packed["Roughness"], 1.0)
    for name, has_normal in (("LayerOnlyDdna", True), ("LayerOnlyGloss", False)):
        inputs = principled(bpy.data.materials[name]).inputs
        assert inputs["Normal"].is_linked == has_normal, name
        if has_normal:
            verify_sample(_roughness(name), 0.5, FINISH_PALETTE_BYTE / 255.0)
        else:
            verify_explicit_gloss(_roughness(name), 0.5, FINISH_PALETTE_BYTE / 255.0, GlossChannel.RED)
    inputs = principled(bpy.data.materials["LayerZeroWearAo"]).inputs
    blend = _input(inputs["Roughness"]).from_node
    assert blend.bl_idname == "ShaderNodeGroup"
    factor = _input(blend.inputs["Factor"]).from_node
    assert factor.bl_idname == "ShaderNodeMath" and factor.operation == "MULTIPLY"
    assert factor.inputs[1].default_value == 0.0 and not factor.inputs[1].is_linked
    verify_explicit_gloss(blend.inputs["Roughness A"], 0.5, FINISH_PALETTE_BYTE / 255.0, GlossChannel.RED)
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


def _smoothness(socket: bpy.types.NodeSocket, multiplier: float, palette: float | None = None) -> bpy.types.NodeLink:
    subtract = _input(socket).from_node
    assert subtract.bl_idname == "ShaderNodeMath" and subtract.operation == "SUBTRACT"
    assert subtract.use_clamp
    assert subtract.inputs[0].default_value == 1.0
    smoothness = _input(subtract.inputs[1])
    if multiplier != 1.0:
        scale = smoothness.from_node
        assert scale.bl_idname == "ShaderNodeMath" and scale.operation == "MULTIPLY"
        assert not scale.inputs[1].is_linked
        assert scale.inputs[1].default_value == multiplier
        smoothness = _input(scale.inputs[0])
    if palette is not None:
        scale = smoothness.from_node
        assert scale.bl_idname == "ShaderNodeMath" and scale.operation == "MULTIPLY"
        assert not scale.inputs[1].is_linked
        assert math.isclose(scale.inputs[1].default_value, palette, abs_tol=1e-6)
        smoothness = _input(scale.inputs[0])
    return smoothness


def verify_sample(socket: bpy.types.NodeSocket, multiplier: float, palette: float | None = None) -> None:
    alpha = _smoothness(socket, multiplier, palette)
    assert alpha.from_socket.name == "Alpha"
    assert alpha.from_node.bl_idname == "ShaderNodeTexImage"
    image = alpha.from_node.image
    assert image is not None
    _verify_pixels(image, (128, 127, 255, NORMAL_ALPHA))
    assert image.alpha_mode == "CHANNEL_PACKED"


def verify_explicit_gloss(socket: bpy.types.NodeSocket, multiplier: float = 1.0, palette: float | None = None,
                          channel: GlossChannel = GlossChannel.LUMINANCE) -> None:
    scalar = _smoothness(socket, multiplier, palette)
    expected = "ShaderNodeSeparateColor" if channel is GlossChannel.RED else "ShaderNodeRGBToBW"
    assert scalar.from_node.bl_idname == expected
    assert scalar.from_socket.name == ("Red" if channel is GlossChannel.RED else "Val")
    colour = _input(scalar.from_node.inputs["Color"])
    assert colour.from_socket.name == "Color"
    assert colour.from_node.bl_idname == "ShaderNodeTexImage"
    image = colour.from_node.image
    assert image is not None
    _verify_pixels(image, EXPLICIT_GLOSS_RGBA)
    assert image.alpha_mode == "STRAIGHT"


def _input(socket: bpy.types.NodeSocket) -> bpy.types.NodeLink:
    link, = socket.links
    return link


def _verify_pixels(image: bpy.types.Image, rgba: tuple[int, int, int, int]) -> None:
    pixels = tuple(image.pixels)
    assert image.has_data
    assert tuple(image.size) == (2, 2)
    assert image.colorspace_settings.name == "Non-Color"
    expected = tuple(value / 255.0 for value in rgba)
    assert len(pixels) == len(expected) * 4
    assert all(math.isclose(actual, authored, abs_tol=1e-6)
               for actual, authored in zip(pixels, expected * 4, strict=True)), pixels


def _measure_exact_finish(location: Path) -> None:
    width = height = 64
    cases = (
        ("neutral", f"Gloss_{FINISH_PALETTE_BYTE}_Neutral_Sample", 0.75,
         1.0 - NORMAL_ALPHA / 255 * FINISH_PALETTE_BYTE / 255, EXACT_SAMPLE_TOLERANCE),
        ("authored_half", f"Gloss_{FINISH_PALETTE_BYTE}_Half_Sample", 0.5,
         1.0 - NORMAL_ALPHA / 255 * 0.5 * FINISH_PALETTE_BYTE / 255, EXACT_SAMPLE_TOLERANCE),
        ("explicit_gloss", "ExplicitGloss", 0.25,
         EXPLICIT_GLOSS_GREEN / 255, GLOSS_QUANTIZATION_TOLERANCE),
        ("alpha_free_normal", "ExplicitGlossAlphaFree", 0.0,
         EXPLICIT_GLOSS_GREEN / 255, GLOSS_QUANTIZATION_TOLERANCE),
        ("packed_ddna_precedence", "ExplicitGlossPackedDdna", -0.25,
         59 / 255, GLOSS_QUANTIZATION_TOLERANCE),
        ("child_ddna", "LayerOnlyDdna", -0.5,
         1.0 - 0.5 * NORMAL_ALPHA / 255 * FINISH_PALETTE_BYTE / 255, EXACT_SAMPLE_TOLERANCE),
        ("child_gloss", "LayerOnlyGloss", -0.75,
         1.0 - 0.5 * EXPLICIT_GLOSS_RGBA[0] / 255 * FINISH_PALETTE_BYTE / 255, GLOSS_QUANTIZATION_TOLERANCE),
    )
    cases += (("zero_wear_ao", "LayerZeroWearAo", 0.875,
               1.0 - 0.5 * EXPLICIT_GLOSS_RGBA[0] / 255 * FINISH_PALETTE_BYTE / 255,
               GLOSS_QUANTIZATION_TOLERANCE),)
    probes = tuple((bpy.data.materials[name], "Roughness", y) for _, name, y, _, _ in cases)
    target = location.parent.with_name(f"{location.parent.name}-roughness.exr")
    pixels = render_linear_signals(probes, target, width, height)
    observed = {}
    for label, _, y, expected, tolerance in cases:
        row = height // 2 + round(y * width / 2)
        start = (row * width + width // 2) * 4
        sample = pixels[start : start + 3]
        assert all(math.isclose(value, expected, abs_tol=tolerance)
                   for value in sample), (label, sample, expected)
        observed[label] = sample
    print(json.dumps({
        "blender": bpy.app.version_string,
        "blender_build": bpy.app.build_hash.decode("ascii"),
        "palette_byte": FINISH_PALETTE_BYTE,
        "ddna_alpha_byte": NORMAL_ALPHA,
        "abs_tolerance": EXACT_SAMPLE_TOLERANCE,
        "explicit_gloss_rgba": EXPLICIT_GLOSS_RGBA,
        "explicit_gloss_glb_green_byte": EXPLICIT_GLOSS_GREEN,
        "gloss_quantization_tolerance": GLOSS_QUANTIZATION_TOLERANCE,
        "roughness": observed,
    }))


if __name__ == "__main__":
    verify(Path(sys.argv[-1]))
    print(f"Blender {bpy.app.version_string} verified packed-alpha, RGB Gloss and palette roughness inputs.")
