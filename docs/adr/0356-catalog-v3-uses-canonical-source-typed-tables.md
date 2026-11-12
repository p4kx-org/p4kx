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

Gameplay-property transformations use one owner-local flat preorder arena. The original
root/child slots and node facts retain source states and direct contributions; root,
nodes and nodeIndex are only structural controls. Each independently owned original
occurrence has a separate node, even when values are equal. Unknown classes keep their
actual type and Unsupported payload with the original type selector. No recursive
wire mirror, global node identity, transformation execution or equality deduplication
is introduced. This preserves the existing legal source depth through ordinary JSON
parsers. Schema, local link validation and independent original-source ownership proof
have separate responsibilities; a valid local index cannot certify a foreign source.

Sequence.transformations requires the original field's declared StrongPointer array storage.
A readable Class array is a projection error even when its materialized values look identical;
diagnostic publication retains those original values and occurrences. Declaration checking uses
the same original Field as the value read, without guessing storage from Value or type-name text.

The u32 nodeIndex convention is sufficient for this accepted graph: the global StrongPointer pool
has a u32 entry count N, each additional owned child consumes a distinct original pool slot, and
the root adds one node. Reusing a pointer target produces an original OwnedTwice error instead of
another owned node. Thus the largest node index is at most N. This bound depends on the checked
StrongPointer-array declaration; it is not a general bound on arbitrary materialized Class arrays
or open source-value arenas. The existing canonical tree resource policy remains separate.

This check establishes data type and array storage, not declared target inheritance. The canonical
reader follows the stored strong-pointer target independently of the declaration target word;
that existing decode behavior is unchanged. No type name is inferred from the raw u16 target.

## Texts, identity and migration

Use separate texts.publicNames and texts.gameTexts domains, with distinct typed references.
Public short names and short UI format templates stay public even if the same key/bytes also occur
as protected description or lore. Both protected text rows and protected owner references carry
x-gameText; exchange tests follow reference closure through both text forms.

Keep one canonical row/identifier per existing domain and exact authored key. Each actually
available language has TextForms with independently sourced sourceText and displayText, inside the
existing sourced translations map. sourceText is the exact winning decoded INI value; displayText
preserves the existing escape-decoding/trimming result. Missing keys/placeholders keep both forms
absent, an empty source or display string remains present, and a missing language file adds no
language entry. No fallback or template evaluation becomes stored data. Names/descriptions consume
displayText; UI format consumers use sourceText as inert data. The two values come from one
original lookup and retain the existing once-per-occurrence display-based summary counts.

Both forms carry direct actual origins: sourceText uses its authored key, winning language payload
and justified original Entry-decoding contributions; displayText also uses the actual display
Conversion. Whole-Entry decoding facts do not claim a particular winning value was altered.
SourceUse handles describe input/coordinate/role, so actual Conversion handles may deduplicate
without fictional stage locations. Preserve all existing identifier inputs, key/winner rules and
normalization results. Replace inline key/en/de wire duplicates with the canonical references and
language maps; the old normalized value is displayText, not a second legacy view.

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

## Contextual resource-network placements

Original link and room rows use the physical owner Record, component ordinal
and authored slot. Contextual bindings use the published Loadout array
position and nested port ordinals. The root or nested owner determines the
admissible direct endpoints. Original weak targets match their admitted
physical port coordinates. Each physical name candidate survives if Loadout
selection collapses a duplicate. Nodes reference selected resource components
and retain stored relay evidence. A docked vehicle keeps its own root context.
Derived bindings retain their actual original selector and same-context name
uses in their immediate source cell. Physical pointer matches retain their
own selected candidate evidence.
[Declared resource-network topology](../catalog-resource-topology.md) defines
the placement and resolution states.
## Ship facility configurations

A facility definition is identified by the original Entity Record index and
`Components` slot. The `configurations.facilities` `seat` kind carries the original
`SCItemSeatParams` member data and its owned ejection and actor-attachment
definitions. Placement consumers can use the physical component key with
their contextual placement identity. See [ship facility configurations](../catalog-ship-facilities.md).

A direct ship-room placement uses the physical RoomAssociation slot, the
actual SOCpak and descriptor Entry, the original streaming declaration node,
and the SOC Entry, chunk and Entity slot. This key preserves repeated authored
placements even when the same class and SOC Entry are shared. The original
Entity class selector joins to one physical DataForge Record before linking
its published component definitions. The original coordinates remain
container-local. Room coverage records incomplete or unexamined branches
so an empty result can be interpreted within its inspected scope.
