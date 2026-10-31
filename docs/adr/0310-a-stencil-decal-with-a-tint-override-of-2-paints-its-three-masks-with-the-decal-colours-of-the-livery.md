# A stencil decal with a tint override of 2 paints its three masks with the decal colours of the livery

Decided for issue #979. It amends ADR-0298.

## Context

A stencil decal drawn by its stencil map takes the primary entry of the tint palette when its
`StencilTintOverride` is above 0 (ADR-0298). Every `TintPaletteTree` Record also states
`decalColorR`, `decalColorG` and `decalColorB`, and the stencil maps hold up to three masks, one in
each colour channel. The data alone does not say how they belong together; in-game observations
and measurements on a real Archive do, for part of the question.

## Findings

Measured on LIVE; the counts are a snapshot of that build, not invariants. The census tests assert
the shape of each finding, not the counts.

- **Palettes state decal colours.** All 2,423 `TintPaletteTree` Records state the three decal
  colours (sRGB bytes) and `decalTexture`. They are white unless a livery sets them: 907 palettes
  set a red, 587 a green and 536 a blue colour that is not white.
- **`StencilTintOverride` is 0, 1 or 2.** Of the 1,893 stencil decals it is 0 in 1,377, 1 in 326
  and 2 in 190.
- **Stencil maps hold masks.** The stencil map of the RSI Ursa Medivac logos has different shapes
  in its red, green and blue channels: a thin strip, a line of text, and the cross and lettering.
  Of the 209 materials with a stencil map, 7 name a file the Archive lacks; in 102 of the others
  green and blue stay within 32 levels of red (one mask in three channels), and 73 of those set a
  second or third `StencilDiffuseColor` of their own.
- **The shader is not in the Archive.** It holds the compiled shader caches only.

## Observations in game

Seen by eye in screenshots of the owner, in a hangar:

- **RSI Ursa Medivac, Respite livery** (palette `rsi_ursa_medivac_black_white_grey_vip`): the
  entries are greys (60, 106 and 22 of 255); `decalColorR` is a dark red (100, 15, 25),
  `decalColorG` near black (10, 10, 10) and `decalColorB` a grey (144, 144, 144). The logos are
  stencil decals with a `StencilTintOverride` of 2 and a white `StencilDiffuseColor`. In the game
  the cross and the large lettering are light grey, the small "RSI URSA" on the roof is near black,
  and the small "+MEDIVAC" box at the rear has a dark red edge. Red appears in no palette entry and
  not in the stencil colour; it can only be `decalColorR`.
- **MISC Freelancer, standard livery** (consistent with `misc_freelancer_dur_default`): the emblem
  on the blue-grey panel of the rear outer wing flap is darker than the panel and neither rust nor
  white, where `decalColorR` is (10, 10, 10) and green and blue are white. This is the livery
  emblem slot (`$TintPaletteDecal`, `StencilTintOverride` 0), not the decal this ADR draws.

## Decision

A stencil decal drawn by its stencil map with `StencilTintOverride` of 2 or more paints its masks
with the livery's decal colours: the red mask with `decalColorR`, the green mask with
`decalColorG` and the blue mask with `decalColorB`, over each other in that order (the blue colour
on top), with the strongest mask as the opacity. `StencilDiffuseColor` does not colour such a decal.

- **Model.** `PaletteSlot::Decal` is what the material takes (`Material::palette`);
  `TintPalette::decal` holds the colours, and `TintPalette::apply` gives the material
  `decal_colours` and a white diffuse colour, or no palette at all when the palette states none.
- **GLB.** The base colour image of the stencil map is painted with the colours of the livery; the
  base colour factor is white. Images with different decal colours are different images.
- **Package 1.7.** A palette states `decal`, the three colours; a material states the `palette`
  value `decal`. The add-on reads only that: it splits the stencil image into its channels, mixes
  the three colours by the masks and takes the maximum of the masks as the alpha. A palette without
  `decal` leaves the decal in its stencil colour and the red channel as its opacity.
- **Override 1** keeps the stand-in of ADR-0298, the primary palette entry.

## Open

Each point is settled by one observation in the game.

1. **What an override of 1 takes.** Candidates: the primary palette entry (the stand-in), or a
   decal colour. Observe the two materials `ident_a_primary_tint` (1) and `ident_a_secondary_stencil`
   (2), which share one stencil map, on a livery whose primary entry and `decalColorR` differ.
2. **The order of overlapping masks.** The three masks of a stencil map overlap where two are
   non-zero. The blue colour is on top here because the thin red edge of the Ursa lettering showed
   no red where it lies over the grey band. The Freelancer emblem, whose three masks coincide,
   shows the red colour, and layered materials put red over green over blue (ADR-0097), so red on
   top is as likely. Observe a decal with overlapping masks.
3. **The second and third stencil colour.** `StencilDiffuseColor2` and `StencilDiffuseColor3` stay
   custom properties. For an override of 0, whether green and blue select them is unknown: 73
   materials with one mask in three channels set colours of their own.
4. **The livery emblem** (`$TintPaletteDecal`) is not drawn. The Freelancer emblem suggests it
   takes `decalColorR` where the masks coincide; drawing it needs the order of item 2.

## Consequences

The census tests (`stencil_params.rs`, `real_palette_decals.rs`, `real_ursa_decals.rs`) are the
measurements and rerun on any Archive. The Blender package format is 1.7.
