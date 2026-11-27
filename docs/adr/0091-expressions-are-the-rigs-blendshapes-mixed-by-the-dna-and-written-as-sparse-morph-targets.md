# Expressions are the rig's blendshapes, mixed by the DNA and written as sparse morph targets

Decided for issue #153. Evidence: `p4kx chf glb` over the default characters the Archive ships
(`masculinedefault.chf`, `femininedefault.chf`), LOD 0: 399 targets for the male and 406 for the
female head, the GLBs passing the checker of the testkit with the sparse and morph target rules
added (`crates/p4kx-assembly/tests/real_archive/real_head_expressions.rs`). No player's file was
used. The layouts of the `RigLogic` chunk and of the expression morphs are format knowledge from
the MIT-licensed StarBreaker project, checked against the Archive.

## What an expression is

The head rig has 495 blendshapes: 205 controls and 290 correctives. The `RigLogic` chunk of the
head skin names them. It is the chunk `0x0A2485B6`, a `u64` with the length of the rig and then the
rig `RIG V1.9`. The controls are in a pool of null-separated names behind the header (`CTL_PSD`,
`CTL_R_brow_down translateY`, `CTL_expressions blinkR` …). Control 0, `CTL_PSD`, is the constant
control and moves nothing. A corrective is a row of five control numbers; it fires when those
controls are active together. It is named by the controls it combines, without control 0 and
each once, joined by `*`. The numbers of the blendshapes in slot 19 of the DNA rig (the expression
morphs) run over the controls and then the correctives, in this order; the reader refuses a skin
whose names do not count as many blendshapes as the rig.

The `u64` in front of the rig is checked against the chunk: it must not exceed the bytes that
follow it, and at most 7 bytes of the chunk's 8-byte alignment padding may remain behind the rig,
which is then cut to the declared length. The chunk is read once per head, for the morph targets
and for the face joints, and the parsed names stay with the head part for the animation.

## The Character's blendshape

Slot 19 holds, per stock head and blendshape, the vertices the blendshape moves on that head. A
Character's blendshape is the sum over the heads of their blendshape at the weight the head has at
the vertex: the weights the DNA gives the head in each region (`DnaMatrix`), summed over the
regions by the vertex's region mask, as the neutral face does it. What the heads leave of 1 belongs
to the base head, whose blendshape is the cell of head 0. A region without blends is the base
head's. The result moves the Character's own neutral face, so a morph target at weight 1 is that
expression on that face (`ExpressionBlend`). Moves under 0.1 micrometre are dropped, and a
blendshape that moves no vertex is not a target.

The cells are indexed by the vertex of the geometry file's stream, like the neutral morphs; the
target is mapped onto the vertices of the model's mesh through `Mesh::source`, which can name one
stream vertex for several mesh vertices.

## The glTF

`MorphTarget` (name, ascending vertices, one move per vertex) is given to a piece with
`RigPiece::with_morphs` and applies to the piece's first mesh. Each target is a `POSITION` delta
accessor that is sparse: it has no base buffer view, only the moved vertices (16-bit indices, 32-bit
above 65535 vertices) and their moves, converted to Y up like the positions. The male head of 5584
vertices has 399 targets that move 221654 vertices together; they cost 3.3 MB (the GLB grows from
1.58 to 4.90 MB), dense they would cost 26.7 MB. The accessor's `min` and `max` are those of the dense data, which includes the 0 of the
vertices the target leaves. Every primitive of the mesh lists the same accessors, `mesh.weights`
are all 0 and `mesh.extras.targetNames` holds the names; the names are written into the typed
`extras` of the mesh when the mesh is pushed (the `extras` feature of `gltf-json` is on), so they
belong to the mesh they are made for. The morphs deform the mesh before the skin does, so they combine with the body skeleton, and
`NORMAL` and `TANGENT` deltas are not written (the viewers recompute the shading and the blendshapes
move vertices by millimetres). A viewer that supports sparse accessors, which glTF 2.0 requires,
shows them; the first eight targets are the ones a viewer must support, the rest may be ignored by a
small one.

The testkit checker validates sparse accessors (bounds and alignment of both views, no target or
stride on them, ascending indices below the count, exact `min` and `max` over the dense data) and
morph targets (the same number on every primitive, a `POSITION` delta of the vertex count with
`min` and `max`, default weights and target names that match).

The Khronos glTF-Validator runs as the CI job `glb-validator`: the package `gltf-validator` at an
exact version from a lockfile with its integrity hash (`.github/glb-validator`) validates the GLBs
that the test `the_synthetic_glbs_are_written_for_the_validator` writes from synthetic fixtures (a
head with morph targets, a skinned mesh, a mesh with per-primitive vertex colors, an emissive
material, and a scene with mixed UV availability). Any error fails the
job and warnings are listed. Real game data is never sent to CI, so the real default-head GLBs are
not validated there; they come from the same writer code paths as the synthetic head.

## What the export does

`Piece::with_expressions` and `Head::with_expressions` take `Expressions`: none (the default), all,
or the named ones. A name matches without regard to case and to the prefix `CTL_`; a name no
blendshape has is `unknown_expression`. `p4kx chf glb` writes all expressions at LOD 0 and offers
`--expression NAME` (repeatable) and `--no-expressions`. Failures that leave the shaped head
untouched are `morphs_failed`: the skin has no `RigLogic` chunk, its names do not fit the rig, or the
rig has no expression morphs. The command exits with code 1 after writing the file. The rigs of
the lower levels of detail (`_lod1.dna` …) have no slot 19; their heads get the targets of level 0
carried over (ADR-0293).

## What is not in it

A dressed head (ADR-0090) gives the expressions to the piece named `head` only: the eyes and teeth
have none. The hair, brows, lashes and beard follow them with targets of their own (ADR-0200).

The blendshapes are the correctives of the rig; the large movements of a face (a blink, an open
jaw) are done by the facial joints, whose deltas are the delta records of the rig (slot 31), and the
amplitude of the targets is millimetres (`CTL_expressions blinkL` moves 702 vertices by at most
2.5 mm). The body skeleton has none of the facial joints (ADR-0084), so the export has the
correctives only, and the Character's own joint positions (`DnaRig::neutral_joint_offsets`) do not
change anything in a static export. Evaluating the rig into weights and joint motion over time, and
the facial skeleton it needs, belong to the face animation (#154). Slot 17 of the rig, which
StarBreaker uses to attenuate the blendshapes of the skin's `BShapesGpu` per vertex, is not applied
here; the cells of slot 19 are used as they are.

Amendment (#589): the `BShapesGpu` layout and its comparison with the cells of the base head of the
DNA rig are in ADR-0281; they agree within one quantisation step. Amendment (#533): ADR-0293 records
why slot 17 stays unapplied and how the lower levels get their targets.

## Considered options

- Dense morph target accessors: 27 MB for a head, eight times the sparse ones.
- Decode the `BShapesGpu` chunk of the head skin and add the Character's correction to it: needs a
  third decoder, and slot 19's cell for head 0 is that data already.
- Enable `extras` in `gltf-json`: changes the type of every `extras` field and its few hundred
  initialisations for one array of names.
- Name the correctives `corrective_<n>`: the names of the controls they combine say which
  expressions they belong to.
