# Layered materials are drawn from the textures of their layers, blended by the blend mask

Decided for issue #551, stage 2 of #547. It builds on ADR-0097 (layered materials are the blend of
their layer files), ADR-0042 (textured materials), ADR-0138 (the texture budget is checked from
headers and each source is decoded once) and ADR-0140 (`layers_without_diffuse`). Evidence: the
LIVE Archive, the `LayerBlend_V2` material `m_marine_bdu_01_01_eckhart.mtl` of Miles Eckhart's
clothing with its layer files and textures, the `HardSurface` materials of `AEGS_Gladius.mtl` with
theirs, and the renders named below.

## What the data says

- `LayerBlend_V2` (clothing, props): a material holds no diffuse map. It holds a normal map
  (`TexSlot3`, `_ddn`), a wear map (`TexSlot11`, `_wear`, grey), a **blend map** (`TexSlot12`,
  `_blend`) and `TexSlot13` (`_hal`). It lists eight layers, `BaseLayer1` to `BaseLayer4` and
  `WearLayer1` to `WearLayer4`, each with a `TintColor` and its own `UVTiling` (20 to 180).
  Looked at in the atlas of the BDU, the blend map has in nearly every texel a pure black, red,
  green or blue, in regions that follow the garment's panels, pockets and seams: it is the choice
  between the four base layers.
- `HardSurface` (ship hulls, weapons): a material holds no texture at all and two layers,
  `Primary` and `Wear`, with `PaletteTint`; the choice between them is made by the vertex colours
  (`%VERTCOLORS`) and `WearBlendBase`, neither of which a texture can hold.
- A layer file is a material of the shader `Layer`: `TexSlot1` the diffuse map, `TexSlot2` the
  normal map with smoothness (`_ddna`), `TexSlot7` a detail map, in a `HardSurface` layer also
  `TexSlot8` a `_blend` of its own. The slot carries a `TexMod` with `TileU` and `TileV`. The
  layer's `Diffuse` is 1 where a texture carries the colour (`vhcl_lf_panels_painted_fine_a_base`)
  and below 1 where it is a tint of it (`cotton_02`, 0.89).
- The data does not say which `UVTiling` and `TileU` combine, which channel of the blend map is
  which layer, how the wear map weighs the wear layers, or what a layer's own `_blend` does. The
  decisions below are the readings that agree with the atlas and the game's look; they are
  recorded so that a later finding can replace them.

## Decision

**What is drawn.** In the textures mode a material with layers whose files were read (ADR-0097)
and no diffuse map of its own gets a base colour image blended from the layers' diffuse maps, and,
when a layer has a normal map, a normal image blended from them, in place of the flat colour and
of the material's own normal map alone. A material that has a diffuse map, or whose layers have
neither a diffuse nor a normal map, is drawn as before, byte for byte.

**Layers.** Each layer has the diffuse colour `layer Diffuse x tint` of ADR-0097 (the tint of the
palette entry for a palette layer), which its diffuse map multiplies in linear light, and moves
from that towards its specular colour by its metal share, as the flat colour does. A layer without
a map, or whose map cannot be loaded, contributes that flat colour and is reported when the map
failed. At most eight layers are blended; the layers beyond keep out.

**Shares.** Without a blend map the layers have the shares of ADR-0097 by position and wear share,
so the image averages to the flat colour. With a blend map (`_blend` in any slot of the material)
the first four layers have the shares of the map (assumed: the first the rest of 1 less red,
green and blue, the second red, the third green, the fourth blue, normalised over the layers that
exist; superseded by ADR-0207: red, green and blue take the first three by precedence, the rest
the fourth),
taken from the texel of the map, together `1 - w` where `w` is the share of the layers from the
fifth on; those share `w` by position as before. The wear map is not read; `WearBlendBase` is the
only wear.

**Tiling.** A layer repeats `UVTiling x TileU` times over U and `UVTiling x TileV` over V
(assumed: the two multiply; each capped at 4096; a malformed or non-positive value is 1). The image has the size of the blend map,
else of the first layer's map whose header can be read, at the texture mip; a blend map that is
missing, cannot be loaded or has no pixels is treated alike: the layers have their shares by
position, and the failure is listed once. A map is averaged down by a box filter, in
linear light for a diffuse map, to the texels one repeat covers in the image before it is sampled
with wrap-around and bilinear interpolation, so a layer tiled 80 times gives its mean colour with
the structure that survives and no noise; the average is the "cheaper first step" of the issue and
falls out of the same code.

**Normals.** The slopes (X and Y) of the layers' normal maps are weighted by the same shares and
added, with the slopes of the material's own normal map (`ddn`), which is sampled at the same texel
and not tiled; Z is rebuilt, Y inverted for glTF as for every normal map. The metallic-roughness
image and the factors stay as they are: roughness and metalness come from `shading`.

