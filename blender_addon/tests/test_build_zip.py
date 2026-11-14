# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of the add-on zip builder."""

from __future__ import annotations

import tomllib
import zipfile
from pathlib import Path

import build_zip

ROOT = Path(__file__).resolve().parents[1]


def test_zip_holds_the_addon_at_its_root_and_is_reproducible(tmp_path: Path) -> None:
    first = build_zip.build_zip(tmp_path / "a")
    second = build_zip.build_zip(tmp_path / "b")
    assert first.read_bytes() == second.read_bytes()
    with zipfile.ZipFile(first) as archive:
        names = archive.namelist()
    assert "blender_manifest.toml" in names
    assert "__init__.py" in names
    assert "glb-json-budget.bytes" in names
    assert names == sorted(names)
    assert not any("__pycache__" in name for name in names)


def test_the_manifest_states_the_supported_blender_version() -> None:
    manifest = tomllib.loads((ROOT / "p4kx_importer" / "blender_manifest.toml").read_text("utf-8"))
    assert manifest["type"] == "add-on"
    assert manifest["blender_version_min"] == "5.2.0"
    assert manifest["id"] == "p4kx_importer"


def test_every_python_file_carries_the_spdx_header() -> None:
    for path in ROOT.rglob("*.py"):
        if "__pycache__" in path.parts or ".venv" in path.parts:
            continue
        lines = path.read_text("utf-8").splitlines()
        assert lines[:2] == [
            "# SPDX-License-Identifier: MIT OR Apache-2.0",
            "# SPDX-FileCopyrightText: 2026 the P4KX contributors",
        ], path
