# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""The rig record of a package mesh and its validation, without Blender."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

MAX_RIG_ITEMS = 100_000


class RigError(ValueError):
    """A rig record is malformed."""


@dataclass(frozen=True, slots=True)
class RigClip:
    """An animation of a Character as its GLB file holds it."""

    name: str
    duration_seconds: float
    channels: int


@dataclass(frozen=True, slots=True)
class RigPart:
    """A skin worn on the skeleton: its node name in the GLB, its origin and its morph targets."""

    name: str
    origin: str
    morph_targets: int


@dataclass(frozen=True, slots=True)
class Rig:
    """What a rigged GLB file holds: bones, skinned parts, morph targets and animations.

    ``body`` is the Character's body variant; a creature has none.
    """

    body: str | None
    bones: int
    skinned_meshes: int
    morph_targets: int
    parts: tuple[RigPart, ...]
    animations: tuple[RigClip, ...]


def parse_rig(value: object, where: str) -> Rig:
    """Returns the rig of a mesh; raises ``RigError`` for a record with invalid values."""
    entry = _object(value, where)
    parts = tuple(
        _part(item, f"{where}.parts[{position}]")
        for position, item in enumerate(_array(entry.get("parts"), f"{where}.parts"))
    )
    animations = tuple(
        _clip(item, f"{where}.animations[{position}]")
        for position, item in enumerate(_array(entry.get("animations"), f"{where}.animations"))
    )
    body, skeleton = entry.get("body"), entry.get("skeleton")
    if (body is not None and not isinstance(body, str)) or not isinstance(skeleton, str):
        raise RigError(f"{where}: body, when present, and skeleton must be strings.")
    _count(entry, "embedded_textures", where)
    return Rig(
        body=body,
        bones=_count(entry, "bones", where),
        skinned_meshes=_count(entry, "skinned_meshes", where),
        morph_targets=_count(entry, "morph_targets", where),
        parts=parts,
        animations=animations,
    )


def _part(item: object, where: str) -> RigPart:
    entry = _object(item, where)
    name, origin, file = entry.get("name"), entry.get("origin"), entry.get("file")
    if not isinstance(name, str) or not isinstance(origin, str) or not isinstance(file, str):
        raise RigError(f"{where}: name, origin and file must be strings.")
    return RigPart(name, origin, _count(entry, "morph_targets", where))


def _clip(item: object, where: str) -> RigClip:
    entry = _object(item, where)
    name, seconds = entry.get("name"), entry.get("duration_seconds")
    if not isinstance(name, str):
        raise RigError(f"{where}: name must be a string.")
    if not isinstance(seconds, (int, float)) or isinstance(seconds, bool) or not 0.0 <= seconds < 1e9:
        raise RigError(f"{where}: duration_seconds must be a finite non-negative number.")
    if not isinstance(entry.get("additive"), bool):
        raise RigError(f"{where}: additive must be a boolean.")
    _count(entry, "keys", where)
    return RigClip(name, float(seconds), _count(entry, "channels", where))


def _count(entry: dict[str, Any], key: str, where: str) -> int:
    value = entry.get(key)
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= 2**31:
        raise RigError(f"{where}: {key} must be a non-negative integer.")
    return value


def _object(value: object, where: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise RigError(f"{where} must be an object.")
    return value


def _array(value: object, where: str) -> list[Any]:
    if not isinstance(value, list):
        raise RigError(f"{where} must be an array.")
    if len(value) > MAX_RIG_ITEMS:
        raise RigError(f"{where} has more than {MAX_RIG_ITEMS} entries.")
    return value
