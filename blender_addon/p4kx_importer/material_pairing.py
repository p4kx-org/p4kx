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
) -> dict[int, str]:
    """Pairs manifest ``(index, name)`` entries with created ``(name, p4kx_index)`` materials.

    A created material that carries ``p4kx_index`` pairs with the manifest entry of that index.
    The rest are paired by name, as ``pair_materials`` does, among the entries still unpaired.
    """
    known = {index for index, _ in manifest}
    pairs: dict[int, str] = {}
    for name, index in sorted(created, key=lambda item: (item[0], -1 if item[1] is None else item[1])):
        if index in known and index not in pairs:
            pairs[index] = name
    taken = set(pairs.values())
    rest_manifest = [(index, name) for index, name in manifest if index not in pairs]
    rest_created = [name for name, _ in created if name not in taken]
    pairs.update(pair_materials(rest_manifest, rest_created))
    return pairs
