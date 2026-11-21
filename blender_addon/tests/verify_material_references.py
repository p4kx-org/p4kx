# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies source-built material-reference packages in an isolated Blender process."""

from __future__ import annotations

import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

import addon_utils
import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import verify_exported_package as shared  # noqa: E402
from p4kx_importer import importer, package  # noqa: E402


def hashes(root: Path) -> dict[str, str]:
    """Hashes the source or package files without following symbolic links."""
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file() and not path.is_symlink() and "__pycache__" not in path.parts
    }


def load_case(folder: Path, count: int):
    """Imports one static fixture with its expected asset and instance counts."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    loaded = package.load_package(folder / "scene.json")
    shared.require(not loaded.warnings, "the synthetic package has source warnings")
    result = importer.import_package(bpy.context, loaded)
    bpy.context.view_layer.update()
    shared.require(not result.warnings, "the importer added a warning")
    shared.require(result.meshes == count and result.instances == count, "mesh or instance count differs")
    shared.require(result.objects == len(loaded.nodes) == count, "a package node is absent")
    shared.require(result.armatures == result.actions == result.shape_keys == 0, "the static fixture gained a rig")
    shared.require(len(bpy.data.meshes) == count, "mesh datablock count differs")
    return loaded, result


def material_triangles(mesh) -> Counter:
    """Counts actual polygon assignments on one imported cube."""
    shared.require(len(mesh.polygons) == 12, "the cube lost triangles")
    counts = Counter()
    for polygon in mesh.polygons:
        material = mesh.materials[polygon.material_index]
        shared.require(material is not None, "a polygon has no material")
        counts[material.name.split(".")[0]] += 1
    return counts


def verify_colour(material, expected: tuple[float, float, float]) -> None:
    """Checks a source colour in the material's connected Principled graph."""
    surfaces = [node for node in material.node_tree.nodes if node.bl_idname == "ShaderNodeBsdfPrincipled"]
    shared.require(len(surfaces) == 1, "expected one Principled surface")
    colours = shared.colour_constants(surfaces[0].inputs["Base Color"], set())
    shared.require(expected in colours, f"wrong source colour for {material.name}")


def input_uses_image(socket, image, seen: set[int]) -> bool:
    """Tests whether a loaded image contributes to a shader input."""
    for link in socket.links:
        node = link.from_node
        identity = node.as_pointer()
        if identity in seen:
            continue
        seen.add(identity)
        if node.type == "TEX_IMAGE" and node.image == image:
            return True
        if any(input_uses_image(item, image, seen) for item in node.inputs if item.enabled):
            return True
    return False


