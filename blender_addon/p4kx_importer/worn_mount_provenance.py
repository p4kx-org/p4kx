# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates field origins for a character's mounted GLB roots."""

from __future__ import annotations

import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .manifest_fields import PackageError, read_object
from .package_digests import file_sha256, is_sha256
from .source_evidence import SourceEvidence

VERSION = (1, 21)
_INDEX = r"(0|[1-9][0-9]{0,9})"
_TRANSFORMS = ("translation", "rotation", "scale")
_FIELD = re.compile(rf"/nodes/{_INDEX}/(?:children/{_INDEX}|({'|'.join(_TRANSFORMS)}))")
_BYTE_LENGTH = re.compile(r"[1-9][0-9]{0,19}")


@dataclass(frozen=True, slots=True)
class WornMountProvenance:
    """One verified origin inventory's addresses in a bound GLB."""

    glb: str
    byte_length: int
    sha256: str
    children: tuple[tuple[int, int], ...]
    transforms: tuple[tuple[int, str], ...]

    def check_mesh(self, glb: str, version: tuple[int, int]) -> None:
        """Checks the package version and declared mesh identity."""
        if version < VERSION or self.glb != glb:
            raise PackageError("Worn mount provenance has a different GLB or package version.")

    def check_artifact(self, path: Path, length: int) -> None:
        """Checks the measured GLB byte length and digest."""
        try:
            if length != self.byte_length or file_sha256(path) != self.sha256:
                raise PackageError("Worn mount provenance GLB length or SHA-256 differs.")
        except OSError as error:
            raise PackageError("Worn mount provenance GLB digest cannot be read.") from error

    def inspect(self, document: dict[str, Any], *, bones: int) -> None:
        """Checks rigid roots and transforms after GLB reference validation."""
        nodes = document.get("nodes", [])
        first_bone = len(nodes) - bones
        skins = document.get("skins", [])
        joints = {index for skin in skins for index in skin.get("joints", [])}
        # The rig writer emits bone nodes after all model nodes, including without a skin.
        if not 0 <= first_bone < len(nodes) or (skins and (len(joints) != bones or min(joints) != first_bone)):
            raise PackageError("Worn mount provenance bone count disagrees with the GLB rig.")
        mounted: set[int] = set()
        for parent, slot in self.children:
            if not first_bone <= parent < len(nodes):
                raise PackageError("Worn mount provenance parent is not a GLB bone.")
            children = nodes[parent].get("children", [])
            if slot >= len(children):
                raise PackageError("Worn mount provenance child position is outside the GLB bone.")
            child = children[slot]
            if child >= first_bone or child in mounted or "skin" in nodes[child]:
                raise PackageError("Worn mount provenance child is not a unique rigid GLB root.")
            if "matrix" in nodes[child]:
                raise PackageError("Worn mount provenance root uses a matrix instead of local transforms.")
            mounted.add(child)
        _check_parents(document, nodes, mounted)
        emitted = {(node, field) for node in mounted for field in _TRANSFORMS if field in nodes[node]}
        proved = set(self.transforms)
        if proved - emitted:
            raise PackageError("Worn mount provenance transform is absent from its GLB root.")
        if emitted - proved:
            raise PackageError("Worn mount provenance is missing an emitted GLB root transform.")


def _check_parents(document: dict[str, Any], nodes: list[dict[str, Any]], mounted: set[int]) -> None:
    counts = Counter(child for node in nodes for child in node.get("children", []) if child in mounted)
    scene_roots = (root for scene in document.get("scenes", []) for root in scene.get("nodes", []))
    if any(count != 1 for count in counts.values()) or any(root in mounted for root in scene_roots):
        raise PackageError("Worn mount provenance child has ambiguous GLB parentage.")


def read(value: object) -> WornMountProvenance | None:
    """Reads the optional carrier and checks its immediate field origins."""
    if value is None:
        return None
    section = read_object(value, "worn mount provenance")
    glb = section.get("glb")
    artifact = read_object(section.get("artifact"), "worn mount artifact")
    length, digest = artifact.get("byteLength"), artifact.get("sha256")
    if (
        not isinstance(glb, str) or not glb
        or not isinstance(length, str) or not _BYTE_LENGTH.fullmatch(length) or int(length) >= 1 << 64
        or not is_sha256(digest)
    ):
        raise PackageError("Worn mount provenance has an invalid GLB binding.")
    evidence = SourceEvidence.read(section.get("sourceInventory"), "worn mount")
    fields = read_object(section.get("fieldOrigins"), "worn mount field origins")
    children, transforms = [], []
    for pointer, origin in fields.items():
        match = _FIELD.fullmatch(pointer) if isinstance(pointer, str) else None
        if match is None:
            raise PackageError("Worn mount provenance has an invalid GLB field address.")
        node, child, transform = match.groups()
        if child is not None:
            children.append((int(node), int(child)))
        else:
            transforms.append((int(node), transform))
        evidence.check_origin(origin)
    if not children:
        raise PackageError("Worn mount provenance has no addressed rigid GLB root.")
    return WornMountProvenance(glb, int(length), digest, tuple(children), tuple(transforms))
