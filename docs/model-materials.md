# Material libraries and plain-colour materials

What the reader of `p4kx-model` assumes about `.mtl` files, and how `p4kx-gltf` turns them into
plain colours. Surveyed on 2026-10-20 over all 26,083 `.mtl` Entries of LIVE. The tests are
`crates/p4kx-extraction/tests/real_archive/materials.rs` (`#[ignore]`d, run with `P4KX_DATA_P4K`;
they pass on LIVE) and the API tests of `p4kx-model` and `p4kx-gltf`.

## The file

A `.mtl` Entry is CryXml (the converted XML is 2 KB to 133 KB; the reader caps a file at
16 MiB). The root is `<Material>`. 19,126 files have a `<SubMaterials>` list (up to 116
sub-materials; the engine's limit is 128, the reader's 1,024), and 6,957 are one material that
serves every material ID. The material ID of a submesh is the position in the list. No file nests
a `SubMaterials` list in a sub-material or refers to another `.mtl` file by `File`, so there is no
sub-material cycle to follow; nested lists are not read, which also bounds the recursion.

Attributes per material: `Name`, `MtlFlags`, `Shader`, `StringGenMask`, `SurfaceType`, `Diffuse`,
`Specular`, `Emissive` (three comma-separated floats), `Opacity`, `Shininess` (0 to 255), `Glow`,
`AlphaTest`, and in the root `DefaultPalette`. A material may also have `Textures/Texture`
(`Map="TexSlot<n>"`, `File`), `PublicParams` and `MatLayers`; the reader keeps the texture slots
(placeholders such as `$TintPaletteDecal` and `$RenderToTexture` included, empty files left out)
and also keeps the layers, the shader parameters, the other attributes and elements and the
texture slots' attributes and modifiers as the file has them, for the Blender package
([ADR-0092](adr/0092-package-materials-are-written-as-the-games-raw-inputs-in-format-1-1.md)).
The layers' `Path` names another `.mtl` file (a single material with its own textures and
parameters), which the package resolves.

- `MtlFlags` is a signed 32-bit number in the file (negative values occur); the reader keeps its
  32 bits. Bit `0x400` is the engine's no-draw flag: it is set on 24,626 materials, exactly the
  24,626 whose shader is `NoDraw` (one material each way differs). Bit `0x2` (two-sided) is set
  on 18,854 materials, mostly `HardSurface` and `Illum`.
- The colours are linear: `0.0097212186` is sRGB 0.1 linearised. glTF's factors are linear too,
  so they are written as they are.
- `Emissive` of `1,1,1` without `Glow` is common on `Illum` (8,243 of 13,625 materials with a
  non-zero emissive colour have no `Glow`), so the emissive colour counts only times `Glow`.
- Missing attributes read as white diffuse, a dim dielectric specular (0.04), no emission,
  opacity 1 and shininess 10. Malformed or non-finite numbers read like missing ones.

## Finding the library

The `MtlName` chunk names the library. The name is

- a path below `Data` without extension (`Objects/Spaceships/Ships/AEGS/Gladius/AEGS_Gladius`),
  the common case, resolved ignoring case and the kind of separator;
- a bare name (`f_facial_hair_021`), which the engine takes relative to the model: it is looked
  for as `<name>.mtl` beside the model first, then below `Data`;
- rarely with a `Data/` prefix and `.mtl` extension (`Data/Objects/.../m_qrt_combat_..._01.mtl`),
  which are stripped.

Of 150 sampled `.skin` files per Archive, 104 find their library; character skins often name a
library that lives in another folder and is chosen by the item's Entity, not by the skin (for
example `m_cds_legacy_medium_helmet_01_01_hd_sec`). Of the static models with a library name,
66 of 69 (LIVE) find it; the rest name libraries the Archive does not hold
(`Objects_default_grid_blue`, `kbar_s1-3_bas_1`). A library that is missing is a status of the
model, not an error: the model reads and exports with the default material.

In an Assembly the library comes from the Entity: the Material of the Record's
`SGeometryResourceParams` geometry replaces the library the model names, and the model's own
library applies only when the Record names none (ADR-0040).

The Archive adapter also accepts one leading forward slash as the virtual `Data` root:
`/Objects/Test/paint.mtl` maps to `Data/Objects/Test/paint.mtl`. An optional
`Data/` immediately below that root is stripped once. Exactly two leading forward slashes
introduce the supported authoring form `//host/prefix[/prefix...]/Data/<relative-library>`.
It requires at least a host and one prefix component before exactly one case-insensitive
`Data` component, plus a non-empty relative library. For example,
`//authoring-host/branch/Data/Objects/Test/paint.mtl` maps to the same Entry.
The host and prefix are labels only; no network or host filesystem lookup occurs. A rooted
reference with a bare final name searches below Data directly, without the sibling fallback.

