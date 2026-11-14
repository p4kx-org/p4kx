# Animation source export

`p4kx media export ARCHIVE --out DIR --kind animation` writes the shared
`animation.jsonl` stream. The source inventory retains the exact decoded Entry bytes
of selected CAF, DBA and AIM files and the two global animation images through the normal RawReference. Filter expressions,
nested Archive traversal and atomic publication follow the [media export contract](media-export.md).

| Namespace | Indices | Payload |
| --- | --- | --- |
| `animation-entry` | none | Filename family, disposition and chunk count |
| `animation-chunk` | original chunk table ordinal | Numeric type, known type name, layout version, byte order, nullable chunk ID and Entry-relative extent |
| `animation-clip` | original clip ordinal, zero for CAF | Original 48 metadata bytes, full flag word, interpreted timing, physical ranges and DBA name |
| `animation-controller` | clip ordinal, controller ordinal | Original ID and both 12-byte channel declarations, with independent sample outcomes |
| `animation-dba-metadata` | original clip ordinal | Uncovered DBA metadata and name with explicitly unavailable controller data |
| `animation-image-index` | original chunk table ordinal | Global image extent and layout disposition with the CAF prefix, record count and trailer when proved |
| `animation-image-record` | original chunk table ordinal, original record ordinal | Physical CAF record and path field ranges, original path text or lexical error, and the independent loose Entry lookup |
| `animation-image-directional-path` | original chunk table ordinal, observed path ordinal | Count-reconciled fixed-width AIM path field with the exact source span and independent loose Entry lookup |

All ranges are half-open and relative to the original retained Entry. IVO chunk extents come
from the validated directory and may include padding. DBA controller keys can refer to a
shared pool beyond the data chunk's computed extent. The name interval includes the original
NUL byte. Invalid UTF-8 leaves its text view null while the raw name remains recoverable.
Duplicate or empty controller rows retain their own physical ordinals.

When the DBA controller-block cursor cannot continue, a diagnostic generation can
still include each structurally readable metadata/name pair after the last covered
clip. These partial rows keep the original physical ordinal and contain no inferred
controller interval or key count. A metadata row and a clip row never count the same
ordinal twice. Count `animation-clip` and `animation-dba-metadata` together to count
all recorded clip declarations in a diagnostic generation. A malformed name stops
metadata recovery at the last provable record.

Each decoded channel has one `u16` little-endian chunk of original frame indices and one
`f32` little-endian chunk of decoded values. Values are grouped by key. Quaternion components
are x, y, z, w in CryEngine axes, normalized with continuous sign. Translations are x, y, z in
CryEngine axes and metres. Frame and value chunk references carry the source encoding intervals.
The original packed descriptor word has the time encoding in its low byte. Additive metadata
marks the stored clip semantics. The default inventory describes source clips independently.
Additive composition is outside this export.

AIM keeps its raw bytes and chunk table with the `opaque-aim` disposition. An invalid container,
malformed clip or unreadable declared channel produces an explicit issue. A readable sibling
channel and subsequent structurally identifiable clips remain in a diagnostic generation.
`--diagnostic` preserves those rows without replacing a complete index. The provider processes
one retained Entry at a time and writes decoded arrays one channel at a time. It shares the
export's source, record, chunk and output budgets. Clip and controller inventories follow authored
order in full.

The field-origin companion addresses every JSON field. Decoded chunk receipts bind the actual
source descriptor and encoded interval to each emitted artifact occurrence. A source-backed
field from an encrypted Entry is marked independently of the Entry name or suffix derived
from the Archive directory.

## Global animation images

The provider recognizes the canonical `Data/Animations/Animations.img` and
`Data/Animations/DirectionalBlends.img` paths with normal Archive case and slash matching.
A CAF image chunk of type `0x370275B7` v0x902 with prefix format word 2 has an
eight-byte prefix, ordered 308-byte records and an eight-byte trailer. The 256-byte
path field starts at each record's beginning. The ranges recover each original record,
its path bytes and the trailer from the retained Entry. The CAF and DBA path CRC32
words are retained from their original four-byte intervals. The DBA word is null in
the projection when its original value is zero. Duplicate paths keep their own
ordinals. An invalid UTF-8 or unterminated field keeps its physical interval and explicit
disposition while later fixed-width records remain readable. Unknown versions and
malformed framing expose the original chunk and a diagnostic disposition. A canonical
image in a different container format or without its expected chunk retains the same
original Entry and chunk table and records a partial source diagnosis.

