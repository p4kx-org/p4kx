# Stencil decals are cut out by their stencil map and never drawn as solid quads

Decided for issue #856. It amends ADR-0100 (the add-on's material graphs), whose decal family drew
every `MeshDecal` as a plain surface, and the GLB writer's decal rule.

## Context

A `MeshDecal` with `STENCIL_MAP` in its generation mask draws a motif: the red channel of the map in
`TexSlot7` is its opacity, `StencilDiffuseColor` its colour, `StencilOpacity` its strength. The
package named that map `other`, so both drawings ignored it and drew the whole quad in the diffuse
colour (the ANVL Paladin's branding was a white quad at 80 %). Of the roughly 1,900 stencil decals
in LIVE, about 1,400 have neither a diffuse map nor a file in `TexSlot7`: they hold the placeholder
`$TintPaletteDecal` (the game fills in a livery's emblem at run time) or nothing. About 200 name a
stencil file, and the file names say nothing (`_stencil`, `_a`, `_diff`, `_blend`, `_mask`,
`_curves`, `_edgewear`), so the slot decides. About 210 have a diffuse map. `stencil_census.rs`
prints that census for a real Archive.

## Decision

**One rule, in p4kx-model.** `Material::is_stencil_decal` (`MeshDecal` with `STENCIL_MAP`),
`stencil_texture` (the file in `TexSlot7`, none for a placeholder or when a diffuse map is there),
`stencil` (the parameters), `stencil_motif` (the parameters of a decal drawn by its stencil map) and
`is_stencil_without_motif` live in `mtl_stencil.rs`. What a diffuse map is has one predicate,
`is_diffuse_map` (the role of the file, so `TexSlot1 = x_id.tif` is none), which the GLB writer's
texture sources use too. Every decal is drawn by exactly one of its stencil map, its diffuse map or
nothing; `stencil_census.rs` checks that on a real Archive.

**The map.** `TextureRole::Stencil` (package usage `stencil`, format 1.6) is the file in `TexSlot7`
of a stencil decal drawn by it. A decal with a diffuse map of its own keeps being drawn by that map;
the stencil map is then not a stencil, so the usage is `other`.

**The stencil parameters count for the motif only.** `StencilDiffuseColor`, `StencilOpacity` and
`StencilTintOverride` apply to a decal drawn by its stencil map (`stencil_motif`), in the GLB, the
package and the add-on alike. A decal drawn by its diffuse map keeps its own colour and opacity.

**The GLB.** The stencil map is the opacity map of the decal, as the opacity map of hair cards
(`Source::opacity`): the base colour image is white with the red channel as alpha, the base colour
factor is the stencil colour, and the alpha factor is the material's opacity times `StencilOpacity`
(0 stays 0). `Shading::colour` takes the stencil colour, so the viewers draw the same.

**A map that cannot be drawn.** A path `canonical()` refuses, a missing file and a file that does
not decode all leave the decal without a base colour image, whether it is drawn by its stencil map
or by its diffuse map. Like a hair cap, every stencil decal then has an alpha factor of 0 (blended):
it is fully transparent, never a solid quad. This agrees with the package, which states such a
decal as not drawn. The primitives stay in the GLB (the decision needs the load, which comes after
the geometry plan), and so does the colours mode, which loads no image, leave a stencil decal
invisible. The failure is reported as a texture fallback as before.

**The package and the add-on.** The package states `stencil` (`drawn`, `colour`, `opacity`) per
stencil decal. `drawn` is the final decision, made after the textures are written
(`settle_stencil`): false for a decal with no motif and for one whose stencil and diffuse maps were
not written. Colour and opacity are stated for a decal drawn by its stencil map only. The add-on
builds a Transparent BSDF when `drawn` is false, and also when `drawn` is true but no stencil or
diffuse image loads in Blender (an image that cannot be loaded would otherwise fall back to a solid
quad). It takes the alpha from the red channel of the `stencil` texture (which wins over a mask map,
the damage mask of the same material), the colour from `stencil.colour`, else `diffuse`, and
multiplies the opacity by `stencil.opacity`. Packages before 1.6 state no `stencil`; the add-on
draws their decals as it did before #937 and keeps no copy of the rule.

**No motif, no quad.** A stencil decal with neither a stencil map nor a diffuse map has nothing to
draw. `Material::is_colourless_decal` is true for it, so the GLB leaves it out.

**`StencilTintOverride`.** Above 0 it is read as: the decal takes the livery's colour. The tint
palette the package reads has no decal colours, so a decal drawn by its stencil map takes the primary
entry, as the add-on already did for `$TintPaletteDecal`: `Material::palette` is `primary`, and the
existing palette rule (entry tint wins over `StencilDiffuseColor`) applies in the GLB and the
add-on alike. A decal without a motif is not drawn, so its override has nothing to tint and its
`palette` stays absent; the same holds for a decal drawn by its diffuse map.

## Consequences

`StencilDiffuseColor2` and `StencilDiffuseColor3` stay custom properties: whether the green and blue
channels of the map select them is not settled by the files. The palette record's own decal colours
are not read; reading them would replace the primary-entry approximation. Both are follow-ups.
