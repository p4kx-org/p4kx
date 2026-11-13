# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Imports a synthetic package in headless Blender and checks objects, instancing and materials.

Run with ``blender --background --factory-startup --python blender_smoke.py`` or, with the
``bpy`` wheel, ``python blender_smoke.py``.
"""

from __future__ import annotations

import os
import platform
import struct
import sys
import tempfile
from importlib import import_module
from pathlib import Path
from types import ModuleType

import bpy
import addon_utils
from mathutils import Vector

HERE = Path(__file__).resolve().parent
INSTALLED_ROOT = os.environ.get("P4KX_TEST_INSTALLED_BLENDER_ROOT")
ADDON = "bl_ext.user_default.p4kx_importer" if INSTALLED_ROOT else "p4kx_importer"
sys.path.insert(0, str(HERE))
if not INSTALLED_ROOT:
    sys.path.insert(1, str(HERE.parent))

import synthetic  # noqa: E402
import synthetic_character  # noqa: E402
import synthetic_materials  # noqa: E402


def addon_module(name: str) -> ModuleType:
    """Imports a module from the selected add-on."""
    return import_module(f"{ADDON}.{name}")


extension = import_module(ADDON)
importer = addon_module("importer")
lights = addon_module("lights")
package = addon_module("package")
glass_tint = addon_module("glass_tint")
materials = addon_module("materials")
material_graph = addon_module("material_graph")
material_model = addon_module("material_model")


def check(condition: bool, message: str) -> None:
    """Raises ``SystemExit`` with ``message`` when ``condition`` is false."""
    if not condition:
        raise SystemExit(f"smoke test failed: {message}")


def check_interior(folder: Path) -> None:
    """Imports ``synthetic.lit_manifest()`` and checks its containers, object and lights."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    loaded = package.load_package(synthetic.write_package(folder, synthetic.lit_manifest()))
    result = importer.import_package(bpy.context, loaded, light_strength=2.0)
    check(result.lights == 3, f"lights: {result.lights}")
    check(result.objects == 10, f"node objects: {result.objects}")
    check(result.meshes == 2, f"meshes: {result.meshes}")
    light_objects = [o for o in bpy.data.objects if o.type == "LIGHT"]
    check(len(light_objects) == 3, f"light objects: {len(light_objects)}")
    check(len(bpy.data.lights) == 3, f"light data: {len(bpy.data.lights)}")

    lamp = bpy.data.objects["lamp"]
    check(lamp.data.type == "POINT", f"lamp type {lamp.data.type}")
    check(abs(lamp.data.energy - 3.0 * lights.WATTS_PER_UNIT * 2.0) < 1e-4, f"energy {lamp.data.energy}")
    check(lamp.data.use_custom_distance and abs(lamp.data.cutoff_distance - 6.0) < 1e-5, "range")
    check(tuple(round(c, 4) for c in lamp.data.color) == (1.0, 0.5, 0.25), "colour")
    check(tuple(lamp.scale) == (1.0, 1.0, 1.0), f"light scale {tuple(lamp.scale)}")
    beam = bpy.data.objects["beam"]
    check(beam.data.type == "SPOT", f"beam type {beam.data.type}")
    check(abs(beam.data.spot_size - 1.0471976) < 1e-4, f"spot size {beam.data.spot_size}")
    check(abs(beam.data.spot_blend - 1.0) < 1e-5, f"spot blend {beam.data.spot_blend}")
    sun = bpy.data.objects["sun"]
    check(sun.data.type == "SUN", f"sun type {sun.data.type}")
    check(abs(sun.data.energy - 1.5 * lights.SUN_STRENGTH_PER_UNIT * 2.0) < 1e-4, "sun strength")

    check(bpy.data.objects["room"].parent is bpy.data.objects["Interior"], "room below the group")
    check(bpy.data.objects["lamp"].parent is bpy.data.objects["room"], "lamp below the room")
    stool = bpy.data.objects["stool"]
    box = bpy.data.objects["hardpoint_c/Box"]
    check(stool.instance_collection is box.instance_collection, "the stool shares the box mesh")

    bpy.context.view_layer.update()
    shines = sun.matrix_world.to_quaternion() @ Vector((0.0, 0.0, -1.0))
    check(
        all(abs(a - b) < 1e-4 for a, b in zip(shines, (-1.0, 0.0, 0.0))),
        f"the sun shines along {tuple(shines)}, expected (-1, 0, 0)",
    )
    where = lamp.matrix_world.translation
    check(
        all(abs(a - b) < 1e-4 for a, b in zip(where, (-6.0, 0.0, 2.5))),
        f"the lamp is at {tuple(where)}, expected (-6, 0, 2.5)",
    )


