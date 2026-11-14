# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the native static emission and material opacity produced by the package importer."""

from __future__ import annotations

import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402


def verify(location: Path) -> None:
    package = load_package(location)
    assert package.version == (1, 18)
    imported = import_package(bpy.context, package)
    assert imported.meshes == 1 and not imported.warnings, imported.warnings
    tree, bsdf = _emission()
    assert math.isclose(bsdf.inputs["Emission Strength"].default_value, 2.0)
    assert math.isclose(bsdf.inputs["Alpha"].default_value, 0.45, abs_tol=1e-6)
    mix = bsdf.inputs["Emission Color"].links[0].from_node
    assert mix.bl_idname == "ShaderNodeMix" and mix.blend_type == "MULTIPLY"
    assert not mix.clamp_result
    operand = next(socket for socket in mix.inputs if socket.name == "B" and socket.enabled)
    assert all(math.isclose(actual, expected, abs_tol=1e-6)
               for actual, expected in zip(operand.default_value[:3], (0.2, 0.4, 0.8), strict=True))
    image_node = next(node for node in tree.nodes if node.bl_idname == "ShaderNodeTexImage")
    assert image_node.image is not None and image_node.image.colorspace_settings.name == "Linear Rec.709"
    _render(tree, bsdf)


def _emission():
    for material in bpy.data.materials:
        if (tree := material.node_tree) is None:
            continue
        for node in tree.nodes:
            if node.bl_idname == "ShaderNodeBsdfPrincipled" and node.inputs["Emission Color"].is_linked:
                return tree, node
    raise AssertionError("No source-linked shader")


def _render(tree, bsdf) -> None:
    for item in bpy.context.scene.objects:
        item.hide_render = True
    for collection in list(bpy.data.collections):
        for item in collection.objects:
            item.hide_render = True
    bpy.ops.mesh.primitive_plane_add(size=4)
    plane = bpy.context.object
    material = next(material for material in bpy.data.materials if material.node_tree == tree)
    plane.data.materials.append(material)
    bpy.ops.object.camera_add(location=(0.0, 0.0, 2.0))
    camera = bpy.context.object
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 2.0
    scene = bpy.context.scene
    scene.camera = camera
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 12
    scene.world = bpy.data.worlds.new("Black")
    scene.world.color = (0.0, 0.0, 0.0)
    scene.render.resolution_x = scene.render.resolution_y = 16
    scene.render.resolution_percentage = 100
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "OPEN_EXR"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.film_transparent = True
    bsdf.inputs["Base Color"].default_value = (0.0, 0.0, 0.0, 1.0)
    opacity = bsdf.inputs["Alpha"]
    opacity.default_value = 1.0
    scene.render.filepath = str(Path(sys.argv[-1]).parent / "opaque-emission.exr")
    bpy.ops.render.render(write_still=True)
    observed = bpy.data.images.load(scene.render.filepath).pixels[(8 * 16 + 8) * 4:(8 * 16 + 9) * 4]
    assert all(math.isclose(actual, expected, abs_tol=0.025)
               for actual, expected in zip(observed, (0.2, 0.2, 0.2, 1.0), strict=True)), tuple(observed)
    opacity.default_value = 0.45
    scene.render.filepath = str(Path(sys.argv[-1]).parent / "covered-emission.exr")
    bpy.ops.render.render(write_still=True)
    covered = bpy.data.images.load(scene.render.filepath).pixels[(8 * 16 + 8) * 4:(8 * 16 + 9) * 4]
    assert 0.2 < covered[3] < 0.7, tuple(covered)


if __name__ == "__main__":
    verify(Path(sys.argv[-1]))
    print(f"Blender {bpy.app.version_string} rendered source-derived emission and coverage.")