Every component is validated before lookup, including the discarded authoring prefix.
Empty, `.`, `..`, colon/drive/stream syntax and control characters are rejected. Three or
more leading slashes, a backslash-rooted path, a UNC path without the admitted forward-slash
authoring form, no `Data` component, multiple `Data` components in an authoring reference,
and a missing relative suffix are `Unreadable`. Mixed separators after the admitted leading
forward slashes and ASCII case-insensitive Entry matching remain supported. More than one
matching physical Entry is ambiguous and is refused, even when the bytes match.

Only the Archive adapter applies these mappings. The default companion adapter and the disk
adapter retain the relative-path contract; rooted/authoring references never cause a disk read.
The disk adapter reads an admitted `<last component>.mtl` beside the model.

The original source spelling stays in `Model.material_library`, `MaterialLibrary.path` and
`MaterialStatus` diagnostics. The optional `MaterialLibrary.target` /
`MaterialStatus::target()` is the matched physical Entry spelling, or the canonical attempted
`Data/...` path when no Entry exists. It is absent when admission fails, a mapping is ambiguous,
the resolver does not report a destination, or a library is parsed directly. Model JSON adds
`library.target` with the same nullable contract. A missing or unreadable mapped library names
both the original reference and the target; missing libraries still export the default material.
The shared layer cache retains its separate relative layer-path contract.

Synthetic and optional local consumer verification is documented in
[material-reference-verification.md](material-reference-verification.md).

## Plain colours in glTF

| glTF | from |
| --- | --- |
| base colour | `Diffuse`; for metals `Specular` |
| alpha | `Opacity`; 1 with `AlphaTest` (masked); at most 0.3 for `GlassPBR` |
| alpha mode | `MASK` with cutoff `AlphaTest` when above 0; `BLEND` for `GlassPBR`, the decal shaders `MeshDecal` and `Decal`, and `Opacity` below 1; else `OPAQUE` |
| double sided | `MtlFlags` two-sided, and the decal shaders |
| metallic | 1 when `Specular` is bright (luminance at least 0.5) and the `SurfaceType` starts with `metal` or `Diffuse` is dark (luminance under 0.2); else 0 |
| roughness | 1 - `Shininess` / 255; 0.5 for `HardSurface`, `LayerBlend_V2` and `Layer`, whose gloss comes from textures and layers |
| emissive | `Emissive` times `Glow`, divided by its brightest channel when that exceeds 1 |

