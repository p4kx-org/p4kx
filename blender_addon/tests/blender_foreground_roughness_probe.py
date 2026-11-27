# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Measure imported roughness on uniquely tagged foreground pixels."""

from __future__ import annotations

from dataclasses import dataclass
import math
from pathlib import Path

import bpy
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Vector

from blender_signal_probe import configure_linear_signal_render, principled, render_configured_linear_signals

_SIZE = 128
_CAMERA_SCALE = 2.0
_PLANE_SIZE = 0.24
_CONTROL_TOLERANCE = 0.0002
_CONTROL_GREEN = (0.125, 0.25, 0.625, 0.875)
_BACKGROUND_PIXELS = ((8, 8), (120, 120))
_PROBE_COLUMNS = (20, 64, 108)
_PROBE_ROWS = (16, 48, 80, 112)
_TAG_LEVELS = 16.0


@dataclass(frozen=True)
class RoughnessReadback:
    """Holds the measured scalar pairs and the verified pixel counts."""

    pairs: tuple[tuple[float, float], ...]
    foreground_pixels: int
    scalar_controls: int
    background_pixels: int


@dataclass(frozen=True)
class _Probe:
    pixel: tuple[int, int]
    red_tag: float
    control_green: float | None


def _scene(target: Path) -> tuple[bpy.types.Scene, bpy.types.Object]:
    scene = bpy.context.scene
    for item in scene.objects:
        item.hide_render = True
    bpy.ops.object.camera_add(location=(0.0, 0.0, 2.0), rotation=(0.0, 0.0, 0.0))
    camera = bpy.context.object
    camera.data.type = "ORTHO"
    camera.data.sensor_fit = "HORIZONTAL"
    camera.data.ortho_scale = _CAMERA_SCALE
    scene.camera = camera
    configure_linear_signal_render(scene, target, _SIZE, _SIZE, samples=4)
    scene.render.pixel_aspect_x = scene.render.pixel_aspect_y = 1.0
    scene.render.film_transparent = True
    return scene, camera


def _material(source: bpy.types.Material | None, probe: _Probe) -> bpy.types.Material:
    material = source.copy() if source is not None else bpy.data.materials.new("Linear scalar control")
    material.use_nodes = True
    tree = material.node_tree
    combine = tree.nodes.new("ShaderNodeCombineColor")
    combine.mode = "RGB"
    combine.inputs[0].default_value = probe.red_tag
    combine.inputs[2].default_value = 1.0 - probe.red_tag
    if source is None:
        assert probe.control_green is not None, probe
        combine.inputs[1].default_value = probe.control_green
    else:
        roughness = principled(material).inputs["Roughness"]
        if roughness.is_linked:
            tree.links.new(roughness.links[0].from_socket, combine.inputs[1])
        else:
            combine.inputs[1].default_value = roughness.default_value
    emission = tree.nodes.new("ShaderNodeEmission")
    output = next(node for node in tree.nodes if node.type == "OUTPUT_MATERIAL" and node.is_active_output)
    tree.links.new(combine.outputs[0], emission.inputs["Color"])
    tree.links.new(emission.outputs[0], output.inputs["Surface"])
    return material


def _place(
    scene: bpy.types.Scene, camera: bpy.types.Object, material: bpy.types.Material,
    pixel: tuple[int, int], uv: tuple[float, float],
) -> None:
    x, y = (_CAMERA_SCALE * ((coord + 0.5) / _SIZE - 0.5) for coord in pixel)
    bpy.ops.mesh.primitive_plane_add(size=_PLANE_SIZE, location=(x, y, 0.0))
    plane = bpy.context.object
    plane.data.materials.append(material)
    for loop in plane.data.uv_layers.active.data:
        loop.uv = uv
    bpy.context.view_layer.update()
    point = plane.matrix_world @ Vector((0.0, 0.0, 0.0))
    projected = world_to_camera_view(scene, camera, point)
    assert projected.z > 0, (pixel, tuple(projected))
    assert all(abs(coord * _SIZE - intended - 0.5) < 0.001
               for coord, intended in zip(projected[:2], pixel, strict=True)), (pixel, tuple(projected))


def _measure(pixels: tuple[float, ...], probes: tuple[_Probe, ...]) -> RoughnessReadback:
    def rgba(pixel: tuple[int, int]) -> tuple[float, ...]:
        x, y = pixel
        offset = (y * _SIZE + x) * 4
        return pixels[offset:offset + 4]

    for position in _BACKGROUND_PIXELS:
        sample = rgba(position)
        assert all(abs(value) < _CONTROL_TOLERANCE for value in sample), (position, sample)

    scalar_values = []
    scalar_controls = 0
    for probe in probes:
        red, green, blue, alpha = rgba(probe.pixel)
        observed = (red, blue, alpha)
        expected = (probe.red_tag, 1.0 - probe.red_tag, 1.0)
        assert all(math.isfinite(value) for value in (*observed, green)), (probe, rgba(probe.pixel))
        assert all(abs(value - reference) <= _CONTROL_TOLERANCE
                   for value, reference in zip(observed, expected, strict=True)), (probe, observed)
        assert -_CONTROL_TOLERANCE <= green <= 1.0 + _CONTROL_TOLERANCE, (probe, green)
        if probe.control_green is None:
            scalar_values.append(green)
        else:
            assert abs(green - probe.control_green) <= _CONTROL_TOLERANCE, (probe, green)
            scalar_controls += 1
    pairs = tuple(zip(scalar_values[::2], scalar_values[1::2], strict=True))
    return RoughnessReadback(pairs, len(probes), scalar_controls, len(_BACKGROUND_PIXELS))


def render_roughness_pairs(
    first: bpy.types.Material,
    second: bpy.types.Material,
    uvs: tuple[tuple[float, float], ...],
    target: Path,
) -> RoughnessReadback:
    """Samples each imported pair and a known linear scalar at the original UVs."""
    scene, camera = _scene(target)
    probes = []
    columns = tuple(zip(_PROBE_COLUMNS, (first, second, None), strict=True))
    for row, uv, literal in zip(_PROBE_ROWS, uvs, _CONTROL_GREEN, strict=True):
        for column, source in columns:
            probe = _Probe((column, row), (len(probes) + 1) / _TAG_LEVELS,
                           literal if source is None else None)
            _place(scene, camera, _material(source, probe), probe.pixel, uv)
            probes.append(probe)
    return _measure(render_configured_linear_signals(scene), tuple(probes))
