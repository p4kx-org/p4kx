# Source encryption provenance is explicit per data value

The marker describes stored original input origin, not encryption of the exported output bytes. Implementation owner: #1125, coordinated by #1115.
It does not change key policy, decryption policy, filtering, file contents or public game-text rules.

## Physical observations and actual contributions

Each physical source retains identity, ordered container ancestry and storedEncryption with closed
states encrypted, unencrypted or unknown. Capture raw declaration kind/word/length and precise
coordinates at parsing; source evidence must survive the 1024-sample issue cap. Keep the existing
is_encrypted decoder-policy bool unchanged. Missing/malformed/conflicting CIG/v2 declarations do
not become plain evidence. Standard ZIP flags and unsupported schemes remain separately visible;
contradictory/unsupported observations cannot produce a false unencrypted conclusion.

Every container ancestor must be retained even when it is traversed but not selected for output.
Archive metadata and read/parse/decrypt/verification outcomes remain independent. A known encrypted
source stays known encrypted after a failed read; its retained metadata can describe that source.
This alone does not make that failed payload a contribution to some other emitted value.

A contribution is an actual retained payload fact, authored selector used to choose that result, or
input used in its conversion. Untouched reference targets, merely planned dependencies, completely
failed reads and read bytes discarded without use are separately recorded as reference/attempted
observations; they do not taint the emitted data's origin. A retained/used prefix does contribute.
Subject origins in Entry listings/failures remain visible independently of payload contributions.
Equal decoded bytes or equal artifact digests never merge distinct encrypted/plain source uses.

## Filter index

For a precisely named data/result scope, containsEncryptedSource is:

- true if any actual contribution or one of its ancestors is proven encrypted;
- false only if the contributor inventory is complete, every ancestor is known, and all contributors
  and ancestors are proven unencrypted;
- null otherwise, including missing old metadata and external files without original byte-bound proof.

An explicitly tool-authored value with no physical input may be false with a complete empty input
set. An incomplete empty set is unknown. Completeness of this contributor set is separate from
Filter completeness, source read health and catalog health. The index is derived from authoritative
source/use references, never from Archive-wide encrypted counts or ArchiveKeyState::Given.

Original-provenance receipts must bind exact bytes and length before restoring proof to disk inputs;
name, path, mtime, successful parsing or plaintext are insufficient. Reuse existing hashes where
available; no full-Archive hash on every preview merely to label encryption. Mismatched receipts
cannot retain their asserted origin. Never store Archive keys in provenance.

## Carriers and versions

Service wrappers carry neutral owned metadata; format crates acquire no Archive dependency.
Archive owns physical declaration capture; extraction owns the common serializable source/use
contract consumed by export, media, assembly, Blender, CLI, GUI and MCP. Extend existing source
and container identities, not a second traversal/registry system.

Catalog v3 MUST expose containsEncryptedSource true/false/null and actual source-use references
DIRECTLY on every exported source-backed field/data value, through the shared typed sourced-value
wrapper. Cover nested members/list elements, localized variants, source-backed identities and link
values. Consumers must not reconstruct the per-field flag from a global or row-only field. A row or
catalog summary is additional. Mixed-input tests prove distinct fields in one row can have different
markers, with the exact source/selector/conversion uses that justify each one. x-gameText protection
remains an independent axis and encryption metadata alone grants no public publication permission.

Catalog v3 carries shared sources, actual contribution sets and row/fact references plus the derived
index. Localized values reference their actual language Entry, not every loaded locale. A standalone
DCB and a localization Archive are separate actual origins; never claim the DCB came from whichever
Archive was supplied for enrichment.

Every structured tool output has the same immediate per-field obligation. Existing compatible
JSON wrappers may use a typed fieldOrigins map at exact field paths, with the already materialized
containsEncryptedSource marker, actual sourceUsesRefs and sourceSetComplete. Row-only origin is
insufficient. GUI details/previews and MCP structured results reuse these wrappers. Historical
readers treat absent old metadata as unknown; current writers must emit the applicable carrier.

