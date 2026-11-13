# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks native fixture receipts and consumes the exact packages from each producer."""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

from p4kx_importer import package  # noqa: E402
from p4kx_importer.mesh_preflight import preflight_all_meshes  # noqa: E402
from p4kx_importer.package_digests import file_sha256, is_sha256, read_expected  # noqa: E402

ARTIFACT_PREFIX = "p4kx-blender-package-"
CASES = ("assembly", "livery")
SYSTEMS = {"windows": "Windows", "linux": "Linux", "macos": "Darwin"}
MACHINES = {"amd64": {"amd64", "x86_64"}, "arm64": {"arm64", "aarch64"}}


def require(condition: bool, message: str) -> None:
    """Fails the artifact gate with the offending scope."""
    if not condition:
        raise ValueError(message)


def file_hashes(root: Path) -> dict[str, str]:
    """Inventories each exact portable path and refuses linked entries."""
    require(root.is_dir(), f"Missing package directory: {root.name}")
    files = {}
    for entry in root.rglob("*"):
        path = entry.relative_to(root).as_posix()
        require(not entry.is_symlink() and not entry.is_junction(), f"Linked package entry: {path}")
        if entry.is_file():
            files[path] = file_sha256(entry)
        else:
            require(entry.is_dir(), f"Unexpected package entry: {path}")
    return files


def read_receipt(folder: Path, row: dict, revision: str, run_id: str) -> dict[str, str]:
    """Binds the downloaded artifact to its native target and workflow."""
    target = row["target"]
    require(
        {entry.name for entry in folder.iterdir()} == {"packages", "receipt.json"},
        f"{target}: expected packages and one receipt",
    )
    receipt_path = folder / "receipt.json"
    packages = folder / "packages"
    require(
        not receipt_path.is_symlink() and receipt_path.is_file()
        and not packages.is_symlink() and not packages.is_junction(),
        f"{target}: receipt and package root must be regular entries",
    )
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    expected = {
        "schema": 1,
        "revision": revision,
        "run_id": run_id,
        "target": target,
        "os": row["os"],
        "arch": row["arch"],
    }
    for key, value in expected.items():
        require(receipt.get(key) == value, f"{target}: receipt {key} differs")
    executable = receipt.get("exporter_sha256")
    require(
        is_sha256(executable),
        f"{target}: invalid native exporter digest",
    )
    hashes = read_expected(receipt)
    require(bool(hashes), f"{target}: receipt has no files")
    require(hashes == file_hashes(packages), f"{target}: received package paths or bytes differ from receipt")
    return hashes


def manifests(folder: Path, hashes: dict[str, str]) -> dict[str, dict]:
    """Loads the package with the add-on and checks its complete referenced assets."""
    root = folder / "packages"
    require({entry.name for entry in root.iterdir()} == set(CASES), "Wrong fixture cases")
    expected = set()
    documents = {}
    for case in CASES:
        directory = root / case
        loaded = package.load_package(directory)
        preflight = preflight_all_meshes(loaded)
        require(
            not preflight.problems,
            f"{case}: package mesh preflight failed: " + " | ".join(
                problem.message() for problem in preflight.problems
            ),
        )
        expected.add(f"{case}/scene.json")
        expected.update(f"{case}/{asset.file}" for asset in loaded.meshes.values())
        expected.update(f"{case}/{asset.file}" for asset in loaded.textures.values())
        documents[case] = json.loads((directory / "scene.json").read_text(encoding="utf-8"))
    require(expected == hashes.keys(), "Package assets differ from the exact manifest references")
    return documents


def verify_exchange(root: Path, rows: list[dict], revision: str, run_id: str) -> dict[str, Path]:
    """Verifies actual package bytes and compares every producer's manifest semantics."""
    folders = {row["target"]: root / (ARTIFACT_PREFIX + row["target"]) for row in rows}
    require(len(folders) == len(rows) and bool(folders), "Expected unique native producer targets")
    require(
        {entry.name for entry in root.iterdir()} == {folder.name for folder in folders.values()},
        "Downloaded artifact targets differ from the native matrix",
    )
    reference = None
    for row in rows:
        target = row["target"]
        folder = folders[target]
        require(folder.is_dir() and not folder.is_symlink() and not folder.is_junction(), f"{target}: wrong artifact folder")
        documents = manifests(folder, read_receipt(folder, row, revision, run_id))
        if reference is None:
            reference = documents
        require(documents == reference, f"{target}: cross-target package manifest differs")
    return folders


def verify_headless(folders: dict[str, Path], version: str) -> None:
    """Uses the existing exported-fixture scene checks on the received packages."""
    import bpy

    from verify_exported_package import verify_packages

    expected = tuple(int(part) for part in version.split("."))
    require(bpy.app.version == expected, f"Expected bpy {version}, found {bpy.app.version_string}")
    for target, folder in folders.items():
        verify_packages(folder / "packages", render_images=False)
        print(f"Blender {version} imported {target} assembly and livery")


def main() -> None:
    """Runs the native Python or headless Blender consumer of the artifact exchange."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--bpy-version")
    cli = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    arguments = parser.parse_args(cli)

    rows = json.loads(os.environ["P4KX_PLATFORM_MATRIX"])["include"]
    consumer = next(row for row in rows if row["target"] == arguments.target)
    require(
        platform.system() == SYSTEMS[consumer["os"]] and platform.machine().lower() in MACHINES[consumer["arch"]],
        f"Python process does not match native consumer {arguments.target}",
    )
    folders = verify_exchange(arguments.root, rows, os.environ["GITHUB_SHA"], os.environ["GITHUB_RUN_ID"])
    print(f"{arguments.target} validated the exact packages from {len(folders)} native producers")
    if arguments.bpy_version:
        verify_headless(folders, arguments.bpy_version)


if __name__ == "__main__":
    main()
