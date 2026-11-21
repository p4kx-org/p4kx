# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests DDS package identity and byte bindings."""

from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from p4kx_importer.package import PackageError, load_package
from p4kx_importer.validate_package import validate_package
from synthetic import manifest, write_package


def dds_fixture(root: Path, **overrides: object) -> Path:
    data = manifest(version="1.14")
    texture = data["textures"][0]
    texture.update(
        {
            "file": "textures/t0001.dds",
            "format": "dds",
            "dds_asset": {
                "artifact": {"byteLength": "5", "sha256": hashlib.sha256(b"image").hexdigest()}
            },
            **overrides,
        }
    )
    package = write_package(root, data)
    (root / texture["file"]).write_bytes(b"image")
    return package


def test_dds_requires_the_written_identity_and_retains_png(tmp_path: Path) -> None:
    package = load_package(dds_fixture(tmp_path))
    assert package.version == (1, 14)
    assert package.textures["t0001"].format == "dds"
    assert package.textures["t0001"].byte_length == 5
    assert package.textures["t0001"].dimensions == (1, 1)
    assert package.textures["t0002"].format == "png"
    assert package.warnings == []


@pytest.mark.parametrize(
    ("field", "value", "message"),
    [
        ("file", "textures/t0001.png", "texture format"),
        ("file", "textures/t0009.dds", "DDS texture identity"),
        ("dds_fallback", "split_alpha", "DDS texture identity"),
        ("dds_asset", {"artifact": {"byteLength": "05", "sha256": "0" * 64}}, "DDS artifact digest"),
        ("dds_asset", {"artifact": {"byteLength": "5", "sha256": "not a digest"}}, "DDS artifact digest"),
        ("dds_asset", {"artifact": {"byteLength": 5, "sha256": "0" * 64}}, "DDS artifact digest"),
        ("width", True, "DDS texture dimensions"),
    ],
)
def test_an_invalid_dds_binding_is_refused(
    tmp_path: Path, field: str, value: object, message: str
) -> None:
    with pytest.raises(PackageError, match=message):
        load_package(dds_fixture(tmp_path, **{field: value}))


def test_structure_validation_checks_the_dds_bytes(tmp_path: Path) -> None:
    fixture = dds_fixture(tmp_path)
    report = validate_package(fixture)
    assert report["structure"]["asset_problems"] == []
    (tmp_path / "textures/t0001.dds").write_bytes(b"alter")
    report = validate_package(fixture)
    assert report["structure"]["status"] == "failed"
    assert report["structure"]["asset_problems"] == [
        {"path": "textures/t0001.dds", "detail": "DDS byte binding differs."}
    ]
