# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Exercises optional Character body-zone metadata and its references."""

from __future__ import annotations

import json
import struct
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from p4kx_importer.body_zones import UnavailableBodyZones, VerifiedBodyZones
from p4kx_importer.glb_preflight import check_glb
from p4kx_importer.mesh_preflight import preflight_meshes
from p4kx_importer.package import PackageError, load_package, parse_manifest
from p4kx_importer.package_digests import file_sha256
from synthetic_character import character_manifest, rig_glb

_GLB_HEADER = struct.Struct("<4sIII4s")
_CHUNK_HEADER = struct.Struct("<I4s")


@pytest.fixture
def zoned_character() -> dict[str, Any]:
    data = character_manifest(version="1.24")
    data["nodes"] = data["nodes"][:1]
    data["meshes"] = data["meshes"][:1]
    mesh = data["meshes"][0]
    mesh.update(vertices=6, triangles=2)
    rig = mesh["rig"]
    rig.update(skinned_meshes=2, morph_targets=2)
    rig["parts"].append({"name": "vest", "origin": "piece", "file": "vest.skin", "morph_targets": 1})
    rig["body_zones"] = {
        "glb_sha256": "0" * 64,
        "source_inventory": {
            "sources": [{"id": "entry", "ancestors": [], "storedEncryption": "encrypted"}],
            "uses": [{
                "id": "use", "source": "entry", "scope": "payload", "role": "conversion",
                "effectiveEncryption": "encrypted",
                "location": {"type": "dataForge", "recordIndex": 7, "field": "Chunks"},
            }],
        },
        "field_origins": {
            "/parts/0/coverage/covers/0": {
                "sourceUsesRefs": ["use"],
                "sourceSetComplete": False,
                "containsEncryptedSource": True,
            },
        },
        "body": {
            "state": "verified",
            "primitives": [{"zone": "neck_zone", "mesh": 0, "primitive": 0, "model_lod": 2}],
            "omitted": ["torso01_zone"],
        },
        "parts": [{
            "part": 1,
            "meshes": [1],
            "primitives": [{"zone": "torso01_zone", "mesh": 1, "primitive": 0, "model_lod": 2}],
            "coverage": {
                "guid": "01234567-89ab-cdef-0123-456789abcdef",
                "record_index": 7,
                "record_path": "libs/foundry/records/vest.xml",
                "chunks_complete": False,
                "covers": ["torso01_zone"],
                "conditional": ["neck_zone"],
            },
        }],
    }
    return data


def _rig(data: dict[str, Any]) -> dict[str, Any]:
    return data["meshes"][0]["rig"]


def _write_package(folder: Path, data: dict[str, Any]) -> Path:
    raw = rig_glb()
    _, _, _, length, kind = _GLB_HEADER.unpack_from(raw)
    json_end = _GLB_HEADER.size + length
    document = json.loads(raw[_GLB_HEADER.size : json_end])
    binary = raw[json_end:]
    document["meshes"].append(document["meshes"][0])
    document["nodes"].append({"name": "vest", "mesh": 1, "skin": 0})
    document["scenes"][0]["nodes"].append(3)
    payload = json.dumps(document).encode()
    payload += b" " * (-len(payload) % 4)
    glb_header = _GLB_HEADER.pack(b"glTF", 2, _GLB_HEADER.size + len(payload) + len(binary), len(payload), kind)
    path = folder / data["meshes"][0]["file"]
    path.parent.mkdir()
    path.write_bytes(glb_header + payload + binary)
    _rig(data)["body_zones"]["glb_sha256"] = file_sha256(path)
    scene = folder / "scene.json"
    scene.write_text(json.dumps(data), encoding="utf-8")
    return scene