These are heuristics for a file without textures, after the findings of the StarBreaker review
(#104). A material that draws nothing (the no-draw flag, the `NoDraw` shader or opacity 0) gets no
glTF material and its submeshes are not written; the summary counts them as hidden. A material ID
beyond the sub-materials or any ID when the library is missing gets one shared fallback surface.
Archive-backed model, Entity, Interior and rigged Character GLB exports and their Blender packages
use a verified, drawable single material from the unique
`Data/Materials/material_default.mtl` Entry when that source is available. An explicit
non-material position in an authored list keeps its slot and can use the same physical
fallback. A hidden authored material remains suppressed. Disk models and unreadable or
unsupported archive fallbacks use the synthetic light-grey surface. Interior models
inside SOCPAKs use the same fallback policy as the enclosing game's Archive. The observed CHF
export attributes the emitted material and affected images to the fallback Entry, including
selected material layers and image sources.

`HardSurface` and `LayerBlend_V2` (23,770 and 46,806 materials) take their colour from `MatLayers`
and the tint palette, so their plain colour is the white `Diffuse`; tints are #127.

## Textured materials

Surveyed on 2026-10-20 over LIVE: 36,786 distinct texture files in the `.mtl` files, none of them
encrypted (like the models, the `.mtl` files and every `.dds` Entry), so no Archive key is needed
to read them. The tests are `crates/p4kx-extraction/tests/real_archive/textured_materials.rs`
(`#[ignore]`d) and the API tests of `p4kx-gltf` and the CLI tests `model_textures.rs`.

### Which file is which

A texture's `File` is a source image (`Objects/./plate_diff.tif`); the Archive holds the `.dds`
of that name, split into parts and with an alpha tail. The loader is given `Objects/./plate_diff.dds` below `Data`, found ignoring case and the kind of
separator through the path index. A path that is empty, absolute, drive-qualified or has an empty,
`.` or `..` component, or a control character, never reaches the loader and is listed; the engine
placeholders (`$RenderToTexture`, `$TintPaletteDecal`) are no files and are skipped silently.

The slot says little by itself (`TexSlot1` is the diffuse map of `Illum`, `HardSurface`, `Layer`,
`MeshDecal` and `Decal`, but the blend mask of `Organic`; `TexSlot2` and `TexSlot3` hold the
normal map of the shader families that use layers), so the role comes from the slot and the last
`_` part of the file name together:

| role | slot | name ends in | LIVE (largest classes) |
| --- | --- | --- | --- |
| diffuse | `TexSlot1` | anything but `ao`, `blend`, `mask`, `opac`, `id`, `scalp`, `dir`, `dirm`, `hal`, `wear`, `displ`, `disp`, `detail`, `gloss`, `ddna`, `ddn` | `_diff` 49,803 `Illum`, 4,073 `HardSurface`, 3,764 `MeshDecal` |
| normal | `TexSlot2`, else `TexSlot3` | `_ddna` (smoothness in the alpha tail) or `_ddn` | 39,267 + 16,021 `Illum`, 17,626 + 13,907 `LayerBlend_V2` |
| specular | `TexSlot4` | `_spec` | 18,499 `Illum`, 997 `LayerBlend_V2` |

A hair cap (`HairPBR` with `%HAIR_CAP` in its generation mask) has its `_scalp` density map in
`TexSlot1`; it is written like an opacity map (white, the red channel as alpha) and the material is
`BLEND`, transparent without the map. The map of the other body is swapped for the one of the
body folder (`male` / `female`) of the material library (ADR-0213).

Everything else is not read as a material's own map: the second layer (`TexSlot9` `_diff`,
`TexSlot11` `_wear`, `TexSlot13` `_hal`), detail maps (`TexSlot6`) and displacement (`TexSlot8`).
The layers of `HardSurface` and `LayerBlend_V2` are drawn from the `TexSlot1` diffuse and `TexSlot2`
normal maps of their layer files, blended by the `_blend` map of the material (`TexSlot12`), with
the tiling of `UVTiling` and `TexMod` baked into the image; a material without layer maps keeps the
flat colour of its layers (ADR-0097, ADR-0147). `KHR_texture_transform` is not written.

### How the textures look

Of 5,255 sampled files (every seventh), 192 are not in the Archive under the `.dds` name. The
formats: normal maps `BC5_SNORM` with a `BC4_UNORM` alpha tail (the smoothness, 1,047 of 1,066
sampled `_ddna`; the others have no tail and give no roughness), diffuse maps `BC1_UNORM`, `BC3_UNORM` or `BC7_UNORM_SRGB`,
specular maps `BC1_UNORM`. The `UNORM` ones are not flagged as sRGB although they are authored as
sRGB, so every diffuse and specular byte is taken as sRGB, as glTF wants a base colour texture. A
cubemap, array or volume is no surface texture and is listed as unreadable.

### Mapping to glTF

| glTF | made of |
| --- | --- |
| base colour texture | the diffuse map; alpha only for `MASK` and `BLEND` materials, else opaque (an opaque material ignores the alpha channel, and an opaque image compresses better) |
| normal texture | the `_ddna`/`_ddn` map: X and the reconstructed Z (BC5 stores X and Y only, Z is `V(1-x�-y�)`) as decoded, **green inverted**, opaque |
| metallic-roughness texture | green = roughness = 1 - smoothness (the alpha tail of a `_ddna`), blue = metalness from the specular map, red full |
| factors | with a metallic-roughness texture that carries the channel 1, else the plain value of the table above; with a diffuse map the `Diffuse` colour is the base colour factor |

*Roughness.* The smoothness is a linear 0 to 1 value, the same scale as the plain `Shininess`
(1 - `Shininess` / 255), so the image and the factor agree. StarBreaker maps the texture with
`V(1-s)`; no evidence in the files favours that, so the linear inverse is used for both. A
material's own `Shininess` does not multiply the texture (it varies from 0 to 255 over materials
with a normal map, without a pattern that would say it is a multiplier).

