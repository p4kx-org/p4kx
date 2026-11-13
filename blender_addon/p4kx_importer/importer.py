# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Builds Blender objects from a validated package: shared meshes once, nodes as instances."""

from __future__ import annotations

from contextlib import AbstractContextManager, nullcontext
from dataclasses import dataclass, field
from pathlib import Path

import bpy
from mathutils import Quaternion, Vector

from .glass_tint import drop_tint_without_colours
from .lights import gltf_to_blender_light_quaternion, light_settings
from .material_surface import GLTF_COLOUR_ATTRIBUTE
from .mesh_preflight import preflight_meshes
from .materials import ImageCache, apply_materials
from .light_model import Light
from .package import (
    Mesh,
    Node,
    Package,
    PackageError,
    blender_name,
    gltf_to_blender_quaternion,
    gltf_to_blender_scale,
    gltf_to_blender_vector,
)


from . import soc_scene
from .soc_importer import import_soc_nodes


@dataclass(slots=True)
class ImportResult:
    """What an import created, for the report to the user."""

    root: bpy.types.Collection
    objects: int = 0
    instances: int = 0
    lights: int = 0
    meshes: int = 0
    armatures: int = 0
    shape_keys: int = 0
    actions: int = 0
    warnings: list[str] = field(default_factory=list)


def import_package(
    context: bpy.types.Context, package: Package, light_strength: float = 1.0
) -> ImportResult:
    """Imports ``package`` into the scene of ``context`` and returns what was created.

    Each mesh used by a node is imported once into a hidden template collection; every node that
    draws it becomes an empty that instances that collection. A node of the kind ``light`` becomes
    a Blender light with the settings of ``lights.light_settings`` at ``light_strength``. A node
    whose mesh has a ``rig`` is a Character or a creature: its GLB is imported once for that
    node, not instanced, so that its armature, shape keys and actions can be posed and animated,
    and the imported top-level objects become children of the node's empty. Raises
    ``PackageError`` when the glTF importer of Blender is not available.
    """
    if not hasattr(bpy.ops.import_scene, "gltf"):
        raise PackageError("Enable Blender's 'glTF 2.0 format' add-on first.")
    authored = soc_scene.load(package.root, package.authored_soc) if package.authored_soc else []
    preflight = preflight_meshes(package)
    root = bpy.data.collections.new(blender_name(package.entity_name))
    context.scene.collection.children.link(root)
    result = ImportResult(root=root)
    result.warnings.extend(problem.message() for problem in preflight.problems)
    templates = _import_meshes(context, package, root, result, preflight.paths)
    created: dict[int, bpy.types.Object] = {}
    for node in package.nodes:
        template = templates.get(node.mesh) if node.mesh else None
        if node.light is not None:
            created[node.index] = _create_light(node, node.light, light_strength, created, root)
            result.lights += 1
        else:
            created[node.index] = _create_object(node, template, created, root)
            mesh = package.meshes[node.mesh] if node.mesh else None
            if mesh is not None and mesh.rig is not None and mesh.id in preflight.paths:
                _import_rigged(context, mesh, preflight.paths[mesh.id], created[node.index], root, result)
        result.objects += 1
        result.instances += template is not None
    import_soc_nodes(root, authored)
    result.warnings.extend(package.warnings)
    return result


def _import_meshes(
    context: bpy.types.Context,
    package: Package,
    root: bpy.types.Collection,
    result: ImportResult,
    paths: dict[str, Path],
) -> dict[str, bpy.types.Collection]:
    holder = bpy.data.collections.new(blender_name(f"{package.entity_name} meshes"))
    root.children.link(holder)
    holder_layer = _layer_collection(context.view_layer.layer_collection, holder)
    images = ImageCache(package)
    templates: dict[str, bpy.types.Collection] = {}
    for mesh_id, path in paths.items():
        mesh = package.meshes[mesh_id]
        if mesh.rig is not None:
            continue
        template = bpy.data.collections.new(mesh_id)
        holder.children.link(template)
        if _import_glb(context, package, mesh, path, template, images):
            templates[mesh_id] = template
            result.meshes += 1
        else:
            package.warnings.append(f"Mesh {mesh_id} could not be imported.")
    if holder_layer is not None:
        holder_layer.exclude = True
    return templates


