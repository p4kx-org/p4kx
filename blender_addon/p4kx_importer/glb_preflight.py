# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the structure of a self-contained package GLB before import."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from struct import Struct
from typing import Any, BinaryIO

from .glb_geometry import GeometryCounts, geometry_counts
from .glb_material_slots import material_slot_issue
from .glb_references import validate_references
from .manifest_fields import PackageError, read_object

MAX_GLB_JSON_BYTES = int(Path(__file__).with_name("glb-json-budget.bytes").read_text(encoding="ascii"))

_HEADER = Struct("<4sII")
_CHUNK_HEADER = Struct("<I4s")
_JSON = b"JSON"
_BIN = b"BIN\x00"


@dataclass(frozen=True, slots=True)
class GlbInfo:
    """Structural facts read without loading a binary chunk."""

    bytes: int
    materials: int
    geometry: GeometryCounts | None
    positions: tuple[tuple[int, int] | None, ...]
    material_slot_issue: str | None
    primitive_bindings: tuple[tuple[tuple[int | None, int | None], ...], ...]


def check_glb(path: Path) -> GlbInfo:
    """Returns structural facts for an embedded GLB or raises a package error."""
    try:
        with path.open("rb") as stream:
            return _check_stream(stream)
    except PackageError as error:
        raise PackageError(f"{path.name}: {error}") from error
    except OSError as error:
        raise PackageError(f"Cannot read {path.name}: {error.strerror}.") from error


def _check_stream(stream: BinaryIO) -> GlbInfo:
    actual_size = stream.seek(0, 2)
    stream.seek(0)
    raw = stream.read(_HEADER.size)
    if len(raw) != _HEADER.size:
        raise PackageError("GLB header is truncated.")
    magic, version, declared_size = _HEADER.unpack(raw)
    if magic != b"glTF":
        raise PackageError("File is not a GLB.")
    if version != 2:
        raise PackageError(f"Unsupported GLB version {version}.")
    if declared_size != actual_size:
        raise PackageError("GLB total length does not match the file size.")

    json_length, kind = _read_chunk(stream, actual_size)
    if kind != _JSON:
        raise PackageError("GLB has no JSON chunk in the first position.")
    if json_length > MAX_GLB_JSON_BYTES:
        raise PackageError(
            f"GLB JSON chunk needs {json_length} bytes, exceeding the package budget of {MAX_GLB_JSON_BYTES} bytes."
        )
    document = _read_document(stream, json_length)
    bin_length, bin_offset = _skip_chunks(stream, actual_size)
    data_length, material_count = validate_references(document, bin_length)
    if bin_offset is not None and bin_length is not None and bin_length > data_length:
        stream.seek(bin_offset + data_length)
        padding = stream.read(bin_length - data_length)
        if padding != bytes(bin_length - data_length):
            raise PackageError("GLB BIN padding is invalid.")
    return GlbInfo(
        actual_size,
        material_count,
        geometry_counts(document),
        _position_accessors(document),
        material_slot_issue(document.get("materials", [])),
        tuple(
            tuple((row.get("indices"), row.get("material")) for row in mesh["primitives"])
            for mesh in document.get("meshes", [])
        ),
    )


def _position_accessors(document: dict[str, Any]) -> tuple[tuple[int, int] | None, ...]:
    positions = []
    accessors = document.get("accessors", [])
    for mesh in document.get("meshes", []):
        refs = {primitive["attributes"].get("POSITION") for primitive in mesh["primitives"]}
        if len(refs) != 1 or None in refs:
            positions.append(None)
        else:
            index = refs.pop()
            positions.append((index, accessors[index]["count"]))
    return tuple(positions)


def _read_chunk(stream: BinaryIO, total: int) -> tuple[int, bytes]:
    raw = stream.read(_CHUNK_HEADER.size)
    if len(raw) != _CHUNK_HEADER.size:
        raise PackageError("GLB chunk header is truncated.")
    length, kind = _CHUNK_HEADER.unpack(raw)
    if length % 4 != 0:
        raise PackageError("GLB chunk length is not aligned to four bytes.")
    if length > total - stream.tell():
        raise PackageError("GLB chunk extends past the file.")
    return length, kind


def _read_document(stream: BinaryIO, length: int) -> dict[str, Any]:
    raw = stream.read(length)
    if len(raw) != length:
        raise PackageError("GLB JSON chunk is truncated.")
    try:
        document = json.loads(raw.decode("utf-8"))
    except (RecursionError, ValueError) as error:
        raise PackageError("GLB JSON chunk is invalid.") from error
    return read_object(document, "GLB JSON")


def _skip_chunks(stream: BinaryIO, total: int) -> tuple[int | None, int | None]:
    bin_length = bin_offset = None
    after_json = stream.tell()
    while stream.tell() < total:
        start = stream.tell()
        length, kind = _read_chunk(stream, total)
        if kind == _JSON or (kind == _BIN and start != after_json):
            raise PackageError("GLB chunks are duplicated or out of order.")
        if kind == _BIN:
            bin_length, bin_offset = length, stream.tell()
        stream.seek(length, 1)
    return bin_length, bin_offset
