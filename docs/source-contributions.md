# Shared source contributions

`p4kx-extraction` supplies the shared origin types used by catalog schema 3 and
field-addressed output companions. Runtime observations and serialized descriptions
have separate authority. This API does not itself migrate every tool output; the
remaining carrier coverage belongs to #1125.

## Gameplay-property occurrences

Canonical gameplay definitions retain the original physical Record/body coordinates
through a flat owner-local transformation arena. Each actual root, Sequence array and
child slot keeps its sourced carrier. Node grouping, root links and nodeIndex are
independent structural controls. They do not acquire a stored integer or Conversion
source. Both sourceType and supported/Unsupported payload choice use the actual
original path.$type Selector; the neighboring sourceType carrier does not substitute
for the payload's own contribution set. Numeric and Enum fields retain their original
Payloads, widths and full paths, without transformation or condition evaluation.

Numeric coverage requires the actual Scale.scale Single receipt on an emitted physical
definition. The owning traversal policy only stages possible coverage; equal bits,
nominal GUID aliases, excluded rows and repeated captures cannot excuse another token.
Public property names and UI templates use the shared TextForms lookup, with exact
source text and separate display conversion. See the [gameplay-property contract](catalog-gameplay-properties.md).
## Runtime observations

`SourceUses` owns one physical-source/use inventory. `observe` accepts an actual
Archive Entry origin, an observed standalone DataForge file, a completed whole-Archive
file observation, an original completed build-manifest read, or explicitly unknown
input. Paths, equal decoded bytes, digest
strings and serialized references do not establish that binding.

`SourceScope::at` records an actual payload, selector or conversion contribution.
Attempted and untouched-reference observations remain visible without contributing
to another value. Foreign-inventory handles are excluded and make the contribution
set incomplete. A `SourceUse` shares its original immutable registered description;
cloning the handle does not copy its reference/location strings or create a registry.
`reference`, `role` and `description` borrow those original facts; `encryption` returns
the classification captured for that byte domain. `reference`, `role` and `encryption`
are ordinary methods, so callers cannot use them in const evaluation.

Current use descriptions always serialize `effectiveEncryption` with the closed states
`encrypted`, `unencrypted` and `unknown`. It is captured at original use registration
and shared immutably by the handle and description; the description's
`effective_encryption` getter returns that same fact. It includes exactly the observed
byte domain and can differ from the leaf physical source's `storedEncryption`.
Descriptive readers may use this evidence to validate an already direct field marker;
it does not replace that marker or restore runtime authority. Absence means unknown
only where the governing older output format permits omission; current descriptions
require the field. Catalog v3 does not accept a missing current field as a legacy default.

An `archiveDirectory` source represents the admitted directory of a successfully opened root
Archive. Its `archiveDirectoryPaths` selector covers the complete half-open range of consulted
path positions, from zero through the directory's admitted Entry count. The positions include
separate occurrences of repeated paths. A successfully opened empty directory has range
`[0, 0)`. The source uses Archive directory metadata and has no containing Entries.
Returned field values continue to identify their physical Entry records individually.

The `archiveDirectoryTreePaths` selector uses the same owning root directory with a
half-open range in EntryTree path order. This order folds ASCII case and path separators,
then compares original path bytes. A folder's descendants occupy one contiguous range in
this order. Entry receipts identify any individual physical occurrences returned from it.

`SourceUses::shared_origin` captures the original contributors common to several values.
`value_with_shared_origin` adds each value's own contributors from the same inventory.
Each value still serializes its complete sorted, deduplicated `sourceUsesRefs` array
and the marker derived from that union. The normal writer streams the shared and local
references together. Calling `FieldOrigin::source_uses_refs` materializes a contiguous
slice for that value.

`SourceUses::freeze` preserves the original sorted wire descriptions. Runtime owner
tokens and handles are not serialized or restored by a metadata reader.

