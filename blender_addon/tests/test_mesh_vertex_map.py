# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks artifact, POSITION and source-reference closure of streamed mesh maps."""

from hashlib import sha256
import json

import pytest

from p4kx_importer import mesh_vertex_map
from p4kx_importer.manifest_fields import PackageError
from synthetic import glb


def _unknown():
    return {"containsEncryptedSource": None, "sourceUsesRefs": [], "sourceSetComplete": False}


def _rows(binary):
    return [
        {"type": "header", "format": "p4kx-mesh-vertex-map", "version": 1},
        {"type": "mesh", "mesh": 0, "positionAccessor": 0, "vertices": "3", "mapping": "present", **_unknown()},
        *(
            {"type": "vertex", "mesh": 0, "vertex": index, "sourceVertex": index + 4, **_unknown()}
            for index in range(3)
        ),
        {
            "type": "artifact",
            "glb": "meshes/m0001.glb",
            "artifact": {"byteLength": str(len(binary)), "sha256": sha256(binary).hexdigest()},
        },
        {"type": "end", "meshes": "1", "vertices": "3", "sources": "0", "sourceUses": "0"},
    ]


@pytest.mark.parametrize(
    ("position", "changes", "error"),
    [
        (None, None, None),
        (1, {"positionAccessor": 1}, "POSITION accessor"),
        (2, {"sourceUsesRefs": ["unclosed"]}, "row counts are incomplete"),
        (2, {"vertex": 2}, "POSITION order"),
        (5, {"artifact": {"byteLength": "0", "sha256": "0" * 64}}, "length or digest"),
        (6, {"vertices": "4"}, "row counts are incomplete"),
    ],
)
def test_mesh_map_binds_exact_accessors_and_finished_rows(tmp_path, position, changes, error):
    artifact = tmp_path / "m0001.glb"
    binary = glb("mesh", ["material"])
    artifact.write_bytes(binary)
    rows = _rows(binary)
    if position is not None:
        rows[position].update(changes)
    companion = tmp_path / "m0001.vertex-map.jsonl"
    companion.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
    if error is None:
        mesh_vertex_map.validate(companion, "meshes/m0001.glb", artifact)
    else:
        with pytest.raises(PackageError, match=error):
            mesh_vertex_map.validate(companion, "meshes/m0001.glb", artifact)
