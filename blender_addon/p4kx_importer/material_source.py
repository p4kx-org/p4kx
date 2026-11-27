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
from .source_evidence import SourceEvidence, follows_pointer

_FORMAT = "p4kx-blender-material-source"
_VERSION = 1


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
        self.evidence = SourceEvidence("authored material")
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
            self.evidence.source(read_object(row.get("source"), "material physical source"))
        elif kind == "sourceUse" and self.phase <= 2:
            self.phase = 2
            self.evidence.source_use(read_object(row.get("sourceUse"), "material source use"))
        elif kind == "field":
            self.phase = 3
            self.field(row)
        elif kind == "end":
            self.end(row)
        else:
            raise PackageError("The authored material rows are out of order.")

    def field(self, row: dict[str, Any]) -> None:
        if row.keys() != {"type", "path", "containsEncryptedSource", "sourceSetComplete", "sourceUsesRefs"}:
            raise PackageError("An authored material field is malformed.")
        path = row["path"]
        if (
            not isinstance(path, str) or path in self.paths or not follows_pointer(self.authored, path)
            or (path != "/authored" and not path.startswith("/authored/"))
        ):
            raise PackageError("An authored material field path is unresolved or duplicated.")
        self.evidence.check_origin({
            key: value for key, value in row.items() if key not in {"type", "path"}
        })
        self.paths.add(path)

    def end(self, row: dict[str, Any]) -> None:
        counts = {"sources": len(self.evidence.sources), "uses": len(self.evidence.uses), "fields": len(self.paths)}
        if (
            row.keys() != {*counts, "type"}
            or any(type(row.get(name)) is not int or row[name] != count for name, count in counts.items())
        ):
            raise PackageError("The authored material source counts or identities are incomplete.")
        self.evidence.check_ancestors()
        self.ended = True
