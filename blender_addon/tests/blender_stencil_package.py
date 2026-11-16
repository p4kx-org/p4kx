# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the linear colour and coverage of a source-built stencil in Blender."""

from __future__ import annotations

import json
import math
import os
import sys
from pathlib import Path

import bpy

ADDON = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ADDON))

from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import load_package  # noqa: E402
from p4kx_importer.package_digests import file_sha256  # noqa: E402

PIXELS_PER_CELL = 16
WIDTH = 8 * PIXELS_PER_CELL
HEIGHT = 48
TOLERANCE = 0.012
PAINTED_COLOURS = (
    (1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 1.0),
    (0.0, 0.0, 1.0),
    (127 / 255, 0.0, 128 / 255),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 0.0),
    (1.0, 0.0, 0.0),
)
PAINTED_COVERAGE = (1.0, 1.0, 1.0, 1.0, 1.0, 64 / 255, 0.0, 128 / 255)
FALLBACK_COLOURS = ((0.25, 0.5, 0.75),) * 8
FALLBACK_COVERAGE = (1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 128 / 255)


def _hashes(root: Path, pattern: str) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): file_sha256(path)
        for path in sorted(root.rglob(pattern))
        if path.is_file()
    }


def _principled(material: bpy.types.Material) -> bpy.types.Node:
    tree = material.node_tree
    nodes = tuple(node for node in tree.nodes if node.bl_idname == "ShaderNodeBsdfPrincipled")
    assert len(nodes) == 1, material.name
    return nodes[0]


def _add_probe(material: bpy.types.Material, signal: str, y: float) -> None:
    # Route the imported socket to emission for measurement without lighting or alpha compositing.
    probe = material.copy()
    tree = probe.node_tree
    channel = _principled(probe).inputs[signal]
    output = next(node for node in tree.nodes if node.type == "OUTPUT_MATERIAL" and node.is_active_output)
    emission = tree.nodes.new("ShaderNodeEmission")
    if channel.is_linked:
        tree.links.new(channel.links[0].from_socket, emission.inputs["Color"])
    else:
        assert signal == "Base Color", "the imported coverage must come from the mask"
        emission.inputs["Color"].default_value = channel.default_value
    tree.links.new(emission.outputs[0], output.inputs["Surface"])

    bpy.ops.mesh.primitive_plane_add(size=2, location=(0.0, y, 0.0))
    plane = bpy.context.object
    plane.scale.y = 0.10
    plane.data.materials.append(probe)


def _fixed_raster(material: bpy.types.Material, target: Path) -> tuple[float, ...]:
    for item in bpy.context.scene.objects:
        item.hide_render = True
    _add_probe(material, "Base Color", 0.1875)
    _add_probe(material, "Alpha", -0.1875)
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
    scene.render.resolution_x = WIDTH
    scene.render.resolution_y = HEIGHT
    scene.render.resolution_percentage = 100
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "OPEN_EXR"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "32"
    scene.render.filepath = str(target)
    bpy.ops.render.render(write_still=True)
    image = bpy.data.images.load(scene.render.filepath, check_existing=False)
    assert tuple(image.size) == (WIDTH, HEIGHT), tuple(image.size)
    return tuple(image.pixels)


def _rgb(pixels: tuple[float, ...], cell: int, row: int) -> tuple[float, ...]:
    start = (row * WIDTH + cell * PIXELS_PER_CELL + PIXELS_PER_CELL // 2) * 4
    return pixels[start : start + 3]


def _verify_case(root: Path, painted: bool) -> dict[str, object]:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    package = load_package(root / "scene.json")
    result = import_package(bpy.context, package)
    assert result.meshes == 1 and not result.warnings and not package.warnings, (
        result.meshes, result.warnings, package.warnings
    )
    material = bpy.data.materials["TestStencil"]
    shader = _principled(material)
    assert shader.inputs["Alpha"].is_linked, "coverage comes from the imported stencil"
    assert shader.inputs["Base Color"].is_linked == painted, "only the palette branch samples colour"
    image_nodes = [
        node for node in material.node_tree.nodes
        if node.bl_idname == "ShaderNodeTexImage" and node.image is not None
    ]
    assert image_nodes and len({node.image for node in image_nodes}) == 1
    image = image_nodes[0].image
    assert tuple(image.size) == (32, 4), tuple(image.size)
    assert image.colorspace_settings.name == "Non-Color", image.colorspace_settings.name

    render = root.parent / f"{root.name}-stencil.exr"
    pixels = _fixed_raster(material, render)
    expected_colours = PAINTED_COLOURS if painted else FALLBACK_COLOURS
    expected_coverage = PAINTED_COVERAGE if painted else FALLBACK_COVERAGE
    facts = []
    for cell, (colour, coverage) in enumerate(zip(expected_colours, expected_coverage, strict=True)):
        actual_colour = _rgb(pixels, cell, 3 * HEIGHT // 4)
        actual_coverage = _rgb(pixels, cell, HEIGHT // 4)
        for expected, actual in ((colour, actual_colour), ((coverage,) * 3, actual_coverage)):
            assert all(math.isclose(a, e, abs_tol=TOLERANCE) for a, e in zip(actual, expected, strict=True)), (
                root.name, cell, actual, expected
            )
        facts.append({"rgb": actual_colour, "coverage": actual_coverage})
    return {
        "meshes": result.meshes,
        "warnings": result.warnings,
        "file_sha256": _hashes(root, "*"),
        "render_sha256": file_sha256(render),
        "samples": facts,
    }


def verify(root: Path) -> None:
    revision = os.environ["P4KX_SOURCE_REVISION"]
    report = {
        "source_revision": revision,
        "blender_version": bpy.app.version_string,
        "blender_build": bpy.app.build_hash.decode("ascii"),
        "importer_sha256": _hashes(ADDON / "p4kx_importer", "*.py"),
        "linear_sample_tolerance": TOLERANCE,
        "painted": _verify_case(root / "painted", painted=True),
        "fallback": _verify_case(root / "fallback", painted=False),
    }
    (root / "stencil-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    verify(Path(sys.argv[sys.argv.index("--") + 1]))
    print(f"Blender {bpy.app.version_string} verified stencil colour and coverage sockets.")
