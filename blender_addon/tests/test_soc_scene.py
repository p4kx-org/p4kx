# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Fixtures and checks for the optional authored SOC hierarchy."""

from pathlib import Path


from p4kx_importer import package, soc_scene
import pytest
from synthetic import manifest
from synthetic_soc_scene import scene


def test_authored_hierarchy_retains_scoped_transforms_and_markers():
    nodes = soc_scene.nodes(scene())
    assert [node.kind for node in nodes] == ["container", "room", "door", "area", "segment", "sensor", "container"]
    assert [node.parent for node in nodes] == [None, 0, 0, 0, 3, 0, 0]
    assert [row[3] for row in nodes[1].matrix] == [5, 6, 7]
    assert [row[3] for row in nodes[2].matrix] == [4, 2, 3]
    assert [row[3] for row in nodes[4].matrix] == [9, 8, 7]
    assert nodes[4].origin is True
    assert [row[3] for row in nodes[6].matrix] == [8, 3, 2]


def test_soc_package_reference_only_accepts_fixed_member():
    data = manifest(version="1.13")
    assert package.parse_manifest(data, Path(".")).authored_soc is None
    data["authored_soc"] = {"file": "authored/soc-scene.json", "schema_version": 1}
    assert package.parse_manifest(data, Path(".")).authored_soc == soc_scene.SOC_SCENE_FILE
    for bad in ["../soc-scene.json", "authored/SOC-SCENE.json", "c:/soc-scene.json"]:
        data["authored_soc"]["file"] = bad
        with pytest.raises(package.PackageError, match="supported SOC scene"):
            package.parse_manifest(data, Path("."))


def test_soc_placement_must_refer_to_earlier_occurrence():
    data = scene()
    data["occurrences"][1]["placement"] = [1, 0]
    with pytest.raises(package.PackageError, match="earlier parent"):
        soc_scene.nodes(data)


def test_missing_optional_soc_companion_is_reported(tmp_path):
    with pytest.raises(package.PackageError, match="Cannot read the authored SOC scene"):
        soc_scene.load(tmp_path, soc_scene.SOC_SCENE_FILE)
