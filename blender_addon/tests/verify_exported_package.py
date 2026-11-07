# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies synthetic packages produced by the Rust exporter in an isolated Blender scene."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import addon_utils
import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from p4kx_importer import importer, package  # noqa: E402


def require(condition: bool, message: str) -> None:
    """Stops verification when a required consumer invariant fails."""
    if not condition:
        raise RuntimeError(message)


def close_vector(actual: Vector, expected: tuple[float, float, float]) -> bool:
    """Returns whether a vector agrees with its authored fixture position."""
    return (actual - Vector(expected)).length < 1e-5


def scene_node(root: bpy.types.Collection, name: str) -> bpy.types.Object:
    """Finds one authored node among the direct scene objects, excluding mesh templates."""
    matches = [
        obj for obj in root.objects
        if obj.name == name or (
            obj.name.startswith(name + ".") and obj.name[len(name) + 1:].isdigit()
        )
    ]
    require(len(matches) == 1, f"authored scene node is absent or ambiguous: {name}")
    return matches[0]


def verify_assembly(root: bpy.types.Collection) -> None:
    """Checks authored Z-up attachment positions after both format conversions."""
    positions = {
        "TEST_Ship": (0.0, 0.0, 0.0),
        "hardpoint_weapon_nose/TEST_Mount": (0.0, 4.0, 1.0),
        "hardpoint_class_2/TEST_Gun": (0.0, 4.0, 1.5),
        "hardpoint_relay/TEST_Relay": (3.0, 0.0, 0.0),
    }
    for name, expected in positions.items():
        obj = scene_node(root, name)
        require(close_vector(obj.matrix_world.translation, expected), f"attachment world position differs: {name}")
        if name in {"hardpoint_weapon_nose/TEST_Mount", "hardpoint_class_2/TEST_Gun"}:
            direction = obj.matrix_world.to_3x3() @ Vector((1.0, 0.0, 0.0))
            require(close_vector(direction, (0.0, 1.0, 0.0)), "authored quarter-turn was lost")
    require(scene_node(root, "hardpoint_class_2/TEST_Gun").parent is scene_node(root, "hardpoint_weapon_nose/TEST_Mount"), "gun hierarchy differs from the authored loadout")


def colour_constants(socket, visited: set[int]) -> set[tuple[float, float, float]]:
    """Returns colour constants contributing to a material input."""
    if not socket.is_linked:
        if socket.type == "RGBA":
            return {tuple(round(value, 5) for value in socket.default_value[:3])}
        return set()
    colours = set()
    for link in socket.links:
        node = link.from_node
        identity = node.as_pointer()
        if identity in visited:
            continue
        visited.add(identity)
        for incoming in (item for item in node.inputs if item.enabled):
            colours.update(colour_constants(incoming, visited))
    return colours


def verify_palette(root: bpy.types.Collection, name: str, expected: dict[str, tuple[float, float, float]]) -> None:
    """Checks authored palette constants in the connected shader graphs of one instance."""
    instance = scene_node(root, name)
    require(instance is not None and instance.instance_collection is not None, "a palette instance is absent")
    found = set()
    for obj in instance.instance_collection.all_objects:
        if obj.type != "MESH":
            continue
        for material in obj.data.materials:
            role = material.name.split(".")[0]
            if role not in expected:
                continue
            surfaces = [node for node in material.node_tree.nodes if node.bl_idname == "ShaderNodeBsdfPrincipled"]
            require(len(surfaces) == 1, "a fixture material must have one surface")
            colours = colour_constants(surfaces[0].inputs["Base Color"], set())
            require(expected[role] in colours, f"connected palette colour differs for {name}/{role}")
            found.add(role)
    require(found == set(expected), "a source palette material role is absent")


def verify_livery(root: bpy.types.Collection) -> None:
    """Checks decoded texture size, shared instances and authored palette channels."""
    images = [image for image in bpy.data.images if image.source == "FILE"]
    require(len(images) == 1, "the livery fixture must load exactly one shared texture")
    require(tuple(images[0].size) == (2, 2), "mip 2 of the authored 8x8 texture must be 2x2")
    verify_palette(root, "TEST_Livery", {"caps": (1.0, 0.0, 0.0), "sides": (0.0, 1.0, 0.0)})
    verify_palette(root, "hardpoint_painted/TEST_Painted", {"caps": (0.0, 1.0, 1.0), "sides": (1.0, 0.0, 1.0)})
    painted = scene_node(root, "hardpoint_painted/TEST_Painted")
    twin = scene_node(root, "hardpoint_twin/TEST_Painted")
    require(painted.instance_collection is twin.instance_collection, "identical source geometry and palette must stay shared")