Preserve raw stdout payloads and existing binary/Record XML/JSON/CSV dialects. A portable companion
addresses each typed field, cell or subresource and carries its immediate marker and exact source
uses. An opaque payload is one explicitly opaque data value, not a fabricated byte/pixel EAV table.
Do not inject reserved keys, columns, XML comments or GLB chunks, or prefix raw bytes. Companions
bind committed relative artifact paths, lengths and digests, and remain usable after files move.
Raw/table stdout has a versioned metadata side channel and an explicit durable companion option;
document that retaining only raw stdout discards that separate metadata.

Reuse existing bounded generation commits for Media/packages; failures preserve their previous
complete generation. Ordinary Extraction/standalone adapters retain their per-file publication
semantics. Before replacing a file, invalidate its old origin receipt, including when replacement
bytes could be identical but originate elsewhere. Publish a new trusted receipt only after that
file commits. A crash or companion failure may leave a published file with unknown origin, never
an apparently trusted stale receipt. Do not claim cross-file atomicity from a single rename.
Current run completion requires all applicable receipts; receipt/read integrity and failure before,
between and after artifact/receipt publication are tested. Missing artifacts, binding mismatch,
interrupted receipts and skipped bytes without matching proof cannot acquire trusted origin.

Media Source/InputReceipt/Artifact use existing full physical identities and bounded JSONL writer.
Blender packages distinguish per-node/mesh/material/texture/rig/animation contributions. Standalone
and comparative tools retain old/new provenance independently. Existing independent version rules
remain binding: additive metadata with old-document defaults-to-unknown is not an automatic v3 bump;
Blender's own minor-version rule still applies. Keys/settings/discovery/static schemas are not game
payload and receive no fabricated Entry flag.

### Media Build carrier

The original adjacent manifest is an external File with unknown P4K lineage.
Archive caches its eager read outcome separately from Build value equality and
public read errors. Only completed Parsed/Malformed reads create an opaque
original-read owner with the observed basename and byte count; containers share
that owner. Missing/Unreadable reads retain Attempted observations without a
completed payload contributor. Equal paths, bytes, values or serialized metadata
cannot create that runtime owner.

Media binds the original observation before callbacks. A private inventory/root
binding and bounded projection token check coherence of the already-bound value;
the token cannot restore authority. Parsed members retain their own logical
manifest Payload/Conversion plus the successful parse-shape Selector. Generated
malformed null retains its actual parse Selector/Conversion without a fabricated
stored-null Payload. Other RootSource metadata remains explicitly unmigrated.

The Archive-root placement addresses `/data/build` under profile `archiveBuild`.
Every fresh generation also emits stream/profile `indexBuild`, independently of
Archive kind selection, at ordinal decimal string `"0"`. The latter addresses
`/source/build` and hashes only the compact nullable typed Build value without a
newline. An object has six nodes (root plus its five members); null has one.
Both placements reuse original logical source uses. This projection digest is
separate from whole-Index generation integrity.

Declared indexBuild receipts require canonical actual object/null shape and exact
hash, paths, node coverage, single occurrence, completion and EOF. Missing Build
keys, missing nullable patch keys, unknown members and sequence-shaped objects
cannot be normalized into certified current coverage. Historical absence of this
scope stays unknown and can be reused as such or regenerated; it grants no missing
proof. The existing Index base bound already includes the Build projection, so
there is no new manifest input cap or payload-output budget.

This extends the unreleased `p4kx.field-origins/1` capability while Media stays
`1.0`. The 0.5.0 catalog writer emits schema `3` only. These three version
contracts are independent. [Media export](../media-export.md#build-declaration-origins)
and [shared observations](../source-contributions.md#build-manifest-observations)
define the concrete carrier and runtime API.

## Proof required

A synthetic Basetool consumer, another structured tool output and a field-addressed companion each prove immediate plain/encrypted/unknown fields in one row without global/row/provenanceRef recomputation. Synthetic cases cover v1/v2 recognized/missing/short/invalid-length/unknown/conflicting declarations,
issue-cap independence, duplicate paths, encrypted ancestors with plain children, unselected traversed
ancestors, disk unknown, byte-bound receipt mismatch, mixed field/language inputs, deduplicated
artifacts with different origins, failed/unused target isolation, retained prefixes and empty source
sets. Test actual shared CLI/GUI/MCP paths and persistent companion carriers, schemas/defaults,
streaming bounds, source changes and cancellation/output failure. Installed-Archive evidence stays
private and subject to the existing user deferral. This document is a target contract, not a claim
that all carriers already implement it.
