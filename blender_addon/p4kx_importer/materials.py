# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Builds the Blender materials of an imported mesh from the package's descriptions (ADR-0100).

The mapping from a description to nodes is ``material_graph``; this module only creates what that
mapping describes, and the basic setup for materials without a description.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import TYPE_CHECKING, Any

import bpy

from .graph_groups import group_spec
from .graph_spec import GraphSpec, Socket
from .material_graph import BuildContext, build_graph
from .material_basic import basic_description
from .material_model import Palette
from .material_pairing import INDEX_PROPERTY, index_of, pair_by_index
from .package import Mesh, Package, PackageError, Texture, resolve_file
from .package_digests import file_sha256

if TYPE_CHECKING:
    from .package import MeshMaterial

_COLOUR_SPACE_NAMES = {
    "srgb": "sRGB",
    "linear": "Non-Color",
    "data": "Non-Color",
    "linear_colour": "Linear Rec.709",
}
_ALPHA_MODES = {"opacity": "STRAIGHT", "smoothness": "CHANNEL_PACKED", "none": "NONE"}
_COLUMN = 280.0
_ROW = 260.0


class ImageCache:
    """Loads each texture of a package once and sets its colour space and alpha handling."""

    def __init__(self, package: Package) -> None:
        self._package = package
        self._images: dict[str, bpy.types.Image | None] = {}

    def get(self, texture_id: str) -> bpy.types.Image | None:
        """Returns the image of a texture, or ``None`` when its file cannot be loaded."""
        if texture_id not in self._images:
            texture = self._package.textures.get(texture_id)
            self._images[texture_id] = self._load(texture) if texture else None
        return self._images[texture_id]

    def loads(self, texture_id: str) -> bool:
        """Returns whether the file of a texture loads; loads it on the first call."""
        return self.get(texture_id) is not None

    def _load(self, texture: Texture) -> bpy.types.Image | None:
        try:
            path = resolve_file(self._package.root, texture.file)
            if texture.format == "dds" and (
                path.stat().st_size != texture.byte_length or file_sha256(path) != texture.sha256
            ):
                raise PackageError("DDS file differs from its package byte binding.")
            image = bpy.data.images.load(str(path), check_existing=True)
            if texture.format == "dds":
                pixels = image.pixels[:4]
                if len(pixels) != 4 or tuple(image.size) != texture.dimensions or not image.has_data:
                    raise PackageError("Blender cannot decode the declared DDS mip.")
        except (OSError, PackageError, RuntimeError):
            self._package.warnings.append(f"Texture {texture.id} could not be loaded.")
            return None
        image.colorspace_settings.name = _COLOUR_SPACE_NAMES[texture.encoding or texture.colour_space]
        image.alpha_mode = _ALPHA_MODES[texture.alpha]
        return image


def apply_materials(
    created: Iterable[bpy.types.Material], package: Package, mesh: Mesh, images: ImageCache
) -> None:
    """Builds the node trees of the glTF materials one mesh import created.

    A glTF material is paired with the mesh's entry by its ``p4kx_index`` custom property, else
    by name. An entry with a description gets the node tree of that description; an entry
    without one, or a 1.0 package, gets the basic setup from the textures of the entry.
    """
    materials = {material.name: material for material in created}
    pairs = pair_by_index(
        [(entry.index, entry.name) for entry in mesh.materials],
        [(name, index_of(m.get(INDEX_PROPERTY))) for name, m in materials.items()],
    )
    context = _context(
        package, images, package.palettes.get(mesh.palette_id) if mesh.palette_id else None
    )
    for entry in mesh.materials:
        blender = materials.get(pairs.get(entry.index, ""))
        if blender is None:
            continue
        description = package.material_descriptions.get(entry.material or "")
        if description is not None:
            spec = build_graph(description, context)
            for warning in spec.warnings:
                if warning not in package.warnings:
                    package.warnings.append(warning)
            build_tree(blender, spec, images)
        elif entry.textures:
            apply_textures(blender, entry, package, images)