def geometry_bounds() -> tuple[Vector, Vector]:
    """Returns finite evaluated bounds of visible collection instances."""
    points = []
    graph = bpy.context.evaluated_depsgraph_get()
    for instance in graph.object_instances:
        if instance.object.type != "MESH":
            continue
        points.extend(instance.matrix_world @ Vector(corner) for corner in instance.object.bound_box)
    require(bool(points), "the consumer scene has no evaluated mesh instances")
    require(all(math.isfinite(value) for point in points for value in point), "evaluated bounds are non-finite")
    low = Vector(tuple(min(point[axis] for point in points) for axis in range(3)))
    high = Vector(tuple(max(point[axis] for point in points) for axis in range(3)))
    return low, high


def render(path: Path, low: Vector, high: Vector) -> None:
    """Renders one fixed-angle CPU image of the verified synthetic scene."""
    scene = bpy.context.scene
    centre = (low + high) * 0.5
    radius = max((high - low).length * 0.5, 0.5)
    camera = bpy.data.objects.new("Consumer verification camera", bpy.data.cameras.new("Consumer verification camera"))
    scene.collection.objects.link(camera)
    camera.location = centre + Vector((2.5, -3.0, 2.0)) * radius
    camera.rotation_euler = (centre - camera.location).to_track_quat("-Z", "Y").to_euler()
    camera.data.clip_end = radius * 100
    scene.camera = camera
    light = bpy.data.objects.new("Consumer verification light", bpy.data.lights.new("Consumer verification light", "AREA"))
    scene.collection.objects.link(light)
    light.location = centre + Vector((1.0, -2.0, 3.0)) * radius
    light.data.energy = 800 * radius * radius
    light.data.shape = "DISK"
    light.data.size = radius * 3
    scene.world = bpy.data.worlds.new("Consumer verification world")
    scene.world.color = (0.12, 0.12, 0.12)
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 8
    scene.render.resolution_x = 320
    scene.render.resolution_y = 240
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = str(path)
    scene.view_settings.view_transform = "Standard"
    bpy.ops.render.render(write_still=True)
    require(path.is_file() and path.stat().st_size > 1024, "the fixed-camera render was not written")


def verify(folder: Path, case: str, expected_meshes: int) -> dict[str, object]:
    """Imports an actual exported fixture and verifies its scene and material bindings."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    loaded = package.load_package(folder / "scene.json")
    before = tuple(loaded.warnings)
    result = importer.import_package(bpy.context, loaded)
    bpy.context.view_layer.update()
    require(result.meshes == expected_meshes, "the number of consumed mesh assets differs from the fixture")
    require(result.objects == len(loaded.nodes), "the importer omitted a declared scene node")
    require(result.instances == sum(node.mesh is not None for node in loaded.nodes), "mesh instance count differs")
    require(tuple(result.warnings) == before, "the importer added conversion or material warnings")
    meshes = list(bpy.data.meshes)
    require(bool(meshes), "no actual mesh data was imported")
    for mesh in meshes:
        require(len(mesh.polygons) == 12, "the authored cube must retain twelve triangles")
        require(all(math.isfinite(value) for vertex in mesh.vertices for value in vertex.co), "a mesh contains non-finite vertices")
        require(bool(mesh.materials) and all(material is not None for material in mesh.materials), "a mesh lost its material assignment")
    if case == "assembly":
        verify_assembly(result.root)
    else:
        verify_livery(result.root)
    low, high = geometry_bounds()
    expected_bounds = {
        "assembly": ((-2.0, -2.0, 0.0), (13.2, 14.2, 4.0)),
        "livery": ((8.0, -2.0, 0.0), (12.0, 2.0, 4.0)),
    }
    for actual, expected in zip((low, high), expected_bounds[case]):
        require((actual - Vector(expected)).length < 1e-4, "authored mesh bounds differ")
    render(folder.parent / f"{case}-consumer.png", low, high)
    return {
        "case": case, "objects": result.objects, "instances": result.instances,
        "mesh_assets": result.meshes, "mesh_data": len(meshes), "image_files": sum(image.source == "FILE" for image in bpy.data.images),
        "source_warnings": list(before), "bounds_min": list(low), "bounds_max": list(high),
    }


def main() -> None:
    """Verifies both source-built synthetic packages and writes the local evidence report."""
    arguments = sys.argv[sys.argv.index("--") + 1:]
    require(len(arguments) == 1, "provide the absolute package root")
    root = Path(arguments[0])
    require(root.is_absolute(), "package root must be absolute")
    cases = [verify(root / "assembly", "assembly", 3), verify(root / "livery", "livery", 4)]
    report = {"blender": bpy.app.version_string, "cases": cases}
    (root / "consumer-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Rust-exported synthetic package consumer verification passed")


if __name__ == "__main__":
    main()
