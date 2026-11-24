# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Reads the optional Character body zone and worn-part maps."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, ClassVar, Literal

from .manifest_fields import PackageError, read_array, read_object, read_text
from .package_digests import file_sha256, is_sha256
from .source_evidence import SourceEvidence, follows_pointer

VERSION = (1, 24)
_GUID = re.compile(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", re.IGNORECASE)
_U32_MAX = (1 << 32) - 1


@dataclass(frozen=True, slots=True)
class BodyZonePrimitive:
    """A visible body zone at an exact mesh and primitive in the GLB."""

    zone: str
    mesh: int
    primitive: int
    model_lod: int


@dataclass(frozen=True, slots=True)
class VerifiedBodyZones:
    """The visible body primitives and the zones omitted in this GLB."""

    state: ClassVar[Literal["verified"]] = "verified"
    primitives: tuple[BodyZonePrimitive, ...]
    omitted: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class UnavailableBodyZones:
    """The recorded reason the body geometry could not be mapped."""

    state: ClassVar[Literal["unavailable"]] = "unavailable"
    reason: str


@dataclass(frozen=True, slots=True)
class ClothingBodyCoverage:
    """The coverage supplied by a worn item's physical DataForge Record."""

    guid: str
    record_index: int | None
    record_path: str
    chunks_complete: bool
    covers: tuple[str, ...]
    conditional: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class PartBodyZones:
    """A worn rig part's meshes and optional authored coverage."""

    part: int
    meshes: tuple[int, ...]
    coverage: ClothingBodyCoverage | None


@dataclass(frozen=True, slots=True)
class BodyZoneMap:
    """The body zones and emitted worn parts of a Character rig."""

    body: VerifiedBodyZones | UnavailableBodyZones
    parts: tuple[PartBodyZones, ...]
    glb_sha256: str

    def check_artifact(self, path: Path, verified_sha256: str | None) -> None:
        """Checks the GLB digest or reuses a digest verified against the same file."""
        try:
            digest = file_sha256(path) if verified_sha256 is None else verified_sha256
        except OSError as error:
            raise PackageError("Character body zone GLB digest cannot be read.") from error
        if digest != self.glb_sha256:
            raise PackageError("Character body zone GLB SHA-256 differs.")

    def check_glb(self, primitive_counts: tuple[int, ...]) -> None:
        """Checks that the declared mesh and primitive addresses exist in the GLB."""
        if isinstance(self.body, VerifiedBodyZones):
            for row in self.body.primitives:
                if row.mesh >= len(primitive_counts) or row.primitive >= primitive_counts[row.mesh]:
                    raise PackageError(
                        f"Body zone {row.zone!r} references an out-of-range GLB primitive "
                        f"({row.mesh}, {row.primitive})."
                    )
        for row in self.parts:
            if any(mesh >= len(primitive_counts) for mesh in row.meshes):
                raise PackageError(f"Worn rig part {row.part} references an out-of-range GLB mesh.")


def read(value: object, where: str, part_origins: tuple[str, ...]) -> BodyZoneMap:
    """Reads the Character map and checks manifest-local ownership references."""
    entry = read_object(value, where)
    digest = entry.get("glb_sha256")
    if not is_sha256(digest):
        raise PackageError(f"{where}: glb_sha256 must be a lowercase SHA-256 digest.")
    body = _body(entry.get("body"), f"{where}.body")
    parts: list[PartBodyZones] = []
    seen_parts: set[int] = set()
    seen_meshes = {row.mesh for row in body.primitives} if isinstance(body, VerifiedBodyZones) else set()
    for index, item in enumerate(read_array(entry.get("parts"), f"{where}.parts")):
        part_where = f"{where}.parts[{index}]"
        row = _part(item, part_where)
        if row.part >= len(part_origins) or part_origins[row.part] != "piece":
            raise PackageError(f"{part_where}: part must reference a worn rig piece.")
        if row.part in seen_parts:
            raise PackageError(f"{part_where}: duplicate worn rig part.")
        if seen_meshes.intersection(row.meshes) or len(set(row.meshes)) != len(row.meshes):
            raise PackageError(f"{part_where}: a GLB mesh is assigned more than once.")
        seen_parts.add(row.part)
        seen_meshes.update(row.meshes)
        parts.append(row)
    _check_origins(entry, where)
    return BodyZoneMap(body, tuple(parts), digest)


def _check_origins(entry: dict[str, Any], where: str) -> None:
    if "source_inventory" not in entry and "field_origins" not in entry:
        return
    evidence = SourceEvidence.read(entry.get("source_inventory"), f"{where} source")
    fields = read_object(entry.get("field_origins"), f"{where}.field_origins")
    payload = {"body": entry["body"], "parts": entry["parts"]}
    for pointer, origin in fields.items():
        if not isinstance(pointer, str) or not pointer or not follows_pointer(payload, pointer):
            raise PackageError(f"{where}: a body zone field origin has an unresolved JSON Pointer.")
        evidence.check_origin(origin)


def _body(value: object, where: str) -> VerifiedBodyZones | UnavailableBodyZones:
    entry = read_object(value, where)
    state = entry.get("state")
    if state == "unavailable":
        return UnavailableBodyZones(_nonempty_text(entry, "reason", where))
    if state != "verified":
        raise PackageError(f"{where}: body zone state must be verified or unavailable.")
    primitives = tuple(
        _primitive(item, f"{where}.primitives[{index}]")
        for index, item in enumerate(read_array(entry.get("primitives"), f"{where}.primitives"))
    )
    omitted = _zone_names(entry.get("omitted"), f"{where}.omitted")
    if len({(row.mesh, row.primitive) for row in primitives}) != len(primitives):
        raise PackageError(f"{where}: duplicate GLB primitive address.")
    if {row.zone for row in primitives}.intersection(omitted):
        raise PackageError(f"{where}: an omitted zone also has emitted primitives.")
    return VerifiedBodyZones(primitives, omitted)


def _primitive(value: object, where: str) -> BodyZonePrimitive:
    entry = read_object(value, where)
    return BodyZonePrimitive(
        _nonempty_text(entry, "zone", where),
        _unsigned(entry.get("mesh"), f"{where}.mesh"),
        _unsigned(entry.get("primitive"), f"{where}.primitive"),
        _unsigned(entry.get("model_lod"), f"{where}.model_lod", _U32_MAX),
    )


def _part(value: object, where: str) -> PartBodyZones:
    entry = read_object(value, where)
    meshes = tuple(
        _unsigned(mesh, f"{where}.meshes[{index}]")
        for index, mesh in enumerate(read_array(entry.get("meshes"), f"{where}.meshes"))
    )
    if not meshes:
        raise PackageError(f"{where}: a worn part needs at least one GLB mesh.")
    if "coverage" not in entry:
        raise PackageError(f"{where}: coverage must be an object or null.")
    coverage = entry["coverage"]
    return PartBodyZones(
        _unsigned(entry.get("part"), f"{where}.part"),
        meshes,
        None if coverage is None else _coverage(coverage, f"{where}.coverage"),
    )


def _coverage(value: object, where: str) -> ClothingBodyCoverage:
    entry = read_object(value, where)
    guid = read_text(entry, "guid", where)
    if _GUID.fullmatch(guid) is None:
        raise PackageError(f"{where}: guid must be a DataForge Record GUID.")
    record_index = entry.get("record_index")
    chunks_complete = entry.get("chunks_complete")
    if type(chunks_complete) is not bool:
        raise PackageError(f"{where}: chunks_complete must be a boolean.")
    covers = _zone_names(entry.get("covers"), f"{where}.covers")
    conditional = _zone_names(entry.get("conditional"), f"{where}.conditional")
    if set(covers).intersection(conditional):
        raise PackageError(f"{where}: a zone cannot be covered both unconditionally and conditionally.")
    return ClothingBodyCoverage(
        guid,
        None if record_index is None else _unsigned(record_index, f"{where}.record_index", _U32_MAX),
        _nonempty_text(entry, "record_path", where),
        chunks_complete,
        covers,
        conditional,
    )


def _zone_names(value: object, where: str) -> tuple[str, ...]:
    names = read_array(value, where)
    if any(not isinstance(name, str) or not name for name in names):
        raise PackageError(f"{where}: zone names must be non-empty strings.")
    if len(set(names)) != len(names):
        raise PackageError(f"{where}: duplicate zone name.")
    return tuple(names)


def _nonempty_text(entry: dict[str, Any], key: str, where: str) -> str:
    value = read_text(entry, key, where)
    if not value:
        raise PackageError(f"{where}: {key} must be non-empty.")
    return value


def _unsigned(value: object, where: str, maximum: int | None = None) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise PackageError(f"{where} must be a non-negative integer.")
    if maximum is not None and value > maximum:
        raise PackageError(f"{where} exceeds {maximum}.")
    return value
