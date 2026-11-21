# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validate and consume separately stored DDNA normal and smoothness samples."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

from .manifest_fields import PackageError, read_object, read_text

if TYPE_CHECKING:
    from .package import Texture

MINOR = 17


@dataclass(frozen=True, slots=True)
class Pair:
    """Association of a raw normal image with its same-mip smoothness image."""

    smoothness: str
    selected_mip: int


def pairing(
    entry: dict[str, Any], where: str, version: tuple[int, int]
) -> Pair | None:
    """Returns the companion ID and common mip of a declared DDNA sample pair."""
    raw = entry.get("ddna_pair")
    if raw is None:
        return None
    if version < (1, MINOR):
        raise PackageError(f"{where}: DDNA pairs require format 1.{MINOR}.")
    pair = read_object(raw, f"{where}.ddna_pair")
    if pair.get("normal_axis") != "cryengine_y_down" or pair.get("channel") != "red":
        raise PackageError(f"{where}: unsupported DDNA sample convention.")
    selected_mip = pair.get("selected_mip")
    if type(selected_mip) is not int or selected_mip < 0:
        raise PackageError(f"{where}: invalid DDNA sample mip.")
    return Pair(read_text(pair, "smoothness", f"{where}.ddna_pair"), selected_mip)


def validate_pairs(textures: Mapping[str, Texture]) -> None:
    """Checks pair topology and matching image types before material use."""
    for primary in textures.values():
        if (pair := primary.ddna) is None:
            continue
        companion = textures.get(pair.smoothness)
        if (
            companion is None
            or companion.id == primary.id
            or companion.ddna is not None
            or not _compatible(primary, companion, pair.selected_mip)
        ):
            raise PackageError(f"Texture {primary.id} has an invalid DDNA companion.")


def _compatible(primary: Texture, companion: Texture, selected_mip: int) -> bool:
    return (
        primary.format == companion.format == "dds"
        and primary.dimensions == companion.dimensions
        and primary.encoding == companion.encoding == "data"
        and primary.colour_space == companion.colour_space == "linear"
        and primary.alpha == companion.alpha == "none"
        and primary.dds_pixel_format == "BC3_UNORM"
        and companion.dds_pixel_format == "BC4_UNORM"
        and primary.dds_mip == companion.dds_mip == selected_mip
    )
