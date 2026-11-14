# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Reads the authored SOC package companion and its non-mesh hierarchy."""

from __future__ import annotations

import base64
import binascii
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .manifest_fields import PackageError, read_object
from .package_paths import require_exact_asset_case, resolve_file

SOC_SCENE_FILE = "authored/soc-scene.json"
SOC_SCENE_VERSION = 1
MatrixRows = tuple[tuple[float, ...], ...]


@dataclass(frozen=True, slots=True)
class SocNode:
    """One positioned authored element for the optional Blender hierarchy."""

    label: str
    parent: int | None
    matrix: MatrixRows | None
    origin: bool | None
    kind: str


def parse_ref(value: object) -> str | None:
    """Validates the package reference without following arbitrary paths."""
    if value is None:
        return None
    reference = read_object(value, "authored_soc")
    version = reference.get("schema_version")
    if reference.get("file") != SOC_SCENE_FILE or type(version) is not int or version != SOC_SCENE_VERSION:
        raise PackageError("authored_soc must refer to the supported SOC scene companion.")
    return SOC_SCENE_FILE


def load(root: Path, reference: str) -> list[SocNode]:
    """Reads the companion under the package root and prepares the hierarchy."""
    require_exact_asset_case(root, [reference])
    path = resolve_file(root, reference)
    try:
        with path.open(encoding="utf-8") as source:
            document = json.load(source)
    except (OSError, RecursionError, ValueError) as error:
        raise PackageError(f"Cannot read the authored SOC scene: {error}.") from error
    return nodes(document)


def nodes(value: object) -> list[SocNode]:
    """Returns container, room, segment, door and sensor metadata nodes."""
    document = read_object(value, "SOC scene")
    if document.get("format") != "p4kx-soc-scene" or type(document.get("schemaVersion")) is not int or document["schemaVersion"] != SOC_SCENE_VERSION:
        raise PackageError("The authored SOC scene has an unsupported format or schema version.")
    occurrences = _array(document.get("occurrences"), "SOC occurrences")
    _validate_included_objects(document, occurrences)
    hierarchy: list[SocNode] = []
    container_indices: list[int] = []
    for index, raw in enumerate(occurrences):
        occurrence = read_object(raw, "SOC occurrence")
        placement = occurrence.get("placement")
        parent, matrix, origin = _placement(placement, occurrences, container_indices, index)
        container = len(hierarchy)
        hierarchy.append(SocNode(f"Container {index}", parent, matrix, origin, "container"))
        container_indices.append(container)
        _add_rooms(occurrence, container, hierarchy)
        _add_entities(occurrence, container, hierarchy)
    return hierarchy


def _validate_included_objects(document: dict[str, Any], occurrences: list[Any]) -> None:
    included = _array(document.get("includedObjects", []), "SOC IncludedObjects")
    known = set()
    for item in included:
        table = read_object(item, "SOC IncludedObjects chunk")
        source = table.get("source")
        ordinal = _ordinal(table, "IncludedObjects chunk")
        if not isinstance(source, str) or not source or (source, ordinal) in known:
            raise PackageError("A SOC IncludedObjects chunk must have a unique physical identity.")
        known.add((source, ordinal))
        _validate_included_payload(table)
        read_object(table.get("tables"), "SOC IncludedObjects tables")
    for occurrence in occurrences:
        streams = _array(read_object(occurrence, "SOC occurrence").get("streams"), "SOC streams")
        for stream in streams:
            stream = read_object(stream, "SOC stream")
            refs = _array(stream.get("includedObjects", []), "SOC IncludedObjects references")
            if refs:
                member = read_object(stream.get("member"), "SOC stream member")
                _validate_included_refs(refs, included, member)


