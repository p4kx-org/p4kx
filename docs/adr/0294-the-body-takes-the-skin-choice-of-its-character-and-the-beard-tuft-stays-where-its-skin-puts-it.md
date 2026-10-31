# The body takes the skin choice of its Character, and the beard tuft stays where its skin puts it

Decided for issue #760; amends ADR-0090 (a head wears the items of its head port with the
materials its Character chose), which applied only the choices for the head's items. Evidence: the
LIVE Archive, `masculinedefault.chf`, `femininedefault.chf` and the 41 CHF files it ships.

## What the data says

**Skin tone.** A CHF holds a material choice for `body_itemport` beside the one for
`head_itemport`: a base material and, per sub-material (`body_m`, `limbs_m`), the same `BodyColor`
as the head. The Assembly applied only the head-port choices, so the default body kept its own
library (`m_body.mtl`, diffuse `m_body_02`, `SourceAverageColor` 0.41, 0.15, 0.11) and showed
pink next to a head rebased to the dark `BodyColor`.

**Beard.** The strands under the chin of `m_facial_hair_045` are the vertices that the skin
weights mostly to `HH_short_beard_x00_y02`, a bone of the beard's own chain
(`HH_short_beard_x00_y01` below `FACIAL_LOD1_C_Jaw`) that the body skeleton lacks. In the exported
GLB they bind to `FACIAL_LOD1_C_Jaw`, its nearest ancestor in the skeleton extended by the head's
joints, which hangs below `FACIAL_C_FacialRoot` and `Head`: the tuft turns with the head and moves
with the jaw. Its bind position lies about 2 cm below the chin tip.

Measured once by hand on the LIVE skin `m_facial_hair_045`: the raw skin read through
`p4kx model glb` and imported into Blender has 87 vertices whose strongest weight is that bone,
1.4 cm (median, at most 2.6 cm) from the head surface, before any shaping. The ignored test
`the_beards_of_the_shipped_characters_stay_within_a_few_millimetres_of_the_head_they_follow`
measures the following over all shipped beards: it adds at most 6 mm to a strand's distance from
the head, so the following kernel does not cause the tuft.

## Decision

- **The choice for `body_itemport` styles the default body** like a head item: its base material
  replaces the body's library and its colours rebase the skin sub-materials, see `Look`. It is
  carried by `Piece::body()`, a piece that makes no part, which `Head::dressed` appends when the
  Character made the choice; when several are worn only the first counts. A Character without
  one keeps the default body.
- **The beard tuft is a data property.** The skin places those strands there; the export writes
  the bind pose and does not pull them to the chin.

## Consequences

Body and head share the tone the Character chose. Binding hair and beard to the facial joints is
ADR-0299 (#726); the tuft's own bones then weigh as the anchor of that binding.
