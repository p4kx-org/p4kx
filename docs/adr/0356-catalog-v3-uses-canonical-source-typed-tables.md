# Catalog v3 uses canonical source-typed tables

This is the selected target contract for the unpublished catalog v3. It is not a claim that the
current serializer already implements the complete design. Implementation and acceptance remain
open in #1048/#1115/#1116. The user's explicit instruction to align all work with v3 and remove
legacy structure authorizes this form migration. It supersedes the old draft's field-path promise;
source meanings, units, selection, authored order and existing identity algorithms remain binding.

## One document, named typed tables

Retain schemaVersion = 3, source, completeness and measured exportHealth. Group named datasets
under definitions, configurations, declarations, placements, projections and texts; include the
shared physical sources and provenance inventory. A dataset has its typed rows, explicit availability
and typed conflict variants. Successful empty, family absent and unavailable are different states.
Every published dataset has a documented source scope and matching selection/health accounting.
The Kind vocabulary remains extensible. This is not an EAV document or a generic Record dump.

Definitions own reusable identity; configurations own declared component/global settings;
declarations own ordered authored uses; placements own context-qualified instances; projections
own explicitly named choices/derived views. The same definition in two placements remains two
placements. Physical source identity, nominal GUID, authored/content identity and output row ID
remain distinct. A resolved physical reference and its catalog materialization are separate axes;
reading a dependency never silently includes its filtered-out definition row.

Remove items[].stats and the redundant flat legacy wire views rather than retain parallel payloads.
Health, resources, inventory and other components are independently present even without a known
specialist component. Repeated components retain source location and order. Shared ammunition is
one definition with ordered primary/secondary/magazine bindings, not copied per role. Global
ResourceNetworkParams is configurations.resourceNetworkParameters, with its actual source owner,
not copied onto each item or folded into ship topology. Retain every #1059 field, including
shipPassDestroyedPartDamageToComponent, shipPassDestroyedPartDamageToChild,
shipDestructionBufferMultiplier and shipDestructionHealthThreshold.

The complete family routing is in [the data-family map](../catalog-v3-data-map.md). Its current Kind
names and all 51 catalog tickets are an inventory, not proof of implemented fields. The map
includes the separate #1059 global configuration. Preserve Buff references without inventing
a separate definition provider before a real owning type/identity/source scope is established.

## Source facts and exact values

Use closed typed source states: present, absent, null, unreadable, unsupported and invalid. A present
empty list, false, zero and empty string remain values. Null slots and repeated authored members
retain position. A failed inventory is never an empty successful dataset. Preserve the existing
strict MissionPropertySet/Generator atomicity rather than quietly partially retaining an unsupported
property. Ordinary readable facts and required projection failures remain separately accounted.

Store Int64/UInt64 as range-checked canonical decimal strings. Store f32/f64 as typed IEEE values
with exactly 8/16 hexadecimal raw-bit digits and a nullable width-correct round-trip decimal
representation. Capture bits before conversion/arithmetic. Legal NaN/infinity are present raw facts,
not intrinsically malformed or incomplete; finite is null. Required finite semantic projections can
fail separately. Preserve signed zero, NaN payload/signaling bits, source widths and raw units.
No blanket narrowing through f64/f32, zero substitution or clamp.

Exporter-owned states that grant health, resolution or accounting meaning are closed schema
controls. Game vocabularies, Kind identifiers and extensible negative reason codes remain open
strings with documented known values. Unknown controls are rejected; unknown labels never default
to a successful state. Existing closed payload unions remain closed and emitted nullable keys remain
required. Additive unknown object fields stay accepted.

## Texts, identity and migration

Use separate texts.publicNames and texts.gameTexts domains, with distinct typed references.
Public short-name use stays public even if the same key/bytes also occur as protected description
or lore. Both protected text rows and protected owner references carry x-gameText; exchange tests
must follow reference closure, not merely inspect field names. Keep all actually available supported
language codes, exact keys, decoding/trimming rules and per-language source provenance. No guessed
translation or fallback becomes stored text. Replace duplicated key/en/de wire fields with these
canonical references and language maps; document their exact old-value reconstruction.

Freeze existing content-identity inputs independently of new wire serialization. MissionPropertySet
keeps its current owner/location hash and stored-byte UUID mapping. Spawn/query content IDs use
frozen typed IdentityPayload inputs with the old order/number/null/key representation; adding raw
facts/provenance must not incidentally rehash them. Equal identity hashes require tuple comparison.
Invalid/all-zero authored identities retain typed diagnostic source variants; do not invent GUIDs.

Re-export original sources for a complete v3 artifact. An old artifact cannot recover missing-vs-null,
lost numeric bits, physical indices or encryption history; an import transformation cannot manufacture
healthy source proof. Keep the released v0.4.2 schema fixture frozen. Provide field mapping, preserved
selection/projection recipes and consumer version refusal before import/deletion. Other formats own
their versions independently.

## Encryption origin, shared outputs and completion

Capture original physical Entry declarations before reads, retain all container ancestors, and carry
actual data/selector/conversion contribution uses through source wrappers. Detailed rules are in the
companion encryption-origin decision. Per-source declaration, read success, contributor completeness,
selection completeness and publication health are separate facts. External files without verified
byte-bound original provenance remain unknown. Source and per-fact references are authoritative;
containsEncryptedSource is their conservative filter index. Every source-backed exported field/data
value exposes this nullable marker directly alongside its actual source-use references, using a
shared typed sourced-value wrapper. A row/global flag or a reference that requires consumers to
recompute the field marker is insufficient. Per-field source/selector/conversion contributors remain
precise; the row aggregate is supplementary. Include source-backed identity/text/reference values
and nested list members, not just top-level item statistics. Schema/control bookkeeping is not
silently reclassified as game data. Basetool can classify each game value independently; x-gameText
remains a separate protection axis.

Use one immutable prepared typed result for plain JSON, compressed JSON and schema-driven callers.
Repeated sizing/compression passes may serialize that result, never reread/project the sources.
Keep the existing atomic catalog file adapter and its cancellation/change/cleanup ownership; no
frontend-specific writer. Source preparation peak memory and serialization streaming are measured
separately. Deduplicate common sources/texts/use sets without collapsing physical provenance.

Retain one Build/document/upload. Measure exact 64,000,000-byte upload and 1,000,000,000-byte decoded
Basetool budgets and peak memory with complete fields, languages and provenance. Existing size
warnings are transport facts, not invented read failures. Do not silently drop data, split exports or
impose a new global export refusal without a measured concrete transport decision.

Synthetic checks must exercise mixed/unknown/encrypted origins, exact numbers, healthy zero-row
projection, strict controls, source resolution versus selection, complete reference closure, duplicate
physical sources, conflict/invalid identity and cancel/write preservation. Native Windows/Linux on
AMD64/ARM64 remain mandatory. The user-deferred installed-Archive acceptance remains deferred;
synthetic model checks are not substitutes for that evidence.


## Original Record metadata and header coordinates

Canonical Record families reuse the [shared eight-field header](../catalog-record-headers.md),
including exact stored fileName and separate layout-aware metadataTag. Successful format reads
retain one borrowed original metadata snapshot; no successful row is invented for a strict
metadata admission failure. The existing SourceLocation subresource namespace
dataforgeRecordHeader explicitly separates all original header facts from body paths. Derived
className/recordPath retain their actual name/type/filename contributors and branch roles;
authored Reference GUIDs remain at their original body slots. Correcting colliding coordinates
may change SourceUse IDs within unreleased v3, while catalog identities, values, selection,
order, counts, the origin protocol union and standalone Record serialization stay unchanged.
