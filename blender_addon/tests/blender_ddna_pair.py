# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks DDNA sample parity and material wiring in the synthetic writer's two packages."""

from __future__ import annotations

import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.material_graph import BuildContext, build_graph  # noqa: E402
from p4kx_importer.materials import ImageCache, build_tree  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402


def verify(folder: Path) -> None:
    dds, png = (load_package(folder / name) for name in ("dds", "png"))
    assert dds.version == (1, 17)
    normal = next(texture for texture in dds.textures.values() if texture.ddna is not None)
    smoothness = normal.ddna.smoothness
    assert normal.ddna.selected_mip == 1
    assert png.textures[normal.id].alpha == "smoothness"

    dds_cache, png_cache = ImageCache(dds), ImageCache(png)
    normal_image = dds_cache.get(normal.id)
    smoothness_image = dds_cache.get(smoothness)
    png_image = png_cache.get(normal.id)
    for image, mode in ((normal_image, "NONE"), (smoothness_image, "NONE"), (png_image, "CHANNEL_PACKED")):
        assert image is not None, dds.warnings + png.warnings
        assert tuple(image.size) == (3, 2)
        assert image.colorspace_settings.name == "Non-Color"
        assert image.alpha_mode == mode
    for offset in range(0, 24, 4):
        red, green, blue, _ = normal_image.pixels[offset:offset + 4]
        sampled = (red, 1.0 - green, blue, smoothness_image.pixels[offset])
        composed = png_image.pixels[offset:offset + 4]
        for observed, expected in zip(sampled, composed, strict=True):
            assert math.isclose(observed, expected, abs_tol=2.0 / 255.0), (sampled, composed)

    imported = import_package(bpy.context, dds)
    assert imported.meshes > 0, imported.warnings
    assert all("Shader 'ZedPBR'" in warning for warning in imported.warnings), imported.warnings
    trees = [material.node_tree for material in bpy.data.materials if material.node_tree is not None]
    assert any(verify_wiring(tree, normal_image, smoothness_image) for tree in trees)

    direct = next(
        desc for desc in dds.material_descriptions.values()
        if desc.slot("normal") is not None and desc.slot("normal").texture == normal.id
    )
    context = BuildContext(
        dds.material_descriptions, None, {normal.id: "none"},
        loadable=dds_cache.loads, ddna_pairs={normal.id: smoothness},
    )
    material = bpy.data.materials.new("Direct DDNA verification")
    build_tree(material, build_graph(direct, context), dds_cache)
    assert verify_wiring(material.node_tree, normal_image, smoothness_image)
    assert all("Shader 'ZedPBR'" in warning for warning in dds.warnings), dds.warnings
    assert not png.warnings, png.warnings


def verify_wiring(tree, normal_image, smoothness_image) -> bool:
    for normal in tree.nodes:
        if normal.bl_idname != "ShaderNodeTexImage" or normal.image != normal_image:
            continue
        for smooth in tree.nodes:
            if smooth.bl_idname != "ShaderNodeTexImage" or smooth.image != smoothness_image:
                continue
            assert _sampler_mapping(normal) == _sampler_mapping(smooth)
            normal_channels = normal.outputs["Color"].links[0].to_node
            assert normal_channels.bl_idname == "ShaderNodeSeparateColor"
            flip = normal_channels.outputs["Green"].links[0].to_node
            assert flip.operation == "SUBTRACT" and flip.inputs[0].default_value == 1.0
            combine = flip.outputs[0].links[0].to_node
            assert combine.bl_idname == "ShaderNodeCombineColor"
            assert combine.inputs["Red"].links[0].from_socket == normal_channels.outputs["Red"]
            assert combine.inputs["Blue"].links[0].from_socket == normal_channels.outputs["Blue"]
            assert combine.outputs["Color"].links[0].to_node.bl_idname == "ShaderNodeNormalMap"
            smooth_channels = smooth.outputs["Color"].links[0].to_node
            assert smooth_channels.bl_idname == "ShaderNodeSeparateColor"
            roughness = smooth_channels.outputs["Red"].links[0].to_node
            if roughness.operation == "MULTIPLY":
                roughness = roughness.outputs[0].links[0].to_node
            assert roughness.operation == "SUBTRACT" and roughness.inputs[0].default_value == 1.0
            return True
    return False


def _sampler_mapping(image_node):
    vector = image_node.inputs["Vector"]
    if not vector.is_linked:
        return None
    source = vector.links[0].from_node
    if source.bl_idname != "ShaderNodeMapping":
        return source.bl_idname, vector.links[0].from_socket.name
    return tuple(tuple(source.inputs[name].default_value) for name in ("Location", "Scale"))


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
    print(f"Blender {bpy.app.version_string} DDNA pair verified")
