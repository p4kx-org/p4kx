# What follows the head is bound to the facial joints through the skin weights of the head vertices around it

Decided for issue #726; amends ADR-0200 (hair, brows and beard follow the expressions of the head),
which left the facial joints out, and ADR-0090. Evidence: the LIVE Archive, the shipped CHF files
with a beard, and `facerom_t0_face` (ADR-0098).

## What the data says

The large movements of a face (an open jaw, a blink) are done by the facial joints (ADR-0098); the
blendshapes move vertices by millimetres. The hair, brows, lashes and beard are skins of their own
that are weighted to the body skeleton (`Head`) and to bones of their own chains, so a facial
animation moved the head and the blendshape targets of ADR-0200 only: the beard did not follow an
opening jaw.

## Decision

- **Binding.** A vertex that follows the head takes the bones of the head vertices around it. The
  head vertices within 10 cm weigh as the neutral follow weighs them (ADR-0090: inverse square of
  the distance, faded to zero at 10 cm), and the vertex's own bones weigh as the anchor of that
  kernel (`ANCHOR`), so a strand that hangs far from the head keeps its own binding and a
  vertex on the head takes the head's. Each head vertex contributes its weights as fractions of
  their sum, and the result is the weighted sum over the bones.
- **Skeleton.** The bones of the head skin that the following model lacks are appended to its
  bones by name (`extend_skeleton`, without offsets), so its influences name them; the export maps
  them by name to the skeleton extended by the head, as for any skin (ADR-0079).
- **Bounds.** A vertex keeps its strongest eight bones (`MAX_INFLUENCES`, two glTF sets) and the
  weights are written as bytes that add up to 255, the rounding going to the strongest bone. Ties
  go to the lower bone and the kernel visits the head vertices in a fixed order, so the binding is
  deterministic. A mesh that is not skinned and a vertex with no head vertex within 10 cm are left
  as they are.
- **Scope.** Every mesh of the first level of the following model, at every level of detail, as
  the neutral follow; the morph targets stay at level 0 (ADR-0200; lower levels are #961).

## Measured

On the LIVE Archive, `real_hair_joints.rs`, over the shipped CHF files:

- 67 to 69 % (brows, lashes) and 73 to 100 % (beards) of the weight of the following parts sits on
  facial joints, at most eight bones per vertex.
- `mendo_ren.chf` with `facerom_t0_face` (jaw opening, blinks): the beard moves by up to 54 mm,
  the brows by 7 mm and the lashes by 5 mm through the skin weights; the GLB passes the checker
  and imports into Blender 5.2 headless with 403 bones and one action, the lashes weighted to
  `FACIAL_LOD0_L_EyelidLowerSkin1` and its neighbours.
- That GLB, with plain colours and the clip, is 25.8 MB and has 1,548 morph targets. A vertex with
  more than four bones adds the second influence set (12 bytes), which is the only growth.

## Consequences

The beard turns with the jaw and the lashes follow the eyelids in a facial animation. The
following pieces are no longer skinned to their own bones alone; a viewer that ignores the
facial joints shows them where the head's own skin puts them, as before. The Euler order and
multiplication side of the facial joints stay open (#627) and apply to these vertices as to the
head.
