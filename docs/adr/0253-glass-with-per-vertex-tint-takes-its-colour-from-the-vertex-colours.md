# Glass with PER_VERTEX_TINT takes its colour from the vertex colours

Decided for issue #797. Amends ADR-0100 (the glass row).

## Context

`Glass_NoRefract` of the Cutlass Black is `GlassPBR` with the generation mask `PER_VERTEX_TINT`,
a white diffuse colour and no palette entry. The add-on gave it a white Transmission colour, the
same as every other glass without a palette entry, so the lamp lenses it covers drew as bright,
untinted glass.

The vertex colours of its triangles are not masks. On the lamp lenses the RGB values are neutral
greys that fall from 0.5 at the lens to 0 at the rim, with alpha 1: a tint. The vertex colours of
`glass_ext` on the same mesh, which has no `PER_VERTEX_TINT`, are the blue and white mask data of
the hull and stay unused for glass.

## Decision

- A `GlassPBR` material whose generation mask names `PER_VERTEX_TINT` takes its
  Transmission colour (Base Color) from the vertex colours: the colour attribute, times the
  description's colour (the `glass` palette entry when the material names it, else the diffuse
  colour) unless that colour is white.
- The vertex alpha is not used; the alpha follows the glass rule of ADR-0100.
- The rule is structural: the shader family and the generation mask, never a material or ship name.
- The node reads the attribute `Color`, which Blender's glTF import writes for `COLOR_0`. Glass on
  a mesh without that attribute drops the tint and keeps its plain colour, because the node would
  read black.
- One rule decides which materials read the vertex colours: `Material::reads_vertex_colours` in
  p4kx-model. It is true for `GlassPBR` with `PER_VERTEX_TINT`, and for a layered surface
  (`HardSurface`, `Illum`, `LayerBlend`, `LayerBlend_V2`, `Layer`, `Simple`) that has no blend map
  (a `_blend`, `_mask` or `_opac` texture) and at least one layer to blend over its base.
- The GLB writer writes `COLOR_0` per primitive, only where the primitive's material reads the
  vertex colours; a mesh with no such primitive has no colour data at all. glTF multiplies `COLOR_0`
  into the base colour, so glass without the flag, such as `glass_ext` next to mask colours, is no
  longer tinted or faded by mask data in glTF viewers (#877).
- The package states the rule's result as `reads_vertex_colours` in every material description
  (format 1.4), and the add-on builds its vertex-colour nodes from it, so the two outputs cannot
  drift. A package before 1.4 has no statement; the add-on then applies the rule it had before.
- The shader families and the texture roles the rule uses have one home each in p4kx-model:
  `Material::is_glass`, `Material::is_surface_shader` and `Material::texture_role`
  (`TextureRole`). The package writer, the GLB writer and the renderer take them from there. A
  texture that is an engine placeholder (`$…`) names no file and counts as no diffuse and no blend
  map; file names are trimmed.
- The glass shader is `GlassPBR` alone: the shader census of the LIVE Archive (the ignored test
  `shader_census` in `p4kx-extraction`) finds 8,293 `GlassPBR` materials, 1,035 of them with
  `PER_VERTEX_TINT` (all read the vertex colours), and no material with the shader `Glass`. The
  add-on dropped `Glass` from its glass family with it.

## Consequences

- Tinted glass is no longer white in Blender; it is as dark as its vertex colours say.
- Glass without `PER_VERTEX_TINT` is unchanged.
- The game's own shader is not available; the rule rests on the vertex data. A glass whose vertex
  colours are masks but which carries the flag would draw dark in the masked parts.

## Amendment (issue #1001)

The GLB writes `COLOR_0` for tinted glass only; the package also writes it for layered surfaces,
where the add-on reads it as layer shares. See ADR-0318.
