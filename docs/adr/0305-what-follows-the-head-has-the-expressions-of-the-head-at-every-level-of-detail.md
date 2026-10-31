# What follows the head has the expressions of the head at every level of detail

Decided for issue #961; amends ADR-0200 (hair, brows and beard follow the expressions of the head),
whose targets stopped at level 0, and ADR-0293 (the expressions of a lower level head). Evidence:
the LIVE Archive and the stock male head, measured by the ignored test `real_hair_expressions`.

## Decision

- **Same kernel, any level.** The hair, brows, lashes and beard derive their targets from the
  targets the head has at the level built, with the kernel of ADR-0200: each vertex takes the move of
  the head vertices within 10 cm, against the anchor, and drops moves under a fifth of the target's
  largest. The head's targets at levels 1 to 3 are those of level 0 carried over its surface
  (ADR-0293), and the following mesh and the head mesh are both of that level, so nothing else
  changes: the restriction to level 0 is gone.
- **Weights.** A piece keeps, for each of its targets, the head target it stands for. A facial
  animation exists at level 0 only: the head has a face link there and nowhere else. The check
  behind `follow_weights_skipped` therefore applies only where the head has a face link, at level 0.
  Lower levels carry the targets without weights, as the head's own targets are, and report
  nothing.
- **Reports.** `follow_morphs_first_mesh_only` counts the meshes of the level built, not of level 0,
  and its text no longer names level 0.
- **Default.** All expressions at every level, as decided for the head in ADR-0293;
  `--no-expressions` leaves out the targets of the head and of what follows it.

## Cost

Stock male default character, dressed with its hair, brows, lashes, beard and scalp, whole GLB:

| LOD | without expressions | with | growth |
|-----|--------------------:|-----:|-------:|
| 0   | 9,506,264 B | 34,363,628 B | 3.6x |
| 1   | 6,431,504 B | 22,012,688 B | 3.4x |
| 2   | 4,924,516 B | 19,059,076 B | 3.9x |
| 3   | 1,815,356 B |  6,220,872 B | 3.4x |

Targets per follower at LOD 1: hair 96, brows 131, lashes 128, beard 376, scalp 272, of the head's
399; at LOD 3: 106, 162, 119, 371, 268. The real-Archive test asserts that the growth stays under
five times at every level and that the GLB writes every target.
