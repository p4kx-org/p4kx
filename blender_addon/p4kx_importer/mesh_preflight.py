# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates the mesh files used by the package before scene creation."""

from __future__ import annotations

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


def preflight_meshes(package: Package) -> MeshPreflight:
    """Checks every mesh of a supported node without mutating Blender or the package."""
    wanted = sorted({node.mesh for node in package.nodes if node.mesh is not None})
    paths: dict[str, Path] = {}
    problems: list[MeshProblem] = []
    for mesh_id in wanted:
        try:
            paths[mesh_id] = _preflight_mesh(package.root, package.meshes[mesh_id])
        except PackageError as error:
            problems.append(MeshProblem(mesh_id, str(error)))
    return MeshPreflight(paths, tuple(problems))


def _preflight_mesh(root: Path, mesh: Mesh) -> Path:
    path = resolve_file(root, mesh.file)
    if not path.is_file():
        raise PackageError(f"{path.name} is missing or is not a regular file.")
    info = check_glb(path)
    if mesh.bytes and mesh.bytes != info.bytes:
        raise PackageError(f"{path.name}: GLB size does not match the manifest.")
    seen: set[int] = set()
    for material in mesh.materials:
        if material.index >= info.materials:
            raise PackageError(f"{path.name}: manifest material index {material.index} is out of range.")
        if material.index in seen:
            raise PackageError(f"{path.name}: manifest material index {material.index} is duplicated.")
        seen.add(material.index)
    return path