**One image, once.** An image is keyed by what it is blended of (the blend map, and per layer its
map, repeats, share and, for the base colour, colour, specular colour and metalness), so
materials with equal blends share it; the normal key leaves the colours out, so the palettes of a
hull share one normal image. The maps count with the sources of the job in the decode groups of
ADR-0138: each is decoded once per group, in parallel and with nothing waiting on another, the
size of the image is reserved from the header of the blend map or the first layer's map that can
be read, and a layered job whose sources together exceed 2^27 pixels by their headers is refused
before anything is decoded (a source whose size is not told counts for the whole limit, one whose
header cannot be read for nothing, as it cannot be decoded), so no job decodes more than the limit.
A map without pixels counts as not loaded. The budget of 256 images and 2^29 pixels of ADR-0042 is
unchanged; a refusal for the image count says so in its own reason, and a layered image is named
by the texture that sizes it followed by `(layered base colour image of N layers)` or
`(layered normal image of N layers)`. The normal images of layered materials are made
after every other image, so that when the images run out every material has its colours before a
layered one has its normals. A layered material whose base colour image was refused keeps its
flat colour; one whose layered normal image was refused is drawn with its own normal map, if it has
one, which is made after the layered normals, and otherwise without; each refusal is reported.
Such a fallback is made only when needed (#660): it is skipped, with no decode and no image slot,
when every layered normal image that has it as its fallback was made, and a group of images ends
before a fallback whose layered normal images are still to be made. A plain normal image that some
material draws as its own is not a fallback and is always made.
A texture path of a layer or of the blend map that is not safe is a `texture_fallbacks` entry too.
The base colour image of a layered material keeps the metalness factor of its layers.

**The viewer.** `p4kx_gltf::layered_base_textures` makes the base colour images of the layered
materials of one wave of the viewer's textures with the same code: a few at a time under one
shared source store, so that the textures of one wave are decoded once and no more than the
decode limit plus the held cap is in memory, whatever the number of materials; a material whose
sources exceed the limit by their headers gets none. `layered_base_key` names an image. The viewer draws such a material with the
image and without the colour factor, and keeps drawing the vertex colours as before.

**Reporting.** `layers_without_diffuse` counts the layered materials that no layer gives a diffuse
map to (`draws_layer_diffuse`), no longer every layered material without a diffuse map of its own.
A layer map that cannot be loaded is a `texture_fallbacks` entry as for any texture.

## Measured

`p4kx armour glb --loadout NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart --materials
textures` on LIVE: 15 images (13.5 million pixels) before, 31 (30.2 million) after at the default
mip; the nine layered materials of the shirt, pants, boots and jacket that `layers_without_diffuse`
listed are drawn from their layers (the pockets, panels, knee pads, zip and cuff stripes the blend
map chooses appear), and none is listed. `p4kx entity glb AEGS_Gladius --materials textures
--texture-mip 2`: 165 images before, 256 (the cap) after; of the materials, 130 had a texture
before and 186 have after; the hull panels show their seams and the fine layer structure, and the
normal images of 87 later layered materials, and the plain fallback of one of them, are refused for
the cap with the reason that the image count is spent; no other image is refused.

## Consequences

The vertex colours of a ship (`%VERTCOLORS`), which choose between the `Primary` and the `Wear`
layer in the game, are still multiplied into the colour by viewers, as before; the hulls look
as they did apart from the seams and the structure, until those are read as shares. The wear map,
the layers' own `_blend`, the detail map (`TexSlot7`) and `TexSlot13` are not drawn. A large
Entity such as a whole ship with every part reaches the 256 images and refuses the later normal
images; a higher mip does not help, only fewer parts or a later change of the cap. Several layers
decoded for one image are the cost: at mip 0 a layered material can hold nine 2048 pixel maps.

## Considered Options

- Bake with the mean colour of each layer only: cheap, but it leaves the blend map unused and the
  clothing as flat as before; kept as what a heavily tiled layer averages to.
- Sample the layers in the shader with `KHR_texture_transform` and several texture slots: glTF has
  no way to blend textures by a mask in its core material; rejected.
- Read the wear map as the weight of the wear layers: the wear layers' choice is not in the data
  (ADR-0140 lists it among the unread maps), and a guess would repaint the clothing; left for a
  finding.
- Put the layered normals first: every layered normal image costs a slot that a plain material's
  colour would use; rejected for the order above.

## Amendment (issue #1001)

The vertex colours of a layered ship hull are no longer multiplied into the colour by viewers of a
GLB: it carries none for layered surfaces (ADR-0318). The package keeps them for the add-on.
