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
  and 2 in 190; no other value occurs.
- **Stencil maps hold masks.** The stencil map of the RSI Ursa Medivac logos has different shapes
  in its red, green and blue channels: a thin strip, a line of text, and the cross and lettering.
  Of the 209 materials with a stencil map, 7 name a file the Archive lacks; in 102 of the others
  green and blue stay within 32 levels of red (one mask in three channels), and 73 of those set a
  second or third `StencilDiffuseColor` of their own.
- **The shader is not in the Archive.** It holds the compiled shader caches only.

## Observations in game

Seen by eye in screenshots of the owner, in a hangar. The evidence for the decision rests on one
livery of one ship.

- **RSI Ursa Medivac, Respite livery** (palette `rsi_ursa_medivac_black_white_grey_vip`): the
  entries are greys (60, 106 and 22 of 255); `decalColorR` is a dark red (100, 15, 25),
  `decalColorG` near black (10, 10, 10) and `decalColorB` a grey (144, 144, 144). The logos are
  stencil decals with a `StencilTintOverride` of 2 and a white `StencilDiffuseColor`. In the game
  the cross and the large lettering are light grey, the small "RSI URSA" on the roof is near black,
  and the small "+MEDIVAC" box at the rear has a dark red edge. Red appears in no palette entry and
  not in the stencil colour; it can only be `decalColorR`.
- **Order of overlapping masks.** On the same ship, the long "+MEDIVAC" band on the side of the
  hull is plain light grey with no red tint, though the red mask of the stencil map lies under the
  grey band: where the masks overlap, blue shows and red does not. This is a weak observation (one
  band, seen at hangar light); look at the edge of the band on the side of the Ursa Medivac.
- **MISC Freelancer, standard livery** (consistent with `misc_freelancer_dur_default`): the emblem
  on the blue-grey panel of the rear outer wing flap is darker than the panel and neither rust nor
  white, where `decalColorR` is (10, 10, 10) and green and blue are white. This is the livery
  emblem slot (`$TintPaletteDecal`, `StencilTintOverride` 0), not the decal this ADR draws. Its
  three masks coincide, and red shows.

## Decision

A stencil decal drawn by its stencil map with a `StencilTintOverride` that rounds to 2 paints its
masks with the livery's decal colours: the red mask with `decalColorR`, the green mask with
`decalColorG` and the blue mask with `decalColorB`. `StencilDiffuseColor` does not colour such a
decal. The masks are layers with the mask as their alpha, red below green below blue (provisional,
see Open 2): a colour counts by its mask and by what the masks above leave of it, the colour of a
texel is the weighted mean of the three, and the opacity is the strongest mask. A texel that only
one mask covers partly therefore takes none of the other colours. Any other value above 0, which
LIVE does not have, keeps the stand-in below.

- **Model.** `StencilTint` tells where the colour comes from; `PaletteSlot::Decal` is what the
  material takes (`Material::palette`); `TintPalette::decal` holds the colours, and
  `TintPalette::apply` gives the material `decal_colours` and a white diffuse colour, or no palette
  at all when the palette states none.
- **GLB.** The base colour image of the stencil map is painted with the colours of the livery; the
  base colour factor is white. Images with different decal colours are different images.
- **Package 1.7.** A palette states `decal`, the three colours; a material states the `palette`
  value `decal`. The add-on reads only that: it splits the stencil image into its channels,
  weights and sums the three colours by the masks and takes the maximum of the masks as the alpha.
  A palette without `decal` leaves the decal in its stencil colour and the red channel as its
  opacity.
- **Override 1** keeps the stand-in of ADR-0298, the primary palette entry.

## Open

Each point is settled by one observation in the game.

1. **What an override of 1 takes.** Candidates: the primary palette entry (the stand-in), or a
   decal colour. Observe the two materials `ident_a_primary_tint` (1) and `ident_a_secondary_stencil`
   (2), which share one stencil map, on a livery whose primary entry and `decalColorR` differ.
2. **The order of overlapping masks (provisional: blue on top).** The Freelancer emblem, whose
   three masks coincide, shows the red colour, and layered materials put red over green over blue
   (ADR-0097), so red on top is as likely. Observe a decal whose masks overlap in a wide area.
3. **A second ship.** The decision rests on one livery of one ship. A cheap confirmation is a
   second ship with a decal of an override of 2 in a livery whose three decal colours differ from
   each other and from its paint entries; the census finds them.
4. **The second and third stencil colour.** `StencilDiffuseColor2` and `StencilDiffuseColor3` stay
   custom properties. For an override of 0, whether green and blue select them is unknown: 73
   materials with one mask in three channels set colours of their own.
5. **The livery emblem** (`$TintPaletteDecal`) is not drawn. The Freelancer emblem suggests it
   takes `decalColorR` where the masks coincide; drawing it needs the order of item 2.

## Consequences

The census tests (`stencil_params.rs`, `real_palette_decals.rs`, `real_ursa_decals.rs`) are the
measurements and rerun on any Archive. The Blender package format is 1.7.