`Archive::observed_entry` borrows an exact admitted Entry; a detached Entry copy with
the same index does not qualify. `ObservedEntry` borrows stored details, while its
`ObservedEntryBinding` retains only the original root/ancestor ownership and fixed
coordinates. Unknown ancestor ownership remains distinct across independent opens;
serialized IDs, structural equality or hashes cannot recreate that authority.
`ObservedEntry::snapshot` is the explicit full-detail copy.

`SourceInput::ArchiveView` registers the borrowed evidence in the same `SourceUses`
inventory as a full Entry origin. `observe_with_details` selects `SourceDetails::Full`
or `CaptureLimited` independently of physical identity and encryption. A limited
observation retains an `archiveEntryProof` with the original decimal `entryIndex`
and `details: "captureLimited"`, the real stored encryption and all ancestor rows.
It does not claim that the payload is absent, unreadable or unsupported. Full detail
can upgrade that same source row; a later limited observation cannot downgrade it
or rebind existing source-use references. Fully proved default full-detail outputs
retain their existing source/use representation. For unproved containing views,
new ancestor registrations reuse their actual already-observed prefix rather than
copy its identity from the current leaf. Such registrations can change subsequent
document-local IDs and row counts; existing rows and use references remain intact.
Repeated legacy full leaf observations retain their original rows. Explicit view
reuse adopts the first actual registered row without removing later duplicate rows.

`SourceScope::at_opaque` and `SourceUses::opaque_use` preserve an actual contributing
source when its original coordinate is omitted. `SourceCoordinateOmission::CaptureLimit`
is reserved for optional coordinates not admitted under the capture policy.
`SourceCoordinateOmission::Unavailable` records a proved contribution whose original
coordinate cannot be supplied by the owning operation. Retain exact original coordinates
whenever available; this reason does not justify dropping reader evidence or replacing
an avoidable traversal cut with an opaque location. Both reasons retain the original
source, byte domain, role and encryption witness. A contributing opaque use makes the
contributor set incomplete; a known encrypted witness still yields true. A read attempt
alone remains Attempted and cannot become an opaque Conversion. These operations impose
no new global metadata, writer or schema quota. The closed `p4kx.field-origins/1` omission
vocabulary admits `captureLimit` and `unavailable`; missing and unknown reasons are refused.
## Physical sources and byte domains

| Observation | Retained meaning |
| --- | --- |
| Archive Entry | Actual indexed Entry, original containing Entries and stored declarations. |
| Whole Archive file | Same opaque Archive input owner, distinct file identity and actual completed byte length; stored encryption is unknown without an honest declaration. |
| Standalone DataForge file | Observed file identity and metadata; original P4K lineage is unknown. |
| Build manifest | Same completed manifest read, basename and byte length; external File lineage is unknown, including malformed content. |
| Unknown input | No trusted physical binding is claimed. |

The whole-Archive-file observation belongs immediately after the existing completed hash/read
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

Pure classification helpers reconcile descriptive facts without creating an input binding.
`SourceDeclarationKind::interpret` applies the original field-shape and raw-word rules;
`SourceEncryptionState::from_declarations` combines all declaration occurrences with the
record format and ZIP flags through the owning Archive classifier. The corresponding
Archive helpers are `DeclaredEncryption::interpret`, `DeclaredEncryption::reconcile`
and `SourceEncryption::from_declarations`. Declaration consensus requires matching states
from every occurrence: contradictory declarations remain unknown. This differs from a field's
contributing-source union, where a known encrypted contributor dominates unknown or plain
contributors. These helpers do not read payloads, restore original handles or establish
that serialized facts describe an authentic Archive.

Partial descriptive facts use the same owning classifier without erasing retained evidence.
`Fact` readers preserve all six existing `SourceValue` states. The neutral
`EncryptionFieldEvidence<T>` distinguishes present, absent, known malformed and unavailable
facts: only unreadable or omitted capture is unavailable; null, invalid and unsupported
source facts are known malformed. An absent CIG length is malformed, while an absent v2
length is its defined shape. Absent ZIP flags use the canonical absent-header rule;
unreadable flags preserve possible alternatives. Known missing/invalid words and available
short bytes cannot become arbitrary encrypted/plain values when another fact is unreadable.

