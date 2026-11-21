# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Derives writer-comparable triangle and vertex counts from GLB accessors."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class GeometryCounts:
    """Counts over emitted mesh definitions and their triangle primitives."""

    vertices: int
    triangles: int


def geometry_counts(document: dict[str, Any]) -> GeometryCounts | None:
    """Returns counts for triangle meshes whose attribute and index references are checked."""
    accessors = document.get("accessors", [])
    vertices = triangles = 0
    for mesh in document.get("meshes", []):
        positions: set[int] = set()
        for primitive in mesh.get("primitives", []):
            if primitive.get("mode", 4) != 4:
                return None
            position = primitive["attributes"].get("POSITION")
            if position is None:
                return None
            positions.add(position)
            indices = primitive.get("indices", position)
            triangles += accessors[indices]["count"] // 3
        vertices += sum(accessors[index]["count"] for index in positions)
    return GeometryCounts(vertices, triangles)
