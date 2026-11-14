# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates separate DDNA assets before they are made available to a material."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

import pytest

from p4kx_importer.package import PackageError, parse_manifest
from synthetic import manifest
from synthetic_materials import texture


def _manifest(version: str = "1.17") -> dict[str, Any]:
    def dds_asset(format_name: str) -> dict[str, Any]:
        return {
            "mip_level": 1,
            "dds": {"normalized": {"format": format_name}},
            "artifact": {"byteLength": "5", "sha256": hashlib.sha256(b"image").hexdigest()},
        }

    def dds(number: int, format_name: str) -> dict[str, Any]:
        return {
            **texture(number, "linear", "none"),
            "format": "dds",
            "file": f"textures/t{number:04d}.dds",
            "encoding": "data",
            "dds_asset": dds_asset(format_name),
        }

    normal = dds(2, "BC3_UNORM")
    normal["ddna_pair"] = {
        "smoothness": "t0003",
        "normal_axis": "cryengine_y_down",
        "channel": "red",
        "selected_mip": 1,
    }
    return manifest(version=version, textures=[texture(1, "srgb", "opacity"), normal, dds(3, "BC4_UNORM")])


def test_valid_pair_exposes_its_companion_and_selected_mip(tmp_path: Path) -> None:
    package = parse_manifest(_manifest(), tmp_path)
    pair = package.textures["t0002"].ddna
    assert pair is not None
    assert pair.smoothness == "t0003"
    assert pair.selected_mip == 1
    assert package.textures["t0003"].ddna is None
    assert package.warnings == []


@pytest.mark.parametrize(
    ("field", "value"),
    [("smoothness", "t0002"), ("smoothness", "t9999"), ("selected_mip", 2),
     ("selected_mip", False), ("normal_axis", "y_up"), ("channel", "alpha")],
)
def test_invalid_pair_relationship_is_rejected(tmp_path: Path, field: str, value: Any) -> None:
    data = _manifest()
    data["textures"][1]["ddna_pair"][field] = value
    with pytest.raises(PackageError, match="DDNA"):
        parse_manifest(data, tmp_path)


@pytest.mark.parametrize(
    ("field", "value"),
    [("width", 2), ("alpha", "opacity"), ("encoding", "linear_colour"), ("colour_space", "srgb")],
)
def test_pair_requires_compatible_dimensions_and_data_sampling(
    tmp_path: Path, field: str, value: Any
) -> None:
    data = _manifest()
    data["textures"][2][field] = value
    with pytest.raises(PackageError, match="invalid DDNA companion"):
        parse_manifest(data, tmp_path)


def test_pair_requires_the_documented_format_contract(tmp_path: Path) -> None:
    data = _manifest("1.16")
    with pytest.raises(PackageError, match="DDNA pairs require"):
        parse_manifest(data, tmp_path)
