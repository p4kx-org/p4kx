# The GLB writes vertex colours for tinted glass only, and the package also for layer shares

Decided for issue #1001. It amends ADR-0253 (which materials read the vertex colours) and
ADR-0147 (the vertex colours of ship hulls). It builds on ADR-0097, ADR-0100 and ADR-0207.

## Context

The RSI Ursa Medivac with the Respite livery (palette `rsi_ursa_medivac_black_white_grey_vip`,
primary, secondary and tertiary entries in greys) drew a purple hull in a GLB, in the colours and
in the textures mode, where the game shows a dark grey and black one. The cause is not in the
palette, a layer tint, the wear share or a colour space step: the palette materials of the hull
(`Painted_Metal_Tiled` and its variants, `HardSurface`, two layers, no blend map) are drawn in
the grey of the palette, and that grey is multiplied by `COLOR_0`. The vertex colours of the hull
meshes are strongly coloured (the mean of the main hull mesh is about 1.0, 0.28, 0.58): they are
the shares of the layers (red: no wear, green and blue: further layers; ADR-0100), no tint. A glTF
viewer multiplies `COLOR_0` into the base colour, so every layered hull of every livery was
tinted by its own mask data. The Blender add-on reads them as shares and always drew the hull
grey. The flat colour and the baked layer images of the GLB already contain the blend of the
layers by position and `WearBlendBase` (ADR-0097, ADR-0147), so a second use of the shares in the
GLB cannot be right.

## Decision

- **Two rules, one per use.** `Material::tints_by_vertex_colours` is true for `GlassPBR` with
  `PER_VERTEX_TINT`, the one material whose vertex colours are a colour (ADR-0253).
  `Material::shares_layers_by_vertex_colours` is true for a layered surface without a blend map.
  `Material::reads_vertex_colours` stays their union and is what the package states.
- **GLB.** `COLOR_0` is written on the primitives of tinted glass only, so no viewer tints a hull
  with mask data. `GlbOptions::layer_vertex_colours` (default off, `with_layer_vertex_colours`)
  also writes them for layered surfaces, for a consumer that reads them as shares.
- **Package.** The package writers (entity, interior and character meshes) set that option, so
  the package meshes carry `COLOR_0` on exactly the primitives of `reads_vertex_colours`, as
  before; the format, the manifest and the add-on do not change.
- `--no-vertex-colours` and `GlbOptions::vertex_colours` switch both off, as before.

## Consequences

- A GLB of a layered ship hull is drawn in the colours of its palette and layers in every viewer;
  the wear detail the shares chose in the game stays undrawn in a GLB, as ADR-0147 said.
- A GLB loses `COLOR_0` on layered materials and so gets smaller; nothing reads it there as a
  colour.
- The Blender package is unchanged: the package of the Ursa Medivac has the same size and renders
  the hull grey.
