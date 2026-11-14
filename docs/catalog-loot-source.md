# Complete Loot source values

`definitions.lootPoolFilters` is the canonical `CatalogTable` for selected physical
`PoolFilterRecord` sources. Its availability is `available`, `absent` or `unavailable`, independently
of row count. The Kind and completeness key remain `lootPoolFilters`. There is no second root
`lootPoolFilters` dataset or recursive normal-output filter DTO.

`definitions.lootSecondaryChoiceSingleLayers`, `definitions.lootSecondaryChoiceMultiLayers`
and `definitions.lootThemes` use the same owner-local value format for the exact Record types
`LootV3SecondaryChoicesSingleLayerRecord`, `LootV3SecondaryChoicesMultiLayerRecord` and
`LootThemeRecord`. They have independent availability, selection, completeness and health keyed by
their dataset names. Single-layer choices retain their original ordered class entries and concrete
selectors. Multi-layer choices retain ordered Reference slots and original target headers even when
the target body is outside the filter. Themes retain the concrete type and fields of their original
owned theme value.

`definitions.hierarchicalContextualDataPresets` retains the original HCD array, concrete context
types and source selectors. `definitions.lootGenerationGlobalParams` retains authored category
ordering and special event archetypes. `definitions.subHarvestableConfigs` and
`definitions.subHarvestableMultiConfigs` retain the complete wrapped configurations, including
inline alternatives, theme links, and links to an independently defined slot preset. Only the
existing slot-preset selection also emits a convenience row under the top-level
`subHarvestableMultiConfigs`. Its GUID joins the canonical row, and the Kind counts the physical
definition once. The definitions include every Record of their exact source family and do not
simulate loot selection.

Each row has the common eight independently sourced header fields, ordered root `fields`, and a
flat `values` vector. A field contains its sourced original `name`, a declaration with sourced
`dataType`, `isArray` and `target`, and `value: {"valueIndex":"0"}`. These are actual decoder
storage declarations, not classifications inferred from the decoded Value. Array elements use the
same actual declaring field. Field names may repeat and are never map keys. A semantic named lookup
must explicitly handle ambiguity; positional consumers retain each original declaration.

Every materialized owned Value occupies one slot in original depth-first preorder. The root Struct
itself is represented by the header/root fields and does not occupy an additional slot. Struct and
Array values occupy slots before their children. Empty containers, null/error array slots and equal
subtrees remain distinct. Struct payloads hold their actual sourced `sourceType` and ordered fields;
Array payloads hold ordered local references. Local references use canonical unsigned decimal u64
strings, are checked at vector access, and refer only inside their owning row. They are controls,
not game values or original byte/pool/instance positions. References never expand target bodies.

## Immediate data boundaries

Each node has `value: SourcedValue<SourcePayload>` and required nullable `originalError`. A scalar's
immediate `node.value` is its one source-backed datum. Its typed scalar encoding inside that cell
is not wrapped a second time with the identical origin. Basetool can filter that immediate cell
using `containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`. Distinct source facts,
such as a declaration name/type/array flag/target, nested concrete type, Reference target metadata
or original error parameter, retain their own immediate sourced cell. Variant labels, local indices
and null `originalError` applicability are controls and do not acquire invented source witnesses.

Boolean false, integer zero, exact empty/whitespace strings, source Null and Unreadable remain
distinct. `ResourceNumber` preserves every integer width, signed/unsigned64 decimal encoding and
Single/Double IEEE bits, including signed zero and nonfinite values. Nonfinite publication requires
an exact committed receipt from the actual emitted physical Loot occurrence. A different family,
owner, ordinal, path, type, width, bits or cardinality cannot supply that receipt. Existing non-Loot
contracts and uncovered-number failures keep their meaning.

String/Locale payloads retain exact `sourceText` and an explicit `public` or `gameText` domain.
Known PoolFilterInstance name/mode and approved short propertyName/unitFormat facts are public;
source Enum labels remain Enum. Unclassified String/Locale domains conservatively use gameText
and `x-gameText`. This is a documented fallback, not a claim that a name heuristic established a
particular game-text meaning. It adds no translations or text registry and never trims/evaluates
the source. Text domain and encrypted origin are independent.

## Original source operations

The build-owned actual Forge/CatalogOrigins pair admits the real Record/tree through both owner
guards. One registry-free iterative DFS implementation serves the cached canonical Inspector and
the Loot projection. These are two separate tree traversals with the same kernel; there is no
additional decode or second numeric-inspection pass. Only the Scope-owned adapter can mint its
own source-bound visits. A raw visit cannot be combined with another Scope, and serialized values,
GUIDs, paths, indices, equality or source-inventory snapshots cannot recreate original authority.

