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
`stencil_texture` (the file in `TexSlot7`, none for a placeholder), `stencil` (`Stencil`: colour,
opacity) and `is_stencil_without_motif` live in `mtl_stencil.rs`. The GLB writer, the package and
through the package the add-on all read them; nothing derives them twice.

**The map.** `TextureRole::Stencil` (package usage `stencil`, format 1.6) is the file in `TexSlot7`
of a stencil decal. A decal with a diffuse map of its own keeps being drawn by that map; the stencil
map is then not a stencil, so the usage is `other` and the package does not state a motif that
nothing draws.

**The GLB.** The stencil map is the opacity map of the decal, as the opacity map of hair cards
(`Source::opacity`): the base colour image is white with the red channel as alpha, the base colour
factor is the stencil colour, and the alpha factor is the material's opacity times `StencilOpacity`.
`Shading::colour` takes the stencil colour, so the viewers draw the same.

**The add-on.** The package states `stencil` (`colour`, `opacity`) per stencil decal. The add-on
takes the alpha from the red channel of the `stencil` texture (which wins over a mask map, the
damage mask of the same material), the colour from `stencil.colour`, else `diffuse`, and multiplies
the opacity by `stencil.opacity`. Packages before 1.6 state no `stencil`; the add-on draws their
decals as it did before #937 and keeps no copy of the rule.

**No motif, no quad.** A stencil decal with neither a stencil map nor a diffuse map has nothing to
draw. `Material::is_colourless_decal` is true for it, so the GLB leaves it out, and the add-on builds
a Transparent BSDF.

**`StencilTintOverride`.** Above 0 it is read as: the decal takes the livery's colour. The tint
palette the package reads has no decal colours, so such a decal takes the primary entry, as the add-on
already did for `$TintPaletteDecal`: `Material::palette` is `primary`, and the existing palette rule
(entry tint wins over `StencilDiffuseColor`) applies in the GLB and the add-on alike.

## Consequences

`StencilDiffuseColor2` and `StencilDiffuseColor3` stay custom properties: whether the green and blue
channels of the map select them is not settled by the files. The palette record's own decal colours
are not read; reading them would replace the primary-entry approximation. Both are follow-ups.