*Metalness.* There is no metalness map in the engine. The specular map and `Specular` colour give
it: a texel is metal in proportion to the linear luminance of its tinted specular colour, from
0.15 (a dielectric's reflectance, at most) to 0.5 (a metal's, at least). The base colour of a
metal is its specular colour and its diffuse colour is dark (`Diffuse` 0, `Specular` 1 is common:
the `chipped_paint` and `Metal_*` materials of the Gladius), so with a specular map the image
holds `Diffuse` times the diffuse map moved towards `Specular` times the specular map by the
texel's metalness, in linear light, and the base colour factor is white. The key of that image
holds both colours, so materials that differ only in them do not share it. A diffuse map without
a specular map (or whose specular map failed to load) is a dielectric when the metal heuristic of
the plain colours does not take the material for a metal: the `Diffuse` colour stays a factor and
the image is shared. A metal keeps the plain metal instead: the `Specular` colour is the base
colour factor, the metalness 1, and the diffuse map multiplies the factor untouched. A specular map is only used with a diffuse
map to blend with. Different sizes are sampled to the larger (nearest texel).

*Alpha.* A masked material uses the alpha channel of its diffuse map with the cutoff
`AlphaTest`, which closes the open note of #104 that `MASK` needs the texture alpha; the factor
stays 1. A `GlassPBR` material keeps the alpha factor of at most 0.3 and blends, times the alpha of
its diffuse map; its glass layers and dirt maps are layered materials (#127).

### Green channel of the normal map

glTF's normal textures point green up the image (+Y, OpenGL), and viewers that do not take the
`TANGENT` attribute derive a bitangent that points up the image. The engine's point down. Two
independent measurements on LIVE:

- The stored tangent frame (docs/model-lods-and-vertex-attributes.md): over 600 sampled models
  (4.06 million triangles) `cross(normal, tangent) * w` points along +V, down the image, in
  99.2 % of the triangles, and the tangent along +U in 99.7 %.
- The curl of the normal maps: a normal map of a height field `h` has `(-?h/?x, �?h/?y)`, so the
  change of its red channel down the rows and the change of its green channel along the columns
  are opposite when green points up and agree when it points down, whatever the height. Of
  2,905 sampled `_ddna` and `_ddn` maps, 1,450 with a clear correlation (above 0.2 in magnitude)
  agree in 1,337 cases (92 %): green points down. Per use in a shader family (a second sample of
  every second map): `Illum` slot 2 `_ddna` 92 % (6,397 of 6,928), `LayerBlend_V2` slot 3 `_ddn`
  and `_ddna` 94 % and 98 %, `MeshDecal` 99.8 %. The measurement was checked on synthetic
  height fields of both conventions. A minority of `_ddn` maps of `Hair` (82 % up) and `GlassPBR`
  (71 % up) and a few grilles (`rsi_bengal_grating`, `anvl_rubber_padding`) have green up; they
  stay as the engine has them, and render inverted in the engine too if they are authored wrong.

So the writer flips the green channel of every normal map (`G' = 255 - G`) and negates the
handedness `w` of the tangents, which makes the frame `(T, cross(N, T) * w')` point up the image
as glTF's does; a viewer that uses the `TANGENT` attribute and one that derives its own agree.
`TANGENT.w` is written for every export, with or without textures.

### Images in the file

The PNGs are embedded in the binary chunk of the GLB, one buffer view per image, so the file is
one piece; writing them next to it would break a GLB moved on its own. An image is identified by
what it is made of (its source files, with the colours that go into it): materials that share it
share it in the file, and the summary counts it once. The most common sharing is the normal map
across the materials of one ship. At most 256 images and 2^29 pixels (a 4k texture is 2^24) are
embedded; the images beyond are listed as over budget and their materials keep their colours. A
single texture of more than 2^26 pixels is refused. They are decoded a few at a time and encoded
in parallel. `--texture-mip n` selects mip `n`, 0 the largest; a texture with fewer mips gives its
last. The Gladius interior (17 images) takes 6.1 s and 19.2 MB at mip 0 and 0.64 s and 4.4 MB at
mip 2.

Not examined: emissive and occlusion maps (`TexSlot9` `_diff` of `Illum` with `%BLENDLAYER` is the
second layer, not emission), the PBR extensions for glass (#127) and `KHR_texture_transform`.

## Texture transfer in GLB

The glTF material slot determines how a consumer reads the texture. Base colour RGB is
sRGB and its opacity remains linear. Normal and packed roughness or metalness channels
carry linear shader data. Proven linear DDS colour is converted to sRGB for base colour,
including layer and specular mixing in linear light. Unknown colour declarations retain
the existing sRGB interpretation and remain unknown in the source metadata.

Embedded image extras record the output transfer and each original declaration. A
decoded colour encoding is present only when the corresponding image conversion
consults it. Rigged GLB receipts address these individual values and their original
DDS header words. See the [glTF material specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials).