def _import_glb(
    context: bpy.types.Context,
    package: Package,
    mesh: Mesh,
    path: Path,
    template: bpy.types.Collection,
    images: ImageCache,
) -> bool:
    layer = _layer_collection(context.view_layer.layer_collection, template)
    previous = context.view_layer.active_layer_collection
    context.view_layer.active_layer_collection = layer
    before = _snapshot()
    try:
        status = _run_gltf(context, path)
    except RuntimeError:
        status = set()
    finally:
        context.view_layer.active_layer_collection = previous
    if "FINISHED" not in status:
        _discard_new(before)
        return False
    created = [material for material in bpy.data.materials if material not in before["materials"]]
    apply_materials(created, package, mesh, images)
    drop_tint_without_colours(created, [o for o in bpy.data.objects if o not in before["objects"]])
    return True


def _import_rigged(
    context: bpy.types.Context,
    mesh: Mesh,
    path: Path,
    parent: bpy.types.Object,
    root: bpy.types.Collection,
    result: ImportResult,
) -> None:
    layer = _layer_collection(context.view_layer.layer_collection, root)
    previous = context.view_layer.active_layer_collection
    if layer is not None:
        context.view_layer.active_layer_collection = layer
    before = _snapshot()
    try:
        status = _run_gltf(context, path)
    except RuntimeError:
        status = set()
    finally:
        context.view_layer.active_layer_collection = previous
    if "FINISHED" not in status:
        _discard_new(before)
        result.warnings.append(f"Mesh {mesh.id} could not be imported.")
        return
    created = [obj for obj in bpy.data.objects if obj not in before["objects"]]
    for obj in created:
        if obj.parent is None:
            obj.parent = parent
        result.armatures += obj.type == "ARMATURE"
        keys = obj.data.shape_keys if obj.type == "MESH" else None
        result.shape_keys += max(len(keys.key_blocks) - 1, 0) if keys is not None else 0
        if obj.type == "MESH":
            for material in obj.data.materials:
                if material is not None:
                    _drop_vertex_colour_tint(material)
    result.actions += len(set(bpy.data.actions) - before["actions"])
    result.meshes += 1


_DATA_BLOCKS = ("objects", "meshes", "armatures", "materials", "actions", "images", "cameras", "lights")


def _snapshot() -> dict[str, set]:
    """Returns the datablocks a glTF import may create, to remove them if the import fails."""
    return {name: set(getattr(bpy.data, name)) for name in _DATA_BLOCKS}


def _discard_new(before: dict[str, set]) -> None:
    """Removes every datablock that is not in ``before``, objects first."""
    for name in _DATA_BLOCKS:
        blocks = getattr(bpy.data, name)
        for block in [block for block in blocks if block not in before[name]]:
            blocks.remove(block)


def _run_gltf(context: bpy.types.Context, path: Path) -> set[str]:
    with _gltf_context(context):
        return bpy.ops.import_scene.gltf(filepath=str(path))


def _gltf_context(context: bpy.types.Context) -> AbstractContextManager[object]:
    """Returns a context with a 3D view when the caller has none, as glTF armatures need one."""
    if hasattr(context, "object"):
        return nullcontext()
    window = next(iter(context.window_manager.windows), None)
    area = next((a for a in window.screen.areas if a.type == "VIEW_3D"), None) if window else None
    region = next((r for r in area.regions if r.type == "WINDOW"), None) if area else None
    if window is None or area is None or region is None:
        return nullcontext()
    return context.temp_override(window=window, area=area, region=region)


def _drop_vertex_colour_tint(material: bpy.types.Material) -> None:
    """Removes the glTF vertex-colour mask from a rigged material's base colour."""
    tree = material.node_tree
    if tree is None or "p4kx_family" in material:
        return
    for shader in tree.nodes:
        if shader.bl_idname == "ShaderNodeBsdfPrincipled":
            _drop_base_colour_mask(tree, shader.inputs["Base Color"])


