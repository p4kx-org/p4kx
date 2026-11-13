# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Package preflight keeps GLB material tags bound to the actual array slots."""

from pathlib import Path

import pytest

from p4kx_importer import mesh_preflight, package
from p4kx_importer.material_pairing import pair_by_index
from p4kx_importer.validate_package import validate_package
from synthetic import glb, manifest, write_package

_MISSING = object()


@pytest.mark.parametrize(
    ("version", "tag_0", "tag_1", "valid"),
    [
        ("1.19", 0, 1, True),
        ("1.19", 1, 0, False),
        ("1.19", 0, 0, False),
        ("1.19", 4, 1, False),
        ("1.19", None, 1, False),
        ("1.19", False, 1, False),
        ("1.19", 0.0, 1, False),
        ("1.0", _MISSING, _MISSING, True),
        ("1.2", _MISSING, _MISSING, True),
    ],
)
def test_preflight_checks_material_tag_even_when_display_names_repeat(
    tmp_path: Path, version: str, tag_0: object, tag_1: object, valid: bool
) -> None:
    data = manifest(version=version)
    mesh = data["meshes"][0]
    mesh["triangles"] = 2
    mesh["materials"] = [
        {"index": index, "name": "duplicate", "textures": {}}
        for index in range(2)
    ]
    scene = write_package(tmp_path, data)
    path = tmp_path / mesh["file"]
    tags = [
        {"name": "duplicate", **({} if value is _MISSING else {"extras": {"p4kx_index": value}})}
        for value in (tag_0, tag_1)
    ]
    path.write_bytes(glb("synthetic", ["duplicate", "duplicate"], external={"materials": tags}))

    preflight = mesh_preflight.preflight_meshes(package.load_package(scene))
    report = validate_package(scene)
    if valid:
        assert "m0001" in preflight.paths
        assert not preflight.problems
        assert report["structure"]["status"] == "valid"
    else:
        assert "m0001" not in preflight.paths
        assert len(preflight.problems) == 1
        assert "extras.p4kx_index" in preflight.problems[0].detail
        assert report["structure"]["status"] == "failed"


def test_correct_tags_pair_repeated_material_names_in_output_slot_order() -> None:
    manifest_slots = [(0, "duplicate"), (1, "duplicate")]
    blender_materials = [("duplicate", 1), ("duplicate.001", 0)]
    assert pair_by_index(manifest_slots, blender_materials) == {0: "duplicate.001", 1: "duplicate"}
