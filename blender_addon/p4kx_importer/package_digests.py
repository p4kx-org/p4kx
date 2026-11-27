# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks expected SHA-256 digests against known package files."""

from __future__ import annotations

import hashlib
import json
import re
import stat
from pathlib import Path
from typing import Any

RECEIPT_NAME = "receipt.json"
_SHA256 = re.compile(r"[0-9a-f]{64}")


def is_sha256(value: object) -> bool:
    """Checks the lowercase hexadecimal representation used by package receipts."""
    return isinstance(value, str) and _SHA256.fullmatch(value) is not None


def file_sha256(path: Path) -> str:
    """Returns the SHA-256 of a package file using a bounded streaming read."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def default_receipt(root: Path) -> Path | None:
    """Finds a fixed-name regular package receipt using exact spelling."""
    try:
        names = [entry.name for entry in root.iterdir() if entry.name.casefold() == RECEIPT_NAME]
    except FileNotFoundError:
        return None
    if not names:
        return None
    if names != [RECEIPT_NAME]:
        raise ValueError("The package digest receipt must use the exact declared spelling.")
    path = root / RECEIPT_NAME
    metadata = path.lstat()
    redirected = getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
    if not stat.S_ISREG(metadata.st_mode) or redirected:
        raise ValueError("The package digest receipt is not a regular file.")
    return path


def verify_digests(
    declared: set[str], files: dict[str, Path], expected: dict[str, str]
) -> dict[str, Any]:
    """Reports verified, missing, mismatching and unknown digest entries."""
    verified: list[str] = []
    mismatches: list[str] = []
    unavailable: list[str] = []
    for name, digest in sorted(expected.items()):
        if name not in declared:
            continue
        path = files.get(name)
        if path is None:
            unavailable.append(name)
            continue
        try:
            actual = file_sha256(path)
        except OSError:
            unavailable.append(name)
            continue
        (verified if actual == digest else mismatches).append(name)
    absent = sorted(declared - expected.keys())
    unknown = sorted(expected.keys() - declared)
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


def load_expected(receipt: Path) -> dict[str, str]:
    """Loads digest rows from a receipt selected by the caller or the package."""
    with receipt.open("rb") as stream:
        return read_expected(json.load(stream))


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
