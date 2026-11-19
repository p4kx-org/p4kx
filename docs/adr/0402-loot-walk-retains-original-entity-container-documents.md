# ADR-0402: Retain original container-link documents reached by the Loot walk

Status: accepted

## Context

The existing Loot derivation interprets the first nonempty `objectContainer` and first valid
`multiConfigRef` found in document order. It follows the first container path even when the
configuration is absent. A configless source can therefore produce a real missing-target Health
observation without an original declaration row. Health is an operation report and cannot replace
lossless authored values. A document can contain several XML `Entity` elements, equal GUIDs and repeated
attributes; global first-pair interpretation does not establish their ownership.

## Decision

Schema v3 additively includes `declarations.lootEntityContainerDocuments`. Its scope is only the
physical `entdata/*.entxml` documents actually reached by the existing Loot container walk and
containing an `objectContainer` attribute. It does not inventory all Entity files in the Archive,
all references authored in unvisited containers, or every placement instance.

Each row identifies its actual containing container and physical nested Entry. Its normalized
`nodes` contain each relevant link/config node and nearest actual XML `Entity` owner once, with original
node/parent/Entity positions, names and ordered original attributes. An XML `Entity` owner is established
only by XML ancestry, including self for that element's own attribute. XML `Entity` GUID equality never
merges owners. Parent positions refer to the original XML, so a parent need not itself be emitted.

`links` contains each actual `objectContainer` attribute occurrence in document preorder and
attribute order, including empty or invalid original text. Node and attribute positions identify a
site within its physical document. `configReferences` independently retains every original
`multiConfigRef` attribute in that document, its parsed GUID state and exact node/attribute
coordinates referring to the one stored raw attribute. Null GUIDs remain
present GUIDs; empty or malformed GUID text remains raw with typed invalid interpretation. No
configuration is copied onto a link or paired through a global Entity GUID.

Each link retains the unchanged authored selector and sourced directory lookup status/candidates:
`resolved`, `notFound`, `ambiguous`, `unsupportedSelector` or `incompleteDirectory`. This is the
existing exact normalized/root-candidate lookup, not a basename association. A candidate proves
only a physical directory target, not readable content or a completed placement. Negative
membership cites one original root-directory survey shared by the walk. A recovered directory or
unsupported selector cannot certify a complete candidate set. Actual duplicates remain ambiguous
even when other directory membership is incomplete.

For an unambiguous, completely observed selected attribute whose literal lookup has no target
and whose grammar is unsupported by the ordinary lookup, admit a complete `Data/ObjectContainers/`
virtual key after a validated drive-qualified, parent-relative, or timestamped authoring prefix.
This repair requires one structural root and one physical candidate in the intact root directory.
The original literal, exact positional Selector, conversion and directory evidence remain
available. A required path without an exact candidate remains unsupported. Duplicate physical
candidates remain ambiguous. The canonical document lookup and traversal consume the same selected
result. The rule describes P4KX's lookup for packed data.

Immediate values, original collection surveys and lookup results retain their real source-use
sets, completeness and encryption marker. The successfully parsed original Entry content supplies
collection/ancestry surveys; exact node/attribute coordinates supply original values. No origin is
reconstructed from a DTO, path equality, GUID equality or a Health example.

A single existing indexed row spool stores the new table in physical outer/nested Entry order.
Original rows are released after each parallel frontier is merged; only existing traversal
summaries and spool keys remain. Repeated traversal roots do not multiply a physical document,
and equal paths or equal bytes at different physical Entries remain distinct. Cancellation and
spool replay failures retain typed failure handling. No quantity/depth quota is introduced.

Availability describes this reached source family: available once such a document is observed,
absent after a successful reached-scope inspection with none, or unavailable when failures prevent
that absence from being established. Availability does not certify complete derivation or a
whole-Archive Entity census.

## Consequences

Normal publication retains a `notFound` lookup from a completely observed original selected
attribute when the directory proves an empty candidate set. The `verifiedMissingReference` warning
records that outcome. The derived Loot counts for the unavailable container remain absent.
Required unproved lookups and source read or projection failures still refuse publication before
output creation and preserve the previous complete generation.

Correcting the existing first-pair derivation or any global owner conflation requires separate
source-backed ownership proof. This carrier makes those originals inspectable without claiming
that correction. Its SDK strategy is a future source-scoped adapter sharing this reader and exact
physical identities; this change adds no direct-query API or SDK completeness claim.