The directional chunk `0x6B8DF259` v0x901 starts with a little-endian record count.
Each `animation-image-directional-record` row keeps its physical ordinal, inclusive
record length, original second header word, 256-byte path field and stored AIM path
CRC32. A length walk must reconcile exactly with the end of the original chunk.
`opaque_tail` addresses the bytes after the path CRC through the record's end in
the retained original Entry. The export assigns no pose or transform semantics to
those bytes. A valid path gets a separate `animation-image-directional-path` row
at the same ordinal and ordinary owning-Archive lookup. An invalid path does not
renumber later records. When full framing cannot be established, the path-only
projection still requires all counted and aligned original path fields. An
unreconciled count keeps the `directional-opaque` disposition and original bytes.

The installed replay checked LIVE, PTU, EPTU, HOTFIX and TECH-PREVIEW. All five
channels contained the same directional image bytes. A length-only walk and an
independent aligned path scan agreed at all 551 records and the final chunk end.
The original CRC words agree with IEEE CRC32 of the ASCII-lowercased path. The
common byte image supplies one distinct observed layout.

A record with a fully reconciled counted pose section also has an
`animation-image-directional-pose-index` row at `[chunk, record]`. Its three
four-byte words are the pose count, extension count and an uninterpreted
header word. The header starts 2,156 bytes after the path field begins. Each
`animation-image-directional-pose-block` row at `[chunk, record, pose]` gives
two byte-addressed arrays, `entries4` and `entries3`, including each original
little-endian count word and its value range. The packed value strides are
four and three little-endian `f32` lanes. Consumers can read those lanes from
the retained original Entry, preserving component order and exact bits.
Zero-count arrays keep an empty value range and their count word.

The range named `extensions` holds the trailing 552-byte units as original
bytes. If the counts, boundaries and extension size do not exhaust the
physical record, the exporter retains the outer record and its `opaque_tail`.
The installed common byte image reconciles 8,835 pose blocks across 551 records,
including six opaque units in five records. Coordinate conventions, channel
ownership, header flags and extension semantics are unconfirmed.

CAF and AIM path lookups describe a physical loose Entry in the owning Archive.
They distinguish missing, ambiguous and filtered identities. A successful lookup
needs no target payload read. The path depends on the image payload. A resolved
target identity also depends on the target's directory metadata.

A nonzero DBA selector matches the IEEE CRC32 of the normalized DBA path below
`Data/`. The lookup retains physical duplicate candidates and selection status.
Only a unique selected bank supplies a clip inventory. The `dba_clip` result then
matches the complete CAF path against the actual database names and lists their
physical clip ordinals, name ranges and metadata ranges. The clip data carries
separate payload origins, including encryption. A filtered, ambiguous or unreadable
bank cannot establish a clip match. A bounded or incomplete metadata inventory
reports that its observed candidate list is incomplete.

## Installed Archive verification

The opt-in Media census selects every CAF, DBA and AIM Entry across the root and nested
containers. Each family uses a separate temporary generation. It verifies physical selection,
raw bytes, chunk and clip order, original controller words, declared and decoded key counts,
and the typed sample bytes and source ranges. It reconciles the counts with the header reader
where that reader covers the selected source. Legacy header-budget refusals are counted
separately. The streaming source reader still checks the complete available declarations.
Diagnostic rows are verified and an incomplete census fails the check.

The separate global-image export replay selects the two original root image families and
excludes nested containers. It compares the retained SHA-256 artifacts byte-for-byte against
verified original Entries, checks the CAF prefix, record and trailer extents, and reconciles
the directional path spans with the original chunk and count.

Configure the Archive paths and temporary directory as described in [CLAUDE.md](../CLAUDE.md).
The test uses `P4KX_DATA_P4K` and the optional `P4KX_DATA_P4K_B`, then reports aggregate
counts without recording asset names or payloads.

