# Canonical gameplay properties

Schema v3 emits one `definitions.gameplayProperties` typed table and no flat
`gameplayProperties` payload. Its availability measures whether the original
`CraftingGameplayPropertyDef` family is declared, independently of successful row
count. The existing Kind, crafted-properties path prefix, Filter and development
leftover rules select physical definitions. Equal nominal GUIDs retain distinct
physical rows, stable source order within a GUID tie and honest identity diagnostics.

Each row has the shared eight sourced Record headers and four original fields:

| Field | Original contract | Wire value when present |
| --- | --- | --- |
| `propertyName` | Locale | Public-name reference |
| `displayTransformation` | StrongPointer to a display-transformation base | Local flat transformation tree |
| `unitFormat` | Locale | Public-name reference for an inert UI template |
| `nameOverrides` | Class array of `CraftingPropertyNameOverride` | Ordered sourced override slots |

All source carriers retain their state, explicit nullable value, direct encryption
marker, source-use references and contributor completeness. Absent, null, unreadable,
invalid and unsupported are distinct; a present empty string/list is not absence.
Unexpected fields and repeated names that a named DTO cannot preserve are diagnosed.
This does not prohibit repeated field names in the DataForge format.

## Flat transformation tree

The present transformation value contains `root: {nodeIndex: 0}` and `nodes`.
`root`, `nodes`, the grouping of a node and `nodeIndex` are structural controls,
not authored game values or source identities. Links refer only to this property's
vector. Each original owned occurrence creates one node in depth-first preorder,
following stored Sequence slots. Equal independently owned occurrences remain
separate. A null, invalid or unreadable child slot creates no node.

Each node has independently sourced `sourceType` and `payload`. The closed payload
tags are `emptyBase`, `convertFactorToNegatedPercentChange`,
`convertFactorToPercentChange`, `convertValueToFactorOfBaseValue`, `scale` and
`sequence`. Both original base types use `emptyBase`, with their actual distinct
source types retained. Scale contains its sourced `scale`; Sequence contains the
sourced `transformations` array and its individually sourced local links. Operations
are declarations: no transform arithmetic, formatting or stat matching is executed.

An unknown class remains a node with its actual type and Unsupported payload.
Its payload disposition retains the original type selector; it is not inferred from
a neighboring field's origins. Unknown fields do not create guessed edges. Original
InvalidPointer, OwnedTwice and depth errors remain unreadable at the original slot.
An actual repeated pointer is not converted into a successful shared node or cycle.

The reader uses an explicit stack to traverse every materialized Sequence slot in order.
It keeps one mutable field path for the current traversal. The flat output does not add a
Sequence nesting limit or expand each edge into recursive JSON envelopes. Consumers use
ordinary bounded JSON parsing.
Schema checks required cells, closed payload tags and u32 link storage. A canonical
graph consumer additionally checks root zero, range, exactly one owner per node,
cycles, reachability and preorder. It accepts equal independently original nodes.
A copied foreign node that has internally valid links can be rejected only by the
independent original-source/origin check; structural indices are not source authority.

## Overrides, text and numbers

Every override keeps its concrete sourced type and sourced payload containing
`propertyName` and `condition`. Condition sources retain both base types and ItemType.
The latter has ordered, individually sourced `matchItemTypes` and `matchSubTypes`
Enum labels. Empty labels, repeated labels and null/unreadable slots retain their
actual original states. Unknown labels stay open data; no condition is evaluated.

Names and `unitFormat` reuse `texts.publicNames` and its existing domain/key IDs.
For each available language, `translations.value[language]` has direct `sourceText`
and `displayText` carriers. Display consumers use the existing normalized display
form; UI template consumers use the exact winning decoded `sourceText` as inert
data. Whitespace, percent syntax and literal escapes are preserved there. No third
language wrapper, protected-text annotation or second lookup registry is introduced.
Missing keys, placeholders, empty translations and missing language files keep their
existing distinct states and once-per-occurrence display-based summary counting.

Scale preserves exact ResourceNumber storage, including Single bits, signed zero,
NaN payloads and infinity. Readable wrong-width numeric storage remains visible and
separately violates the declared Single contract. Only exact original Scale.scale
Single receipts attached to an emitted physical definition can cover nonfinite raw
tokens. A matching GUID, another field with the same bits, filtered dependency or
repeated getter cannot provide coverage for a different original token.

## Original contributions and publication

Body coordinates use the original physical Record index and full authored path,
including original array positions. Source type and payload class selection use the
actual `path.$type` Selector. Numeric and Enum data use their original Payloads.
Root/child/list carriers keep the original read contributions when their Rust payload
is mapped to local links; no Conversion is minted for an exporter-owned index.
Header coordinates remain separate under `dataforgeRecordHeader`.

Text forms use their authored Locale selector, actual original language lookup and
winner payload. Source text has only justified original Entry-decoding Conversion;
display text additionally retains actual display conversion. Unknown source lineage
stays unknown, and no entire row/Record is used as a substitute for field origins.

The existing field-read diagnostics and normal/diagnostic publication policy remain.
Strict source failures omit the physical row once; malformed/unsupported readable
facts remain explicit and refuse normal publication. Filtered and intentionally
excluded rows retain their actual reads and health but supply no emitted numeric
receipts or text occurrences. Cancellation and atomic publication keep the previous
complete output. Public Rust `read_crafting` remains an independent interpreted API;
it does not imply schema-v2 output or a second legacy wire view.
