# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates one bounded JSONL mesh vertex map against its referenced GLB."""

from collections.abc import Iterator
from pathlib import Path
import json
from typing import Any, BinaryIO

from .glb_preflight import check_glb
from .manifest_fields import PackageError
from .package_digests import file_sha256, is_sha256

_FORMAT = "p4kx-mesh-vertex-map"
_BATCH_VERTICES = 4096
_MAX_BATCH_USES = 3 * _BATCH_VERTICES + 1


def validate(path: Path, artifact: str, glb: Path) -> None:
    """Checks output accessors, row completion, and measured GLB identity."""
    info = check_glb(glb)
    try:
        with path.open("rb") as stream:
            state = _State(info.positions)
            for line in _lines(stream):
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise PackageError("A vertex map row is not an object.")
                state.accept(row)
            binding = state.binding
            if not state.ended or binding is None:
                raise PackageError("Vertex map is missing its artifact binding or end row.")
        digest = binding.get("artifact")
        if not isinstance(digest, dict) or binding.get("glb") != artifact:
            raise PackageError("Vertex map points to a different GLB file.")
        if _number(digest.get("byteLength")) != info.bytes or not is_sha256(digest.get("sha256")):
            raise PackageError("Vertex map GLB length or digest is invalid.")
        if file_sha256(glb) != digest["sha256"]:
            raise PackageError("Vertex map GLB SHA-256 differs from the output.")
    except PackageError:
        raise
    except (OSError, ValueError, RecursionError) as error:
        raise PackageError("Vertex map could not be read.") from error


def _lines(stream: BinaryIO) -> Iterator[bytes]:
    for line in stream:
        if not line.endswith(b"\n"):
            raise PackageError("Vertex map row has no newline.")
        yield line


