# Package materials are written as the game's raw inputs in format 1.1

Decided for issue #158 (M13.2); it extends ADR-0083 and uses the palettes of ADR-0080. The node
trees that #160 builds in Blender read what is decided here.

## Decision

**The package describes materials; it does not build them.** `scene.json` gets a `materials`
array of material descriptions, and `palettes`. A description holds what the game's `.mtl` file
holds for a material: shader, generation mask, surface type, flags, colours and scalars, texture
slots, layers and shader parameters. It holds no renderer concept: no Blender node, no
interpretation of what a shader does. The add-on, or any other importer, decides how to build
a surface from them. Where the writer derives a value (`alpha_mode`, a texture's `usage`, the
`palette` entry a material takes) the derivation is documented and the raw fields it comes from
stay next to it.

**Raw first, typed where a reader needs it.** The reader of the `.mtl` files in `p4kx-model` keeps
the layers (`MatLayers`, all attributes), the shader parameters (`PublicParams`), every other
attribute of the material, the attributes and child elements of each texture slot (`TexMod`
carries the UV tiling and offset) and every other child element, as name and text value in file
order. Texture slots and layers retain all admitted original occurrences, including empty and
repeated layers. Raw unknown element fragments retain the separate depth-8 and count-4096
budgets; they do not charge or cut typed layer declarations. A raw-fragment cut is recorded in
`Material::truncated` and as a package issue. Retained authored syntax and layer companion reads
have their own explicit budgets; these are not file-format maxima or reasons to hide complete
advertised layer fields.
Descriptions are keyed by library path and index, with the model's folder for a bare library name. A parameter, attribute or element the writer has never heard of is therefore
in the package, unchanged: no list of known parameters has to be maintained for a new game
version. The few values importers always need as numbers (`TintColor`, `WearTint`, `UVTiling`,
`GlossMult` of a layer) are also written typed; a malformed one is absent, and the raw text stays.

**Layers are materials.** A layer's `Path` names a `.mtl` file, which is a material with its own
textures, parameters and layers. It is described like any other material and referenced by id,
so a layer file that ten hull materials use is written once. A layer file with a sub-material
list is entered by its `Submtl` name, else at its first entry; layers are followed to a depth of
8, and a file that names itself refers to its own description. A layer file that is missing or
unreadable is an issue (`origin` `material`, `kind` `layer_material`) and the layer has no
`material`.

**Descriptions and palettes are written once and referenced.** A description is identified by its
library path (normalised: `/`, no `Data/`, no `.mtl`, case folded) and sub-material index, a
palette by its value. Meshes reference the descriptions of the materials they are drawn with
(`materials[].material`) and their palette (`palette_id`). The description is the file's, with no
palette applied, so one description serves every livery; the mesh's palette says which colours
the `palette` entries take.

**All texture slots are written.** The `textures` of the package now hold every texture file a
described material names, not only the three glTF roles of 1.0: blend, mask, detail, wear and
occlusion maps are part of how the game layers a surface. They are named by id from the slot
(`textures[].texture`); engine placeholders (`$TintPaletteDecal`, `$RenderToTexture`) have no
file and no id. The colour space rule stays: `srgb` for diffuse maps, `linear` for the rest, and
normal maps are flipped to glTF. The package grows accordingly (the report of the PR gives sizes);
`--texture-mip` bounds it.

**Version 1.1, a minor version.** Everything added is an additional field or file; every 1.0 field
and meaning stays, including the `diffuse`/`normal`/`specular` ids of a mesh material. `materials`
and `palettes` are absent from the schema's required list, so a 1.0 manifest still validates and
reads. `Issue.origin` gains `material`. A reader of 1.0 that ignores the new fields still gets
what it got.

**Meshes carry the glTF material index.** Each material in a package mesh GLB gets `extras`
`{"p4kx_index": <index>}`, because Blender names and orders imported materials by first use. The
package writer adds it to the finished GLB; `entity glb` output does not change.

**Writing needs the Archive's material files.** `write_package` takes the `ModelProvider` that
built the Assembly, to read the layer files; the CLI passes the `ArchiveModels` it already has.

## Consequences

A package of a ship has one description per distinct material of its meshes plus one per
distinct layer file, and as many textures as those name. The add-on of #159 and #160 reads
`materials`, `palettes` and the new texture ids and may ignore everything else. New shader
parameters need no change of the writer, the schema or the add-on's reader: they arrive as
`parameters`. Interior and Character packages (#161, #162) reuse the same descriptions. The
decision which parameters mean what stays in the importer, where it can change without a new
package format.
