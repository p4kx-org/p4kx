# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies the ordinal carried by a package GLB's material extra."""

from collections.abc import Iterable
from typing import Any

INDEX_PROPERTY = "p4kx_index"


def material_slot_issue(materials: Iterable[dict[str, Any]]) -> str | None:
    """Returns the first explicitly tagged material that names a different slot."""
    for index, material in enumerate(materials):
        extras = material.get("extras")
        if not isinstance(extras, dict) or INDEX_PROPERTY not in extras:
            continue
        value = extras[INDEX_PROPERTY]
        if type(value) is not int or value != index:
            return f"GLB materials[{index}].extras.{INDEX_PROPERTY} must equal {index}."
    return None