def check_interior_alone(folder: Path) -> None:
    """Imports ``synthetic.interior_manifest()``: an Interior without an Entity around it."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    loaded = package.load_package(synthetic.write_package(folder, synthetic.interior_manifest()))
    check(not loaded.warnings, f"loaded: {loaded.warnings}")
    result = importer.import_package(bpy.context, loaded)
    check(not result.warnings, f"warnings: {result.warnings}")
    check(result.objects == 4, f"node objects: {result.objects}")
    check(result.lights == 1, f"lights: {result.lights}")
    check(result.meshes == 1, f"meshes: {result.meshes}")
    check(bpy.data.collections.get("ship_interior") is not None, "the collection is named after the Interior")
    root = bpy.data.objects["ship_interior"]
    check(root.parent is None, "the container is the root")
    room = bpy.data.objects["room"]
    check(room.parent is root, "the room is below the container")
    check(bpy.data.objects["stool"].instance_collection is not None, "the stool is instanced")
    check(bpy.data.objects["lamp"].type == "LIGHT", "the lamp is a light")
    bpy.context.view_layer.update()
    check(abs(room.matrix_world.translation.y - 6.0) < 1e-4, f"room at {tuple(room.matrix_world.translation)}")


def reads_gltf_vertex_colour(socket: bpy.types.NodeSocket) -> bool:
    """Checks the live colour inputs between the shader and the imported vertex attribute."""
    pending = [socket]
    seen = set()
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for link in current.links:
            source = link.from_node
            if source.type == "VERTEX_COLOR":
                return True
            if source.type == "ATTRIBUTE" and source.attribute_type == "GEOMETRY" and source.attribute_name == "Color":
                return True
            if source.type in ("MIX", "MIX_RGB"):
                pending.extend(item for item in source.inputs if item.enabled and item.type == "RGBA")
    return False


def check_character(folder: Path, base_colour: tuple[float, ...]) -> None:
    """Imports ``synthetic_character.character_manifest()`` and checks armature, shapes, actions."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    scene = synthetic_character.write_character_package(folder, base_colour=base_colour)
    loaded = package.load_package(scene)
    check(not loaded.warnings, f"loaded: {loaded.warnings}")
    rigged = next(mesh for mesh in loaded.meshes.values() if mesh.rig is not None)
    before = importer._snapshot()
    importer._run_gltf(bpy.context, package.resolve_file(loaded.root, rigged.file))
    plain = [m for m in bpy.data.materials if m not in before["materials"] and m.node_tree]
    check(plain, "the plain glTF import creates a material")
    base_sockets = [
        node.inputs["Base Color"]
        for material in plain
        for node in material.node_tree.nodes
        if node.type == "BSDF_PRINCIPLED"
    ]
    check(any(reads_gltf_vertex_colour(socket) for socket in base_sockets), "glTF uses the original COLOR_0")
    importer._discard_new(before)
    result = importer.import_package(bpy.context, loaded)
    check(not result.warnings, f"warnings: {result.warnings}")
    check(result.armatures == 1, f"armatures: {result.armatures}")
    check(result.shape_keys == 1, f"shape keys: {result.shape_keys}")
    check(result.actions >= 1, f"actions: {result.actions}")
    check(result.meshes == 2, f"meshes: {result.meshes}")

    hero = bpy.data.objects["Hero"]
    check(hero.type == "EMPTY" and hero.instance_collection is None, "the Character is not instanced")
    armatures = [o for o in bpy.data.objects if o.type == "ARMATURE"]
    check(len(armatures) == 1, f"armature objects: {len(armatures)}")
    armature = armatures[0]
    check(armature.parent is hero, f"the armature is below the node: {armature.parent}")
    bones = {bone.name for bone in armature.data.bones}
    check({"root", "tip"} <= bones, f"bones: {bones}")

    skinned = [o for o in bpy.data.objects if o.type == "MESH" and o.find_armature() is armature]
    check(len(skinned) == 1, f"skinned meshes: {len(skinned)}")
    body = skinned[0]
    check({"root", "tip"} <= {group.name for group in body.vertex_groups}, "vertex groups")
    keys = body.data.shape_keys
    check(keys is not None, "the mesh has shape keys")
    names = [block.name for block in keys.key_blocks]
    check(names == ["Basis", "smile"], f"shape keys: {names}")
    check(len(body.data.color_attributes) == 1, "the vertex colours are kept as an attribute")
    bsdf = next(n for n in body.data.materials[0].node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    base = bsdf.inputs["Base Color"]
    check(not reads_gltf_vertex_colour(base), "the vertex colours do not tint the base colour")
    check(not base.is_linked, "the plain material keeps its base factor directly on the shader")
    check(
        all(abs(actual - expected) < 1e-6 for actual, expected in zip(base.default_value, base_colour)),
        f"the authored base factor is preserved: {tuple(base.default_value)}",
    )

    waves = [action for action in bpy.data.actions if "wave" in action.name]
    check(len(waves) >= 1, f"actions: {[action.name for action in bpy.data.actions]}")
    check(
        any(o.animation_data is not None for o in (armature, keys)),
        "the armature or the shape keys are animated",
    )

    prop = bpy.data.objects["Prop"]
    check(prop.instance_collection is not None, "static meshes are still instanced")
    check(prop.parent is hero, "the prop is below the Character")


def check_installed_extension() -> None:
    """Verifies the host and the enabled add-on before changing the fixture scene."""
    if not INSTALLED_ROOT:
        return
    check(platform.system() == "Darwin" and platform.machine() == "arm64", "native Apple Silicon Blender")
    check(bpy.app.version[:2] == (5, 2) and bpy.app.version_cycle == "release", "Blender 5.2 LTS")
    check(all(addon_utils.check(ADDON)), "installed add-on was enabled by Blender")
    root = Path(INSTALLED_ROOT).resolve(strict=True)
    location = Path(extension.__file__).resolve(strict=True)
    check(location.is_relative_to(root), "loaded module belongs to the isolated extension installation")
    print(f"native Blender {bpy.app.version_string} on arm64, extension installed and enabled")


def main() -> None:
    """Runs the import and the checks."""
    check_installed_extension()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    with tempfile.TemporaryDirectory() as folder:
        scene = synthetic.write_package(Path(folder))
        loaded = package.load_package(scene)
        result = importer.import_package(bpy.context, loaded)
        check(result.objects == 4, f"node empties: {result.objects}")
        check(result.instances == 3, f"instances: {result.instances}")
        check(result.meshes == 2, f"meshes: {result.meshes}")

        names = {node.name for node in loaded.nodes}
        nodes = [o for o in bpy.data.objects if o.name in names]
        check(len(nodes) == 4, f"named node objects: {len(nodes)}")
        gun_a = bpy.data.objects["hardpoint_a/Gun"]
        gun_b = bpy.data.objects["hardpoint_b/Gun"]
        box = bpy.data.objects["hardpoint_c/Box"]
        check(gun_a.instance_collection is gun_b.instance_collection, "guns share a collection")
        check(gun_a.instance_collection is not box.instance_collection, "box has its own")
        check(gun_a.parent is bpy.data.objects["TEST_Ship"], "hierarchy kept")
        check(tuple(round(v, 4) for v in gun_a.location) == (1.0, -3.0, 2.0), "axes converted")

        bpy.context.view_layer.update()
        depsgraph = bpy.context.evaluated_depsgraph_get()
        placed = [
            inst.object.matrix_world @ inst.object.data.vertices[1].co
            for inst in depsgraph.object_instances
            if inst.is_instance and inst.parent.original is gun_a and inst.object.type == "MESH"
        ]
        check(len(placed) == 1, f"instanced gun meshes: {len(placed)}")
        expected = (3.0, 2.0, 2.0)
        check(
            all(abs(a - b) < 1e-4 for a, b in zip(placed[0], expected)),
            f"world position {tuple(placed[0])}, expected {expected}",
        )

        meshes = [o for o in bpy.data.objects if o.type == "MESH"]
        listing = [(o.name, o.type) for o in bpy.data.objects]
        check(len(meshes) == 2, f"mesh objects imported once each: {listing}")

        material = bpy.data.materials["Gun_mat"]
        kinds = {n.type for n in material.node_tree.nodes}
        check({"TEX_IMAGE", "NORMAL_MAP", "BSDF_PRINCIPLED"} <= kinds, f"nodes: {kinds}")
        images = [n.image for n in material.node_tree.nodes if n.type == "TEX_IMAGE"]
        spaces = {image.colorspace_settings.name for image in images}
        check(spaces == {"sRGB", "Non-Color"}, f"colour spaces: {spaces}")
        check(not result.warnings, f"warnings: {result.warnings}")

        check_interior(Path(folder) / "lit")
        check_interior_alone(Path(folder) / "interior_alone")
        check_character(Path(folder) / "character", synthetic_character.RIG_BASE_COLOUR)
        check_character(Path(folder) / "character_white", (1.0, 1.0, 1.0, 1.0))

        bpy.ops.wm.read_factory_settings(use_empty=True)
        addon_utils.enable("io_scene_gltf2", default_set=False)
        extension.register()
        status = bpy.ops.import_scene.p4kx_package(filepath=str(scene))
        check(status == {"FINISHED"}, f"operator: {status}")
        check(len([o for o in bpy.data.objects if o.type == "MESH"]) == 2, "operator import")

        newer = synthetic.manifest(version="2.0")
        newer_scene = synthetic.write_package(Path(folder) / "newer", newer)
        try:
            bpy.ops.import_scene.p4kx_package(filepath=str(newer_scene))
        except RuntimeError as error:
            check("Update the P4KX Blender add-on" in str(error), f"message: {error}")
        else:
            check(False, "a newer package must be refused")
        extension.unregister()

        check_invalid_package_mesh(Path(folder) / "invalid_mesh")
        check_layered_material(Path(folder) / "layered")
        check_glass_tint()

    print("smoke test passed")


def check_invalid_package_mesh(folder: Path) -> None:
    """Checks malformed mesh diagnostics, scene timing and import of intact meshes."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    scene = synthetic.write_package(folder)
    broken = folder / "meshes/m0001.glb"
    content = bytearray(broken.read_bytes())
    struct.pack_into("<I", content, 4, 9)
    broken.write_bytes(content)

    loaded = package.load_package(scene)
    original_preflight = importer.preflight_meshes
    original_run = importer._run_gltf
    collections_before = set(bpy.data.collections)
    imports = []

    def preflight(candidate):
        check(set(bpy.data.collections) == collections_before, "preflight runs before scene creation")
        result = original_preflight(candidate)
        check(len(result.problems) == 1, "invalid GLB has a structured preflight result")
        check(result.problems[0].mesh_id == "m0001", "preflight identifies invalid mesh")
        return result

    def run(context, path):
        imports.append(path.name)
        return original_run(context, path)

    importer.preflight_meshes = preflight
    importer._run_gltf = run
    try:
        result = importer.import_package(bpy.context, loaded)
    finally:
        importer.preflight_meshes = original_preflight
        importer._run_gltf = original_run

    check(imports == ["m0002.glb"], f"only the valid GLB reached Blender: {imports}")
    check(result.meshes == 1 and result.instances == 1, "intact GLB still imports")
    check(
        any("m0001" in warning and "Unsupported GLB version" in warning for warning in result.warnings),
        f"corrupt asset warning: {result.warnings}",
    )


def check_glass_tint() -> None:
    """Checks the vertex-colour tint of glass: kept with colours, plain without, kept by the drop."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    raw = synthetic_materials.description(
        "x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], diffuse=[0.5, 0.25, 1.0]
    )
    descriptions = material_model.parse_materials([raw], [])
    spec = material_graph.build_graph(descriptions["x0001"], material_graph.BuildContext(descriptions, None, {}))

    images = materials.ImageCache(package.parse_manifest(synthetic.manifest(), Path(".")))

    def glass(name: str) -> bpy.types.Material:
        material = bpy.data.materials.new(name)
        materials.build_tree(material, spec, images)
        return material

    def mesh_object(name: str, material: bpy.types.Material, colours: bool) -> bpy.types.Object:
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([(0, 0, 0), (1, 0, 0), (0, 1, 0)], [], [(0, 1, 2)])
        mesh.materials.append(material)
        if colours:
            mesh.color_attributes.new("Color", "BYTE_COLOR", "CORNER")
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        return obj

    def vertex_nodes(material: bpy.types.Material) -> list[bpy.types.Node]:
        return [n for n in material.node_tree.nodes if n.type == "VERTEX_COLOR"]

    tinted, plain = glass("tinted"), glass("plain")
    check(len(vertex_nodes(tinted)) == 1, "the glass tree has a vertex colour node")
    check(vertex_nodes(tinted)[0].layer_name == "Color", "the node names the glTF colour attribute")
    nodes = tinted.node_tree.nodes
    base_in = next(n for n in nodes if n.type == "BSDF_PRINCIPLED").inputs["Base Color"]
    tint_in = next(n for n in nodes if n.type == "BSDF_TRANSPARENT").inputs["Color"]
    check(
        base_in.is_linked
        and tint_in.is_linked
        and base_in.links[0].from_socket == tint_in.links[0].from_socket,
        "the Transparent Color and the Principled Base Color share one source",
    )
    importer._drop_vertex_colour_tint(tinted)
    check(len(vertex_nodes(tinted)) == 1, "the glTF tint drop leaves described glass alone")

    objects = [mesh_object("with", tinted, True), mesh_object("without", plain, False)]
    glass_tint.drop_tint_without_colours([tinted, plain], objects)
    check(len(vertex_nodes(tinted)) == 1, "glass on a mesh with colours keeps its tint")
    check(not vertex_nodes(plain), "glass on a mesh without colours loses its tint")
    bsdf = next(n for n in plain.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    colour = tuple(round(v, 4) for v in bsdf.inputs["Base Color"].default_value[:3])
    check(not bsdf.inputs["Base Color"].is_linked and colour == (0.5, 0.25, 1.0), f"plain colour {colour}")
    tint_in = next(n for n in plain.node_tree.nodes if n.type == "BSDF_TRANSPARENT").inputs["Color"]
    tint_colour = tuple(round(v, 4) for v in tint_in.default_value[:3])
    check(not tint_in.is_linked and tint_colour == (0.5, 0.25, 1.0), f"plain transparent tint {tint_colour}")
    check(all(link.is_valid for link in plain.node_tree.links), "the plain glass has no invalid link")


def check_layered_material(folder: Path) -> None:
    """Imports a 1.1 package and checks the node tree of a layered, tinted material."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    addon_utils.enable("io_scene_gltf2", default_set=False)
    scene = synthetic.write_package(folder, synthetic_materials.manifest_11())
    loaded = package.load_package(scene)
    check(loaded.version == (1, 1) and not loaded.warnings, f"loaded: {loaded.warnings}")
    result = importer.import_package(bpy.context, loaded)
    check(any("WeirdShader" in w for w in result.warnings), f"warnings: {result.warnings}")

    material = bpy.data.materials["Gun_mat"]
    check(material.get("p4kx_index") == 0, "glTF extras arrive as a custom property")
    check(material.get("p4kx_material") == "x0001", "description id is kept")
    nodes = material.node_tree.nodes
    groups = {n.node_tree.name for n in nodes if n.type == "GROUP"}
    check(groups == {"P4KX Layer Blend v1", "P4KX Wear v1"}, f"groups: {groups}")

    def group_count(prefix: str) -> int:
        return len([g for g in bpy.data.node_groups if g.name.startswith(prefix)])

    check(group_count("P4KX Layer Blend") == 1, "the layer blend group exists once")
    check(group_count("P4KX Wear") == 1, "the wear group exists once")
    inner = next(g for g in bpy.data.node_groups if g.name == "P4KX Layer Blend v1")
    check({"MIX", "VECT_MATH", "GROUP_INPUT", "GROUP_OUTPUT"} <= {n.type for n in inner.nodes}, "blend group contents")
    spaces = {n.image.colorspace_settings.name for n in nodes if n.type == "TEX_IMAGE"}
    check(spaces == {"sRGB", "Non-Color"}, f"colour spaces: {spaces}")
    tints = []
    for n in nodes:
        if n.type == "MIX" and n.blend_type == "MULTIPLY":
            socket = next(s for s in n.inputs if s.name == "B" and s.enabled)
            if not socket.is_linked:
                tints.append(tuple(round(v, 4) for v in socket.default_value[:3]))
    check((0.8, 0.09, 0.08) in tints, f"palette tint on the first layer: {tints}")
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    source = bsdf.inputs["Normal"].links[0].from_node
    check(source.type == "GROUP", f"normal comes from the blend group: {source.type}")
    metallic = bsdf.inputs["Metallic"].links[0].from_node
    check(metallic.type == "GROUP", f"metallic comes from the blend group: {metallic.type}")

    second = bpy.data.materials["Box_mat2"]
    check(second.get("p4kx_material") == "x0001", "second material pairs by index")
    unknown = bpy.data.materials["Box_mat"]
    check(unknown.get("p4kx_family") == "unknown", "unknown shader falls back")
    check(not any(n.type == "GROUP" for n in unknown.node_tree.nodes), "basic fallback")
    kinds = {n.type for n in unknown.node_tree.nodes}
    check({"MAP_RANGE", "MIX", "TEX_IMAGE"} <= kinds, f"specular map nodes: {kinds}")
    unknown_bsdf = next(n for n in unknown.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    check(unknown_bsdf.inputs["Metallic"].is_linked, "metallic from the specular map")
    check(unknown_bsdf.inputs["Specular IOR Level"].is_linked, "specular level from the map")
    check(unknown_bsdf.inputs["Base Color"].is_linked, "base colour mixed with the specular map")

    importer.import_package(bpy.context, package.load_package(scene))
    check(group_count("P4KX Layer Blend") == 1, "a second import reuses the blend group")
    check(group_count("P4KX Wear") == 1, "a second import reuses the wear group")


main()
