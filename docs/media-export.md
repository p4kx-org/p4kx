# Versioned media export

`p4kx media export ARCHIVE --out DIR` writes a versioned, streaming media export. Its format is
`p4kx-media`, `format_version: "1.0"`; catalog schema versions do not apply.

```powershell
p4kx media export 'D:\Games\StarCitizen\PTU\Data.p4k' --out 'D:\Exports\media' --kind archive
p4kx media export 'D:\Games\StarCitizen\PTU\Data.p4k' --out 'D:\Exports\selected' --filter '*.bnk' --exclude '*.socpak'
p4kx media schema
p4kx media schema --record
p4kx media schema --reference
p4kx media schema --source
```

Repeat `--kind` for an explicit selection. The known names are `archive`, `model`, `material`,
`texture`, `shader`, `animation`, `animation-definition`, `audio` and `audio-control`. Omission
selects every registered provider. The built-in providers are the permanent minimal `archive`
provider and the source-preserving `shader` provider. An explicitly requested kind without an
available provider returns a typed capability
failure and a non-zero exit status, even if the selection would otherwise be empty.

`--kind shader` exports nested `.ext` declarations and ordered Property fields, with exact raw
source ranges and lossless mask strings. Selected `.cfib`, `.cfxb` and `.shcb` caches are inventoried
with explicitly unsupported semantic decoding. Material-to-definition links are separate work;
the provider does not compile or execute shaders. [Shader projection](media-shader-backend.md)
defines its precise typed fields, statuses and counting units.

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

## Files and identities

The root `media-index.json` identifies one complete generation under `generations/<sha256>/`.
That directory holds `<kind>.jsonl` streams, `sources.jsonl`, `artifacts.jsonl`, `raw/<sha256>` payloads and any
`chunks/<sha256>` typed outputs. Artifact references are relative to the index's `artifact_base`.
The artifact manifest itself streams individual artifact names, byte lengths and hashes, so the
index does not collect one entry per source payload in memory.

The independent source inventory has one exact identity and usable raw reference for every
selected retained Entry. Even when every selected domain provider skips an unsupported payload,
this source mapping and its bytes remain available without inventing a successful typed projection.

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
lookup or additional read. The initial provider context exposes the retained source, not an
unbounded Archive handle; later companion readers require an explicit shared-budget read interface.

The root SHA-256 covers every byte read from the root Archive file, including when a Filter selects
only a few Entries. It is a computed fingerprint, not a checksum supplied by the game or a
cryptographic identity inferred from timestamps. ArchiveWatch checks source size, modification
time and Build during the operation; it does not provide an immutable snapshot against an
adversarial writer that preserves this metadata.

Raw sidecars retain the exact decrypted and decompressed selected Entry payloads. The writer uses
one exact physical `&Entry` per unique Extraction destination and validates the actual report,
size and hash. CRC-32, CRC-32C and SHA-256 declarations retain their algorithms and byte scopes;
`not-available`, `not-performed` and `verified` comparisons are distinct from digest computation.
Raw retention does not claim to preserve stored ciphertext, compressed representation, signatures
or unknown root layout bytes. The initial Archive provider explicitly marks raw-layout projection
unsupported while exposing the supported public metadata and Build declaration.

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
| `--max-source-bytes` | 8 TiB | Retention and container-opening decoded work |
| `--max-entry-bytes` | 8 GiB | One retained payload or opened container |
| `--max-records` | 10,000,000 | All metadata and subresource records |
| `--max-record-bytes` | 1 MiB | One JSONL line or index |
| `--max-chunk-bytes` | 64 MiB | One typed binary chunk |
| `--max-output-bytes` | 8 TiB | Generation artifact output work; each independent artifact-verification phase uses the same bound; the index uses its separate record bound |
| `--max-container-depth` | 8 | Ordered physical container chain |
| `--max-value-depth` | 128 | JSON value nesting including its envelope; maximum 512 |
| `--max-references` | 10,000,000 | Reference queries across all provider callbacks |
| `--max-reference-candidates` | 128 | Candidate identities retained for one lookup; metadata bytes also use the record bound |

Byte and count limits must be positive; depth cannot exceed the reader's hard limit. A selected
container payload and its subsequent container-opening work are charged separately. Container
decoding also retains the existing 512 MiB chain budget, entry-count and expansion-ratio checks.
Sizes are checked before Extraction or container allocation. Hashing and copy loops check
cancellation and ArchiveWatch between bounded chunks, and reports are checked before continuing.
First Ctrl+C cancels safely; a second uses the CLI's immediate-interruption convention.

## Publication and recovery

`--conflict skip` keeps an existing complete index after verifying its artifacts. `overwrite`
publishes another complete generation. `if-changed` compares deterministic content generation
SHA-256, including source identity, provider contracts, bounds, counts and artifact hashes, rather
than CRC or file size alone. Existing or newly staged artifacts are verified under the caller's
budgets. An unrelated or invalid index is refused instead of overwritten as an export.

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
root Entries, refusing more than 64 Entries or 8 MiB of decoded data. It excludes container
traversal and independently checks physical source identities, both manifests, artifact hashes,
exact sidecar bytes and schema validity under ArchiveWatch. Temporary output is removed after
successful verification. Test results expose a static failure category rather than source errors.
The root fingerprint still reads the entire Archive; the bounded selection applies to retained
payloads. Run the optimised test once for LIVE and again for PTU:

```powershell
$env:P4KX_DATA_P4K = '<absolute installed Data.p4k path>'
$env:TMP = '<absolute verification scratch directory>'
$env:TEMP = $env:TMP
New-Item -ItemType Directory -Force -Path $env:TMP | Out-Null
cargo test --locked --release -p p4kx-media --test real_archive -- `
  selected_real_payloads_match_physical_entries_manifests_and_sidecar_hashes --exact --ignored
```

This verifies bounded raw preservation and the initial Archive provider. Domain providers, encrypted
payloads, nested real containers and full-Archive semantic coverage require their own checks. No
game files or keys are fixtures; source assets and generated real-Archive output must stay private.
