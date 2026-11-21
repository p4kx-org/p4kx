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

## Vehicle totals and their XML sources

An observer can request original vehicle evidence from the parser that supplies the totals. It
retains the element names and child lists used to select Parts, including the decision to skip a
subtree. The vehicle reader captures the selected Modification, its patch and applied Elem
overrides. The cache preserves the exact Modification spelling because that spelling can select
a different declaration in the same file.

The totals reader reports each metric's consulted fields in calculation order. Presence is
independent of original-coordinate availability. The hull calculation distinguishes an absent
ItemPort, absent flags and an effective empty flags value. Source coordinates use original text
ranges or CryXml indices. Repaired binary XML retains the file operation and leaves the original
coordinate unavailable. Read and application failures have distinct states. File and Elem
application roll back their staged evidence along with the tree on failure. Shared sites and first-use
deduplication keep the original operation identity when several decisions use the same field.

For every vehicle use, Assembly supplies the current physical Entity and its Vehicle request.
The Catalog must bind that request, the observed file operation and the metric's contributing
values to establish a complete source set for a published total.

## Catalog Ship aggregate publication

The Catalog joins the opted-in Assembly witnesses to the current physical Ship caller and
actual Archive admissions. The same ShipReader calculation produces the values and reports each
metric's consulted fields in one pass. The original Ship field walk captures the Physics
operand for potential fallback. Its origin contributes when that fallback is used. The authored
Vehicle selector, Modification state and component selection retain their original source facts.

Mass, its basis and hull health use immediate sourced carriers. Parts membership and requested
Modification/patch/Elem decisions are shared selection evidence. Numeric conversions, rejected
numeric candidates and structural flags belong to their respective metric. The basis consumes
selection evidence. Failed archive lookups use the consulted directory range. Failed payload
reads are recorded as attempts. Repaired XML carries its known file ancestry and reports
unavailable original coordinates.

Both coordinate incompleteness and unreadable requested inputs can accompany a present numeric
result. Without a numeric result, a failed configured read yields `unreadable` and a known empty
source yields `absent`. Positive source mass rounded to zero stays present with its actual
basis. The Catalog and standalone ShipBase share the traversal, fallback and rounding rules.

Capture drains its per-resolution staging before publication. Only successful resolution returns
aggregate cells and a reusable reader. Each cache hit binds the current physical Record to the
observed file operation.


## Selective Ship source snapshot

The selective Ship source query and canonical scan share one source-read, ShipPool/Capture and
conditional-Physics-diagnostic module. A common immutable ShipSource packet holds the exact
original Header, presentation, manufacturer, crew, vehicle selector, size, insurance and
crew-manifest cells, resolved mass, mass basis and hull health, and the typed
Ship base facts projection. It retains the canonical vehicle kind and playability.
The producer moves the same fields into the Ship row and publishes the base facts
in `projections.shipBaseFacts`. The [cargo decision](0416-ship-cargo-projection-retains-physical-mount-evidence.md)
defines its physical mount joins and origin coverage.

The query exposes free Archive/decoded-Entry, session and parsed-view interfaces. Physical
selection admits the same canonical owners. GUID selection retains all physical aliases, and
RecordIndex selects one exact original. Entity class lookup indexes source headers independently
of selected output owners and opens their actual resolver dependencies. Reader caches and original
proof bindings belong to the query's input and source inventory.

Borrowed snapshots reuse the Equipment text, locale and provenance context and attach a separate
typed dependency-coverage result. The caller checks owner and dependency coverage independently.
Both preserve incomplete coverage when a returned numeric value has incomplete source coordinates.
Missing requested inputs retain their failed-operation evidence. Each datum carries its own source
marker, including known encrypted contributors beside unknown inputs.

Cancellation and Archive-change checks surround selection, source capture and callback delivery.
Publish consumer-owned state after successful completion of the outer query and acceptance of
the required coverage. Use the unfiltered Catalog for variant projections.

## Selective resolved Loadouts

Catalog staging and the SDK use the same ordered Loadout resolver and port projector. The
Catalog passes its prepared configuration index. The SDK projects port and attachment
configuration for each reached physical Record and role through the same original readers.
Each demanded role is read once per query. A negative admission decision and an unavailable
dependency have distinct outcomes even when the port has no matching projected row.

The SDK identity of a Loadout is its Build-bound physical root Record. Each port occurrence is
addressed by the sequence of zero-based child positions under that root. This follows the actual
ordered resolver tree, including repeated names, repeated attached Records and nested empty
ports. Query results expose independent owner and dependency coverage before callers publish
their owned cache updates.

## Linked weapon recoil definitions

Each weapon action exposes its authored regular and grappling recoil references and interval.
A resolved reference identifies the physical target Record in the same Build. A GUID can refer
to multiple physical definitions, so a GUID query returns each matching occurrence in Record order.

The Catalog and SDK use the same definition projector for the original header, hands, aim, body,
head and additional fields. A physical query opens the requested definition and preserves each
field's origin. It reports an unreadable selected Record or incomplete field inspection in its
owner coverage. The caller reads the linked definition explicitly, so reading a weapon action keeps
its own dependency scope. A definition query needs neither the weapon Record nor localization.

## Shared component definition Records

The selected component-definition queries use the producer's original definition projectors.
They return canonical physical Records in DataForge order. A GUID is a selection key that can
name several physical owners. A resolved target Record index identifies one target in the same
DataForge. Preserve the original list order, numeric bits, text domains and per-value origins.

Turret modifier admission has two sources. The base definition type is declared directly.
Compatible derived types enter the family through turret definition References authored by
canonically admitted Entity components. A base-only or empty selection can bypass the
component survey. Derived candidates and full-family reads use the same type discovery as the
producer and surface failed and unclassified input owners as incomplete dependency coverage.

Only the consumable subtype definition model has localized fields. Open requested languages
for selected subtype owners and resolve the public-name and protected-text keys in the result
inventory. A consumer handles each linked definition as a separate query. It can reuse one
parsed session across these families, inspect selected-owner and discovery coverage independently,
and commit its own cache after outer success.