def build_tree(material: bpy.types.Material, spec: GraphSpec, images: ImageCache) -> None:
    """Replaces the node tree of ``material`` with the nodes and links of ``spec``."""
    if material.node_tree is None:
        material.use_nodes = True
    tree = material.node_tree
    tree.nodes.clear()
    _populate(tree, spec, images)
    material.use_backface_culling = not spec.two_sided
    material.surface_render_method = "BLENDED" if spec.alpha_mode == "blend" else "DITHERED"
    for name, value in spec.properties.items():
        material[name] = value


def ensure_group(name: str, images: ImageCache) -> bpy.types.ShaderNodeTree:
    """Returns the shared node group ``name``, creating it on the first use in the file."""
    group = bpy.data.node_groups.get(name)
    if group is not None:
        return group
    spec = group_spec(name)
    group = bpy.data.node_groups.new(name, "ShaderNodeTree")
    for socket_name, socket_type, default in spec.inputs:
        item = group.interface.new_socket(socket_name, in_out="INPUT", socket_type=socket_type)
        item.default_value = default
    for socket_name, socket_type in spec.outputs:
        group.interface.new_socket(socket_name, in_out="OUTPUT", socket_type=socket_type)
    _populate(group, spec.graph, images)
    return group


def _populate(tree: bpy.types.NodeTree, spec: GraphSpec, images: ImageCache) -> None:
    nodes: dict[str, bpy.types.Node] = {}
    for node_spec in spec.nodes:
        node = tree.nodes.new(node_spec.kind)
        for name, value in node_spec.props.items():
            setattr(node, name, value)
        if node_spec.texture:
            node.image = images.get(node_spec.texture)
        if node_spec.group:
            node.node_tree = ensure_group(node_spec.group, images)
        nodes[node_spec.key] = node
    for node_spec in spec.nodes:
        for socket, value in node_spec.inputs.items():
            _set_default(_socket(nodes[node_spec.key].inputs, socket), value)
    for link in spec.links:
        source = _socket(nodes[link.source.key].outputs, link.source.socket)
        target = _socket(nodes[link.target.key].inputs, link.target.socket)
        tree.links.new(source, target)
    _layout(nodes, spec)


def _socket(sockets: Any, selector: Socket) -> bpy.types.NodeSocket:
    if isinstance(selector, int):
        return sockets[selector]
    for socket in sockets:
        if socket.name == selector and socket.enabled:
            return socket
    return sockets[selector]


def _set_default(socket: bpy.types.NodeSocket, value: Any) -> None:
    current = getattr(socket, "default_value", None)
    if isinstance(value, tuple) and current is not None and hasattr(current, "__len__"):
        size = len(current)
        value = (*value, 1.0)[:size] if size == 4 else value[:size]
    try:
        socket.default_value = value
    except (TypeError, ValueError):
        return


def _layout(nodes: dict[str, bpy.types.Node], spec: GraphSpec) -> None:
    depth = dict.fromkeys(nodes, 0)
    for _ in range(len(nodes)):
        changed = False
        for link in spec.links:
            wanted = depth[link.source.key] + 1
            if depth[link.target.key] < wanted:
                depth[link.target.key] = wanted
                changed = True
        if not changed:
            break
    rows: dict[int, int] = {}
    for key, node in nodes.items():
        column = depth[key]
        row = rows.get(column, 0)
        rows[column] = row + 1
        node.location = (column * _COLUMN, -row * _ROW)


def apply_textures(
    material: bpy.types.Material, described: MeshMaterial, package: Package, images: ImageCache
) -> None:
    """Connects the textures of ``described`` to the Principled BSDF of ``material``.

    This is the basic setup of format 1.0 packages and of materials without a description. It
    is the graph of a neutral ``Illum`` material with those textures, so a specular map follows
    the same rule as everywhere else.
    """
    spec = build_graph(basic_description(described, package), _context(package, images))
    build_tree(material, spec, images)


def _context(package: Package, images: ImageCache, palette: Palette | None = None) -> BuildContext:
    return BuildContext(
        package.material_descriptions,
        palette,
        {texture.id: texture.alpha for texture in package.textures.values()},
        images.loads,
    )
