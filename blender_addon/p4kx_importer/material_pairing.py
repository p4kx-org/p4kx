# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Pairs the manifest's materials of a mesh with the Blender materials its GLB import created."""

from __future__ import annotations

from collections.abc import Sequence

from .glb_material_slots import INDEX_PROPERTY
from .package import pair_materials


def index_of(value: object) -> int | None:
    """Reads the ``p4kx_index`` custom property Blender imported from the glTF extras."""
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value if value >= 0 else None
    if isinstance(value, float) and value.is_integer() and value >= 0:
        return int(value)
    return None


def pair_by_index(
    manifest: Sequence[tuple[int, str]], created: Sequence[tuple[str, int | None]]
) -> dict[int, list[str]]:
    """Groups every Blender material carrying a manifest slot's index.

    Only unindexed materials participate in the legacy name pairing for unmatched slots.
    """
    known = {index for index, _ in manifest}
    pairs: dict[int, list[str]] = {}
    unindexed = []
    for name, index in sorted(created, key=lambda item: item[0]):
        if index is None:
            unindexed.append(name)
        elif index in known:
            pairs.setdefault(index, []).append(name)
    remaining = [(index, name) for index, name in manifest if index not in pairs]
    pairs.update({index: [name] for index, name in pair_materials(remaining, unindexed).items()})
    return pairs
