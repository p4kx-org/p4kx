# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Reports structural and optional SHA-256 validation of a Blender package."""

from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from itertools import chain
from pathlib import Path
from typing import Any

from . import soc_scene
from .manifest_fields import PackageError
from .mesh_preflight import preflight_all_meshes
from .package import MANIFEST_NAME, load_package, resolve_file
from .package_digests import default_receipt, file_sha256, load_expected, verify_digests


def validate_package(location: Path, receipt: Path | None = None) -> dict[str, Any]:
    """Validates an ordinary package without importing it or writing files."""
    report: dict[str, Any] = {"schema": 1}
    try:
        package = load_package(location)
    except PackageError as error:
        report["structure"] = {"status": "failed", "manifest_error": str(error)}
        report["digests"] = _unavailable_receipt(location, receipt)
        return report

    declared = {MANIFEST_NAME}
    files = {MANIFEST_NAME: package.root / MANIFEST_NAME}
    asset_problems: list[dict[str, str]] = []
    for asset in chain(package.meshes.values(), package.textures.values()):
        declared.add(asset.file)
        try:
            path = resolve_file(package.root, asset.file)
            if not path.is_file():
                raise PackageError("File is missing or is not a regular file.")
            files[asset.file] = path
        except PackageError as error:
            asset_problems.append({"path": asset.file, "detail": str(error)})
    if package.authored_soc:
        reference = package.authored_soc
        declared.add(reference)
        try:
            companion = resolve_file(package.root, reference)
            if companion.is_file():
                files[reference] = companion
            soc_scene.load(package.root, reference)
        except PackageError as error:
            asset_problems.append({"path": reference, "detail": str(error)})
    for texture in package.textures.values():
        if texture.format != "dds" or texture.file not in files:
            continue
        try:
            path = files[texture.file]
            if path.stat().st_size != texture.byte_length or file_sha256(path) != texture.sha256:
                asset_problems.append({"path": texture.file, "detail": "DDS byte binding differs."})
        except OSError:
            asset_problems.append({"path": texture.file, "detail": "DDS byte binding cannot be read."})
    asset_problems.sort(key=lambda problem: problem["path"])
    preflight = preflight_all_meshes(package)
    status = "valid"
    if preflight.problems or asset_problems:
        status = "failed"
    elif preflight.incomplete_counts or package.warnings:
        status = "partial"
    report["structure"] = {
        "status": status,
        "package_version": ".".join(map(str, package.version)),
        "checked_meshes": list(preflight.paths),
        "mesh_problems": [asdict(problem) for problem in preflight.problems],
        "incomplete_mesh_counts": list(preflight.incomplete_counts),
        "asset_problems": asset_problems,
        "manifest_warnings": package.warnings,
    }
    report["digests"] = _verify_receipt(package.root, declared, files, receipt)
    return report


def _unavailable_receipt(location: Path, receipt: Path | None) -> dict[str, str]:
    if receipt is not None:
        return {"status": "unchecked"}
    try:
        root = location if location.is_dir() else location.parent
        found = default_receipt(root)
    except (OSError, ValueError):
        return {"status": "unchecked"}
    return {"status": "absent" if found is None else "unchecked"}


def _verify_receipt(
    root: Path, declared: set[str], files: dict[str, Path], receipt: Path | None
) -> dict[str, Any]:
    try:
        if receipt is None:
            receipt = default_receipt(root)
        if receipt is None:
            return {"status": "absent"}
        return verify_digests(declared, files, load_expected(receipt))
    except OSError:
        return {"status": "failed", "error": "Package digest receipt cannot be read."}
    except (RecursionError, ValueError) as error:
        return {"status": "failed", "error": str(error)}


def main() -> int:
    """Prints the package report as JSON and returns its exit status."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path, help="package folder or scene.json")
    parser.add_argument("--receipt", type=Path, help="external SHA-256 receipt instead of the package receipt")
    arguments = parser.parse_args()
    report = validate_package(arguments.package, arguments.receipt)
    print(json.dumps(report, sort_keys=True))
    return int(report["structure"]["status"] == "failed" or report["digests"]["status"] == "failed")


if __name__ == "__main__":
    raise SystemExit(main())
