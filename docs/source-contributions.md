# Shared source contributions

`p4kx-extraction` supplies the shared origin types used by catalog schema 3 and
field-addressed output companions. Runtime observations and serialized descriptions
have separate authority. This API does not itself migrate every tool output; the
remaining carrier coverage belongs to #1125.

## Runtime observations

`SourceUses` owns one physical-source/use inventory. `observe` accepts an actual
Archive Entry origin, an observed standalone DataForge file, a completed whole-Archive
file observation, or explicitly unknown input. Paths, equal decoded bytes, digest
strings and serialized references do not establish that binding.

`SourceScope::at` records an actual payload, selector or conversion contribution.
Attempted and untouched-reference observations remain visible without contributing
to another value. Foreign-inventory handles are excluded and make the contribution
set incomplete. A `SourceUse` shares its original immutable registered description;
cloning the handle does not copy its reference/location strings or create a registry.
`reference`, `role` and `description` borrow those original facts; `encryption` returns
the classification captured for that byte domain. `reference` and `role` are ordinary
methods, so callers cannot use them in const evaluation.

`SourceUses::freeze` preserves the original sorted wire descriptions. Runtime owner
tokens and handles are not serialized or restored by a metadata reader.

## Physical sources and byte domains

| Observation | Retained meaning |
| --- | --- |
| Archive Entry | Actual indexed Entry, original containing Entries and stored declarations. |
| Whole Archive file | Same opaque Archive input owner, distinct file identity and actual completed byte length; stored encryption is unknown without an honest declaration. |
| Standalone DataForge file | Observed file identity and metadata; original P4K lineage is unknown. |
| Unknown input | No trusted physical binding is claimed. |

The whole-file observation belongs immediately after the existing completed hash/read
and source checkpoint. The producer checks the measured length against the owning
Archive; it must not hash again, substitute an active Entry or reconstruct proof from
the exported root digest. The root-file row is not an invented ancestor of every Entry.

Payload scope includes the source's own declaration and all enclosing Entries.
Archive-directory metadata includes only its enclosing Entries. File metadata has its
own observed domain. `SourceScope::ancestor(position)` selects only an already observed
outermost-first containing Entry and reuses the same inventory. Its metadata excludes
its own payload and all later descendants; out-of-range positions return `None`.

`SourceLocation::Subresource` retains a reader namespace, original ordinal coordinates
and exact field label. It does not turn decoded inner offsets into outer-file offsets.
DataForge positions remain physical Record indices rather than nominal GUID aliases.
Subresource ordinals serialize as canonical decimal strings through `DecimalU64`,
preserving their full range independently of a JSON consumer's numeric precision.

## Field-addressed descriptions

`FieldPath` is an exact escaped JSON Pointer into the complete output record. Object
keys preserve empty text, `/` as `~1` and `~` as `~0`; array indices preserve their
original zero-based positions. The empty pointer addresses an explicitly opaque whole
value. A structured-record producer must cover its applicable fields and elements
instead of using that pointer as a blanket row marker.

`FieldOrigins` is a stable typed map from those paths to the common `FieldOrigin`.
Duplicate ownership is rejected without replacing the existing entry. The origin
retains `containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`: known
encrypted contributions dominate; false requires a complete contributor set of known
plain inputs; other unproved cases remain null. A complete empty contributor set is
valid for a literal tool value. Origin descriptions do not change payload state,
game-text protection, key health or selection completeness.

## Explicit reader policy

`FieldOriginSeed`, `FieldOriginsSeed` and `FieldPathSeed` read descriptive wire metadata.
They implement `serde::de::DeserializeSeed`; the shared types do not expose an unbounded
blanket `Deserialize`. Parsing a description does not create a `SourceInput`,
`SourceScope`, `SourceUse` or trusted original inventory.

| `OriginReaderLimits` field | Consumer policy |
| --- | --- |
| `fields` | Addressed fields in one companion. |
| `metadata_bytes` | Logical copied path/reference storage and fixed carrier storage. |
| `path_bytes` | UTF-8 bytes in one escaped pointer. |
| `field_uses` | Distinct contributing references in one origin. |
| `reference_bytes` | UTF-8 bytes in one document-local use reference. |

One metadata budget spans every field and reference of a companion. Field-count
admission precedes owned key validation/copying even when a streaming map provides
no size hint; reference admission likewise precedes its owned copy. Required nullable
marker keys, duplicate keys/references and inconsistent marker states are checked.
These limits describe retained logical storage rather than exact allocator use.
The caller separately bounds input bytes, JSON nesting and parsing work.

A reader policy is not a shared writer quota or schema maximum. Producers retain
their admitted contributor sets; a consumer that needs a larger aggregate selects
larger explicit limits. In particular, 4,096 references is not a universal catalog
writer limit.

`PathKeys::try_new_observed` observes the borrowed physical Entry within the existing
index-build loop, after successful hashing/index conversion and before insertion.
The original builder delegates to the same loop with a no-op observer. Admission,
cancellation, sorting and observer errors retain their original failure channel;
no second directory scan or candidate vector is introduced.

Verification lives in the Extraction field-origin, reader-admission, runtime-handle,
ancestor and path-index observer tests. Whole-file hash-seam integration and actual
per-field Media emission require their owning producer tests; the shared foundation
alone is not evidence that those output carriers are complete.