`SourceDeclarationEvidence` distinguishes unavailable occurrences from known malformed or
unsupported occurrences. A known absent occurrence contributes nothing; a known empty or
absent whole list has unknown consensus. A malformed whole list/occurrence retains unknown
evidence; an unreadable list/occurrence preserves alternatives. Capture-limited proof omits
optional full detail independently of these explicit fact states.
`SourceEncryptionState::is_possible_from_evidence` checks the stored state through Archive's
`SourceEncryption::possible_from_evidence`. Known unsupported format differs from unavailable
format detail. These pure descriptions create no original runtime proof.

## Build-manifest observations

`Archive::build_manifest_observation` returns the cached observation from the eager
manifest read performed when the root Archive opens. `BuildManifestOutcome`
distinguishes `Missing`, `Unreadable`, `Parsed` and `Malformed`. Only the two
completed-read outcomes provide a `BuildManifestOrigin`, including an empty or
invalid-UTF-8 manifest that could not be parsed. Its `name` and `byte_length`
getters retain the basename and the original completed byte count.

`BuildManifestOrigin::is_same_input` compares original read ownership. Clones and
containers share that owner; independent reads remain distinct even at the same
path with identical bytes or equal Build values. Later manifest changes do not
replace the cached observation. `Build` equality, public `Build::read_from`
values/errors and its optional public-patch enrichment retain their own contracts.

`SourceInput::BuildManifest` registers this original handle through the existing
standalone `file` source kind and shared inventory. Payload and file-metadata
encryption remain unknown because the external manifest proves no original P4K
lineage. A standalone DataForge read of those same bytes has its own read identity.
Paths, serialized Build fields, digests and source-use identifiers cannot recreate
a completed-read handle.