def _validate_included_payload(table: dict[str, Any]) -> None:
    raw = _present(table.get("raw"))
    length = table.get("byteLength")
    if not isinstance(raw, str) or not isinstance(length, str) or len(length) > 20 or not length.isascii() or not length.isdecimal():
        raise PackageError("A SOC IncludedObjects chunk must carry its raw payload and declared length.")
    if len(raw) != 4 * ((int(length) + 2) // 3):
        raise PackageError("A SOC IncludedObjects payload does not match its declared length.")
    block_size = 64 * 1024
    decoded = 0
    try:
        for start in range(0, len(raw), block_size):
            block = raw[start:start + block_size]
            if start + block_size < len(raw) and "=" in block:
                raise PackageError("A SOC IncludedObjects payload contains internal padding.")
            decoded += len(base64.b64decode(block, validate=True))
    except (binascii.Error, ValueError) as error:
        raise PackageError("A SOC IncludedObjects payload has invalid Base64.") from error
    if decoded != int(length):
        raise PackageError("A SOC IncludedObjects payload does not match its declared length.")


def _validate_included_refs(refs: list[Any], included: list[Any], member: dict[str, Any]) -> None:
    seen = set()
    for index in refs:
        if type(index) is not int or not 0 <= index < len(included) or index in seen:
            raise PackageError("A SOC IncludedObjects reference must identify a distinct chunk.")
        if member.get("state") != "present" or included[index].get("source") != member.get("source"):
            raise PackageError("A SOC IncludedObjects reference must belong to its stream member.")
        seen.add(index)


def _placement(
    value: object, occurrences: list[Any], containers: list[int], at: int
) -> tuple[int | None, MatrixRows | None, bool | None]:
    if value is None:
        if at:
            raise PackageError("Only the first SOC occurrence may omit a placement.")
        return None, None, None
    if not isinstance(value, list) or len(value) != 2:
        raise PackageError("A SOC placement must identify its parent and child selector.")
    parent, ordinal = value
    if type(parent) is not int or not 0 <= parent < at or type(ordinal) is not int or ordinal < 0:
        raise PackageError("A SOC placement must refer to an earlier parent occurrence.")
    children = _array(read_object(occurrences[parent], "SOC parent").get("children"), "SOC children")
    if ordinal >= len(children):
        raise PackageError("A SOC placement refers to an unknown child selector.")
    cell = read_object(children[ordinal], "SOC child").get("localTransform")
    return containers[parent], _matrix(cell), _marker(cell)


def _add_rooms(occurrence: dict[str, Any], parent: int, nodes: list[SocNode]) -> None:
    registry = _present(occurrence.get("registry"), "data")
    if registry is None:
        return
    for item in _array(read_object(registry, "room registry").get("rooms"), "rooms"):
        room = read_object(item, "room")

        ordinal = _ordinal(room, "room")
        name = f"Room {ordinal}"
        nodes.append(SocNode(name, parent, _room_matrix(room), _marker(room.get("guid")), "room"))


def _add_entities(occurrence: dict[str, Any], parent: int, nodes: list[SocNode]) -> None:
    for stream in _array(occurrence.get("streams"), "SOC streams"):
        data = _present(read_object(stream, "SOC stream").get("member"), "data")
        if data is None:
            continue
        for raw in _array(data, "SOC chunks"):
            chunk = read_object(raw, "SOC chunk")
            if chunk.get("state") != "present":
                continue
            for entity in _array(chunk.get("entities"), "SOC entities"):
                _add_entity(read_object(entity, "SOC entity"), parent, nodes)


def _add_entity(entity: dict[str, Any], parent: int, nodes: list[SocNode]) -> None:
    kinds = [
        read_object(value, "SOC component").get("kind")
        for value in _array(entity.get("components"), "SOC components")
    ]
    ordinal = _ordinal(entity, "entity")
    class_sensor = _present(entity.get("classSensor")) is True
    if "door" in kinds or "proximitySensor" in kinds or class_sensor:
        role = "Door" if "door" in kinds else "Sensor"
        cell = entity.get("localTransform")
        nodes.append(SocNode(f"{role} {ordinal}", parent, _matrix(cell), _marker(cell), role.lower()))
    areas = _array(entity.get("areas"), "SOC areas")
    if areas:
        placed = len(nodes)
        cell = entity.get("localTransform")
        nodes.append(SocNode(f"Area entity {ordinal}", parent, _matrix(cell), _marker(cell), "area"))
        for raw in areas:
            _add_segments(read_object(raw, "SOC area"), placed, nodes)


def _add_segments(area: dict[str, Any], parent: int, nodes: list[SocNode]) -> None:
    area_ordinal = _ordinal(area, "area")
    for raw in _array(area.get("segments"), "room segments"):
        segment = read_object(raw, "room segment")
        ordinal = _ordinal(segment, "segment")
        cell = segment.get("localTransform")
        transform = _present(cell)
        if transform is None:
            matrix = None
        else:
            data = read_object(transform, "segment transform")
            matrix = _quaternion_matrix(data.get("rotation_wxyz"), data.get("translation"))
        nodes.append(SocNode(f"Segment {area_ordinal}.{ordinal}", parent, matrix, _marker(cell), "segment"))


def _room_matrix(room: dict[str, Any]) -> MatrixRows | None:
    position = _present(room.get("pivotRelativePosition"))
    rotation = _present(room.get("pivotRelativeRotation"))
    if position is None or rotation is None:
        return None
    return _quaternion_matrix(rotation, position)


def _quaternion_matrix(rotation: object, translation: object) -> MatrixRows | None:
    w, x, y, z = _vector(rotation, 4)
    tx, ty, tz = _vector(translation, 3)
    norm = math.hypot(w, x, y, z)
    if norm == 0:
        return None
    w, x, y, z = (component / norm for component in (w, x, y, z))
    return (
        (1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y), tx),
        (2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x), ty),
        (2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y), tz),
    )


def _matrix(cell: object) -> MatrixRows | None:
    raw = _present(cell)
    if raw is None:
        return None
    rows = _array(raw, "SOC affine transform")
    if len(rows) != 3:
        raise PackageError("A SOC affine transform must have three rows.")
    return tuple(_vector(row, 4) for row in rows)


def _vector(value: object, size: int) -> tuple[float, ...]:
    vector = _array(value, "SOC transform vector")
    try:
        finite = all(type(item) in (int, float) and math.isfinite(item) for item in vector)
    except OverflowError:
        finite = False
    if len(vector) != size or not finite:
        raise PackageError("A SOC transform vector contains invalid coordinates.")
    return tuple(float(item) for item in vector)


def _present(value: object, field: str = "value") -> Any:
    if value is None:
        return None
    cell = read_object(value, "SOC source value")
    return cell.get(field) if cell.get("state") == "present" else None


def _marker(value: object) -> bool | None:
    if value is None:
        return None
    marker = read_object(value, "SOC source value").get("containsEncryptedSource")
    if marker is not None and type(marker) is not bool:
        raise PackageError("A SOC source value has an invalid encryption marker.")
    return marker


def _ordinal(value: dict[str, Any], where: str) -> int:
    ordinal = value.get("ordinal")
    if type(ordinal) is not int or ordinal < 0:
        raise PackageError(f"An authored {where} has an invalid source ordinal.")
    return ordinal


def _array(value: object, where: str) -> list[Any]:
    if not isinstance(value, list):
        raise PackageError(f"{where} must be an array.")
    return value
