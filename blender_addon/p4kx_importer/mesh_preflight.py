# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates the mesh files used by the package before scene creation."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from .glb_preflight import check_glb
from .manifest_fields import PackageError
from .package_paths import resolve_file

if TYPE_CHECKING:
    from .package import Mesh, Package


@dataclass(frozen=True, slots=True)
class MeshProblem:
    """One mesh the preflight could not validate."""

    mesh_id: str
    detail: str

    def message(self) -> str:
        """Returns the package warning for this mesh."""
        return f"Mesh {self.mesh_id} could not be imported. {self.detail}"


@dataclass(frozen=True, slots=True)
class MeshPreflight:
    """Validated paths and problems for the meshes the add-on uses."""

    paths: dict[str, Path]
    problems: tuple[MeshProblem, ...]
    incomplete_counts: tuple[str, ...]


def preflight_meshes(package: Package) -> MeshPreflight:
    """Checks every mesh of a supported node without mutating Blender or the package."""
    wanted = {node.mesh for node in package.nodes if node.mesh is not None}
    return _preflight(package, wanted)


def preflight_all_meshes(package: Package) -> MeshPreflight:
    """Checks every mesh declared in the package."""
    return _preflight(package, package.meshes)


def _preflight(package: Package, mesh_ids: Iterable[str]) -> MeshPreflight:
    paths: dict[str, Path] = {}
    problems: list[MeshProblem] = []
    incomplete: list[str] = []
    for mesh_id in sorted(mesh_ids):
        try:
            path, counts_complete = _preflight_mesh(package.root, package.meshes[mesh_id])
            paths[mesh_id] = path
            if not counts_complete:
                incomplete.append(mesh_id)
        except PackageError as error:
            problems.append(MeshProblem(mesh_id, str(error)))
    return MeshPreflight(paths, tuple(problems), tuple(incomplete))


def _preflight_mesh(root: Path, mesh: Mesh) -> tuple[Path, bool]:
    path = resolve_file(root, mesh.file)
    if not path.is_file():
        raise PackageError(f"{path.name} is missing or is not a regular file.")
    info = check_glb(path)
    if mesh.rig is None and info.material_slot_issue is not None:
        raise PackageError(f"{path.name}: {info.material_slot_issue}")
    if mesh.bytes and mesh.bytes != info.bytes:
        raise PackageError(f"{path.name}: GLB size does not match the manifest.")
    if info.geometry is not None:
        _check_count(path, "vertex", mesh.vertices, info.geometry.vertices)
        _check_count(path, "triangle", mesh.triangles, info.geometry.triangles)
    seen: set[int] = set()
    for material in mesh.materials:
        if material.index >= info.materials:
            raise PackageError(f"{path.name}: manifest material index {material.index} is out of range.")
        if material.index in seen:
            raise PackageError(f"{path.name}: manifest material index {material.index} is duplicated.")
        seen.add(material.index)
    counts_complete = (
        info.geometry is not None and mesh.vertices is not None and mesh.triangles is not None
    )
    return path, counts_complete


def _check_count(path: Path, name: str, declared: int | None, actual: int) -> None:
    if declared is not None and declared != actual:
        raise PackageError(f"{path.name}: GLB {name} count {actual} differs from manifest count {declared}.")
