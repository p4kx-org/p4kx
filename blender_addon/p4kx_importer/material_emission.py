# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Reads the explicit static emission preview without applying runtime Glow links."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class StaticEmission:
    """The selected material texture and its authored linear colour and strength."""

    slot: str
    file: str
    tint: tuple[float, float, float]
    strength: float


def parse_emission(value: object) -> StaticEmission | None:
    """Returns the full authored-input preview if all of its fields are valid."""
    if not isinstance(value, dict) or value.get("source_complete") is not True:
        return None
    if value.get("emittance_requested") is not True:
        return None
    preview: Any = value.get("preview")
    if not isinstance(preview, dict):
        return None
    slot = preview.get("slot")
    file = preview.get("file")
    tint = preview.get("tint")
    strength = preview.get("strength")
    if not isinstance(slot, str) or not slot or not isinstance(file, str) or not file:
        return None
    if not _finite(strength) or strength <= 0 or not isinstance(tint, list) or len(tint) != 3:
        return None
    if any(not _finite(channel) or channel < 0 for channel in tint):
        return None
    red, green, blue = tint
    return StaticEmission(slot, file, (red, green, blue), strength)


def _finite(value: object) -> bool:
    return type(value) in (int, float) and -3.4028234663852886e38 <= value <= 3.4028234663852886e38 and math.isfinite(value)
