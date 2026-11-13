# Versioned media export

Providers can write unknown-length encoded output through `MediaWriter::converted_artifact`.
Its callback receives only a bounded protected `Write` sink; every actual output byte shares
the generation's `output_bytes` budget. `ArtifactExtension` accepts a lower-case ASCII
alphanumeric suffix without a dot, bounded by the existing portable filename limit together
with the SHA-256 name. Returned paths are `chunks/<sha256>.<extension>`; an `Artifact` is
returned only after verified placement, manifest admission and converted-output accounting.
Repeated content still charges actual writes and records a converted skip when its exact
output already exists. Typed chunks remain converted outputs; retained metadata chunks do not.
Any sink failure is sticky even when the encoder ignores it. Independent encoder, output,
post-operation source-check and cleanup failures retain their typed causes. A checkpoint
observes cancellation and Archive changes before and after actual output I/O; the operation
does not claim an adversarial immutable snapshot or an encoder processing/memory bound.

`p4kx media export ARCHIVE --out DIR` writes a versioned, streaming media export. Its format is
`p4kx-media`, `format_version: "1.0"`; catalog schema versions do not apply.

```powershell
p4kx media export 'D:\Games\StarCitizen\PTU\Data.p4k' --out 'D:\Exports\media' --kind archive
p4kx media export 'D:\Games\StarCitizen\PTU\Data.p4k' --out 'D:\Exports\selected' --kind audio --filter '*.wem'
p4kx media schema
p4kx media schema --record
p4kx media schema --reference
p4kx media schema --source
```

Repeat `--kind` for an explicit selection. The known names are `archive`, `model`, `material`,
`texture`, `shader`, `animation`, `animation-definition`, `audio` and `audio-control`. Omission
selects every registered provider. The built-in providers are `archive`, `shader`, `material`, `texture`, `model`,
`animation`, `animation-definition`, `audio` and `audio-control`. An explicitly requested kind without an available provider returns a typed
capability failure and a non-zero exit status, even when the selection is empty.

`--kind shader` exports nested `.ext` declarations and ordered Property fields, with exact raw
source ranges and lossless mask strings. Selected `.cfib`, `.cfxb` and `.shcb` caches are inventoried
with explicitly unsupported semantic decoding. Material-to-definition links are separate work;
the provider does not compile or execute shaders. [Shader projection](media-shader-backend.md)
defines its precise typed fields, statuses and counting units.

`--kind material` exports standalone/unused .mtl libraries, original material-set roots,
ordered authored fields, layer/texture references and existing interpreted surfaces without
geometry conversion. [Material source metadata](media-material-metadata.md) defines raw
preservation, physical candidate resolution, bounds and the separate target-readability contract.

`--kind texture` emits a `texture.dds-header` row for each selected colour or alpha DDS
header. The payload shares `texture info --json`'s `source_metadata` fields and adds
the declared header role, available image decoder and expected split count when the stored
length and topology establish it. The fields retain exact original header byte ranges or
Entry directory metadata in the common field-origin receipts. A malformed header produces
a diagnosed row in diagnostic output. Structurally readable words survive normalization
failure. Raw source preservation includes selected split parts.

`texture info --json` exposes the same DDS metadata for the selected header and any readable
alpha header, with field-addressed origins and a physical source-use inventory. Standalone DDS
files retain an observed file binding when available. Original P4K encryption is unknown for
standalone input.

Each selected physical header owns one family graph. `texture.dds-member` identifies every
directory candidate by its original Entry index, with its role, declared size, selection
and key availability. Duplicate paths and unproved raw paths remain separate occurrences.
`texture.dds-expected-part` reports the mip, expected size and physical-match disposition
for each split number established by that header's topology. Missing or mismatched parts
remain explicit. A matching size describes the directory declaration. `texture.dds-family`
records directory completeness and whether all matched path spellings were lossless.
The graph reads the owning Archive directory.

