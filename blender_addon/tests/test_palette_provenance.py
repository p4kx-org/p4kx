# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Palette proofs do not change scene data and reject invalid references."""

from pathlib import Path

import pytest

from p4kx_importer import package
from p4kx_importer.manifest_fields import PackageError
from synthetic import manifest


def _origin(*refs: str) -> dict:
    return {
        "containsEncryptedSource": True if refs else None,
        "sourceSetComplete": bool(refs),
        "sourceUsesRefs": list(refs),
    }


def _example() -> dict:
    data = manifest(version="1.20")
    data["palettes"] = [{
        "id": "p0001", "name": "synthetic",
        **dict.fromkeys(
            ("primary", "secondary", "tertiary"),
            {"tint": [1.0, 0.0, 0.5], "specular": [0.2, 0.0, 0.3], "glossiness": 127.0},
        ),
        "glass": [0.0, 1.0, 0.5],
    }]
    data["meshes"][0]["palette_id"] = "p0001"
    data["palette_provenance"] = {
        "sourceInventory": {
            "sources": [{"id": "s0", "ancestors": [], "storedEncryption": "encrypted"}],
            "uses": [{
                "id": "u0", "source": "s0", "scope": "payload", "role": "conversion",
                "effectiveEncryption": "encrypted",
                "location": {"type": "dataForge", "recordIndex": 4, "field": "root.glassColor.r"},
            }],
        },
        "payloads": [{"palette": 0, "fieldOrigins": {"/glass/0": _origin("u0")}}],
        "selections": [{"kind": "socEntity", "selector": _origin()}],
        "bindings": [{"node": 1, "payload": 0, "selection": 0}],
    }
    return data


def test_palette_inheritance_resolves_a_drawn_ancestor_through_a_meshless_node() -> None:
    data = _example()
    data["nodes"][2]["parent"] = 1
    del data["nodes"][2]["mesh"]
    data["nodes"][3].update(parent=2, mesh="m0001")
    data["palette_provenance"]["bindings"].append({
        "node": 3, "payload": 0, "selection": 0, "inheritedFrom": 1,
    })
    package.parse_manifest(data, Path("."))
    data["nodes"][3]["parent"] = 0
    with pytest.raises(PackageError, match="not an ancestor"):
        package.parse_manifest(data, Path("."))


def test_palette_provenance_leaves_the_rendered_scene_unchanged() -> None:
    data = _example()
    package_with_proof = package.parse_manifest(data, Path("."))
    del data["palette_provenance"]
    assert package_with_proof == package.parse_manifest(data, Path("."))


@pytest.mark.parametrize(
    ("path", "value", "message"),
    [
        (("payloads", 0, "palette"), 1, "palette reference is outside"),
        (("payloads", 0, "fieldOrigins"), {"/glass/3": _origin("u0")}, "field path is unresolved"),
        (("payloads", 0, "fieldOrigins", "/glass/0"), _origin("missing"), "unresolved contributor"),
        (("sourceInventory", "uses", 0, "effectiveEncryption"), "unencrypted", "marker differs"),
        (("bindings", 0, "node"), True, "node reference is outside"),
        (("bindings", 0, "selection"), 2, "selection reference is outside"),
        (("bindings", 0, "inheritedFrom"), 0, "does not match the supplying node"),
        (("bindings", 0, "inheritedFrom"), 1, "inherited node reference is outside"),
    ],
)
def test_palette_provenance_rejects_broken_links(path: tuple, value: object, message: str) -> None:
    data = _example()
    row = data["palette_provenance"]
    for component in path[:-1]:
        row = row[component]
    row[path[-1]] = value
    with pytest.raises(PackageError, match=message):
        package.parse_manifest(data, Path("."))
