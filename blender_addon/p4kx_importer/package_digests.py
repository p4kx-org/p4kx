# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks expected SHA-256 digests against known package files."""

from __future__ import annotations

import hashlib
import re
from pathlib import Path
from typing import Any

_SHA256 = re.compile(r"[0-9a-f]{64}")


def is_sha256(value: object) -> bool:
    """Checks the lowercase hexadecimal representation used by package receipts."""
    return isinstance(value, str) and _SHA256.fullmatch(value) is not None


def file_sha256(path: Path) -> str:
    """Returns the SHA-256 of a package file using a bounded streaming read."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_digests(files: dict[str, Path], expected: dict[str, str] | None) -> dict[str, Any]:
    """Reports verified, missing, mismatching and unknown digest entries."""
    if expected is None:
        return {"status": "absent"}
    verified: list[str] = []
    mismatches: list[str] = []
    unavailable: list[str] = []
    for name, digest in sorted(expected.items()):
        if name not in files:
            continue
        try:
            actual = file_sha256(files[name])
        except OSError:
            unavailable.append(name)
            continue
        (verified if actual == digest else mismatches).append(name)
    absent = sorted(files.keys() - expected.keys())
    unknown = sorted(expected.keys() - files.keys())
    status = "verified"
    if mismatches or unknown or unavailable:
        status = "failed"
    elif absent:
        status = "partial"
    return {
        "status": status,
        "verified": verified,
        "unrecorded": absent,
        "mismatches": mismatches,
        "unknown": unknown,
        "unavailable": unavailable,
    }


def read_expected(document: object) -> dict[str, str]:
    """Reads package-relative digests without resolving the receipt's paths."""
    if not isinstance(document, dict) or type(document.get("schema")) is not int or document["schema"] != 1:
        raise ValueError("Unsupported package digest receipt schema.")
    rows = document.get("files")
    if not isinstance(rows, list):
        raise ValueError("Package digest receipt files must be an array.")
    expected: dict[str, str] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("Package digest receipt entries must be objects.")
        name, digest = row.get("path"), row.get("sha256")
        if not isinstance(name, str) or not name or name in expected:
            raise ValueError("Package digest receipt has an invalid or duplicate path.")
        if not is_sha256(digest):
            raise ValueError("Package digest receipt has an invalid SHA-256.")
        expected[name] = digest
    return expected
