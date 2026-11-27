# Worn clothing hides the body zones its Record covers

Decided for issue #591. Evidence: `p4kx armour glb --loadout NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart`
of the LIVE Archive, rendered from the GLB at the front before and after (a local check, not a
test; the skin of the belly and the thighs showed through before, nothing after), and the
real-Archive tests in `crates/p4kx-assembly/tests/real_archive/real_body_zones.rs`. Builds on ADR-0079 (armour on a
body), ADR-0082 (armour from a Loadout), ADR-0097 (layered materials) and ADR-0109 (story
characters).

## What the data says

Every Entity worn on a body has a component `SCItemClothingParams`. Its `Chunks` are
`SCItemClothingMeshChunkParams`: a `MeshChunk`, the name of a **body zone** (`torso01_zone`,
`l_arm04_zone`, `l_foot_zone`, `underwear_zone`), a `Layer`, the `VisibleLayers` and
`VisibilityConditions`. The body Entity (`m_body_01`, `f_body_01`) lists the 32 zones on layer 0;
a shirt lists the zones it covers on layer 1, a jacket on layer 2, armour up to layer 4. A chunk
whose `VisibleLayers` holds 0 lets the body show through (sleeves that stop above the wrist);
otherwise the body is hidden in the zone. A chunk with `VisibilityConditions` depends on the
items in ports (a backpack). The component also holds `HiddenParts`, ports whose items are not
drawn when the Entity is worn (a hat hides the hair).

The game does not put the clothing over a complete body. The body skin (`m_body.skin`,
`f_body.skin`) is cut into one submesh per zone, and the zones that clothing covers are not
drawn. A `.skinm` descriptor stores a node number. In the version 0x900 IVO layout the
node can index a table after the descriptors. A table word can be matched to a clothing Record's
`MeshChunk` by FNV-1a-32 over the uppercase ASCII name with a terminating zero. The Character
package records a garment primitive's zone only for a unique readable match. On LIVE 13220821
every geometry node of the standard male and female skins resolves this way, to the same zone
as the body table. Hashing without the terminating zero or without uppercasing yields no matches
in either skin. On the same build, the CDS undersuit's two glove node indices match the
item Record. A third node has an unmatched word and its primitives stay in the GLB. Where a static model of a garment has the
zone names (`m_marine_bdu_shirt_01_prop.cgf` names its nodes `torso01_zone`, `l_arm01_zone` …),
its skin's submeshes follow the same order, so the names are real, but the body has no such
file.

## Decision

**Covered zones.** A zone of the body is covered when a worn Entity has an unconditional chunk
for it on a layer above 0 whose `VisibleLayers` lacks 0 (`ZoneChunk`, read by `Wearable::zones`).
Chunks on layer 0 and chunks with visibility conditions cover nothing: the conditions are not
evaluated, and the body is never cut away on a chunk that might not apply. The names are compared
ignoring case.

**The table.** The body mask uses the submesh's node number as an ordinal into the variant's
checked zone-name table. The ordinal indexes a name within the skin and differs from the hash. The male body has 30 zones, numbered 0 to 29; the female body has the bra zone
`underwear_top_zone` as well, which takes 27 and moves the zones after it up (31 zones). All six
levels of detail number alike. `BodyVariant::zone_names` holds the table. It was found by
comparing the two skins: the numbers agree between the male and the female skin zone by zone, and
the order of the names in a shirt's static model agrees with the order of the numbers in the
skin. A zone's side (`l_` has negative X), height, and the order along the arm and the leg were
checked on both skins (`real_body_zones`). The zones `hips_zone` (both bodies) and
`underwear_top_zone` (the male body) have no geometry; the pants cover `hips_zone` and
`underwear_zone`, and `underwear_zone` is the shorts and the female panties and waist band
(materials `underw_m`, `underwear_m`, `waist_m`).

**Fit.** The table is used only when every level of detail of the body skin numbers exactly the
zones 0 to N - 1 of the variant, and, at the first level, the `l_` zones have their vertices at
negative X on average, the `r_` zones at positive X, and the feet lie below every torso zone. A
skin that fails (a new body of a patch) is kept whole and the Armour reports
`body_zones_unknown`, so a patch cannot silently cut the wrong parts. A Record that lists more
than 256 chunks has the rest ignored and reports the same.

**Cut.** `Armour::build` replaces the body's model with a copy whose submeshes and triangle list
no longer hold the covered zones, so every user of the model, the viewer included, sees the cut
(`crates/p4kx-render/tests/armour_zones.rs` counts the triangles of the viewer's scene). The
zones are reported as `body_zones_hidden`. The vertices stay in the model.

## Eckhart

His shirt covers `torso01_zone` to `torso04_zone`, the shoulders, `arm01` to `arm04`, `vneck`
and `underwear_top`; the pants `leg01` to `leg03`, `hips` and `underwear`; the boots the feet;
the jacket the torso, hips and arms on layer 2. The body keeps its head, `arm05` and the hands,
`leg04` (the ankles). 24 of the 30 zones of the male body are hidden. Before, the skin of the
belly, the thighs and the shorts showed through the shirt and the pants.
`real_body_zones` asserts exactly these 24 zones.

## Not decided here

- **Clothing under clothing.** The zones of a garment's own skin are not hidden by a garment on
  a higher layer. ADR-0279 hides whole garments by `HiddenParts` and records why a partly
  covered garment stays whole. The optional Character package map identifies garment primitives
  when the skin word matches an authored zone; applying layer and port conditions is separate.
- **`HiddenParts`.** Applied by ADR-0279: the hat that hides the hair, the suit that hides the
  clothing.
- **Conditions.** ADR-0279 evaluates a condition that names no port and no tag and reports the
  conditional chunks that name a zone of the body.
- **Layer textures.** The clothing's materials use the shader `LayerBlend_V2`, which has no
  diffuse map: its colour comes from the tint palette and the layer maps (wear `TexSlot11`,
  blend `TexSlot12`, `TexSlot13`) beside the normal map. They stay flat colours of their layers
  (ADR-0097); drawing the maps is #551. `Armour::layer_issues` lists, per part, the layered
  materials that have no diffuse map; `armour glb` reports them as `layers_without_diffuse` with
  `--materials textures` only.
- **Holes.** The body is cut for the clothing the Armour holds. A garment that is hidden or left
  out later, as in the Blender package, shows holes in the body there. Follow-up #638 treats the
  related clothing rules.