class _State:
    """Tracks physical output coordinates and closure while accepting rows in order."""

    def __init__(self, positions: tuple[tuple[int, int] | None, ...]):
        self.positions = positions
        self.started = False
        self.ended = False
        self.binding: dict[str, Any] | None = None
        self.meshes = 0
        self.vertices = 0
        self.left = 0
        self.next_vertex = 0
        self.sources: set[str] = set()
        self.source_uses = 0
        self.pending_refs: set[str] = set()
        self.used_sources: set[str] = set()
        self.metadata_uses: set[str] = set()

    def accept(self, row: dict[str, Any]) -> None:
        kind = row.get("type")
        if self.ended or (not self.started and kind != "header") or (self.binding is not None and kind != "end"):
            raise PackageError("Vertex map rows are out of order.")
        if kind == "header":
            if self.started or type(row.get("version")) is not int or row.get("version") != 1 or row.get("format") != _FORMAT:
                raise PackageError("Unknown mesh vertex map format or version.")
            self.started = True
        elif kind == "mesh":
            self.mesh(row)
        elif kind == "vertex":
            self.vertex(row)
        elif kind == "sourceUse":
            source_use = row.get("sourceUse")
            if not isinstance(source_use, dict) or not isinstance(source_use.get("source"), str):
                raise PackageError("A vertex map source use has no source.")
            ident = source_use.get("id")
            if not isinstance(ident, str):
                raise PackageError("A vertex map source use has no identity.")
            if ident in self.pending_refs:
                self.pending_refs.remove(ident)
            elif source_use.get("scope") == "archiveDirectoryMetadata" and ident not in self.metadata_uses:
                self.metadata_uses.add(ident)
            else:
                raise PackageError("A vertex map source use is unreferenced or duplicated.")
            self.used_sources.add(source_use["source"])
            self.source_uses += 1
        elif kind == "source":
            source = row.get("source")
            ident = source.get("id") if isinstance(source, dict) else None
            if not isinstance(ident, str) or ident in self.sources:
                raise PackageError("A vertex map physical source is invalid or duplicated.")
            self.sources.add(ident)
            ancestors = source.get("ancestors")
            if not isinstance(ancestors, list) or any(not isinstance(value, str) for value in ancestors):
                raise PackageError("A vertex map physical source has invalid ancestor identities.")
            self.used_sources.update(ancestors)
        elif kind == "artifact" and self.binding is None:
            self.binding = row
        elif kind == "end":
            self.end(row)
        else:
            raise PackageError("Unknown or repeated vertex map row.")

    def mesh(self, row: dict[str, Any]) -> None:
        mesh = _integer(row.get("mesh"))
        if self.left or mesh != self.meshes or mesh >= len(self.positions):
            raise PackageError("Vertex map mesh order or count differs from the output.")
        accessor = _integer(row.get("positionAccessor"))
        count = _number(row.get("vertices"))
        if self.positions[mesh] != (accessor, count):
            raise PackageError("Vertex map POSITION accessor or count differs from the output.")
        state = row.get("mapping")
        if state not in ("present", "unknown"):
            raise PackageError("Vertex map mapping state is invalid.")
        self.left = count if state == "present" else 0
        self.next_vertex = 0
        self.meshes += 1
        self.references(row)

    def vertex(self, row: dict[str, Any]) -> None:
        if not self.left or _integer(row.get("mesh")) != self.meshes - 1:
            raise PackageError("An extra or misplaced mapped vertex was written.")
        if _integer(row.get("vertex")) != self.next_vertex:
            raise PackageError("Mapped vertices are not in POSITION order.")
        if _integer(row.get("sourceVertex")) > 0xFFFFFFFF:
            raise PackageError("An original vertex ordinal crosses the unsigned index width.")
        self.next_vertex += 1
        self.vertices += 1
        self.left -= 1
        self.references(row)

    def references(self, row: dict[str, Any]) -> None:
        values = row.get("sourceUsesRefs")
        if not isinstance(values, list) or any(not isinstance(value, str) for value in values):
            raise PackageError("Field provenance references are malformed.")
        marker = row.get("containsEncryptedSource")
        if marker is not None and type(marker) is not bool:
            raise PackageError("Field encryption marker is invalid.")
        if type(row.get("sourceSetComplete")) is not bool:
            raise PackageError("Field provenance completion is invalid.")
        if len(values) > 3 or len(set(values)) != len(values):
            raise PackageError("Field provenance references are duplicated or exceed the recorded roles.")
        if marker is False and not row["sourceSetComplete"]:
            raise PackageError("Unencrypted marker requires complete source evidence.")
        if not values and marker is not (False if row["sourceSetComplete"] else None):
            raise PackageError("Field provenance marker lacks a contributor.")
        self.pending_refs.update(values)
        if len(self.pending_refs) > _MAX_BATCH_USES:
            raise PackageError("Vertex map source-use batch was not closed.")

    def end(self, row: dict[str, Any]) -> None:
        actual = (self.meshes, self.vertices, len(self.sources), self.source_uses)
        expected = tuple(_number(row.get(key)) for key in ("meshes", "vertices", "sources", "sourceUses"))
        if (
            self.binding is None or self.left or self.meshes != len(self.positions) or self.pending_refs
            or not self.used_sources.issubset(self.sources) or actual != expected
        ):
            raise PackageError("Vertex map source evidence or row counts are incomplete.")
        self.ended = True


def _number(value: Any) -> int:
    if not isinstance(value, str) or not value.isascii() or not value.isdecimal():
        raise PackageError("Expected a decimal vertex map count.")
    if len(value) > 20 or (len(value) > 1 and value.startswith("0")):
        raise PackageError("Noncanonical vertex map count.")
    parsed = int(value)
    if parsed > 0xFFFFFFFFFFFFFFFF:
        raise PackageError("Vertex map count crosses the unsigned length width.")
    return parsed


def _integer(value: Any) -> int:
    if type(value) is not int or value < 0:
        raise PackageError("Expected a non-negative vertex map ordinal.")
    return value
