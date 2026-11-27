# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Render imported material input signals to linear EXR without surface lighting."""

from __future__ import annotations

from pathlib import Path

import bpy


def principled(material: bpy.types.Material) -> bpy.types.Node:
    tree = material.node_tree
    nodes = tuple(node for node in tree.nodes if node.bl_idname == "ShaderNodeBsdfPrincipled")
    assert len(nodes) == 1, material.name
    return nodes[0]


def _add_probe(
    material: bpy.types.Material, signal: str, y: float, uv: tuple[float, float] | None = None
) -> None:
    # Route the imported socket to emission for measurement without lighting or alpha compositing.
    probe = material.copy()
    tree = probe.node_tree
    channel = principled(probe).inputs[signal]
    output = next(node for node in tree.nodes if node.type == "OUTPUT_MATERIAL" and node.is_active_output)
    emission = tree.nodes.new("ShaderNodeEmission")
    if channel.is_linked:
        tree.links.new(channel.links[0].from_socket, emission.inputs["Color"])
    else:
        value = channel.default_value
        emission.inputs["Color"].default_value = (
            (value, value, value, 1.0) if isinstance(value, float) else value
        )
    tree.links.new(emission.outputs[0], output.inputs["Surface"])

    bpy.ops.mesh.primitive_plane_add(size=2, location=(0.0, y, 0.0))
    plane = bpy.context.object
    plane.scale.y = 0.10
    plane.data.materials.append(probe)
    if uv is not None:
        for loop in plane.data.uv_layers.active.data:
            loop.uv = uv


def configure_linear_signal_render(
    scene: bpy.types.Scene, target: Path, width: int, height: int, *, samples: int = 8,
) -> None:
    """Configures a single-threaded linear emission readback."""
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = samples
    scene.cycles.use_adaptive_sampling = False
    scene.cycles.use_denoising = False
    scene.render.threads_mode = "FIXED"
    scene.render.threads = 1
    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.resolution_percentage = 100
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "OPEN_EXR"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "32"
    scene.render.filepath = str(target)


def render_configured_linear_signals(scene: bpy.types.Scene) -> tuple[float, ...]:
    """Renders the configured scene and reads its linear EXR pixels."""
    bpy.ops.render.render(write_still=True)
    image = bpy.data.images.load(scene.render.filepath, check_existing=False)
    resolution = (scene.render.resolution_x, scene.render.resolution_y)
    assert tuple(image.size) == resolution, (tuple(image.size), resolution)
    return tuple(image.pixels)


def render_linear_signals(
    probes: tuple[tuple[bpy.types.Material, str, float], ...],
    target: Path,
    width: int,
    height: int,
    *,
    uvs: tuple[tuple[float, float], ...] | None = None,
) -> tuple[float, ...]:
    for item in bpy.context.scene.objects:
        item.hide_render = True
    assert uvs is None or len(uvs) == len(probes)
    for index, (material, signal, y) in enumerate(probes):
        _add_probe(material, signal, y, None if uvs is None else uvs[index])
    bpy.ops.object.camera_add(location=(0.0, 0.0, 2.0))
    camera = bpy.context.object
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 2.0
    scene = bpy.context.scene
    scene.camera = camera
    configure_linear_signal_render(scene, target, width, height)
    return render_configured_linear_signals(scene)
