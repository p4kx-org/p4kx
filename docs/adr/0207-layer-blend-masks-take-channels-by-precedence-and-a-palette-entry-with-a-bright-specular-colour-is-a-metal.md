# Layer blend masks take channels by precedence, and a palette entry with a bright specular colour is a metal

Amended by ADR-0400: explicitly selected HardSurface palette finish multiplies sampled
smoothness before its linear inverse; GLB layer roughness uses the same finish controls.

Decided for issues #730 and #733. It amends ADR-0147 (the share of the blend mask), ADR-0100 (the
add-on's surface mapping) and ADR-0195 (painted materials). Evidence: the LIVE Archive, imported
into Blender 5.2 with the add-on and compared against in-game screenshots and the in-game item
renders of the Star Citizen Wiki: the ORC-mkV Hurston Edition worn by
`NPC_Archetypes-Male-Human-Guards-Mission-PrisonTransport_Shotgun`
(`m_cds_legacy_medium_armor_01_01_hd_sec.mtl`), the Cutlass Black, the Arrow and the MISC Prospector.

## What the data says

- The blend map of a `LayerBlend_V2` material is not a choice of pure colours only. The CDS legacy
  medium armour has red, green and blue regions and also magenta, cyan and yellow ones, and the
  black texels between and around its islands. With black on `BaseLayer1` (bare `iron_scratched`
  with a white tint), half of the guard's arms baked bright white; in the game the armour is black
  with yellow stripes and the iron only shows in a few small regions.
- The colours match the game when the channels are read by precedence: red takes `BaseLayer1`,
  green `BaseLayer2`, blue `BaseLayer3`, a set channel above every later one, and the fourth layer
  takes the texels where no channel is set. On the guard the plates (blue) become `BaseLayer3`, the
  black paint, and the stripes and belt (green, cyan) `BaseLayer2`, the yellow leather. The
  community tool StarBreaker reads the mask the same way.
- The vertex colour red of a `HardSurface` hull is pristine paint at 1, so the wear layer shows
  where it is low; `WearBlendBase` scales it. Read as a share of the wear layer it covered two
  thirds of the Cutlass hull in bare metal.
- A palette entry's specular colour says whether it is paint or metal: the Cutlass Black's is
  0.05 and the hull is dark paint in the game, the Prospector's is 0.62 and the hull is bare,
  brushed metal. A painted layer's specular map (the Cutlass's `panels_a_spec.tif`, mean
  luminance 0.725) is a map of the level, not of the metal.
- The detail map of the Arrow's layers packs the detail diffuse in red and the detail normal in
  green and blue; overlaid with all three channels it tinted the hull mauve.

## Decision

**Precedence.** With a blend mask the share of the four base layers at a texel is `r`, `g·(1−r)`,
`b·(1−r)·(1−g)` and `(1−r)·(1−g)·(1−b)`, normalised over the layers that exist and together
`1 − w` as in ADR-0147. The add-on builds the same order: the last base layer at the bottom, then
the others each by its channel, the first by red on top.

- **Scope.** It applies to every layered material with a blend mask (a texture whose name ends in
  `_blend`), whatever its shader, in the bake and in the add-on alike.
- **Positions.** A layer's channel is its position in `MatLayers`, not its index among the layers
  that could be read. The first four positions are the base layers; a layer after them takes its
  share by position, as in ADR-0147 (`WearBlendBase` spread over the layers after the first,
  normalised over the layers present), and the base layers share what is left.
- **Missing base layers.** The game's fourth layer takes the texels where no channel is set. We
  have no evidence of what the game draws when that layer is missing, so a missing layer is
  skipped and the last base layer present takes what no channel of the layers before it claims.
  The bake (`Layered::shares`) and the add-on (`_by_precedence`) both do this, each with a test.
  A material with no base layer present is drawn from its own colours in the add-on and by
  position in the bake.
- **What the add-on keeps.** Layers after the base layers are blended by their share by position
  (`_rest_shares`); a layer's wear tint is applied as without a blend mask; the specular level
  is that of the first base layer; a material with its own diffuse map keeps that map's colour.
  The wear map (`TexSlot11`) and the `WearLayer`s as such are still not drawn.

**Metal paints.** `p4kx_gltf::shading` applies the ordinary metal test to tinted materials again:
a palette entry with a bright specular colour (the Prospector's `misc_prospector_default`, 0.62,
on `metal_bare_a`) paints a metal, while the usual paints (0.04 to 0.2) stay paint. This replaces
ADR-0195's rule that a tinted material is never a metal, which turned the Prospector black. In the
add-on a layer that takes a palette entry or a `TintColor` other than white takes its metal share
from that entry's specular colour, or the layer's own, and not from its specular map, which only
sets the specular level.

**Wear from vertex colours.** Without a blend map the add-on blends the second layer by
`(1 − red) · WearBlendBase`.

**Detail maps.** The add-on overlays the packed detail map with its red channel only.

**Characters in Blender.** The add-on unlinks the vertex colours from the base colour of the glTF
materials of a Character: they are the engine's masks, which Blender's glTF importer would
multiply in. The attribute stays on the mesh. A scripted import without a window context runs the
glTF importer under a temporary override with a 3D view, and removes the objects it created when
the import fails.

## Consequences

The base colour images of `LayerBlend_V2` materials change, also in the viewer, which uses the same
bake. Clothing whose masks are pure red, green and blue now takes the layers one place earlier
than before: red the first (it took the second), green the second, blue the third, and the
texels with no channel the fourth (they took the first).

## Corbel worn geometry reference

A bounded LIVE 13220821 check followed the original worn geometry root and its level-0 companion.
The directory and payload checks bound both entries independently. The stock model reader
consumed each entry once through the verified end of the stream. Together they held 545,441
stored bytes and 1,036,960 decoded bytes.

The normal geometry-only GLB writer emitted one mesh, three primitives and 23,399 triangles.
One nondegenerate triangle was traced from its original IVO index and UV streams to the emitted
accessors. It retained all three indices in order and all six UV0 components exactly after
half-float expansion. The check establishes a source-to-GLB geometry reference.

A follow-up on the same Build bound all three emitted draws to their original material
slots. It used two verified geometry payloads and six verified material payloads,
totalling 1,048,292 decoded bytes. The three draws retained eight, eight and one ordered
material-layer occurrences. Two occurrences contain explicitly blank paths. The probe
took 9.4 seconds and peaked at 960,950,272 bytes of working set.

The faceplate and frame still need spatial identification. A link from the authored
layer mode to the renderer flags is also needed to establish the tint mapping.
