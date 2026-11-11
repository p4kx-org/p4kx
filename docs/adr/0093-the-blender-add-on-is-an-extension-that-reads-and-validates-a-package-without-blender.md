# The Blender add-on is an extension that reads and validates a package without Blender

Decided for issue #159, the add-on tracer of M13 (#13); it reads the package of ADR-0083.

## Decision

**Where and what.** The add-on is the Python package `blender_addon/p4kx_importer/`, an extension
in Blender's extension format (`blender_manifest.toml`, no `bl_info`). It adds
File > Import > "P4KX Package (scene.json)". `blender_addon/build_zip.py` packs the folder into
`p4kx_importer-<version>.zip` with the files at the root of the zip, in a fixed order with fixed
timestamps, which is what Blender installs from disk and what #164 attaches to every release.

**Supported Blender.** The add-on supports Blender 5.2 LTS and 5.3 (Python 3.13). The manifest states
`blender_version_min = "5.2.0"`, and CI tests both Blender versions. It uses only API that is
stable across 5.x: no `blend_method`, no `Material.use_nodes` on a material that already has a
node tree, and the glTF importer that ships with Blender (`io_scene_gltf2`) for the GLB parts.
Older Blender versions are not supported; the manifest, not code, keeps the add-on from being
enabled there.

**Pure reader, thin Blender layer.** `package.py` reads and validates `scene.json` and imports
nothing from Blender; plain pytest tests it. `importer.py`, `materials.py` and `operator.py` are
the only modules that import `bpy`, and the package `__init__` imports them only inside
`register()`. The reader enforces the version rule of ADR-0083: it refuses a package with another
`format`, `basis` or `unit`, refuses a higher major version with a message to update the add-on,
reads a higher minor version with a warning, and ignores fields it does not know, texture
formats it cannot read and nodes of a kind it does not know (with their children).

**Package input is untrusted and never executed.** The reader accepts only the shapes it expects,
checks types, finite numbers (a number too large for a float is refused) and parent indices, caps
`scene.json` at 64 MiB and the nodes, meshes and textures at 200,000 entries each, and takes file
references only as the writer's ids: a mesh file must be `meshes/m<digits>.glb`, a texture file
`textures/t<digits>.png`, and every path is resolved and refused when it leaves the package
folder (symbolic links included). Before Blender's importer sees a mesh, the add-on reads the
GLB header and its JSON chunk (at most 16 MiB) and refuses a mesh whose buffers or images name
an external `uri`; the GLBs of a package embed everything. Names from the package are only
object and material names, shortened to Blender's 63 bytes. A texture with a format, colour
space or alpha meaning this add-on does not know is skipped with a warning, so later minor
versions can add values.

**Shared meshes are collection instances.** A GLB mesh has a node tree of its own, so each mesh
that a node uses is imported once, by the glTF importer, into its own collection inside a
collection that is excluded from the view layer. Every node becomes an empty with the node's
name and local transform, parented as in `scene.json`; a node with a mesh makes its empty an
instance of that mesh's collection. A gun used eight times is one imported mesh and eight
instances. Transforms are converted from glTF's Y-up axes to Blender's Z-up per node
(`(x, y, z)` becomes `(x, -z, y)`, the scale's Y and Z swap), which composes to what the glTF
importer does for the geometry.

**Basic materials until #158.** The materials of a mesh are those of its GLB file. After each
import the add-on pairs the created Blender materials with the manifest's materials by name (a
`.001` suffix is stripped when the bare name is one the manifest knows; equal names are matched
in order) and gives each a basic setup on its Principled BSDF: the diffuse texture to Base Color
(and its alpha to Alpha when the texture's `alpha` is `opacity`), the normal texture through a
Normal Map node, the specular texture to Specular IOR Level, which is a placeholder until the
material descriptions of #158 say what the map means. The colour space of each image is
the manifest's `colour_space`; the alpha of images that hold no opacity is ignored. Normal maps
are already OpenGL, so nothing is inverted. Layered materials, palettes and decals come with
#158 in a later minor version of the package.

**Tests.** `blender_addon/tests/` holds pytest tests of the pure reader (versions, unsafe paths,
tolerance of later minor versions, axis conversion, material pairing, the zip builder) and
`blender_smoke.py`, which imports a synthetic package in Blender and checks the number of nodes,
meshes and instances, the shared collection, the hierarchy, the converted transform, the node
setup, the image colour spaces, the operator and the refusal of a newer package. A CI job runs
both on Windows and Linux; Blender comes from the `bpy` wheel on PyPI, pinned to the supported
versions for each runner.

## Consequences

The add-on version in `blender_manifest.toml` follows the P4KX release; #164 sets it when it
ships the zip. A change of the package's minor version that the add-on should read raises
`SUPPORTED_MINOR` in `package.py`. Raising the supported Blender version is a change of the
manifest, the CI pin and this ADR. Mesh files are not re-validated beyond their path: a corrupt
GLB makes the glTF importer fail, and the add-on reports the mesh as not imported and goes on.