`--texture-mip <MIP>` requests zero-based mip conversion. The default exports texture metadata
and retains selected originals. With a mip requested, `texture.dds-conversion` reports the
outcome for the selected physical header. Each `texture.dds-image` row identifies the exact
layer, face and slice, source and output dimensions, shared image suffix, channel treatment,
and verified PNG or OpenEXR digest. The placement distinguishes a newly written artifact
from an identical artifact already placed in this generation. PNG merges the selected DDS
alpha or smoothness tail into the alpha channel when present. High-range sources use linear
floating-point OpenEXR and retain any authored channels of the high-range mip. Per-image
field origins record the exact original surface byte spans. Converted image rows distinguish
the source declaration from the actual RGB encoding in `source_transfer` and `output_transfer`.
For PNG, `png_transform` identifies how DDS samples became the encoded channels. Known RGB
transfer is tagged. A directly selected DDS alpha tail carries no RGB transfer tag.
`alpha_association` comes from the selected DDS header. For BC1, BC2 and BC3,
`alpha_coverage` counts the original in-bounds zero, partial and full alpha samples
of each image. `alpha_transform` records RGB unassociation when PNG colour requires
it. A separate alpha tail supplies output alpha. The header declaration, block counts,
transformation choice and written artifact each carry their own source origins.
Required siblings are read only
when the immediate owning directory proves a unique lossless Entry. An excluded required
sibling remains private while still contributing its source evidence.

For installed texture verification, set `P4KX_DATA_P4K` and run
`cargo test --release -p p4kx-media --test real_archive texture_headers_and_stratified_media_outputs_match_original_reads -- --ignored`.
Repeat with the LIVE and PTU installations. The test inventories all root DDS header Entries,
verifies the selected families against their original stored payload checks, and compares the
private replay’s Media images byte for byte with ordinary conversion from the installed source.
Set `P4KX_TEXTURE_ORIGINAL_ROOT=1` to also compare one selection directly through the original
root Media export. That option reads and hashes the complete installed Archive.

`--kind model` inventories selected CGF, CGA, CHR and SKIN files and their geometry companion
formats. Each `model-entry` row describes the original container and chunk table. Each
`model-chunk` row preserves the original physical slot, chunk type, version, byte order,
declared ID when present and range within the retained decoded Entry. The IVO range reaches the
next physical boundary and can include padding. Unknown chunk types retain their exact words
and payloads. The chunk rows reference the same hashed raw file as the Entry. Their ranges
do not carry separate digests.

The provider reads the original header and table from retained bytes and charges the validated
row count to the common metadata work budget before allocating the table. A malformed table
is diagnosed in diagnostic output with its original Entry bytes.

`model-companion` rows link a base model to physical geometry and LOD sibling candidates in
its immediately owning Archive. Original observed path spelling wins over a derived candidate
name. A model announcing a geometry companion can report its absence. An observed LOD sibling
can likewise supply the lookup name for its missing counterpart. Each lookup reports its
selected, unselected, missing, ambiguous or unavailable state. A resolved identity establishes
the directory match. Target payload reads and semantic use are separate.

The original decimal LOD suffix survives even when it exceeds the numeric range. The
`canonical_reader_candidate` flag describes whether the existing model reader would
construct that name under its supported naming and LOD range. Additional discovered siblings
remain visible. The directory is indexed once per owning Archive and uses the shared metadata
work and reference budgets.

The model provider also reads selected CDF Entries. A `model-character-definition` row
identifies the authored XML encoding and root, and retains valid alternate roots as opaque.
A malformed document or repaired binary strings are recorded in a partial diagnostic export.
A `model-character-skeleton` row uses the zero-based root child index of each Model
declaration. A `model-character-attachment` row uses the zero-based root child index of its
AttachmentList and the zero-based child index within that list. Nonattachment siblings still
count toward the index. The records share the CDF Entry's retained raw data.

Skeleton File and attachment Binding keep the exact authored path and the owning Archive's
candidate identities. The lookup uses the existing Data-prefixed path convention and falls back
to the exact relative path after a complete miss. Absent attributes, authored empty or invalid
paths, missing targets, duplicate targets and unselected physical targets stay distinguishable.
The references establish physical identities without reading their target payloads. Join them
to model collision or skeleton facts by the root Archive and physical Entry identity.

