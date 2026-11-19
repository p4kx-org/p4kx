# Source-owned standalone facial comparison

A scalar facial clip can contain controller occurrences with no scalar keys and only a
canonical identity rotation. The standalone evaluator initializes nonconstant controls to
zero and forces its constant control at slot zero to one. These modeled defaults do not
establish the game's runtime initialization, blending or prior state.

Keep `Armour::verify_face_rotations` strict. Add an opt-in, verification-feature-only
`verify_standalone_face_rotations_from_caf` that owns the connection between the exact CAF
byte slice, original controller occurrences, canonical decode and actual selected facial rig.
Do not accept caller-supplied neutrality evidence or change normal character animation.

The format layer returns the encoded identity verdict with the exact original frame and
value intervals. It validates supported Float, SmallTree48 and SmallTree64 channels through
the canonical decoder. Float identity requires zero vector components and unit absolute W;
compressed identity requires the encoder's exact identity code. Near-identity, nonunit and
active payloads do not qualify. A zero key count means absence, independently of unused
format and offset words.

Bind every original occurrence, including empty entries, to a known, distinct control slot
in the actually loaded rig. A nonconstant keyed scalar control is required. Remove from the
scalar evaluation view only scalar-less occurrences with absent or proven canonical identity
rotations. Keep scalar-bearing occurrences unchanged. Unknown controls, repeated targets,
active scalar-less rotations, additive clips and skeleton-bound clips remain errors.

Save the original clip bounds and selected rig before deriving the scalar view. Move decoded
key buffers; do not clone them or reload the rig. The comparison uses all four existing
conventions and retains the original declared and observed frame bounds.

The separate result envelope publishes the CAF SHA-256, selected part and rig, a versioned
modeled-default profile and a row for every original occurrence with file-relative source
intervals. The existing comparison records retain their contract. Archive binding, runtime
initialization, reference-image agreement and selection of a convention remain explicitly
unestablished. Private installed acceptance must bind the byte hash and rig sources to its
own immutable Archive receipt; plain CAF bytes do not confer that origin.

See [the verification contract](../face-verification-standalone.md) for the API and proof boundary.