The Media producer binds that observation before provider callbacks. Its private
inventory/root binding and bounded projection token check coherence of an
already-bound Build; the token supplies no authority on its own. Deserialization
restores no runtime binding. Missing/I/O observations are separate `Attempted`
uses on unknown input. A malformed completed read can contribute to a generated
null through its actual parse selector/conversion, without claiming a stored-null
payload. The [Media Build carrier](media-export.md#build-declaration-origins)
defines both output placements and their exact projection hash domain.

## Canonical catalog text forms

Each available language in a canonical text row has `sourceText` and `displayText`, each with
its own immediate source state, value, encryption marker and original-use references. One actual
`TextSet::text` lookup supplies the winning decoded INI RHS; `display_text` derives the existing
display value once. The existing `(Domain, key)` registry and IDs own both forms. There is no
alternate raw-mode registration or third sourced wrapper around the grouping object.

The original authored key is a Selector. An available-locale lookup adds the actual
`LocalizationLookup` Selector, whose selector reference resolves to that original key use. A
winner adds the actual locale Payload to both forms. The original `IniText` encoding/lossy
receipt adds source-form Conversion for BOM removal, UTF16 transcoding or lossy decoding;
plain valid UTF8 needs no such Conversion. This is an Entry-decoding contribution, not proof
that the winning slice itself contained replacement characters. The display form additionally
uses the actual display conversion at that same lookup. Equal actual Conversion handles may
deduplicate because SourceUse identity describes an input/coordinate/role, not processing stages.

Missing keys retain lookup Selector evidence without a winning Payload or text Conversion.
Placeholders perform no locale lookup; a missing language Entry has no translation map member.
The outer translations carrier unions the actual form contributions. Complete contributors may
still have unknown encryption; publication domain and `x-gameText` are independent. The shared
Forms schema is public-neutral, with protection applied by the existing protected domain/owner.

Summary counts retain the existing display-based occurrence semantics once per original lookup,
including unresolved empty displays and once-per-registration placeholders. Neither form nor
canonical row deduplication doubles or suppresses those counters. The [TextForms contract](catalog-text-forms.md)
documents exact consumer paths and source/display meanings.

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

The owning schemas and metadata reader enforce the same correlations. References
are distinct nonempty strings. False requires a complete set; a complete empty set
requires false, and an incomplete empty set requires null. True always needs an
actual contributing reference. A nonempty set can remain unknown even when complete.
Unknown additive metadata remains allowed, independently of those control rules.

Generic schema definitions include their payload schema name, for example
`SourcedValueOfuint32`. Catalog dataset and canonical-text definitions likewise include
the row or reference domain. Adding an unrelated family therefore preserves existing
reference names when payload names differ. Types that deliberately share a schema name
retain their separate schema IDs and may still need the generator's disambiguation.
This naming does not change exported JSON values, source-state correlations or origin
rules; consumers resolve references through their owning schema.

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
## Material Media origins

The owning Material provider selects the closed `materialLibrary`, `materialFailure`,
`materialSourceLimit` or `materialConfigurationFailure` profile. These profiles extend
`p4kx.field-origins/1` for the unreleased 0.5.0 contract; Material payload versions and
serialized payload bytes remain unchanged. A public provider's matching capability or
namespace does not grant a built-in profile.

Every emitted Material JSON node has its own immediate origin. Container nodes combine
actual retained children and applicable structure selectors. A source-derived default
retains the source field/disposition selector even when its result equals a literal.
Fixed surface basis, unapplied tint/decal controls, namespace tags and configured failure
bounds remain independent tool values. Parse failures retain content contributions;
source-size refusals use directory metadata, and invalid configuration adds no content
contribution. Observations from a wholly failed typed parse do not become successful
field inputs.

The Model observer reports the actual chosen branch during its single parse. Neutral
observations are staged until success, then bound to the original MTL input. A
`material.xml` site's indices are original XML child ordinals from the library root;
its field names identify actual queried attributes or structure selectors. Authored
syntax uses logical retained-projection coordinates and Conversion/Selector roles;
nullable lexical, namespace and binary-text facts additionally retain the actual
encoding selector. A generated null never claims a stored source-null payload. Palette
origins retain consulted predecessors and the chosen branch, including absence/default
selectors, and exclude unused later layers and unused fallback branches. Typed scalar,
vector and float wrapper children share that field's actual interpretation inputs.
An independently cut authored tree cannot erase these observations.

Material layer/texture reference sites use declaration coordinates instead of XML byte
positions: `material-layer-declaration` and `material-texture-declaration` indices are
material and retained typed slot ordinals. `material-shader-reference` indices are
material, original authored attribute and optional selected-token ordinals. These
coordinates never claim an XML attribute or byte offset.

Reference origins use observations from the original candidate enumeration and central
admission loop. Retained candidates preserve order and duplicate occurrences. A bounded
cut retains the first observed discarded candidate without consulting the remaining
tail. Copied target Entry paths use their actual metadata and current MTL selector;
resolution, completeness and uniqueness additionally use the original directory
inventory when that inventory contributes. A whole Archive-file inventory has unknown
encryption; it cannot certify a plain root. Nested directory inventory uses the actual
containing Entry payload. Merely mentioning a target never contributes its payload.

Shader reads keep the original private input handle through cache reuse. Each Material
binding supplies fresh current-token selectors without replaying a parse, lookup or
hash. `shader-property` coordinates retain original Property declaration and Name-field
ordinals. Only admitted candidate prefixes and consulted Name/flag predicates contribute
to Property resolution. An input receipt's digest separately uses its actually completed
whole-payload hash. Source descriptions, digests, identifiers and rewritten `use_at`
values cannot recreate original evidence.

Optional site capture is bounded independently per typed field by the existing record
metadata allowance. A cut emits incomplete provenance and keeps an opaque original
source witness, including known encryption. It does not alter payload admission,
projection status or reference counters, and it does not consume a different field's
capture allowance. Receipt validation and generation reuse bind the same original
source/use inventories and exact occurrence bytes as the other Media profiles.
