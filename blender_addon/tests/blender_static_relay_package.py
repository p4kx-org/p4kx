# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies the native static relay import and a translation-only control."""

import json
import re
import sys
from dataclasses import replace
from hashlib import file_digest
from pathlib import Path

import addon_utils
import bpy
from mathutils import Matrix

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from p4kx_importer import package  # noqa: E402
from p4kx_importer.importer import import_package  # noqa: E402
from p4kx_importer.package import blender_name  # noqa: E402
from p4kx_importer.validate_package import validate_package  # noqa: E402

TOLERANCE = 1e-5
SOURCE_AFFINES = {
    "socket frame": ((1, 0, 0, 7), (0, 0, -1, 11), (0, 1, 0, 13)),
    "slot": ((0, -1, 0, 2), (1, 0, 0, 3), (0, 0, 1, 4)),
}


def require(condition, failure):
    if not condition:
        raise RuntimeError(failure)


def hash_file(path):
    with path.open("rb") as source:
        return file_digest(source, "sha256").hexdigest()


def matrix_error(actual, expected):
    return max(
        abs(value - target)
        for actual_row, target_row in zip(actual, expected, strict=True)
        for value, target in zip(actual_row, target_row, strict=True)
    )


def source_affine(source_file):
    model = json.loads(source_file.read_text(encoding="utf-8"))
    helpers = [row for row in model["nodes"] if row["name"] in SOURCE_AFFINES]
    require(len(helpers) == len(SOURCE_AFFINES), "the original helper count differs")
    rows = {row["name"]: row for row in helpers}
    require(set(rows) == set(SOURCE_AFFINES), "the original helper inventory differs")
    require(rows["socket frame"]["parent"] is None, "the socket frame must be a source root")
    require(rows["slot"]["parent"] == rows["socket frame"]["index"], "wrong authored helper parent")
    require(all(tuple(row["scale"]) == (1, 1, 1) and row["helper"] for row in rows.values()),
            "original helper kinds or unit scale differ")
    for name, expected in SOURCE_AFFINES.items():
        require(tuple(map(tuple, rows[name]["transform"])) == expected, f"{name}: fixture affine differs")
    parent = Matrix((*rows["socket frame"]["transform"], (0, 0, 0, 1)))
    socket = Matrix((*rows["slot"]["transform"], (0, 0, 0, 1)))
    return parent @ socket


def require_original_flag(record_file):
    record = json.loads(record_file.read_text(encoding="utf-8"))
    (port_owner,) = (
        component for component in record["Components"]
        if component is not None and component["$type"] == "SItemPortContainerComponentParams"
    )
    (port,) = port_owner["Ports"]
    require(port["Name"] == "slot", "the fixture Record selects a different port")
    constraint = port["AttachmentImplementation"]["constraintParams"]
    require(constraint["noRotation"] is True, "the fixture must author the true rotation flag")


def package_hashes(location):
    return {
        path.relative_to(location).as_posix(): hash_file(path)
        for path in sorted(location.rglob("*"))
        if path.is_file()
    }


def find_object(result, node):
    name = re.escape(blender_name(node.name))
    pattern = re.compile(rf"{name}(?:\.[0-9]{{3,}})?")
    matches = [obj for obj in result.root.objects if pattern.fullmatch(obj.name)]
    require(len(matches) == 1, f"{node.name}: expected one imported manifest object")
    return matches[0]


def import_placement(loaded, root_node, fixture_node, compare, expected):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    result = import_package(bpy.context, loaded)
    require(not result.warnings, f"{compare}: import warnings: {result.warnings}")
    require(result.objects == 2 and result.armatures == 0, f"{compare}: unexpected fixture objects")
    root = find_object(result, root_node)
    attachment = find_object(result, fixture_node)
    require(root.parent is None and attachment.parent == root, f"{compare}: wrong parent/child relationship")
    require(attachment.instance_type == "COLLECTION" and attachment.instance_collection is not None,
            f"{compare}: the fuse must be a normally imported GLB instance")
    require(any(obj.type == "MESH" and len(obj.data.polygons) for obj in attachment.instance_collection.objects),
            f"{compare}: the fuse must contain drawable imported polygons")
    bpy.context.view_layer.update()
    graph = bpy.context.evaluated_depsgraph_get()
    root_world = root.evaluated_get(graph).matrix_world
    world = attachment.evaluated_get(graph).matrix_world.copy()
    local = root_world.inverted() @ world
    require(matrix_error(root_world, Matrix.Identity(4)) <= TOLERANCE, f"{compare}: fixture root is not identity")
    require(matrix_error(world, expected) <= TOLERANCE, f"{compare}: evaluated world matrix differs")
    require(matrix_error(local, expected) <= TOLERANCE, f"{compare}: evaluated parent-relative matrix differs")
    return world, local, result


def main():
    location, source_file, record_file, archive = map(Path, sys.argv[sys.argv.index("--") + 1:])
    require(bpy.app.version >= (5, 2, 0), "the witness requires Blender 5.2 or newer")
    before = package_hashes(location)
    input_paths = (source_file, record_file, archive)
    source_hashes = {path.name: hash_file(path) for path in input_paths}
    require_original_flag(record_file)
    report = validate_package(location)
    require(report["structure"]["status"] == "valid", f"invalid package structure: {report['structure']}")
    require(report["digests"]["status"] == "verified", f"unverified producer receipt: {report['digests']}")
    require(len(report["digests"]["verified"]) == 3, "fixture must attest the manifest and two GLBs")
    loaded = package.load_package(location)
    require(len(loaded.nodes) == 2 and not loaded.textures, "fixture nodes or texture inventory differ")
    root_node, fixture_node = loaded.nodes
    require(root_node.name == "TEST_Relay" and fixture_node.name == "slot/TEST_Fuse"
            and fixture_node.parent == root_node.index, "fixture manifest hierarchy differs")

    expected = source_affine(source_file)
    actual, local, result = import_placement(loaded, root_node, fixture_node, "ordinary import", expected)

    replacement = replace(fixture_node, rotation=(0, 0, 0, 1))
    negative = replace(loaded, nodes=[root_node, replacement], warnings=[])
    negative_expected = Matrix.Translation(expected.translation)
    control, _, _ = import_placement(negative, root_node, replacement, "translation-only control", negative_expected)
    distance = (actual.translation - control.translation).length
    require(distance <= TOLERANCE, "the negative control changed the position")
    control_error = matrix_error(control, expected)
    require(control_error >= 0.99, "translation-only control did not distinguish the source rotation")

    require(before == package_hashes(location), "the package changed during import")
    require(source_hashes == {path.name: hash_file(path) for path in input_paths},
            "a fixture source input changed during import")
    receipt = {
        "blender": bpy.app.version_string,
        "blenderBuild": bpy.app.build_hash.decode("ascii"),
        "sourceInputsSha256": source_hashes,
        "packageSha256": before,
        "warnings": result.warnings,
        "fixtureMeshImported": True,
        "fullWorldAffine": [list(row) for row in actual],
        "fullWorldMaxError": matrix_error(actual, expected),
        "fullParentRelativeMaxError": matrix_error(local, expected),
        "translationOnlyMaxError": control_error,
        "translationOnlyPositionDifferenceMetres": distance,
    }
    print(json.dumps(receipt, sort_keys=True))


if __name__ == "__main__":
    main()
