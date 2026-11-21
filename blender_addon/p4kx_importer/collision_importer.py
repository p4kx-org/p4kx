# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Creates a nonrendering collection of source collision helper meshes."""

from __future__ import annotations

import bpy

from .collision_helpers import Helpers, Mesh


def import_helpers(
    root: bpy.types.Collection,
    document: Helpers | None,
    owners: dict[int, bpy.types.Object],
) -> int:
    """Adds helpers with shared data and original affine geometry."""
    if document is None or not any(document.models):
        return 0
    eligible = [
        instance for instance in document.instances
        if instance.cdf_resolved and document.models[instance.model]
    ]
    if not eligible:
        return 0
    collection = bpy.data.collections.new("P4KX Collision Helpers")
    collection.hide_render = True
    root.children.link(collection)
    created: dict[tuple[int, int], bpy.types.Mesh] = {}
    count = 0
    for instance in eligible:
        for index, collider in enumerate(document.models[instance.model]):
            key = (instance.model, index)
            mesh = created.get(key)
            if mesh is None:
                mesh = _mesh(collider)
                created[key] = mesh
            helper = bpy.data.objects.new(mesh.name, mesh)
            helper.parent = owners[instance.node]
            helper.display_type = "WIRE"
            helper.hide_render = True
            helper["p4kx_role"] = "collision_helper"
            helper["p4kx_source"] = collider.source
            helper["p4kx_chunk"] = collider.chunk
            helper["p4kx_slot"] = collider.slot
            helper["p4kx_packed_key"] = str(collider.packed_key)
            marker = _choose_marker(collider.encrypted, instance.cdf_encrypted)
            if marker is not None:
                helper["p4kx_contains_encrypted_source"] = marker
            collection.objects.link(helper)
            count += 1
    return count


def _mesh(collider: Mesh) -> bpy.types.Mesh:
    # The affine acts on source Z-up coordinates before Blender reads the vertex data.
    rows = collider.frame
    vertices = [
        tuple(sum(row[k] * position[k] for k in range(3)) + row[3] for row in rows)
        for position in collider.positions
    ]
    mesh = bpy.data.meshes.new(f"Collision {collider.chunk}:{collider.slot}")
    mesh.from_pydata(vertices, [], collider.indices)
    mesh.update()
    return mesh


def _choose_marker(mesh: bool | None, placement: bool | None) -> bool | None:
    if mesh is True or placement is True:
        return True
    return None if mesh is None or placement is None else False
