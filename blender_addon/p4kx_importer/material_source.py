# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates streamed authored material evidence independently of scene rendering."""

from __future__ import annotations

import gzip
import json
from pathlib import Path
from typing import Any
import zlib

from .manifest_fields import PackageError, read_array, read_object

_FORMAT = "p4kx-blender-material-source"
_VERSION = 1
_ENCRYPTION = ("encrypted", "unencrypted", "unknown")
_ROLES = ("payload", "selector", "conversion", "attempted", "reference")
_CONTRIBUTORS = ("payload", "selector", "conversion")
_SCOPES = ("payload", "archiveDirectoryMetadata", "fileMetadata")


def parse_refs(value: object) -> dict[int, str]:
    """Reads only canonical, ordinal-matched companion paths from the manifest."""
    if value is None:
        return {}
    references: dict[int, str] = {}
    for ordinal, item in enumerate(read_array(value, "material_libraries")):
        row = read_object(item, "material library")
        if "authored_file" not in row:
            continue
        ref = read_object(row["authored_file"], "authored material reference")
        expected = f"material-sources/s{ordinal:04}.jsonl.gz"
        version = ref.get("schema_version")
        if (
            ref.keys() != {"file", "schema_version"} or ref.get("file") != expected
            or type(version) is not int or version != _VERSION
            or "authored" in row or "authored_origin" in row
        ):
            raise PackageError("The authored material companion reference is invalid.")
        references[ordinal] = expected
    return references


def validate(path: Path, ordinal: int) -> None:
    """Checks the compressed stream, contents, IDs, and field evidence closure."""
    state = _Source(ordinal)
    try:
        with gzip.open(path, "rt", encoding="utf-8", newline="") as stream:
            for line in stream:
                if not line.endswith("\n"):
                    raise PackageError("An authored material row has no newline.")
                state.accept(read_object(json.loads(line), "authored material row"))
    except (OSError, EOFError, RecursionError, ValueError, zlib.error) as error:
        raise PackageError("The authored material companion cannot be read.") from error
    if not state.ended:
        raise PackageError("The authored material companion has no end row.")


class _Source:
    def __init__(self, ordinal: int) -> None:
        self.ordinal = ordinal
        self.started = False
        self.ended = False
        self.authored: dict[str, Any] = {}
        self.sources: dict[str, tuple[str, ...]] = {}
        self.uses: dict[str, tuple[str, str]] = {}
        self.paths: set[str] = set()
        self.phase = 0

    def accept(self, row: dict[str, Any]) -> None:
        kind = row.get("type")
        if self.ended:
            raise PackageError("An authored material row follows the end row.")
        if not self.started:
            expected = {"type": "header", "format": _FORMAT, "version": _VERSION, "library": self.ordinal}
            if row != expected or type(row.get("version")) is not int or type(row.get("library")) is not int:
                raise PackageError("The authored material header or library ordinal is invalid.")
            self.started = True
            return
        if self.phase == 0:
            if kind != "library" or row.keys() != {"type", "authored"}:
                raise PackageError("The authored material library row is missing.")
            self.authored = row
            read_object(row["authored"], "authored material library")
            self.phase = 1
        elif kind == "source" and self.phase == 1:
            self.source(read_object(row.get("source"), "material physical source"))
        elif kind == "sourceUse" and self.phase <= 2:
            self.phase = 2
            self.source_use(read_object(row.get("sourceUse"), "material source use"))
        elif kind == "field":
            self.phase = 3
            self.field(row)
        elif kind == "end":
            self.end(row)
        else:
            raise PackageError("The authored material rows are out of order.")

    def source(self, source: dict[str, Any]) -> None:
        identity = source.get("id")
        ancestors = source.get("ancestors")
        if (
            not _identity(identity) or identity in self.sources
            or not isinstance(ancestors, list) or any(not _identity(value) for value in ancestors)
            or len(set(ancestors)) != len(ancestors) or identity in ancestors
            or source.get("storedEncryption") not in _ENCRYPTION
        ):
            raise PackageError("An authored material physical source is malformed or duplicated.")
        self.sources[identity] = tuple(ancestors)

    def source_use(self, use: dict[str, Any]) -> None:
        identity = use.get("id")
        source = use.get("source")
        scope = use.get("scope")
        role = use.get("role")
        encryption = use.get("effectiveEncryption")
        if (
            not _identity(identity) or identity in self.uses or not _identity(source) or source not in self.sources
            or scope not in _SCOPES or role not in _ROLES or encryption not in _ENCRYPTION
            or not isinstance(use.get("location"), dict)
        ):
            raise PackageError("An authored material source use is malformed or duplicated.")
        self.uses[identity] = role, encryption

    def field(self, row: dict[str, Any]) -> None:
        if row.keys() != {"type", "path", "containsEncryptedSource", "sourceSetComplete", "sourceUsesRefs"}:
            raise PackageError("An authored material field is malformed.")
        path = row["path"]
        if not isinstance(path, str) or path in self.paths or not _follows(self.authored, path):
            raise PackageError("An authored material field path is unresolved or duplicated.")
        references = row["sourceUsesRefs"]
        if (
            not isinstance(references, list)
            or any(not _identity(ref) or ref not in self.uses for ref in references)
            or len(set(references)) != len(references)
            or any(self.uses[ref][0] not in _CONTRIBUTORS for ref in references)
        ):
            raise PackageError("An authored material field has unresolved contributor references.")
        complete = row["sourceSetComplete"]
        if type(complete) is not bool:
            raise PackageError("An authored material field has an invalid completeness marker.")
        encryption = {self.uses[ref][1] for ref in references}
        marker = True if "encrypted" in encryption else False if complete and "unknown" not in encryption else None
        if row["containsEncryptedSource"] is not marker:
            raise PackageError("An authored material field encryption marker differs from its sources.")
        self.paths.add(path)

    def end(self, row: dict[str, Any]) -> None:
        counts = {"sources": len(self.sources), "uses": len(self.uses), "fields": len(self.paths)}
        if (
            row.keys() != {*counts, "type"}
            or any(type(row.get(name)) is not int or row[name] != count for name, count in counts.items())
            or any(ancestor not in self.sources for ancestors in self.sources.values() for ancestor in ancestors)
        ):
            raise PackageError("The authored material source counts or identities are incomplete.")
        self.ended = True


def _identity(value: Any) -> bool:
    return isinstance(value, str) and bool(value)


def _follows(value: Any, pointer: str) -> bool:
    if pointer != "/authored" and not pointer.startswith("/authored/"):
        return False
    for part in pointer.split("/")[1:]:
        if "~" in part.replace("~0", "").replace("~1", ""):
            return False
        key = part.replace("~1", "/").replace("~0", "~")
        if isinstance(value, dict) and key in value:
            value = value[key]
        elif (
            isinstance(value, list) and key.isascii() and key.isdecimal()
            and (key == "0" or not key.startswith("0"))
            and len(key) <= len(str(len(value))) and int(key) < len(value)
        ):
            value = value[int(key)]
        else:
            return False
    return True