def _drop_base_colour_mask(tree: bpy.types.NodeTree, base: bpy.types.NodeSocket) -> None:
    pending = [base]
    seen = set()
    while pending:
        target = pending.pop()
        if target in seen or not target.is_linked:
            continue
        seen.add(target)
        link = target.links[0]
        if _is_gltf_colour(link.from_socket):
            target.default_value = (1.0, 1.0, 1.0, 1.0)
            tree.links.remove(link)
            continue
        mix = link.from_node
        if mix.bl_idname != "ShaderNodeMix" or mix.data_type != "RGBA" or mix.blend_type != "MULTIPLY":
            continue
        factor = mix.inputs[0]
        if factor.is_linked or factor.default_value != 1.0:
            continue
        colours = [socket for socket in mix.inputs if socket.enabled and socket.type == "RGBA"]
        if len(colours) != 2:
            continue
        masks = [socket for socket in colours if socket.is_linked and _is_gltf_colour(socket.links[0].from_socket)]
        if len(masks) != 1:
            pending.extend(colours)
            continue
        other = colours[1] if masks[0] == colours[0] else colours[0]
        if other.is_linked:
            tree.links.new(other.links[0].from_socket, target)
        else:
            target.default_value = other.default_value
            tree.links.remove(link)


def _is_gltf_colour(socket: bpy.types.NodeSocket) -> bool:
    node = socket.node
    return socket.name == "Color" and (
        node.bl_idname == "ShaderNodeVertexColor"
        or (
            node.bl_idname == "ShaderNodeAttribute"
            and node.attribute_type == "GEOMETRY"
            and node.attribute_name == GLTF_COLOUR_ATTRIBUTE
        )
    )


def _create_object(
    node: Node,
    template: bpy.types.Collection | None,
    created: dict[int, bpy.types.Object],
    root: bpy.types.Collection,
) -> bpy.types.Object:
    obj = bpy.data.objects.new(blender_name(node.name), None)
    obj.empty_display_type = "PLAIN_AXES"
    obj.empty_display_size = 0.5
    obj.location = Vector(gltf_to_blender_vector(node.translation))
    obj.rotation_mode = "QUATERNION"
    obj.rotation_quaternion = Quaternion(gltf_to_blender_quaternion(node.rotation))
    obj.scale = Vector(gltf_to_blender_scale(node.scale))
    if template is not None:
        obj.instance_type = "COLLECTION"
        obj.instance_collection = template
    if node.parent is not None:
        obj.parent = created[node.parent]
    root.objects.link(obj)
    return obj


def _create_light(
    node: Node,
    light: Light,
    strength: float,
    created: dict[int, bpy.types.Object],
    root: bpy.types.Collection,
) -> bpy.types.Object:
    settings = light_settings(light, strength)
    data = bpy.data.lights.new(blender_name(node.name), settings.type)
    data.color = settings.color
    data.energy = settings.energy
    data.use_custom_distance = settings.use_custom_distance
    if settings.use_custom_distance:
        data.cutoff_distance = settings.cutoff_distance
    if settings.spot_size is not None and settings.spot_blend is not None:
        data.spot_size = settings.spot_size
        data.spot_blend = settings.spot_blend
    obj = bpy.data.objects.new(blender_name(node.name), data)
    obj.location = Vector(gltf_to_blender_vector(node.translation))
    obj.rotation_mode = "QUATERNION"
    obj.rotation_quaternion = Quaternion(gltf_to_blender_light_quaternion(node.rotation))
    obj.scale = Vector((1.0, 1.0, 1.0))
    if node.parent is not None:
        obj.parent = created[node.parent]
    root.objects.link(obj)
    return obj


def _layer_collection(
    layer: bpy.types.LayerCollection, collection: bpy.types.Collection
) -> bpy.types.LayerCollection | None:
    if layer.collection == collection:
        return layer
    for child in layer.children:
        found = _layer_collection(child, collection)
        if found is not None:
            return found
    return None
