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


def _add_probe(material: bpy.types.Material, signal: str, y: float) -> None:
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


def render_linear_signals(
    probes: tuple[tuple[bpy.types.Material, str, float], ...],
    target: Path,
    width: int,
    height: int,
) -> tuple[float, ...]:
    for item in bpy.context.scene.objects:
        item.hide_render = True
    for material, signal, y in probes:
        _add_probe(material, signal, y)
    bpy.ops.object.camera_add(location=(0.0, 0.0, 2.0))
    camera = bpy.context.object
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 2.0
    scene = bpy.context.scene
    scene.camera = camera
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 8
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
    bpy.ops.render.render(write_still=True)
    image = bpy.data.images.load(scene.render.filepath, check_existing=False)
    assert tuple(image.size) == (width, height), tuple(image.size)
    return tuple(image.pixels)
