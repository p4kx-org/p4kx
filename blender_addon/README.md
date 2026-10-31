# P4KX Blender add-on

Imports a [Blender package](../docs/blender-package.md) written by `p4kx entity package`,
`interior package` or `character package` into Blender with one action: **File > Import > P4KX Package (scene.json)**. Decisions:
[ADR-0093](../docs/adr/0093-the-blender-add-on-is-an-extension-that-reads-and-validates-a-package-without-blender.md).

**Supported Blender: 5.2 LTS** (Python 3.13), as `blender_manifest.toml` states. The glTF 2.0
importer that ships with Blender must be enabled.

## What the import does

- Builds the scene hierarchy as empties with the names and transforms of `scene.json`.
- Imports every shared mesh once and places it as a linked collection instance.
- Loads the textures with the colour space the package states and builds the materials from the
  package's descriptions ([ADR-0100](../docs/adr/0100-the-add-on-builds-material-node-trees-from-the-package-descriptions-by-a-pure-mapping.md)):
  layers blended by their masks, palette tints, wear, glass, decals, holograms and alpha modes,
  with the node groups `P4KX Layer Blend v1` and `P4KX Wear v1` shared by all materials. The glTF
  materials are matched by their `p4kx_index` custom property. Unknown shaders and 1.0 packages
  get a basic setup (diffuse, normal map, specular, alpha); unknown shaders also a warning.
- Builds the Interior of a package written with `--interior`, or of one written by
  `p4kx interior package` on its own: containers and models as empties
  and instances, lights as Blender point, spot and sun lights (mapping in
  [docs/blender-package.md](../docs/blender-package.md#interiors-and-lights)). The import option
  *Light strength* multiplies their energy, because the engine's intensity has no known unit.
- Imports a Character written by `p4kx character package` (format 1.3) once, not as an instance:
  the armature with the bones of the body, the skinned meshes below it, the morph targets of the
  head as shape keys and the animations as actions, all below the empty of the `character` node
  ([ADR-0107](../docs/adr/0107-a-character-is-written-as-one-rigged-glb-mesh-behind-a-character-node.md)).
  The materials are those of Blender's glTF importer, with the textures embedded in the GLB.
- Imports a creature with its skeleton, written by `p4kx entity package` (format 1.7), the same
  way: its `entity` node draws a mesh with a `rig` that has no body variant, so the armature, the
  skinned meshes and the parts on its bones come in once below the node's empty
  ([ADR-0316](../docs/adr/0316-a-creature-with-its-skeleton-is-packaged-as-one-skinned-rig-mesh.md)).
- Refuses a package with a newer major version and says to update the add-on; warns about a
  newer minor version.

## Build and install

```powershell
python blender_addon/build_zip.py out
```

writes `out/p4kx_importer-<version>.zip`. In Blender: Edit > Preferences > Get Extensions >
Install from Disk. The release workflow builds the same zip and attaches it to every GitHub
Release; the manifest `version` must equal the workspace (Release) version, which `cargo xtask
addon` checks in CI.

## Tests

```powershell
cd blender_addon
python -m pip install pytest bpy==5.2.3
python -m pytest
python tests/blender_smoke.py
```

`pytest` runs without Blender. `blender_smoke.py` imports a synthetic package in headless Blender
(the `bpy` module, or `blender --background --factory-startup --python tests/blender_smoke.py`).

`python tests/import_real_package.py <package folder>...` imports packages written from a real
Archive and prints what each import created, see
[docs/blender-package.md](../docs/blender-package.md#verifying-a-package-in-blender).