Physical original-value coordinates are `dataforgeValue/[recordIndex,originalValueOrdinal]/part`. An actual
preorder visit mints that ordinal before descent; an arena index is not used to recover it.
`$value`, `$fieldName`, `$dataType`, `$isArray`, `$target`, `$type` and actual error-parameter tokens
identify logical facts. Array-value/error tokens use their own visit; declaration metadata uses
the original declaring field visit. These coordinates do not claim raw declaring-class indexes,
byte offsets or pointer words that the owning API did not retain.

A stored Reference GUID has exactly its authored caller-body Payload. Actual resolved Record
index/type/path retain the authentic demanded header and actual caller Selector. A Weak-derived
GUID retains the selected original target/owning-Record GUID Payload, same actual weak-slot Selector
and genuine pointer-resolution Conversion. Instance targets use their actual instance type and
retained original location/position inputs, without a fake owning-root type or a name-based second
lookup. Nullable/unreadable pointers never create a successful target capability.

## Complete original errors

A real Error keeps semantic Unreadable with null value and a non-null typed `originalError` at
that same occurrence. All retained parameters remain outside the null semantic payload. There is
no error sampling, category deduplication or join through bounded Health examples. Ordinary and
source-null values have `originalError:null`.

| Error | Exact retained facts or controls |
|---|---|
| invalidString | table, offset |
| unknownEnum / unknownStruct | index |
| invalidBool | byte |
| invalidPointer / ownedTwice / outsideRecord | structIndex, instanceIndex |
| arrayOutOfRange | first, count, len, pool |
| tooManyValues | count, limit |
| tooDeep / pathTooWide | limit |
| danglingReference | exact authored guid |
| unexpectedPointerType | structIndex, declared |
| unexpectedReferenceType | exact reported guid, structIndex, declared |
| instanceTooShort | structIndex, size, needed |

Every independent retained source parameter uses its actual creator's role. Pointer words are
Payload; a Class-array struct index comes from the actual declaring target Selector and its instance
index from original array mapping Conversion. A contradiction with the real declaration is fatal.
Pool bounds are validation Conversion facts. Query-only validation facts whose original operation
inputs are unavailable remain exact Present values with honest unknown/incomplete provenance;
their GUID is not silently promoted to an authored token or admitted target. Parser table/pool
labels and fixed policy limits are controls, not invented game payloads.

This carrier does not change existing source Health classifications. Informational dangling/outside
states do not become a new Normal refusal solely because their parameters are retained. Genuine
read failures still block Normal output. A future Value/ValueError variant without a reviewed
lossless carrier is an explicit preparation failure, with no partial placeholder publication.

The exporter adds no chosen value/depth/byte/error-sample cutoff. Existing canonical-reader resource
failures remain explicit and separate. Cancellation is latched while work remains; unfilled arena
slots, ownership/order contradictions and natural counter overflow fail explicitly. Generated
Schema/golden verification, original-source/consumer mutation controls, full Health capture and
installed acceptance remain separate evidence and must not be inferred from this document.

## Shared value contract

The canonical SourceNode, SourcePayload, SourceField and SourceDeclaration types carry original
owned values independently of the definition family. Loot still has its one complete owner-local
arena, and its exact numeric receipts remain restricted to actual emitted Loot occurrences.
A field name is its own Payload datum; storage declarations and concrete types are Selectors.
The physical namespace is dataforgeValue for every actual original occurrence. There is no second
namespace or compatibility interpretation. All references, source states and creator-sensitive
error parameters retain the same original operations and exact wire values.


## Loot source variants by Build

The exact Record types `LootV3SecondaryChoicesSingleLayerRecord` and
`LootV3SecondaryChoicesMultiLayerRecord` can exist independently. The single
layer owns ordered `choices` Class entries. The multi layer owns ordered
`layers` references to those Records. `LootConstraints.secondaryChoices`
stores a selector wrapper with its own declared target. A theme can also
contain inline secondary-choice layers with separate source identities.

When declared, `LootThemeRecord` supplies theme definitions.
`SubHarvestableConfig.lootThemes`, `SubHarvestableMultiConfig.lootThemes`
and `LootConstraints.themes` refer to those definitions.
`LootGenerationGlobalParams.themeCategoryOrdering` stores source tag order.
`HierarchicalContextualDataPresetRecord.HCDs` can contain
`LocationLootGenerationContextData` with ordered themes and an authored
`parentThemesToIgnore` selector. These declarations describe possible
inputs to loot generation.

The ignored `loot_source_declarations_resolve_to_their_physical_schema`
test checks stored type and target, array shape and the physical Record
census on a configured Archive with complete Build identity. A declared
type can have zero physical Records. The source census is independent of
catalogue export and runtime selection.