def test_reads_typed_body_zones_and_checks_their_glb(tmp_path: Path, zoned_character: dict[str, Any]) -> None:
    package = load_package(_write_package(tmp_path, zoned_character))
    assert package.warnings == []
    rig = package.meshes["m0001"].rig
    assert rig is not None and rig.body_zones is not None
    body = rig.body_zones.body
    assert isinstance(body, VerifiedBodyZones)
    assert body.state == "verified"
    assert body.omitted == ("torso01_zone",)
    primitive, = body.primitives
    assert (primitive.zone, primitive.mesh, primitive.primitive, primitive.model_lod) == (
        "neck_zone", 0, 0, 2,
    )
    part, = rig.body_zones.parts
    assert (rig.parts[part.part].name, part.meshes) == ("vest", (1,))
    zone, = part.primitives
    assert (zone.zone, zone.mesh, zone.primitive, zone.model_lod) == ("torso01_zone", 1, 0, 2)
    assert part.coverage is not None
    assert part.coverage.guid == "01234567-89ab-cdef-0123-456789abcdef"
    assert part.coverage.record_path == "libs/foundry/records/vest.xml"
    assert part.coverage.record_index == 7
    assert part.coverage.chunks_complete is False
    assert part.coverage.covers == ("torso01_zone",)
    assert part.coverage.conditional == ("neck_zone",)
    assert preflight_meshes(package).problems == ()


def test_rejects_worn_zone_pointing_at_another_parts_mesh(zoned_character: dict[str, Any]) -> None:
    _rig(zoned_character)["body_zones"]["parts"][0]["primitives"][0]["mesh"] = 0
    with pytest.raises(PackageError, match="GLB mesh outside its worn part"):
        parse_manifest(zoned_character, Path("."))


def test_rejects_worn_zone_pointing_outside_its_mesh(
    tmp_path: Path, zoned_character: dict[str, Any],
) -> None:
    _rig(zoned_character)["body_zones"]["parts"][0]["primitives"][0]["primitive"] = 1
    package = load_package(_write_package(tmp_path, zoned_character))
    problems = preflight_meshes(package).problems
    problem, = problems
    assert "out-of-range GLB primitive" in problem.detail


def test_rejects_changed_glb_with_the_same_geometry_counts(
    tmp_path: Path, zoned_character: dict[str, Any],
) -> None:
    package = load_package(_write_package(tmp_path, zoned_character))
    path = tmp_path / zoned_character["meshes"][0]["file"]
    original = path.read_bytes()
    original_info = check_glb(path)
    changed = bytearray(original)
    _, _, _, json_length, _ = _GLB_HEADER.unpack_from(changed)
    position_start = _GLB_HEADER.size + json_length + _CHUNK_HEADER.size
    struct.pack_into("<f", changed, position_start, 0.25)
    assert changed != original and len(changed) == len(original)
    path.write_bytes(changed)
    assert check_glb(path).geometry == original_info.geometry

    problem, = preflight_meshes(package).problems
    assert "Character body zone GLB SHA-256 differs." in problem.detail


def test_retains_worn_meshes_when_body_mapping_and_coverage_are_unavailable(
    tmp_path: Path, zoned_character: dict[str, Any],
) -> None:
    zones = _rig(zoned_character)["body_zones"]
    zones["body"] = {"state": "unavailable", "reason": "body geometry is unavailable"}
    zones["parts"][0]["coverage"] = None
    zones["parts"][0].pop("primitives")
    zones["source_inventory"] = {"sources": [], "uses": []}
    zones["field_origins"] = {}
    package = load_package(_write_package(tmp_path, zoned_character))
    rig = package.meshes["m0001"].rig
    assert rig is not None and rig.body_zones is not None
    assert rig.body_zones.body == UnavailableBodyZones("body geometry is unavailable")
    part, = rig.body_zones.parts
    assert part.meshes == (1,) and part.coverage is None
    assert preflight_meshes(package).problems == ()


