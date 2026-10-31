# Hair, brows and beard follow the expressions of the head with morph targets of their own

Decided for issue #546. It amends ADR-0090 (what grows on the head follows the shaped head) and
ADR-0091 (only the head carries the expressions).

## Decision

**A piece that follows the head gets a morph target for each expression target of the head.** The
head's targets are sparse moves of head vertices. A vertex of the hair takes the move of the head
vertices within 10 cm around it, nearer ones counting more, against an anchor that does not move:
the same kernel with which the hair follows the neutral shape (ADR-0090). The result is a sparse
target with the head target's name. A vertex whose move is under 10 micrometres or under a fifth of
the target's largest move is left out of the target, and a target that moves no vertex is left out.
The largest moves are millimetres, so the cut-off is below a millimetre.

**The weights follow the head's.** A facial animation (#154) sets the weights of the head's
targets from the blendshapes of the rig (ADR-0098). A piece that follows the head records which
head target each of its own targets stands for and takes that blendshape's weight, so hair, brows,
lashes and beard move with the face in the same frames. The animation budget counts these weights
too. A piece whose targets do not match the head's blendshapes takes no weights and is listed as
`follow_weights_skipped`.

**Scope.** Only the first mesh of the first level of detail of the piece gets targets, as in
`p4kx_gltf::RigPiece::with_morphs`; a piece with several meshes is listed as
`follow_morphs_first_mesh_only`. The eyes and teeth are shaped by their own rigs and have no
expression targets. Amendment (#726): the facial joints (ADR-0098) are followed too, by binding the
hair to them (ADR-0299). Amendment (#961): at every level of detail, from the targets the head has
there (ADR-0305).

## Consequences

The hair, brows, lashes and beard of a Character with expressions on carry sparse targets, which
adds to the size of the GLB; `--no-expressions` leaves them out together with the head's. On the
default characters at LOD 0 with all expressions, the hair pieces have about 900 targets with
about 1,600 (masculine) and 3,400 (feminine) vertices each, and the GLB grows from 9 and 12 MiB to
33 and 58 MiB, the head's own targets included. The work is data-parallel over the targets, and a
target visits only the hair vertices within 10 cm of a vertex it moves.