```powershell
cargo test --locked -p p4kx-media --test real_archive all_selected_animation_entries_clips_and_keys_reconcile -- --ignored
cargo test --locked -p p4kx-media --test real_archive global_animation_images_match_verified_physical_sources -- --ignored
cargo test --locked -p p4kx-media --test real_archive global_animation_image_export_matches_original_path_fields -- --ignored
```

## Optional skeleton bindings

`p4kx media export Data.p4k --kind animation --animation-skeleton Data/Objects/Rig.chr --out export`
requests one exact relative Entry path in each selected CAF or DBA's immediately owning
Archive. The lookup uses the Archive's normal path matching and keeps physical duplicates
ambiguous. A nested Archive resolves the same path independently. The command reads an
include-filtered dependency when it proves one usable candidate. Explicit exclusions keep the
skeleton unread. An existing generation follows `--conflict` as usual. Use a fresh output
directory or request replacement to add bindings to a previous export.

`animation-skeleton` records the directory reference and skeleton disposition. A ready
skeleton supplies the original chunk and bone count. Missing, duplicate, excluded and
unsupported targets retain their explicit disposition. Unreadable inputs, failed validation
and configured limits follow the shared media error and diagnostic policies. One selected
skeleton is parsed and retained per active owning Archive and the shared input budgets apply.

`animation-controller-binding` is a separate row at the same clip and controller ordinals.
It keeps the first authored controller, the first key-bearing occurrence, the first matching
bone and the number of matching bones. Only the first key-bearing controller binds the first
matching bone. Empty occurrences remain available with no bound bone. Key codec errors remain
on the original controller row. Binding metadata describes numeric ID selection and supplies
no additive composition or sample conversion.

The skeleton contributes to fields that use its actual controller table or selected bone
ID. Controller occurrence ordinals and decoded sample artifacts keep their animation
origin. Existing raw controller and sample rows are independent of the binding request.

## Optional entity clip inventory

`p4kx media export Data.p4k --kind animation --animation-entity ENTITY --out export`
adds the selected entity's rig and clip inventory to `animation.jsonl`. The selector accepts
a class name, GUID or Record path in the preferred DataForge of the root Archive. The selected
DataForge and animation inputs follow the ordinary Media filters. Use a fresh output or
`--conflict overwrite` for a different entity after a complete export.

The entity catalogue, rig, parameter and source-link rows preserve the resolved Loadout order,
each original parameter occurrence and the outcome of each independent lookup. A concrete CAF
may use a previous same-list folder selector. A folder links independently observed physical
CAF and DBA candidates in Archive order. Named DBA selectors do not depend on the ordering of
loose CAF clips. Unexpanded rules remain visible as authored declarations. Filtered physical
inputs keep their identities and selection status without byte-derived fields.

Each entity source is inventoried once per physical Entry. Entity clip rows join its physical
clip ordinal to the ordinary animation record when available. The ordinary records own exact
metadata, controller rows and decoded key artifacts. Per-field provenance uses the original
DataForge, parameter and animation inputs consulted for that value.

A concrete CAF alias also has a bank row for each `$TracksDatabase` declaration in its
`AnimationList`, including declarations after the alias. `animation-entity-named-dba-bank`
uses `[rig, alias parameter, database parameter]` and reports the physical source row,
metadata coverage state and number of observed exact full-path matches. Each
`animation-entity-named-dba-match` adds the original DBA clip ordinal to that address and
links `[source, clip_ordinal]` to the entity clip. It retains the authored path and separate
name and metadata byte ranges. Repeated declarations and multiple matching ordinals remain
independent. A bank can have a complete name inventory even when controller reads fail.
Filtered, missing and damaged banks have explicit states. The loose CAF lookup has its own
row. The catalogue records candidates without choosing runtime playback precedence.

This catalogue resolves DataForge Loadouts. External XML Loadout dependencies and unresolved
branches are reported through the incomplete-loadout disposition. To retain usable observed
records alongside an incomplete scope, run with `--diagnostic`.
