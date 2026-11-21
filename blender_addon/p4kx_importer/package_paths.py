# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Resolves package paths and validates the spelling of referenced assets."""

from collections import defaultdict
from collections.abc import Iterable
from pathlib import Path

from .manifest_fields import PackageError


def require_exact_asset_case(root: Path, paths: Iterable[str]) -> None:
    """Rejects case mismatches in existing asset paths with validated package syntax."""
    directories: dict[str, set[str]] = defaultdict(set)
    for path in paths:
        directory, name = path.split("/")
        directories[directory].add(name)
    if not directories:
        return

    base = root.resolve()
    _check_names(base, directories)
    for directory, names in directories.items():
        _check_names(resolve_file(base, directory), names)


def resolve_file(root: Path, relative: str) -> Path:
    """Returns the absolute path of a package file, refusing anything outside ``root``."""
    base = root.resolve()
    path = (base / relative).resolve()
    if not path.is_relative_to(base):
        raise PackageError(f"File {relative!r} lies outside the package folder.")
    return path


def _check_names(folder: Path, names: Iterable[str]) -> None:
    expected = {name.casefold(): name for name in names}
    wrong_case: dict[str, str] = {}
    try:
        for entry in folder.iterdir():
            actual = entry.name
            folded = actual.casefold()
            wanted = expected.get(folded)
            if wanted is None:
                continue
            if actual == wanted:
                del expected[folded]
                wrong_case.pop(wanted, None)
            else:
                wrong_case[wanted] = min(actual, wrong_case.get(wanted, actual))
    except (FileNotFoundError, NotADirectoryError):
        return
    except OSError as error:
        raise PackageError(f"Cannot check asset names in {folder}: {error.strerror}.") from error
    if wrong_case:
        wanted = min(wrong_case)
        actual = wrong_case[wanted]
        raise PackageError(f"Use the exact case in {folder}: expected {wanted!r}, found {actual!r}.")
