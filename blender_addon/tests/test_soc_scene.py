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


def test_included_objects_refs_are_physical_and_scoped_to_the_stream():
    data = scene()
    stream = data["occurrences"][0]["streams"][0]
    stream["member"]["source"] = "synthetic-source"
    stream["includedObjects"] = [0]
    data["includedObjects"] = [
        {
            "ordinal": 2,
            "source": "synthetic-source",
            "byteLength": "4",
            "raw": {"state": "present", "value": "AQAAAA=="},
            "tables": {"state": "invalid"},
        }
    ]
    assert soc_scene.nodes(data) == soc_scene.nodes(scene())
    for invalid in [[0, 0], [1], [True], [-1]]:
        stream["includedObjects"] = invalid
        with pytest.raises(package.PackageError, match="distinct chunk"):
            soc_scene.nodes(data)
    stream["includedObjects"] = [0]
    chunk = data["includedObjects"][0]
    chunk["raw"]["value"] = "!QAAAA=="
    with pytest.raises(package.PackageError, match="invalid Base64"):
        soc_scene.nodes(data)
    chunk["raw"]["value"] = "AQAAAA=="
    chunk["byteLength"] = "5"
    with pytest.raises(package.PackageError, match="declared length"):
        soc_scene.nodes(data)
    chunk["byteLength"] = "4"
    chunk["source"] = "different-source"
    with pytest.raises(package.PackageError, match="belong to its stream"):
        soc_scene.nodes(data)
