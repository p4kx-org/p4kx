# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Writes a small synthetic Character package (format 1.3) for tests."""

from __future__ import annotations

import json
import struct
from pathlib import Path
from typing import Any

import synthetic

IDENTITY_MATRIX = (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)
TIP_BIND = (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, -1, 0, 1)
QUARTER_TURN_Z = (0.0, 0.0, 0.70710678, 0.70710678)
RIG_BASE_COLOUR = (0.1, 0.1, 0.1, 1.0)


def character_manifest(**overrides: Any) -> dict[str, Any]:
    """Returns a format 1.3 manifest: the Character ``Hero`` (mesh m0001) and a prop (mesh m0002).

    m0001 holds two bones ``root`` and ``tip``, a skinned part ``body`` with the morph target
    ``smile``, and the animation ``wave`` that turns ``tip`` and sets the weight of ``smile``.
    """
    base = synthetic.manifest(version="1.3")
    identity = {
        "translation": [0.0, 0.0, 0.0],
        "rotation": [0.0, 0.0, 0.0, 1.0],
        "scale": [1.0, 1.0, 1.0],
    }
    base["source"]["entity"] = {"class_name": "male", "guid": ""}
    base["nodes"] = [
        {"kind": "character", "name": "Hero", "path": "", "mesh": "m0001", **identity},
        {
            "kind": "entity",
            "name": "Prop",
            "path": "Prop",
            "parent": 0,
            "mesh": "m0002",
            **identity,
            "translation": [0.0, 0.0, 2.0],
        },
    ]
    base["meshes"] = [
        {
            "id": "m0001",
            "file": "meshes/m0001.glb",
            "instances": 1,
            "bytes": 0,
            "vertices": 3,
            "triangles": 1,
            "materials": [],
            "rig": {
                "body": "male",
                "skeleton": "Objects/Characters/Human/male_v7/skeleton.chr",
                "bones": 2,
                "skinned_meshes": 1,
                "morph_targets": 1,
                "embedded_textures": 0,
                "parts": [
                    {"name": "body", "origin": "body", "file": "body.skin", "morph_targets": 1}
                ],
                "animations": [
                    {
                        "name": "wave",
                        "duration_seconds": 1.0,
                        "additive": False,
                        "channels": 2,
                        "keys": 4,
                    }
                ],
            },
        },
        synthetic._mesh("m0002", "Box_mat", {}),
    ]
    base["textures"] = []
    base.update(overrides)
    return base


def write_character_package(
    folder: Path,
    data: dict[str, Any] | None = None,
    *,
    base_colour: tuple[float, ...] = RIG_BASE_COLOUR,
) -> Path:
    """Writes ``data`` (default: ``character_manifest()``) and its files; returns ``scene.json``."""
    data = data if data is not None else character_manifest()
    (folder / "meshes").mkdir(parents=True, exist_ok=True)
    (folder / "textures").mkdir(parents=True, exist_ok=True)
    for mesh in data["meshes"]:
        content = (
            rig_glb(base_colour)
            if mesh.get("rig")
            else synthetic.glb(mesh["id"], [m["name"] for m in mesh["materials"]])
        )
        (folder / mesh["file"]).write_bytes(content)
    path = folder / "scene.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def rig_glb(base_colour: tuple[float, ...] = RIG_BASE_COLOUR) -> bytes:
    """Returns a GLB with two bones, a skinned triangle with one morph target and one animation."""
    blocks = [
        struct.pack("<9f", 0, 0, 0, 1, 0, 0, 0, 1, 0),
        bytes([0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]),
        struct.pack("<12f", 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0),
        struct.pack("<9f", 0, 0, 0, 0, 0, 0.5, 0, 0, 0.5),
        struct.pack("<32f", *IDENTITY_MATRIX, *TIP_BIND),
        struct.pack("<2f", 0.0, 1.0),
        struct.pack("<8f", 0, 0, 0, 1, *QUARTER_TURN_Z),
        struct.pack("<2f", 0.0, 1.0),
        struct.pack("<12f", *([1.0, 0.0, 1.0, 1.0] * 3)),
    ]
    binary = b"".join(blocks)
    views, offset = [], 0
    for block in blocks:
        views.append({"buffer": 0, "byteOffset": offset, "byteLength": len(block)})
        offset += len(block)
    vec3 = {"componentType": 5126, "type": "VEC3"}
    document = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0, 1]}],
        "nodes": [
            {"name": "Hero_body", "mesh": 0, "skin": 0},
            {"name": "root", "children": [2]},
            {"name": "tip", "translation": [0.0, 1.0, 0.0]},
        ],
        "meshes": [
            {
                "name": "Hero_mesh",
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2, "COLOR_0": 8},
                        "targets": [{"POSITION": 3}],
                        "material": 0,
                    }
                ],
                "weights": [0.0],
                "extras": {"targetNames": ["smile"]},
            }
        ],
        "materials": [{"name": "armour", "pbrMetallicRoughness": {"baseColorFactor": base_colour}}],
        "skins": [{"joints": [1, 2], "inverseBindMatrices": 4, "skeleton": 1}],
        "animations": [
            {
                "name": "wave",
                "samplers": [
                    {"input": 5, "output": 6, "interpolation": "LINEAR"},
                    {"input": 5, "output": 7, "interpolation": "LINEAR"},
                ],
                "channels": [
                    {"sampler": 0, "target": {"node": 2, "path": "rotation"}},
                    {"sampler": 1, "target": {"node": 0, "path": "weights"}},
                ],
            }
        ],
        "accessors": [
            {"bufferView": 0, "count": 3, "min": [0, 0, 0], "max": [1, 1, 0], **vec3},
            {"bufferView": 1, "componentType": 5121, "count": 3, "type": "VEC4"},
            {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4"},
            {"bufferView": 3, "count": 3, "min": [0, 0, 0], "max": [0, 0, 0.5], **vec3},
            {"bufferView": 4, "componentType": 5126, "count": 2, "type": "MAT4"},
            {
                "bufferView": 5,
                "componentType": 5126,
                "count": 2,
                "type": "SCALAR",
                "min": [0.0],
                "max": [1.0],
            },
            {"bufferView": 6, "componentType": 5126, "count": 2, "type": "VEC4"},
            {"bufferView": 7, "componentType": 5126, "count": 2, "type": "SCALAR"},
            {"bufferView": 8, "componentType": 5126, "count": 3, "type": "VEC4"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": len(binary)}],
    }
    json_chunk = json.dumps(document).encode("utf-8")
    json_chunk += b" " * (-len(json_chunk) % 4)
    total = 12 + 8 + len(json_chunk) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<I4s", len(json_chunk), b"JSON")
        + json_chunk
        + struct.pack("<I4s", len(binary), b"BIN\x00")
        + binary
    )
