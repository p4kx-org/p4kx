# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates the opt-in versioned helper artifact without opening Blender."""

from __future__ import annotations

import json
import math
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .manifest_fields import PackageError, read_array, read_object

FILE = "collision-helpers.json"
MINOR = 18
VERSION = 1

Vector3 = tuple[float, float, float]
Triangle = tuple[int, int, int]
MatrixRows = tuple[tuple[float, ...], ...]


@dataclass(frozen=True, slots=True)
class Mesh:
    """One validated source collider and its frame relative to the model."""

    positions: tuple[Vector3, ...]
    indices: tuple[Triangle, ...]
    frame: MatrixRows
    source: str
    chunk: int
    slot: int
    packed_key: int
    encrypted: bool | None


@dataclass(frozen=True, slots=True)
class Instance:
    """One model owner in the ordinary package node hierarchy."""

    node: int
    model: int
    cdf_resolved: bool
    cdf_encrypted: bool | None = False


@dataclass(frozen=True, slots=True)
class Helpers:
    """Checked geometry definitions and their owning occurrences."""

    models: tuple[tuple[Mesh, ...], ...]
    instances: tuple[Instance, ...]


def parse_ref(value: object, minor: int) -> str | None:
    """Returns the fixed filename when it is admitted by this manifest version."""
    if value is None:
        return None
    reference = read_object(value, "collision_helpers")
    version = reference.get("schema_version")
    if minor < MINOR or set(reference) != {"file", "schema_version"} or (
        reference.get("file") != FILE or type(version) is not int or version != VERSION
    ):
        raise PackageError("The package collision helper reference is invalid.")
    return FILE


def path_of(root: Path) -> Path:
    """Returns the exact plain-file path of the fixed companion."""
    try:
        names = [entry.name for entry in root.iterdir() if entry.name.casefold() == FILE]
        path = root / FILE
        info = path.lstat()
        redirected = getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
        if names != [FILE] or not stat.S_ISREG(info.st_mode) or redirected:
            raise PackageError("The package collision helper must be an exact plain file.")
        return path
    except OSError as error:
        raise PackageError("The package collision helper cannot be read.") from error


def load(root: Path, nodes: set[int]) -> Helpers:
    """Checks the entire helper geometry and its node references before use."""
    try:
        with path_of(root).open(encoding="utf-8") as stream:
            return parse(json.load(stream), nodes)
    except (OSError, RecursionError, ValueError) as error:
        raise PackageError("The package collision helper document cannot be decoded.") from error


def parse(value: object, nodes: set[int]) -> Helpers:
    """Checks geometry, physical ordinals and instance ownership."""
    data = read_object(value, "collision helpers")
    version = data.get("schema_version")
    if type(version) is not int or version != VERSION:
        raise PackageError("The package collision helper schema version is invalid.")
    models: list[tuple[Mesh, ...]] = []
    sources: set[str] = set()
    for raw in read_array(data.get("models"), "collision models"):
        model = read_object(raw, "collision model")
        source = model.get("source")
        if not isinstance(source, str) or not source or source in sources:
            raise PackageError("A collision model has an invalid physical source.")
        sources.add(source)
        models.append(_meshes(model, source))
    instances = tuple(_instance(raw, nodes, len(models)) for raw in read_array(data.get("instances"), "collision instances"))
    if len({(item.node, item.model) for item in instances}) != len(instances):
        raise PackageError("A collision instance is repeated.")
    return Helpers(tuple(models), instances)


def _instance(raw: object, nodes: set[int], count: int) -> Instance:
    value = read_object(raw, "collision instance")
    node, model = _natural(value.get("node")), _natural(value.get("model"))
    if node not in nodes or model >= count:
        raise PackageError("A collision instance names an unknown node or model.")
    cdf = value.get("cdf")
    if cdf is None:
        return Instance(node, model, True)
    declaration = read_object(cdf, "CDF collision placement")
    frame = _sourced(declaration.get("owner_from_attachment"))
    if frame.get("state") == "present":
        _matrix(frame["value"])
        return Instance(node, model, True, _origin(frame))
    return Instance(node, model, False, _origin(frame))


def _meshes(model: dict[str, Any], source: str) -> tuple[Mesh, ...]:
    emitted: list[Mesh] = []
    seen: set[tuple[int, int]] = set()
    for raw in read_array(model.get("slots"), "collision slots"):
        row = read_object(raw, "collision slot")
        chunk, slot = _natural(row.get("chunk")), _natural(row.get("slot"))
        if (chunk, slot) in seen:
            raise PackageError("A collision slot repeats its physical coordinate.")
        seen.add((chunk, slot))
        key = _natural(_present(row.get("packed_key")), maximum=0xFFFFFFFF)
        geometry, frame = _sourced(row.get("geometry")), _sourced(row.get("frame"))
        positions, faces = _geometry(geometry)
        matrix = _matrix(frame["value"]) if frame["state"] == "present" else None
        if row.get("disposition") == "ready":
            if matrix is None or not faces or key & 0xFFFF == 0xFFFF:
                raise PackageError("A ready collision helper lacks checked geometry or frame.")
            emitted.append(Mesh(positions, faces, matrix, source, chunk, slot, key, _origin(geometry, frame)))
    return tuple(emitted)


def _geometry(cell: dict[str, Any]) -> tuple[tuple[Vector3, ...], tuple[Triangle, ...]]:
    if cell["state"] != "present":
        return (), ()
    value = read_object(cell["value"], "collision geometry")
    vertices = tuple(_numbers(vertex, 3) for vertex in read_array(value.get("positions_z_up_metres"), "collision vertices"))
    faces: list[Triangle] = []
    for raw in read_array(value.get("indices"), "collision faces"):
        if not isinstance(raw, list) or len(raw) != 3:
            raise PackageError("A collision face must have three source indices.")
        face = tuple(_natural(index, maximum=min(0xFFFF, len(vertices) - 1)) for index in raw)
        faces.append(face)
    return vertices, tuple(faces)


def _matrix(value: object) -> MatrixRows:
    if not isinstance(value, list) or len(value) != 3:
        raise PackageError("A collision affine needs three rows.")
    return tuple(_numbers(row, 4) for row in value)


def _numbers(value: object, count: int) -> tuple[float, ...]:
    if not isinstance(value, list) or len(value) != count:
        raise PackageError("A collision coordinate has the wrong shape.")
    if any(type(part) not in {int, float} or not -3.4028236e38 <= part <= 3.4028236e38 or not math.isfinite(part) for part in value):
        raise PackageError("A collision coordinate is outside the finite binary32 domain.")
    return tuple(map(float, value))


def _natural(value: object, maximum: int | None = None) -> int:
    if type(value) is not int or value < 0 or (maximum is not None and value > maximum):
        raise PackageError("A collision ordinal or key is invalid.")
    return value


def _sourced(value: object) -> dict[str, Any]:
    cell = read_object(value, "sourced collision value")
    state = cell.get("state")
    marker = cell.get("containsEncryptedSource")
    if (state != "present" and state not in ("absent", "null", "invalid", "unsupported", "unreadable")) or (
        (state == "present") != (cell.get("value") is not None) or (marker is not None and type(marker) is not bool)
    ):
        raise PackageError("A sourced collision value has inconsistent state or origin.")
    return cell


def _present(value: object) -> Any:
    cell = _sourced(value)
    if cell["state"] != "present":
        raise PackageError("A collision slot has no physical key.")
    return cell["value"]


def _origin(*cells: dict[str, Any]) -> bool | None:
    values = [cell.get("containsEncryptedSource") for cell in cells]
    return True if True in values else (None if None in values else False)
