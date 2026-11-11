# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Controls for the exact native Blender package exchange."""

import hashlib
import json
from pathlib import Path

import pytest

from synthetic import manifest, write_package
from verify_portable_packages import ARTIFACT_PREFIX, CASES, verify_exchange

REVISION = "a" * 40
RUN = "1019"
WINDOWS = {"target": "x86_64-pc-windows-msvc", "os": "windows", "arch": "amd64"}
LINUX = {"target": "aarch64-unknown-linux-gnu", "os": "linux", "arch": "arm64"}


def write_receipt(folder: Path, row: dict, **changes) -> None:
    """Records the synthetic artifact hashes and the selected identity."""
    packages = folder / "packages"
    files = [
        {"path": file.relative_to(packages).as_posix(), "sha256": hashlib.sha256(file.read_bytes()).hexdigest()}
        for file in sorted(packages.rglob("*"))
        if file.is_file()
    ]
    receipt = {
        "schema": 1,
        "revision": REVISION,
        "run_id": RUN,
        "exporter_sha256": "b" * 64,
        "files": files,
        **row,
        **changes,
    }
    (folder / "receipt.json").write_text(json.dumps(receipt), encoding="utf-8")


@pytest.fixture
def exchange(tmp_path: Path) -> Path:
    """Creates two native producer identities with matching portable fixtures."""
    for row in (WINDOWS, LINUX):
        folder = tmp_path / (ARTIFACT_PREFIX + row["target"])
        for case in CASES:
            write_package(folder / "packages" / case)
        write_receipt(folder, row)
    return tmp_path


def test_requires_and_compares_both_native_producer_artifacts(exchange: Path) -> None:
    folders = verify_exchange(exchange, [WINDOWS, LINUX], REVISION, RUN)
    assert folders.keys() == {WINDOWS["target"], LINUX["target"]}


@pytest.mark.parametrize(("changes", "message"), [
    ({"target": WINDOWS["target"]}, "receipt target differs"),
    ({"arch": "amd64"}, "receipt arch differs"),
    ({"revision": "c" * 40}, "receipt revision differs"),
    ({"run_id": "1020"}, "receipt run_id differs"),
])
def test_rejects_misbound_artifact(exchange: Path, changes: dict, message: str) -> None:
    folder = exchange / (ARTIFACT_PREFIX + LINUX["target"])
    write_receipt(folder, LINUX, **changes)
    with pytest.raises(ValueError, match=message):
        verify_exchange(exchange, [WINDOWS, LINUX], REVISION, RUN)


def test_rejects_a_missing_native_producer(exchange: Path) -> None:
    missing = {"target": "aarch64-pc-windows-msvc", "os": "windows", "arch": "arm64"}
    with pytest.raises(ValueError, match="artifact targets differ"):
        verify_exchange(exchange, [WINDOWS, LINUX, missing], REVISION, RUN)


def test_rejects_altered_asset_bytes(exchange: Path) -> None:
    folder = exchange / (ARTIFACT_PREFIX + LINUX["target"])
    (folder / "packages/assembly/meshes/m0001.glb").write_bytes(b"altered")
    with pytest.raises(ValueError, match="bytes differ from receipt"):
        verify_exchange(exchange, [WINDOWS, LINUX], REVISION, RUN)


def test_rejects_an_unreferenced_asset_even_with_updated_hashes(exchange: Path) -> None:
    folder = exchange / (ARTIFACT_PREFIX + LINUX["target"])
    (folder / "packages/assembly/extra.glb").write_bytes(b"extra")
    write_receipt(folder, LINUX)
    with pytest.raises(ValueError, match="exact manifest references"):
        verify_exchange(exchange, [WINDOWS, LINUX], REVISION, RUN)


def test_rejects_semantics_that_differ_between_native_targets(exchange: Path) -> None:
    folder = exchange / (ARTIFACT_PREFIX + LINUX["target"])
    data = manifest()
    data["nodes"][0]["name"] = "Other original name"
    write_package(folder / "packages/assembly", data)
    write_receipt(folder, LINUX)
    with pytest.raises(ValueError, match="cross-target package manifest differs"):
        verify_exchange(exchange, [WINDOWS, LINUX], REVISION, RUN)
