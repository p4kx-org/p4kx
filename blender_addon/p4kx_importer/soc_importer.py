# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Creates an optional non-mesh hierarchy from the authored SOC scene."""

import bpy
from mathutils import Matrix

from .package import blender_name
from .soc_scene import SocNode


def import_soc_nodes(root: bpy.types.Collection, nodes: list[SocNode]) -> None:
    """Adds authored SOC metadata empties alongside the package's visible scene."""
    if not nodes:
        return
    collection = bpy.data.collections.new("Authored SOC")
    root.children.link(collection)
    objects = []
    for node in nodes:
        item = bpy.data.objects.new(blender_name(node.label), None)
        collection.objects.link(item)
        if node.parent is not None:
            item.parent = objects[node.parent]
        if node.matrix is not None:
            item.matrix_basis = Matrix((*node.matrix, (0, 0, 0, 1)))
        else:
            item["p4kx_transform"] = "unavailable"
        item["p4kx_soc_kind"] = node.kind
        if node.origin is not None:
            item["p4kx_contains_encrypted_source"] = node.origin
        item.empty_display_type = "PLAIN_AXES"
        item.hide_render = True
        objects.append(item)