def test_old_packages_can_omit_the_map(zoned_character: dict[str, Any]) -> None:
    zoned_character["version"] = "1.23"
    with pytest.raises(PackageError, match="requires format 1.24"):
        parse_manifest(zoned_character, Path("."))
    del _rig(zoned_character)["body_zones"]
    package = parse_manifest(zoned_character, Path("."))
    rig = package.meshes["m0001"].rig
    assert rig is not None and rig.body_zones is None
    assert package.warnings == []


def test_creature_rigs_cannot_carry_a_character_body_map(zoned_character: dict[str, Any]) -> None:
    _rig(zoned_character).pop("body")
    with pytest.raises(PackageError, match="requires a Character body"):
        parse_manifest(zoned_character, Path("."))


@pytest.mark.parametrize(
    ("change", "error"),
    [
        (lambda zones: zones.pop("glb_sha256"), "glb_sha256 must be a lowercase SHA-256 digest"),
        (lambda zones: zones.update(glb_sha256="A" * 64), "glb_sha256 must be a lowercase SHA-256 digest"),
        (lambda zones: zones["body"].update(state="unproven"), "body zone state"),
        (lambda zones: zones.pop("source_inventory"), "inventory must be an object"),
        (lambda zones: zones.pop("field_origins"), "field_origins must be an object"),
        (lambda zones: zones["field_origins"].update({"/parts/01/coverage/covers/9": {}}), "unresolved JSON Pointer"),
        (
            lambda zones: zones["field_origins"]["/parts/0/coverage/covers/0"].update(sourceUsesRefs=["missing"]),
            "unresolved contributor references",
        ),
        (lambda zones: zones["body"].update(state="unavailable", reason=""), "reason must be non-empty"),
        (lambda zones: zones["body"].update(omitted=["neck_zone"]), "omitted zone also has emitted primitives"),
        (lambda zones: zones["body"]["primitives"].extend(zones["body"]["primitives"][:]), "duplicate GLB primitive"),
        (lambda zones: zones["body"]["primitives"][0].update(model_lod=True), "model_lod must be a non-negative integer"),
        (lambda zones: zones["parts"][0].update(part=0), "part must reference a worn rig piece"),
        (lambda zones: zones["parts"][0].update(part=4), "part must reference a worn rig piece"),
        (lambda zones: zones["parts"][0].update(meshes=[]), "at least one GLB mesh"),
        (
            lambda zones: zones["parts"][0].update(meshes=[0], primitives=[]),
            "GLB mesh is assigned more than once",
        ),
        (lambda zones: zones["parts"].extend(zones["parts"][:]), "duplicate worn rig part"),
        (lambda zones: zones["parts"][0]["coverage"].update(guid="not-a-guid"), "DataForge Record GUID"),
        (lambda zones: zones["parts"][0]["coverage"].update(chunks_complete=0), "chunks_complete must be a boolean"),
        (lambda zones: zones["parts"][0]["coverage"].update(conditional=["torso01_zone"]), "both unconditionally and conditionally"),
    ],
)
def test_rejects_malformed_manifest_references(
    zoned_character: dict[str, Any], change: Callable[[dict[str, Any]], Any], error: str,
) -> None:
    change(_rig(zoned_character)["body_zones"])
    with pytest.raises(PackageError, match=error):
        parse_manifest(zoned_character, Path("."))


@pytest.mark.parametrize(
    ("change", "error"),
    [
        (lambda zones: zones["body"]["primitives"][0].update(mesh=2), "out-of-range GLB primitive"),
        (lambda zones: zones["body"]["primitives"][0].update(primitive=1), "out-of-range GLB primitive"),
        (
            lambda zones: zones["parts"][0].update(meshes=[2], primitives=[]),
            "out-of-range GLB mesh",
        ),
    ],
)
def test_rejects_addresses_outside_glb_bounds(
    tmp_path: Path, zoned_character: dict[str, Any], change: Callable[[dict[str, Any]], Any], error: str,
) -> None:
    change(_rig(zoned_character)["body_zones"])
    package = load_package(_write_package(tmp_path, zoned_character))
    problem, = preflight_meshes(package).problems
    assert error in problem.detail
