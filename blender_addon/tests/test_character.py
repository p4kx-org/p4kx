# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of the Character part of the package reader: the node kind, the rig and its limits."""

from __future__ import annotations

import copy
from pathlib import Path
from typing import Any

import pytest

from p4kx_importer import package
from p4kx_importer.package import PackageError
from synthetic_character import character_manifest, write_character_package


def parse(data: dict[str, Any], root: Path = Path(".")) -> package.Package:
    return package.parse_manifest(data, root)


def test_reads_a_character_with_its_rig() -> None:
    result = parse(character_manifest())
    assert result.version == (1, 3)
    assert result.warnings == []
    hero = result.nodes[0]
    assert (hero.kind, hero.mesh) == ("character", "m0001")
    rig = result.meshes["m0001"].rig
    assert rig is not None
    assert (rig.body, rig.bones, rig.skinned_meshes, rig.morph_targets) == ("male", 2, 1, 1)
    assert [(part.name, part.origin, part.morph_targets) for part in rig.parts] == [
        ("body", "body", 1)
    ]
    assert [(clip.name, clip.duration_seconds, clip.channels) for clip in rig.animations] == [
        ("wave", 1.0, 2)
    ]
    assert result.meshes["m0002"].rig is None


def test_reads_a_creature_as_an_entity_node_with_a_rig_that_has_no_body() -> None:
    data = character_manifest(version="1.7")
    data["nodes"][0]["kind"] = "entity"
    rig = data["meshes"][0]["rig"]
    rig.pop("body")
    rig["parts"][0]["origin"] = "skin"
    result = parse(data)
    assert result.warnings == []
    assert (result.nodes[0].kind, result.nodes[0].mesh) == ("entity", "m0001")
    creature = result.meshes["m0001"].rig
    assert creature is not None
    assert (creature.body, creature.bones, creature.parts[0].origin) == (None, 2, "skin")


def test_the_add_on_reads_format_1_3() -> None:
    assert package.SUPPORTED_MINOR >= 3
    assert parse(character_manifest(version="1.8")).warnings


def test_a_character_node_without_a_rigged_mesh_is_skipped_with_its_children() -> None:
    data = character_manifest()
    data["meshes"][0].pop("rig")
    result = parse(data)
    assert result.nodes == []
    assert result.warnings == ["Character nodes without a rigged mesh are skipped."]


def test_a_character_node_without_a_mesh_is_skipped() -> None:
    data = character_manifest()
    data["nodes"][0].pop("mesh")
    assert [node.name for node in parse(data).nodes] == []


@pytest.mark.parametrize(
    "change",
    [
        lambda rig: rig.update(bones=-1),
        lambda rig: rig.update(bones="2"),
        lambda rig: rig.update(morph_targets=True),
        lambda rig: rig.update(body=3),
        lambda rig: rig.update(parts={}),
        lambda rig: rig["parts"][0].update(name=3),
        lambda rig: rig["animations"][0].update(duration_seconds=float("inf")),
        lambda rig: rig["animations"][0].update(channels=1.5),
        lambda rig: rig.update(animations=[1]),
        lambda rig: rig.pop("skeleton"),
        lambda rig: rig.update(skeleton=7),
        lambda rig: rig.pop("embedded_textures"),
        lambda rig: rig.update(embedded_textures=-1),
        lambda rig: rig["parts"][0].pop("file"),
        lambda rig: rig["parts"][0].update(file=None),
        lambda rig: rig["animations"][0].pop("additive"),
        lambda rig: rig["animations"][0].update(additive=1),
        lambda rig: rig["animations"][0].pop("keys"),
        lambda rig: rig["animations"][0].update(keys="4"),
    ],
)
def test_a_malformed_rig_is_refused(change: Any) -> None:
    data = character_manifest()
    change(data["meshes"][0]["rig"])
    with pytest.raises(PackageError):
        parse(data)


def test_a_rig_that_is_not_an_object_is_refused() -> None:
    data = character_manifest()
    data["meshes"][0]["rig"] = []
    with pytest.raises(PackageError, match="rig must be an object"):
        parse(data)


def test_a_package_of_1_2_without_rigs_still_reads() -> None:
    data = copy.deepcopy(character_manifest(version="1.2"))
    data["nodes"][0]["kind"] = "entity"
    data["meshes"][0].pop("rig")
    result = parse(data)
    assert result.warnings == []
    assert result.meshes["m0001"].rig is None


def test_the_written_character_package_loads(tmp_path: Path) -> None:
    scene = write_character_package(tmp_path)
    result = package.load_package(scene)
    assert result.entity_name == "male"
    assert (tmp_path / result.meshes["m0001"].file).is_file()
    package.check_glb(tmp_path / result.meshes["m0001"].file)
