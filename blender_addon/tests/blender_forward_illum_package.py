# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies an authored Forward Illum package and renders its explicit Emittance case."""

from __future__ import annotations

import math
import sys
from pathlib import Path

import bpy

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

from blender_emission_package import _render  # noqa: E402
from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402

TINT = (0.2, 0.4, 0.8)


def verify(location: Path) -> None:
    result = import_package(bpy.context, load_package(location))
    assert result.meshes > 0, result.warnings
    for name in ("ForwardAbsent", "ForwardZero", "ForwardPositive"):
        _, shader = _material(name)
        _diffuse_coverage(shader)
        emission = shader.inputs["Emission Color"]
        assert not emission.is_linked, name
        strength = shader.inputs["Emission Strength"].default_value
        if name == "ForwardPositive":
            _close(emission.default_value[:3], TINT)
            assert strength == 2.0
        else:
            _close(tuple(channel * strength for channel in emission.default_value[:3]), (0.0, 0.0, 0.0))

    tree, shader = _material("ForwardMapped")
    diffuse = _diffuse_coverage(shader)
    _emittance(shader, diffuse)
    tree.links.remove(_input(shader.inputs["Base Color"]))
    tree.links.remove(_input(shader.inputs["Alpha"]))
    _render(tree, shader, check_coverage=False)


def _material(name: str) -> tuple[bpy.types.NodeTree, bpy.types.Node]:
    tree = bpy.data.materials[name].node_tree
    assert tree is not None, name
    shader = next(node for node in tree.nodes if node.bl_idname == "ShaderNodeBsdfPrincipled")
    return tree, shader


def _diffuse_coverage(shader: bpy.types.Node) -> bpy.types.Image:
    diffuse = _input(shader.inputs["Base Color"])
    assert diffuse.from_socket.name == "Color"
    assert diffuse.from_node.bl_idname == "ShaderNodeTexImage"
    image = diffuse.from_node.image
    _pixels(image, (204, 153, 102, 128))
    assert image.alpha_mode == "STRAIGHT"
    opacity = _input(shader.inputs["Alpha"]).from_node
    assert opacity.bl_idname == "ShaderNodeMath" and opacity.operation == "MULTIPLY"
    alpha = _input(opacity.inputs[0])
    assert alpha.from_node == diffuse.from_node and alpha.from_socket.name == "Alpha"
    assert not opacity.inputs[1].is_linked
    assert math.isclose(opacity.inputs[1].default_value, 0.45, abs_tol=1e-6)
    return image


def _emittance(shader: bpy.types.Node, diffuse: bpy.types.Image) -> None:
    assert shader.inputs["Emission Strength"].default_value == 2.0
    tint = _input(shader.inputs["Emission Color"]).from_node
    assert tint.bl_idname == "ShaderNodeMix" and tint.blend_type == "MULTIPLY"
    assert not tint.clamp_result
    operand = next(socket for socket in tint.inputs if socket.name == "B" and socket.enabled)
    _close(operand.default_value[:3], TINT)
    assert next(socket for socket in tint.inputs if socket.name == "Factor" and socket.enabled).default_value == 1.0
    sample = _input(next(socket for socket in tint.inputs if socket.name == "A" and socket.enabled))
    assert sample.from_socket.name == "Color" and sample.from_node.bl_idname == "ShaderNodeTexImage"
    image = sample.from_node.image
    _pixels(image, (128, 64, 32, 255))
    assert image != diffuse and image.colorspace_settings.name == "Linear Rec.709"


def _input(socket: bpy.types.NodeSocket) -> bpy.types.NodeLink:
    link, = socket.links
    return link


def _pixels(image: bpy.types.Image | None, channels: tuple[int, int, int, int]) -> None:
    assert image is not None
    _close(tuple(image.pixels), tuple(channel / 255.0 for channel in channels))
    assert image.has_data and tuple(image.size) == (1, 1)


def _close(actual: tuple[float, ...], expected: tuple[float, ...]) -> None:
    assert all(math.isclose(found, want, abs_tol=1e-6)
               for found, want in zip(actual, expected, strict=True)), (actual, expected)


if __name__ == "__main__":
    verify(Path(sys.argv[-1]))
    print(f"Blender {bpy.app.version_string} verified Forward Illum wiring and rendered explicit Emittance.")