Bone attachments carry the authored bone name, material override, relative position in metres
and relative quaternion in WXYZ order. An absent position or rotation uses the CDF offset
default for that bone attachment. Malformed values retain their original spelling with a null
decoded value. Nonbone attachments retain any source strings for these offsets. These are
bone-relative declarations. They do not assert that a named bone exists, or apply a skeleton
bind pose to the collider. Each field retains its original text attribute span or CryXml node
and attribute table coordinates, including its source encryption state.

Measured IVO LodDistances chunks emit one `model-lod-distance-words` row per physical
chunk. The v0x900 layout with a 48-byte inferred range carries twelve ordered original
little-endian words. Each word has its unsigned bits and, when finite, its binary32
interpretation. The projection keeps the complete range, including trailing words. It
assigns no switch thresholds, distance units or companion levels to individual words.
Other versions and extents keep their original bytes and an explicit opaque disposition.

IVO StatObjPhysics table version 0x900 additionally emits `model-physics-table` and
`model-physics-slot` rows. The chunk ordinal locates the table. A second ordinal locates each
slot, including duplicate packed keys and the whole-object sentinel. Declared and available
ranges refer to the same retained model Entry. Unsupported versions, damage, signed-distance
fields and unfamiliar markers or shape selectors keep their bytes with explicit dispositions.
Malformed or truncated collision data produces partial diagnostic rows and keeps readable
siblings in that diagnostic generation.

Validated version-2 box, sphere, cylinder and capsule shapes carry original float bits and
finite values. Source-node geometry uses metres and a Z-up frame. A triangle mesh adds
`model-physics-vertex` and `model-physics-triangle` rows keyed by chunk, slot and original element
ordinal. Their fields carry original float words, u16 vertex indices and per-triangle material
bytes. The shared per-field origin receipts identify the exact source bytes. Model material dependencies,
entity-frame collision transforms and converted geometry have separate coverage.

`--kind animation` retains CAF, DBA and AIM sources with chunk and clip inventory, original
metadata and controller words, and typed decoded frame and key chunks.
[Animation source export](media-animation-source.md) describes the layouts and dispositions.

`--kind audio` exports standalone and complete embedded WEM summaries, ordered RIFF chunks,
cue and LIST annotations, physical soundbank sections, original DIDX and HIRC occurrences,
and typed Action and Event fields with immediate origins.
[WEM source metadata](media-audio-metadata.md) and [Soundbank metadata](media-bank-metadata.md)
define the available fields and diagnostic publication.

`--kind audio-control` exports original ATL XML and CryXml syntax, typed control families and
source-scoped reference rows. [ATL audio controls](media-audio-control.md) describes the exact
physical keys, per-field origins and optional Wwise Event links.

The include and exclusion expressions use the existing search Filter syntax. A nested source
can also be matched through `outer.socpak!inner.pak!path`. Excluding a container excludes its
descendants without opening it. A split texture header exclusion also excludes its part Entries.
The pipeline walks physical directory order and traverses `.pak` and `.socpak` containers within
the configured depth and the Archive reader's hard size/depth limits.

