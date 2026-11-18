# Equipment SDK shares original family readers and canonical variant choices

The Equipment SDK and Catalog producer use the same original family interpreters. Selective
queries do not reconstruct canonical rows from JSON, curated DTOs or whole-Catalog output.
Source-backed and derived field values carry their own immediate typed origin cells under
ADR-0357. An arena, row marker, Inventory or owner DCB origin does not substitute for those cells.
The unreleased schema-v3 migration removes redundant legacy views; released-v2 fixtures stay frozen.

## Original equipment families

The first independently usable slice exposes `definitions.items`,
`configurations.itemAttachmentConstraints` and `configurations.itemBounds`. These retain the
canonical original source scope, selection, physical occurrence identity, row order, exact numeric
representations, reference states and text domains. They are not complete raw Entity dumps.

Item definitions use the original Item source reader for presentation, physics and purchase facts.
Attachment constraints and bounds use explicit scopes of the same entity-configuration reader as
the producer. Bounds do not run the unrelated storage or attachment-constraint projections;
attachment constraints do not run occupancy, equipment statistics, loadout or world projections.
Neither configuration family reads localization, even through a session with languages enabled.

Free Archive/decoded-Entry queries and session/parsed-view methods share original Record admission
and source capture. All/GUID/physical selection retains every admitted physical same-GUID owner and
all repeated component slots. Definitions keep canonical GUID order with physical ties;
configuration rows retain physical Record/component order. Family absent, successful empty,
unavailable fields and unreadable source coverage remain distinct. Original text keys use the
existing public-name and protected-text identities and their actual selected locale evidence.

Borrowed snapshots provide complete immutable typed fields, text resolvers, source inventories and
query coverage. Callers publish owned cache changes only after outer success and the necessary
coverage checks. Archive-change, origin-binding and cancellation checks remain in the shared query
session. A narrow successful query does not certify full-Catalog publication. No default total-row
quota or new full-Catalog dependency is introduced.

## Canonical Item, Ship and Loadout projections

The next projection slices normalize `items`, `ships` and `loadouts` rather than provide a permanent
legacy SDK alternative. Existing original definition/configuration fields are referenced or retained
in their canonical source families. Every independently stored classification, sum, target choice,
port field and variant result needs an immediate sourced value at its actual original read or
conversion. The same item in two occupied ports remains two occurrences. Unresolved authored
selectors retain their original data and typed lookup state. Missing original proof remains
unknown/incomplete; it never becomes unencrypted through the owner's DCB alone.

The producer and SDK share the actual Archive-backed resolver, XML/Record receipts and ordering
policy. Read/projection health or a file label alone cannot certify an individual XML datum.
Producer staging and selective response sinks consume the same authenticated rows; an SDK request
does not acquire a second mandatory spool or prepare unrelated Catalog domains.

## Variant choices use the unfiltered canonical domain

`variantOf` is a function of the Build's canonical unfiltered admissible family candidates.
An output filter limits returned rows and does not redefine source facts, grouping or variant
answers. A selected row can therefore name an original winner outside the returned owner set;
its physical source and actual selection proof remain resolvable through the result inventory.

Preserve the existing key interpretation and base comparator: player playability first, then
lexical class name, then textual GUID. A base, a singleton or a candidate without a usable key has
no variant link under the existing rules; physical aliases with the base's same nominal GUID do
not become variants of themselves. Original physical candidate identity remains distinct from
nominal GUID, including duplicate-GUID declarations with different implementation keys.

The candidate survey and winner choice are shared by producer and SDK. Retained key/rank inputs,
actual selectors and sufficient negative-survey evidence contribute to the result. An unclassified
candidate or missing required implementation fact cannot prove a complete null/winner. Preserve
unknown source-set completeness and known actual encrypted contributions independently.

This decision establishes the target for the subsequent Item/Ship/Loadout migration. The original
three-family query slice does not claim to implement those projection or filter changes. Its
acceptance requires typed external-consumer parity against actual canonical output, original source
and numeric closure, selection/failure/cancellation controls and no unintended dependency reads.
Projection migration, source-backed candidate scope and client memory acceptance remain separate
required checks.

## Bounded Item source and classification projection

The selected Item root uses the shared original eight-field Header, sourced first physical
Attachment/Physics/Purchasable ordinals, and independently sourced itemClass and playability.
Its seven redundant raw source fields are removed in favor of physical joins to Item definitions.
Item definitions retain original Type/SubType/Size/Tags/RequiredTags even for a foreign valid
AttachDef; constraint-family admission remains unchanged. The source reader supports the actual
Class or StrongPointer Localization form. First-family selection never chooses a later successful
payload after an earlier null/unreadable one.

The source reader captures text references, original Locale tokens and their exact selectors
together. Internal semantic inputs are not serialized and are not reconstructed from text IDs,
DTO fields or row/global origins. English ClassLine uses its original Description selector and
actual English lookup/display conversion. Item semantic language preparation always admits
English independently of output requests; a source that was not consulted cannot establish
absence. Key-only playability and compact Paint/key identity facts use the same original read,
with the established predicates and stats-key representation preserved.

The shared projection rejects a definition from another physical Record, including a same-GUID
alias, and rejects foreign-inventory header evidence. Producer and subsequent SDK use the same
admitted source objects rather than clone their original interpretation.

The Item source query uses the same original definition frame, physical selection and sourced
class and playability readers as the producer. The full Item flattens the same source fields
beside its variant decision. Source queries return the selected physical owners, including
same-GUID alternatives. English contributes to classification through its actual lookup.
The response keeps presentation languages and text dictionaries empty and reports physical
coverage for the returned fields and their dependencies. Definition coverage describes the
complete definition. Playability preserves an unusable purchase name's state before evaluating
lower-priority rules and consumes the attachment name in the name rule. Variant decisions
require their own unfiltered candidate survey.

This bounded migration does not introduce a provisional unknown variant carrier or weaken
normal-publication guards. variantOf remains the existing nominal value until its actual
unfiltered candidate survey and physical target/status projection are implemented together.
There is no complete root-Item SDK claim or alternate legacy DTO in this source slice.


## Ship Record fields

The Ship source reader checks that the Record and tree describe the same physical owner and
indexes the original fields once. Locale and manufacturer projectors use that index. The reader
uses the same array-prefix selection for `Components` and `StaticEntityClassData`. Earlier
unreadable candidates keep the corresponding component or insurance selection incomplete.

The Ship row flattens the shared eight-field Record header. The export and its source inventory
use the physical Record index for joins. `crewSize` and `AttachDef.Size` retain exact numeric
storage. `vehicleDefinition` retains the original String or Enum value, including empty strings,
case, separators and whitespace. Insurance retains the first exact declaration, its static
ordinal and restore flag. Its parameter container carries its own state, and its numbers retain
their widths, bits and units. Additional original fields remain available.

`playabilityInputs` carries the selected component ordinal, the authored CrewManifest reference
and its target header state. Interpreted playability, XML totals, cargo and variant results
depend on the respective readers and their sources.
