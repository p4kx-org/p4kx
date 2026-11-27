# Standalone facial rotation verification

Enable the `verification` feature of `p4kx-assembly` and call:

```rust,ignore
let comparison = armour.verify_standalone_face_rotations_from_caf(
    file_name,
    caf_bytes,
    models,
    requested_original_frames,
)?;
```

The method parses the supplied CAF itself, loads the facial rig selected by the supplied
Armour and compares the same four rotation conventions as `verify_face_rotations`. The
existing strict method and normal animation/export behavior retain their contract.

The standalone profile sets unkeyed nonconstant controls to zero and the rig's constant
control at slot zero to one. Each original CAF controller must identify a known, distinct
control slot. There must be a keyed nonconstant control. A scalar-less occurrence is accepted
only with absent rotation keys or exact supported canonical encoded identity keys. Keyed
scalar controls keep their original scalar channel; slot zero remains the rig constant.
Unknown or repeated targets and non-neutral scalar-less payloads are refused. Additive and
skeleton-bound sources are refused under the common facial-only admission.

The format reader's `AnimationController::rotation_identity()` returns an optional
`RotationIdentityEvidence`: `None` means no rotation keys, while a present result carries the
canonical encoded-identity verdict and original frame/value byte intervals. Float unit
identity supports either quaternion sign. Compressed identity uses the exact SmallTree48 or
SmallTree64 encoder code. No approximate-angle threshold or caller-provided proof is used.

`StandaloneFaceRotationComparison::comparison()` exposes the common comparison.
`verification_records()` emits these records in order:

- One `standaloneFaceProfile`, `profileVersion: 1`, with the input name and SHA-256,
  selected facial part/rig, rig sizes and the modeled default rule.
- One `standaloneFaceControl` for every original controller-table occurrence, including
  empty entries. `controllerOrdinal` identifies the physical occurrence; `decodedTrack` is
  null when no decoded track exists. `state` is `drivenScalar`, `constantInput` or
  `modeledNeutral`. `modeledValue` is null for driven controls, one for slot zero and zero
  for other modeled defaults. The original scalar and rotation key counts remain present.
- The common comparison and angle records in their existing format.

`sourceBytes` holds half-open intervals relative to the supplied CAF's first byte. It records
controller ID, descriptor, scalar-key count and, when identity was examined, rotation frames
and values. `canonicalIdentityRotation` is null if there was no identity examination or no
rotation channel; a modeled-neutral row with rotation keys must carry true. These intervals
refer to the original bytes, including occurrences omitted from the scalar evaluation view.
The common comparison retains the original declared end frame and last key frame.

The profile explicitly reports `archiveBinding: notEstablished`,
`runtimeInitialization: notEstablished`, `referenceComparison: notPerformed` and a null
`selectedConvention`. A CAF hash proves the supplied bytes; it does not establish Archive
origin, runtime blending, prior control state, an audio clock or which convention matches
the game. Installed verification must join its own immutable source receipts and perform
the separate visual/reference comparison. No game files or identities belong in repository
fixtures.

The synthetic regressions compare all four candidates against an otherwise identical clip
without neutral placeholders. They also cover empty original occurrences, unchanged strict
refusal, original bounds, constant input, near-identity mutation, active/nonunit rotation,
unknown/repeated targets, default-only clips and additive/skeletal refusal.

The additional retained memory is proportional to controller occurrences, plus the existing
four candidate actions. Existing decoded key buffers move into the scalar view; the rig is
loaded once. Rotation identity checking decodes one consulted scalar-less channel at a time
and retains only its verdict and original coordinate intervals.

Decision: [ADR-0413](adr/0413-source-owned-standalone-facial-comparison.md).