def verify_image(folder: Path, loaded, mesh) -> dict[str, object]:
    """Checks the source texture's package file, decoded pixels and shader binding."""
    shared.require(len(loaded.textures) == 1, "the selected material's texture is missing")
    texture = next(iter(loaded.textures.values()))
    images = [image for image in bpy.data.images if image.source == "FILE"]
    shared.require(len(images) == 1, "the fixture must load one package image")
    image = images[0]
    shared.require(Path(bpy.path.abspath(image.filepath)).resolve() == (folder / texture.file).resolve(),
                   "Blender loaded a different image")
    pixels = tuple(image.pixels[:4])
    shared.require(image.has_data and tuple(image.size) == (8, 8) and len(pixels) == 4,
                   "the authored diffuse mip did not decode")
    for material in mesh.materials:
        surface = next(node for node in material.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
        uses_image = input_uses_image(surface.inputs["Base Color"], image, set())
        shared.require(uses_image == (material.name.split(".")[0] == "caps"), "wrong diffuse texture binding")
    return {"file": texture.file, "dimensions": list(image.size), "sample_rgba": list(pixels)}


def hierarchy(collection) -> list[dict[str, object]]:
    """Records actual parent links and transforms of a collection's objects."""
    return [
        {
            "name": obj.name, "parent": obj.parent.name if obj.parent else None,
            "local_position": list(obj.location),
            "world_position": list(obj.matrix_world.translation),
        }
        for obj in sorted(collection.all_objects, key=lambda item: item.name)
    ]


def finish(folder: Path, case: str, result, bindings, high_bound) -> dict[str, object]:
    """Checks authored bounds, writes a controlled render and retains consumer facts."""
    low, high = shared.geometry_bounds()
    for actual, expected in zip((low, high), ((8.0, -2.0, 0.0), high_bound)):
        shared.require((actual - Vector(expected)).length < 1e-4, "authored bounds differ")
    facts = {
        "case": case, "warnings": list(result.warnings), "hierarchy": hierarchy(result.root),
        "mesh_bindings": bindings, "mesh_assets": result.meshes, "instances": result.instances,
        "armatures": result.armatures, "actions": result.actions, "shape_keys": result.shape_keys,
        "images": sum(image.source == "FILE" for image in bpy.data.images),
        "bounds_min": list(low), "bounds_max": list(high),
        "package_sha256": hashes(folder),
    }
    shared.render(folder.parent / f"{case}-consumer.png", low, high)
    return facts


def verify(folder: Path, case: str) -> dict[str, object]:
    """Checks the original two-material cube fixture."""
    loaded, result = load_case(folder, 1)
    root = shared.scene_node(result.root, "TEST_MaterialReference")
    shared.require(root.parent is None and root.instance_collection is not None, "root hierarchy differs")
    shared.require(shared.close_vector(root.matrix_world.translation, (0.0, 0.0, 0.0)), "root moved")
    mesh = next(iter(bpy.data.meshes))
    assignments = material_triangles(mesh)
    shared.require(assignments == {"caps": 4, "sides": 8}, "wrong polygon-to-material bindings")
    expected = {"caps": (0.8, 0.1, 0.1), "sides": (0.2, 0.4, 0.6)}
    for material in mesh.materials:
        verify_colour(material, expected[material.name.split(".")[0]])
    image = verify_image(folder, loaded, mesh)
    facts = finish(
        folder, case, result,
        [{"material_triangles": dict(assignments), "template_hierarchy": hierarchy(root.instance_collection)}],
        (12.0, 2.0, 4.0),
    )
    facts["material_triangles"] = dict(assignments)
    facts["diffuse_image"] = image
    return facts


def collision_binding(loaded, result, raw, expected) -> dict[str, object]:
    """Checks one independent source, description, GLB and Blender material chain."""
    geometry, node_name, material_name, colour, reference = expected
    matching = [mesh for mesh in raw["meshes"] if mesh.get("geometry") == geometry]
    shared.require(len(matching) == 1, f"source mesh absent: {geometry}")
    mesh = loaded.meshes[matching[0]["id"]]
    shared.require(len(mesh.materials) == 1, "expected one material binding per mesh")
    entry = mesh.materials[0]
    shared.require(entry.index == 0 and entry.name == material_name, "GLB material-zero binding differs")
    source = [item for item in raw["materials"] if item["id"] == entry.material]
    shared.require(len(source) == 1, "description ID absent or ambiguous")
    description = source[0]
    shared.require(description["library"] == "paint" and description["index"] == 0, "package material contract differs")
    shared.require(description["name"] == material_name, "a mesh reused the other description")
    shared.require(tuple(round(value, 5) for value in description["diffuse"]) == colour, "source colour differs")
    source_library = raw["material_libraries"][description["source_library"]]
    shared.require(source_library["path"] == reference, "original source reference differs")
    instance = shared.scene_node(result.root, node_name)
    template = instance.instance_collection
    shared.require(template is not None and template.name == mesh.id, "wrong mesh template instance")
    objects = [obj for obj in template.all_objects if obj.type == "MESH"]
    shared.require(len(objects) == 1, "template mesh absent or ambiguous")
    actual = objects[0].data
    assignments = material_triangles(actual)
    shared.require(assignments == {material_name: 12}, "template uses the other material")
    shared.require(len(actual.materials) == 1, "template material slots differ")
    material = actual.materials[0]
    shared.require(material is not None, "template material absent")
    verify_colour(material, colour)
    return {
        "geometry": geometry, "node": node_name, "mesh": mesh.id, "material_index": entry.index,
        "description_id": entry.material, "source_reference": reference,
        "blender_material": material.name, "material_triangles": dict(assignments),
        "source_colour": list(colour), "template_hierarchy": hierarchy(template),
    }


def verify_collision(folder: Path, case: str, rooted: str) -> dict[str, object]:
    """Checks two distinct material-zero descriptions with equal bare library names."""
    loaded, result = load_case(folder, 2)
    shared.require(not loaded.textures and not any(image.source == "FILE" for image in bpy.data.images),
                   "the untextured fixture loaded an external image")
    raw = json.loads((folder / "scene.json").read_text(encoding="utf-8"))
    shared.require(len(raw["materials"]) == 2 and len(raw["material_libraries"]) == 2, "two source libraries must survive")
    root = shared.scene_node(result.root, "TEST_RootMaterial")
    child = shared.scene_node(result.root, "child/TEST_SiblingMaterial")
    shared.require(root.parent is None and child.parent is root, "authored parent chain differs")
    for obj, expected in [(root, (0.0, 0.0, 0.0)), (child, (6.0, 0.0, 0.0))]:
        shared.require(shared.close_vector(obj.location, expected), "authored local offset differs")
        shared.require(shared.close_vector(obj.matrix_world.translation, expected), "authored world offset differs")
    shared.require(root.instance_collection is not child.instance_collection, "distinct mesh assets collapsed")
    bindings = [
        collision_binding(loaded, result, raw, expected)
        for expected in [
            ("Objects/Test/root.cgf", "TEST_RootMaterial", "root_paint", (0.8, 0.1, 0.1), rooted),
            ("Objects/Test/sibling.cgf", "child/TEST_SiblingMaterial", "sibling_paint", (0.2, 0.4, 0.6), "paint"),
        ]
    ]
    shared.require(len({item["description_id"] for item in bindings}) == 2, "description IDs collapsed")
    shared.require(len({item["blender_material"] for item in bindings}) == 2, "Blender materials collapsed")
    return finish(folder, case, result, bindings, (18.0, 2.0, 4.0))


def main() -> None:
    """Verifies all packages and records Blender, importer and source identities."""
    args = sys.argv[sys.argv.index("--") + 1:]
    shared.require(len(args) == 2, "provide absolute package root and source commit")
    root = Path(args[0])
    shared.require(root.is_absolute() and root.is_dir(), "package root must exist and be absolute")
    shared.require(bpy.app.version[:2] in {(5, 2), (5, 3)}, "verify with Blender 5.2 LTS or 5.3")
    importer_root = (HERE.parent / "p4kx_importer").resolve()
    for module in (importer, package):
        shared.require(Path(module.__file__).resolve().parent == importer_root, "the loaded importer is not the reviewed source")
    report = {
        "blender": bpy.app.version_string,
        "blender_build_hash": bpy.app.build_hash.decode("ascii"), "source_commit": args[1],
        "importer_path": str(importer_root), "importer_sha256": hashes(importer_root),
        "cases": [
            verify(root / "virtual-root", "virtual-root"),
            verify(root / "authoring", "authoring"),
            verify_collision(root / "collision-virtual", "collision-virtual", "/paint.mtl"),
            verify_collision(root / "collision-authoring", "collision-authoring", "//host/branch/Data/paint.mtl"),
        ],
    }
    (root / "material-reference-consumer.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    print("Material-reference package consumer verification passed")


if __name__ == "__main__":
    main()