Archive Entry rows include `data.metadata` from the shared
[`EntryMetadataInfo` projection](archive-entry-metadata.md#shared-export-representation):
the exact physical metadata owner, raw unsigned16 DOS words and a separate timezone-free
calendar-validity view. Invalid calendar words remain complete raw metadata; they do not
change CRC/SHA or installation/recovery status. Other Archive-layout enrichment remains
outside this timestamp projection.

The Archive provider also emits [source-layout metadata](media-archive-layout.md) for the root and
each traversed nested Archive. Original directory records, signatures, unknown extras and
installation/free tables refer to bounded, verified raw sidecars. These metadata records do not
add physical Entries or claim that an entire Archive file has been retained.

## Files and identities

The root `media-index.json` identifies one complete generation under `generations/<sha256>/`.
That directory holds `<kind>.jsonl` streams, `sources.jsonl`, `artifacts.jsonl`, `raw/<sha256>` payloads and any
`chunks/<sha256>` typed outputs. Artifact references are relative to the index's `artifact_base`.
The artifact manifest itself streams individual artifact names, byte lengths and hashes, so the
index does not collect one entry per source payload in memory.

The independent source inventory has one exact identity and usable raw reference for every
selected retained Entry. Even when every selected domain provider skips an unsupported payload,
this source mapping and its bytes remain available without inventing a successful typed projection.

Actual required inputs outside the include selection may also have retained source rows. These
additional rows do not change normal traversal selection, physical/provider callback order or
`source_entries.written/skipped` meanings. Source-manifest row count therefore need not equal the
normal written tally. Each such RawReference still has actual verified bytes and an artifact-manifest
entry; repeated input use and its later normal visit share one source receipt. Excluded private inputs
have explicit use receipts instead and never gain a retained-source row. Existing source/record and
artifact admission bounds apply to these outputs, including during validated skip.

Every metadata record contains `kind`, `source`, `subresource`, `status`, `data` and `issues`.
`media schema --record` validates built-in payloads within their owned namespaces; custom provider
namespaces retain the generic record data contract. The library's `record_schema()` remains the
generic envelope; `builtin_record_schema()` supplies the fallible built-in composite schema.
Source identity includes the root SHA-256 and ordered physical container and leaf Entry indices.
Duplicate names and duplicate format-specific short IDs retain their original order and distinct
ordinal identities. A display path is the existing reader's exposed path; its separators can
already be normalised. Original non-UTF-8 name bytes are included only where the public API
provides them. The implementation does not label normalised display text as original path bytes.

Providers classify actual domain reference candidates through the same writer interface. Each
reference preserves the exact source path, GUID spelling or namespaced identifier. Signed and
unsigned numeric identifiers record their declared bit width and decimal text, including all 64
bits without assuming JSON consumers can represent them as floating-point numbers. The shared
states are `resolved`, `missing`, `ambiguous`, `not-selected` and `unavailable`. Filter exclusion,
unrequested kinds, absent providers and unread targets retain separate reasons. A resolved target
proves identity, independently of its projected field coverage or retained raw bytes.

Candidate order and duplicate declarations are retained with full container/physical identities.
Only a complete enumeration can prove a missing or unique target. Count or byte limits instead
produce `unavailable`, with observed candidate counts and an explicitly incomplete bounded list;
counts are lower bounds when iteration stops early. Classification performs no Archive path
lookup or additional read. The provider context exposes the retained source and source-bound physical candidate lookup.
`lookup_input` additionally lends a token only for a complete, unique, lossless candidate.
`SourceContext::with_required_input` uses that token through a checked Read/Seek callback under the
same physical source admission, source-byte/work limits, ArchiveWatch and actual cancellation.

Required access does not select the target or create its provider callback. Selected Entry sources
still retain complete usable raw bytes independently of projection. Necessary include-unmatched
inputs may be retained without selection promotion. An individually excluded part of a selected
texture remains readable for Conversion, but receives no own published RawReference. Its receipt
names the measured decoded-payload digest, exact physical identity and an explicit omission reason;
a digest-only receipt does not claim usable raw preservation. Private input bytes are cleaned
before callback return and never enter a published generation. Excluded containers and descendants
remain unopened; excluded texture header families remain unselected.

Retained input payloads are reused; every actual reader pass and Seek consumes shared work, and
actual read bytes consume the source-byte allowance. Reusing a released excluded input requires
another charged materialisation. Input slots are fallible and their simultaneously live count is
bounded by `--max-sources`; this is an input-bookkeeping bound, not image working-memory capacity.
This interface does not add image conversion, an encoded-artifact sink or durable I/O-failure
diagnostics. Source/Extraction I/O remains a typed terminal failure even with `--diagnostic`.

The root SHA-256 covers every byte read from the root Archive file, including when a Filter selects
only a few Entries. It is a computed fingerprint, not a checksum supplied by the game or a
cryptographic identity inferred from timestamps. ArchiveWatch checks source size, modification
time and Build during the operation; it does not provide an immutable snapshot against an
adversarial writer that preserves this metadata.

Raw sidecars retain the exact decrypted and decompressed selected Entry payloads. The writer uses
one exact physical `&Entry` per unique Extraction destination and validates the actual report,
size and hash. A fixed portable staging filename keeps original directory-like or undecodable
source names out of destination selection while retaining their physical identities and bytes.
This uses the shared raw-payload extraction kernel, including verification and write protection.
CRC-32, CRC-32C and SHA-256 declarations retain their algorithms and byte scopes;
`not-available`, `not-performed` and `verified` comparisons are distinct from digest computation.
Selected Entry retention does not claim stored ciphertext or compressed representations.
The Archive provider separately retains established metadata sections, including opaque signatures
and unknown metadata bytes; absent or unavailable declarations are explicit. Unproved gaps outside
those sections do not receive a raw-preservation claim.

## Values, coverage and counting

`complete` records successfully project every field group their provider claims as projected.
Explicitly unsupported groups remain in its capability declaration. `partial` retains useful
data with diagnosed loss; `failed` diagnoses unusable projection. Every issue has a non-empty,
namespaced stable code, source-local location, severity and technical detail. Requested unsupported
providers cannot produce an apparently successful empty stream.

Source floats use `Float32` or `Float64`: a finite `value` and fixed-width hexadecimal `raw_bits`.
Non-finite values have explicit `value: null` and their original bits, including NaN payloads.
The central serializer rejects unwrapped NaN and infinity before publication. Missing, explicit
zero, unknown, inherited and format-default values use separate origins. Typed chunks declare
dtype, byte order, source order, count, first index, exclusive end, available offset basis and
explicit units/coordinate/frame/sample conventions. Unknown offsets remain absent.

The index counts physical Entries once for traversal and retention; excluded subtree descendants
are not guessed or added to observed totals. Each provider separately counts physical Entry
callbacks, metadata/subresource records and actual converted artifacts. Reused content-addressed
converted chunks count as enumerated/skipped; only a newly materialised artifact counts as written.
Root metadata is a record but not an Entry. Raw payload retention is not conversion. Every tally distinguishes enumerated,
written, partial, failed and skipped units; zero and an unavailable provider are not interchangeable.

## Bounds and cancellation

The default synchronous queue holds one source. No complete Archive payload, all animation samples
or archive-wide output list is accumulated in memory. Defaults are deliberately large enough for
real Archives and can be lowered or raised explicitly:

| Option | Default | Scope |
| --- | ---: | --- |
| `--max-root-bytes` | 4 TiB | Entire root file fingerprint |
| `--max-sources` | 10,000,000 | Observed physical Entries, including filtered Entries |
| `--max-source-bytes` | 8 TiB | Retention, container-opening decoded work and actual metadata-read bytes across sources |
| `--max-metadata-work` | 100,000,000 | Shared metadata cursor/read work items across owning sources and providers |
| `--max-entry-bytes` | 8 GiB | One retained payload or opened container |
| `--max-records` | 10,000,000 | All metadata and subresource records |
| `--max-record-bytes` | 1 MiB | One JSONL line or index |
| `--max-chunk-bytes` | 64 MiB | One typed binary chunk or required source chunk read |
| `--max-output-bytes` | 8 TiB | Generation artifact output work; each independent artifact-verification phase uses the same bound; the index uses its separate record bound |
| `--max-container-depth` | 8 | Ordered physical container chain |
| `--max-value-depth` | 128 | JSON value nesting including its envelope; maximum 512 |
| `--max-references` | 10,000,000 | Reference queries across all provider callbacks |
| `--max-reference-candidates` | 128 | Candidate identities retained in one inline reference result; metadata bytes also use the record bound |

Byte and count limits must be positive; depth cannot exceed the reader's hard limit. A selected
container payload and its subsequent container-opening work are charged separately. Container
decoding also retains the existing 512 MiB chain budget, entry-count and expansion-ratio checks.
Sizes are checked before Extraction or container allocation. Hashing and copy loops check
cancellation and ArchiveWatch between bounded chunks, and reports are checked before continuing.
First Ctrl+C cancels safely; a second uses the CLI's immediate-interruption convention.

The normative `metadata_work` default is 100,000,000. Its default value is omitted canonically
from serialized index limits; a nondefault value is explicit. Omission on read means that same
bound, preserving the generation fingerprint of supported older indexes without the field.

## Publication and recovery

`--conflict skip` keeps an existing complete index after verifying its artifacts. `overwrite`
publishes another complete generation. `if-changed` compares deterministic content generation
SHA-256, including source identity, provider contracts, bounds, counts and artifact hashes, rather
than CRC or file size alone. Existing or newly staged artifacts are verified under the caller's
budgets. Returning an unchanged generation still requires the final cancellation and Archive
change checks. An unrelated or invalid index is refused instead of overwritten as an export.

Normal export refuses an inventory already known to be recovered or truncated before hashing the
entire Archive or retaining payloads. `--diagnostic` explicitly permits retaining an incomplete
generation. Global uncertainty has no invented Entry index or lost-Entry count. A later partial
projection in normal mode also stops without publishing; use `--diagnostic` to retain its report.

The writer stages on the output filesystem, flushes and synchronises artifacts, verifies their
hashes, places an immutable generation and replaces `media-index.json` last. Projection loss writes
`diagnostics/<sha256>/media-index.json` with `complete: false` and `artifact_base: "."` when
`--diagnostic` was requested; it never
replaces a previous complete root index. Fatal failures or cancellation return non-success and
preserve that previous index. The output guard also refuses installation writes and output links.
Windows output paths and their ancestors also refuse junctions and other reparse points; ordinary
input aliases remain supported by the Archive reader. Path checks do not claim protection from an
adversarial process replacing directory entries between filesystem operations.

Writers hold an exclusive OS lock on the persistent `.media.lock` file. Kernel handle lifetime
releases the lock after a crash; do not delete or replace that pathname while a writer can run.
Unreferenced staged or generation directories after an interrupted process do not hide the last
valid root index. Inspect them after all writers stop before removing them. The exporter does
not automatically remove older complete generations.

File synchronisation and same-volume index replacement protect process-level failure on supported
local Windows/Linux filesystems. Linux also synchronises directories. Power-loss guarantees vary
on Windows, network and virtual filesystems; universal transactional durability is not claimed.

## Verification

The synthetic suite injects two independent providers through the public registry and checks
physical duplicates, ordered nested containers, exact raw and chunk bytes, hashes, finite/raw-bit
floats, capability refusal, honest counts, budgets, cancellation, corruption and generation-boundary
failures. A real two-process test verifies exclusion and recovery after a killed writer.

The ignored real-Archive test reads `P4KX_DATA_P4K` and deterministically selects small unencrypted
root Entries, refusing more than 64 Entries or 8 MiB of decoded data. It injects a verification-only
provider through the public registry: the shared service retains the selected raw payloads and the
provider emits no typed records or converted outputs. It excludes container traversal and checks
physical source identities, both manifests, artifact hashes, exact sidecar bytes and schema validity
under ArchiveWatch. Each manifest is bounded to 1 MiB and checked artifacts together to 16 MiB;
the generation's output limit is also 16 MiB. Temporary output is removed after successful
verification. Results expose a static failure category rather than source errors.

The reader still inventories the root directory, traversal examines all physical root Entries, and
the root fingerprint reads the entire Archive. The 8 MiB bound covers retained decoded payloads,
not those root operations. A synthetic regression uses a directory larger than that bound to
distinguish the focused probe from the permanent Archive provider's full-directory metadata reads.
Run the optimised test once for LIVE and again for PTU:

```powershell
$env:P4KX_DATA_P4K = '<absolute installed Data.p4k path>'
$env:TMP = '<absolute verification scratch directory>'
$env:TEMP = $env:TMP
New-Item -ItemType Directory -Force -Path $env:TMP | Out-Null
cargo test --locked --release -p p4kx-media --test real_archive -- `
  selected_real_payloads_match_physical_entries_manifests_and_sidecar_hashes --exact --ignored
```

This verifies foundation raw preservation and accounting through the injectable provider contract.
The permanent Archive provider's default-registry CLI and full-directory metadata projection have
separate synthetic/native checks; this focused probe does not exercise them. Domain providers,
encrypted payloads, nested real containers and full-Archive semantic coverage require their own checks.
No game files or keys are fixtures; source assets and generated real-Archive output must stay private.

## Field origins

Each current generation additionally commits `origin-sources.jsonl`, `origin-uses.jsonl` and
`field-origins.jsonl`. The optional index group `field_origins` identifies their hashes and
the independent capability `p4kx.field-origins/1`. Catalog schema versions do not apply.
The first two files use the original shared PhysicalSource and SourceUseDescription serializers;
they supplement the existing Media `sources.jsonl` and `artifacts.jsonl`.

The field stream addresses every actual JSON node in each emitted record, source row and artifact
row, plus the nullable Build projection in the Index. Record/source/artifact occurrence headers
supply their closed stream kind, contiguous decimal-string ordinal, producer profile and SHA-256
of the original payload line including its newline. The Index Build projection has the separate
hash domain defined below. Field rows carry
absolute escaped RFC 6901 pointers and the immediate shared FieldOrigin:
`containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`. Explicit end coverage
counts and the terminal EOF preserve row and field multiplicities. Repeated equal artifacts may
share their bytes while retaining different origins for each actual manifest occurrence.

Original observed Entry ownership and encryption proof are retained before optional paths,
declaration details or logical coordinates are copied. Local capture cuts are listed on the
affected field. An opaque use retains its actual source owner, domain, role and known encrypted
witness. A cut cannot turn a known encrypted contributor into an unknown or plain marker or
consume later independent fields' optional-capture allowance. Attempts and untouched reference
targets remain inventory observations and do not taint the contributing field set.

The built-in Archive Entry/layout and decoded Shader profiles map their own data domains.
A shader filename comes from directory metadata; its parsed content comes from the payload.
Containing encrypted Entries influence both domains, while a leaf's own encryption does not
change its directory metadata. Computed intrinsic hash values use their actual bytes,
independently of relationship selectors. Their fixed algorithm and scope labels have no source
inputs. A directory-declared checksum and its interpretation still use directory metadata. Public custom-provider records remain explicitly
`unmigrated`, even if they reuse a built-in kind, capability spelling or namespace.

Shader failure origins distinguish the early directory-size check from a diagnostic formed after
content inspection. The format owner's canonical configuration check also identifies unsupported
reader configuration before UTF-8 or lexical inspection. Its failure and zero-range controls are
independent of content, while filenames retain metadata origins. Actual parsed depth, token and
syntax failures keep their consumed payload contributions. Literal configuration bounds and null
counts do not acquire contributions from unused retained bytes.

Other RootSource and general Index fields, compiled shader-cache semantics and other provider
carriers retain explicitly incomplete or unknown coverage in this slice. A profile name and
per-occurrence coverage are separate from existing payload health and record status.
The field writer walks the already-admitted original JSON line without constructing a JSON value
tree, retains shared address prefixes and immutable contribution sets, and writes one field at a
time. Provenance bytes do not spend or enlarge the original record/source/artifact payload budgets.

Descriptive validation checks each captured use against its physical kind and byte domain,
retained raw declaration words/bytes and ZIP facts where available, and the complete transitive
ancestor state. Known encrypted or unknown outer witnesses cannot disappear through a shortened
leaf ancestor list. Missing optional details retain existing minimal proof; legacy alias text is
not treated as a new physical identity rule. Validation does not restore original input handles.

The existing staged generation and index replacement commit all three receipts atomically.
Reuse checks the receipts, payload-line hashes, exact addresses and ordinals, source/use closure,
coverage totals and EOF. Historical index files without this capability have unknown provenance;
they cannot be reported as a freshly migrated current skip. The consumer's finite metadata
policy is separate from producer admission and never changes the original payload bytes.

### Build declaration origins

A fresh generation emits an `indexBuild` occurrence even when the Archive provider
is unselected. With the Archive provider selected, its root record also addresses
the Build under `/data/build`; the Index carrier uses `/source/build`. Each has its
own immediate field origins. Corresponding fields reuse the same original logical
source-use references.

The closed producer profiles are `archiveBuild` for the Archive-root record and
`indexBuild` for the Index projection. The latter stream has exactly one
occurrence with ordinal `"0"`. Its SHA-256 domain is the compact serialization of
the nullable typed `Index.source.build` value, without a trailing newline.
It does not certify the whole Index or its other metadata. Existing generation
integrity separately binds the committed Index and companion receipts.

| Actual Build value | Addressed nodes |
| --- | --- |
| Object | Six: `/source/build` and its `branch`, `version`, `change_number`, `build_id`, `public_patch` children. |
| Explicit null | One: `/source/build`, without invented children. |

The Archive-root placement uses the same child names below `/data/build`.
The original adjacent manifest supplies `Data.Branch`, `Data.Version`,
`Data.RequestedP4ChangeNum` and `Data.BuildId` in the `build-manifest` logical
namespace. Each value retains its own Payload and JSON interpretation Conversion,
plus the successful `required-shape` Selector. These coordinates name logical
parser facts, not raw byte offsets. The Archive reader defaults `public_patch` to null as an independent
tool value; a supplied value without its own binding remains unknown.

The shared [Build-manifest observation](source-contributions.md#build-manifest-observations)
retains the original completed read and byte count. Its wire source kind is
`file`, with unknown original P4K lineage. Equal bytes, a matching path or a
serialized Build cannot establish that original handle. A private coherence token
only checks an already-bound value. Missing or unreadable manifests retain
Attempted observations without contributing a completed File. Malformed completed
reads retain their parse-outcome Selector and Conversion for the generated null,
without inventing a stored-null Payload. Other RootSource fields remain unmigrated.

The reader verifies this projection's hash, ordinal, full field paths, node count,
single occurrence, completion and EOF against the committed receipts. A declared
`indexBuild` scope requires the actual canonical object/null: missing `build` or
`public_patch`, unknown Build members and a sequence-shaped Build are refused.
Historical receipts without this particular scope retain unknown Build coverage;
a verified historical skip preserves that absence, while regeneration emits the
current carrier. Missing the entire origin capability still follows the existing
historical-generation upgrade rule.

This extends the unreleased `p4kx.field-origins/1` contract. Media stays at
`p4kx-media` `1.0`; the catalog writer for 0.5.0 emits only schema `3`.
The projection uses the existing Index/record bound: its compact nullable Build
bytes are part of the Index base payload. It introduces no additional manifest
input limit or payload-output allowance.

### Animation definition source

`--kind animation-definition` retains selected `.chrparams`, `.animevents`,
`.adb`, `.bspace` and `.comb` entries in their original bytes. Their JSONL rows
give ordered XML elements, declarations, text, comments and processing instructions.
Parent and sibling positions reconstruct the source tree. Text XML positions are
half-open byte ranges. CryXml positions address the original node, attribute or
content slot. Unknown XML remains available with the same coordinates.

File-link rows interpret authored track and event database paths, Mannequin
sub-database paths and combined blend-space references. Directory matching is
case-insensitive within the owning Archive and can report every physical match.
The link keeps the original path and the separately normalized lookup path.
Missing, unselected, ambiguous and unsupported paths retain explicit outcomes.
A reference is an edge. Its target contents remain independently selected sources.
Malformed documents are diagnosed with retained original bytes in diagnostic exports.

Within a character parameter `AnimationList`, `named-caf` link rows keep each
concrete animation declaration at the original path attribute's coordinates,
including repetitions. For relative paths the latest preceding `#filepath`
in the same list supplies the directory. An empty directive resets that
scope. Explicit `Animations/` or `Data/` paths address the Archive root.
The row keeps the original name, path and directory text, separately from the
normalized lookup path and candidate identities. The origin of a relative
lookup includes the active directive and its directory attribute. Wildcard
rules remain in the ordered source declarations. A named CAF link looks up
physical loose CAF entries without reading their payload. It does not
identify matches inside a tracks database or establish the clip used at runtime.
