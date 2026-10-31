# The files leave open which livery colour a stencil decal takes and what the second and third stencil colour do

Decided for issue #979. It amends ADR-0298, whose last two sections left the palette's own decal
colours and `StencilDiffuseColor2` / `StencilDiffuseColor3` as follow-ups. The Archive is LIVE.

## Context

ADR-0298 reads `StencilTintOverride` above 0 as "the decal takes the livery's colour" and draws it
with the primary entry of the tint palette, because the palette the package reads states no decal
colours. Two questions were open: whether the palette Record holds decal colours and how a decal
picks one, and whether the green and blue channels of a stencil map select the second and third
stencil colour. `stencil_params.rs` (p4kx-extraction) and `real_palette_decals.rs`
(p4kx-assembly) measure both on a real Archive.

## Findings

**The palette Record states decal colours.** Every one of the 2,423 `TintPaletteTree` Records
states `root/decalColorR`, `root/decalColorG` and `root/decalColorB` (sRGB bytes, like the paint
entries) and `root/decalTexture` (a string, empty in 1,507 of them). The colours are white unless a
livery sets them: 907 palettes set a red, 587 a green and 536 a blue colour that is not white
(`misc_freelancer_dur_default` sets a dark red and leaves green and blue white). The fields are
named after the three channels of a texture, as `decalTexture` is a texture: they colour the
livery's emblem, the motif the game puts into a `$TintPaletteDecal` slot.

**`StencilTintOverride` is not a flag.** Of the 1,893 stencil decals it is 0 in 1,377, 1 in 326 and
2 in 190. Nothing in the files says how 1 and 2 differ. The material names pair them with colours
(`ident_a_primary_tint` is 1, `ident_a_secondary_stencil` is 2, both with the same stencil map),
which fits a choice between palette entries as well as between decal colours.

**The channels do not select the stencil colours on their own.** Of the 209 materials with a stencil
map, 7 name a file the Archive lacks. In 102 of the other 202 the green and blue channels stay
within 32 levels of the red one, so the map holds one mask; 73 of them set a second or third colour
that differs from the first. Were green and blue to select colour 2 and 3, these decals would paint
one mask in three colours at once. The other 100 maps have independent channels, so the channels
are used by some, but not as a plain selector.

**The rule is not in the Archive.** Which colour a decal takes, and how three channels and three
colours combine, is done by the shader. The Archive holds the compiled shader caches only.

## Decision

Nothing in the drawing changes. The files do not settle either question, so the GLB, the package
(format 1.6) and the add-on stay as ADR-0298 left them: a decal drawn by its stencil map with
`StencilTintOverride` above 0 takes the primary palette entry, whatever the value, and
`StencilDiffuseColor2` and `StencilDiffuseColor3` stay custom properties. The red channel is the
motif's opacity and `StencilDiffuseColor` its colour; green and blue are not read.

## Open

1. **Which colour a decal with `StencilTintOverride` 1 or 2 takes.** Candidates: a palette entry
   (1 primary, 2 secondary), or the palette's decal colours (`decalColorR`, `decalColorG`). The
   primary entry stands in until something decides it.
2. **What the green and blue channels of a stencil map do**, and whether `StencilDiffuseColor2` and
   `StencilDiffuseColor3`, or the palette's `decalColorG` and `decalColorB`, colour them.
3. **The livery emblem**: the decals that hold `$TintPaletteDecal` take their motif from
   `decalTexture`, coloured by the three decal colours. Without the rule of item 2 its channel
   semantics are open too; ADR-0298 leaves such a decal undrawn.

A reference render of a livery decal from the game, or the shader's source, settles all three. Until
then the package and the add-on need no new field: they would only state a colour no reader may
use.

## Consequences

No code, package or add-on change; the format stays 1.6. The two census tests are the evidence and
rerun on any Archive: they fail when `StencilTintOverride` takes another value, when a palette stops
stating decal colours, or when no stencil map holds one mask in three channels with three colours.
