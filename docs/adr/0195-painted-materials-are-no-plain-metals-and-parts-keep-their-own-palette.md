# Painted materials are no plain metals and parts keep their own palette

Decided for issue #512. It amends ADR-0080 (palettes of parts) and ADR-0097 (`shading`). Its rule
that a tinted material is never a metal is superseded by ADR-0207: a palette entry with a bright
specular colour paints a metal. Its rule that attached parts inherit nothing from the ship is
superseded by ADR-0223: a part without a palette of its own takes the palette of its ancestor.

## Decision

**A tint is not lost to the metal heuristic.** A material that took a tint palette
(`Material::tinted`) is paint, so `p4kx_gltf::shading` never calls it a plain metal: its base
colour stays the palette's tint, its metalness 0, and its roughness the palette's glossiness. Before,
a dark tint (linear 0.02 to 0.05) with the palette's bright specular colour passed the metal test
and was drawn in the specular colour. Untinted materials, and the layers of layered materials with
their own metal share (ADR-0097), are unchanged.

**Palettes along the Loadout (superseded by ADR-0223).** A part keeps taking only the palette its own Entity's geometry names,
else the default palette of its material library (ADR-0080). The palette of the root ship is not
passed down to the parts attached to it, and a part without a palette of its own stays unpainted.
This is the rule as built and tested (`hardpoint_plain` in the synthetic ship `TEST_Livery`), and
the owner confirmed it: attached parts do not inherit the ship's livery or a Loadout paint. Paint
items of the Paint ports (ADR-0208, variant selection) choose the sub-geometry and palette of the item
they are fitted to and do not change this rule.

**Determinism.** Two runs of `entity glb --materials textures` write identical bytes; a test pins it.

## Consequences

Tinted metallic paints (a bright palette specular colour) are drawn in their tint with a metalness
of 0 in the colours mode and the factors of the textures mode; the specular sheen lives in the
roughness only.
