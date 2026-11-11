# Catalog export health

Catalog version 3 contains required `exportHealth`. `completeness` still describes whether a
Filter restricted each kind. Health describes whether the selected sources and required
dependencies were successfully read and projected. A filtered catalog can be healthy; an
unfiltered diagnostic catalog can be incomplete.

Normal export refuses incomplete data before opening the destination. Explicit diagnostic export
uses `--diagnostic-partial` in the CLI or the separate diagnostic action in the desktop window.
The resulting file records `publicationMode: "diagnosticPartial"` and its actual health. A
successful write does not turn an incomplete diagnostic result into a healthy snapshot.

| Field | Meaning |
|---|---|
| `status` | `complete` or `incomplete`, considering all blocking diagnostics |
| `read`, `projection` | Separate overall canonical-read and typed/derived projection status |
| `publicationMode` | Requested `normal` or explicit `diagnosticPartial` |
| `uniqueSelectedSourceRecords` | Distinct physical source Records across participating kinds |
| `selectedKindRecords` | Sum of per-kind selected Record inputs, including overlaps |
| `writtenRows` | Sum of actual serialized kind rows |
| `kinds` | Per-kind source mode, inventory evidence, stage status, counters and affected identities |
| `metadata`, `failures` | Shared immutable text storage and paired retained human failure cells |
| `failedRecords` | Exact failed sources with four immediate sourced header cells, roles, stages and stable categories |
| `failedEntries` | Exact failed Archive Entry identities and required operations, including nested source paths |
| `unresolvedReferences` | Unsampled unresolved target GUIDs or raw instance coordinates with owner and field |
| `diagnostics` | Full counts with explicit units and at most eight deterministic examples per aggregate |

Source indices identify physical entries in this Build's Record table. Two entries can share a
GUID and still be separate selected or failed sources. Compare GUID/path semantics across Builds;
do not persist an index as a cross-Build identity.

## Original Record subjects and shared text

Each `failedRecords` row carries `recordIndex`, `guid`, `recordPath` and `sourceType` as
immediate `SourcedValue` cells. The first two retain the physical Record position and canonical
GUID. `recordPath` and `sourceType` preserve a readable empty string as present; a failed original getter is unreadable, without an invented null state. Every
retained diagnostic example has the same direct subject cells for `recordIndex`, `recordGuid`
and `recordPath`; an example without an applicable Record has absent subject cells.

Present filename/type text uses `{ "atomIndex": "0" }` as a report-local storage reference into
`exportHealth.metadata.texts`. The canonical decimal index is structural bookkeeping, not an
original source identity. Each referenced text is stored once per exact spelling and shared with
retained human failure text. GUIDs remain direct canonical strings. The text store is sealed once
for the retained report, including Normal refusals; selected but unretained header text contributes
no stored atom or unused header Payload claim.

Importers resolve a text reference only in the same owning report. They classify the semantic
field using that immediate occurrence cell's marker, refs and completeness. The storage atom has
an aggregate origin for its storage datum; it cannot assign a member or replace the occurrence's
origin. Equal text from a plain and encrypted input may share one atom while the plain occurrence
remains plain. Filtering BaseTool members by the atom's aggregate marker would incorrectly
back-taint that plain field.

The Rust API borrows context-bound collections through `kinds()`, `failed_records()`,
`failed_entries()`, `unresolved_references()`, `diagnostics()` and diagnostic `examples()`.
Failed Record and example getters borrow their direct cells; `record_path_text()` and
`source_type_text()` borrow already paired semantic text views without accepting a foreign pool.
Cloning the whole report retains its immutable context. Storage-only `metadata().text(reference)`
does not establish that a reference or occurrence belongs to that report.

The human CatalogFailureView exposes guid(), record_path() and reason() from its private
owning Details borrow. A normal CatalogError::IncompleteExport contains one CatalogRefusal;
health() and source_inventory() borrow that inseparable pair. Public callers cannot replace
one report's text or inventory with another report's values. The separate interpreted
CatalogFailure remains a three-field value without claiming captured original authority.

Original receipts are captured from the admitted Record before its tree read can fail. Header
values, GUIDs, paths, pool indices and structural equality never reconstruct this binding.
Accounting-only observations cannot pass the productive report finish as captured subjects.
The source Registry and bound input are private; visible equality compares values and origins,
not authority. Entry identities and remaining diagnostic body fields retain their separate contracts;
Record subjects and unresolved Reference bodies do not establish complete origin coverage for
all Health data.

A missing file diagnostic attached to a resolved loadout validates the original failed attempts
and retains their requested path. A cached result keeps that path and takes its caller selectors from
the particular resolver invocation that used the result. An included file also retains the
original Include route. The selected caller's fields and the original path each contribute to
the immediate diagnostic datum; other invocations using the same cached file do not contribute.

A separate file-read diagnostic retains the selectors from the original read and the Include
route that led to it. A later cached use has its own resolver context and does not add selectors
to the original file-read datum.

An XML class target diagnostic includes the original class attribute and the physical Record
selected for that class. Its selectors follow the caller and the Include branch that supplied
the selected entry. The original file read can be reused across callers and Include branches.

## Unresolved Reference occurrences

Every retained `unresolvedReferences` row has direct source cells for its physical owner index,
GUID and path, canonical `field`, owning `sourceType`, target GUID/index/path/type/location, and
all applicable `sourcePointer` words. `kind`, `sourceRole`, `status` and the optional pointer
container describe control or applicability. The pointer's `structIndex`, `instanceIndex` and
`declaredTypeIndex` each have their own sourced state; unavailable facts do not become zero.
Long text uses the same report-local text store and paired views as retained Record subjects.
A known inapplicable target fact is absent with a complete empty contribution set.

The reader captures a private receipt from the admitted original Record/tree and actual field
storage before final catalog target selection. A stored Reference GUID retains its own original
body Payload. Selected target index/type/filename facts retain the authentic target header and
current caller Selector. A weak-pointer-derived GUID also retains its actual pointer Conversion.
A class-array error's struct index comes from the original declaration Target Selector; its
instance index is a decoder Conversion. Raw pointer words retain their own Payload contributions.
Query-validation GUID and struct-index facts without a retained target admission keep their exact
values with incomplete origin rather than acquiring invented authored Payloads or resolved targets.

Reference receipts register semantic Payload contributions only for retained unresolved rows.
A target that has an emitted catalog row adds no unresolved Reference Payload claim. Finish
checks the original input and inventory, retains all unresolved occurrences, and seals failures,
subjects, diagnostics and References together once. Equal descriptors, DTOs, source coordinates
and text atoms never recover this authority. The Rust row and pointer fields are private;
getters borrow direct cells, and `field_text()`, `source_type_text()` and target text getters
borrow their already paired occurrence views. A complete report clone preserves the pairing.
Interpreted crafting and ShipReader compatibility values retain their separate public contracts.

The per-kind `sourceRecords` counts reconcile as
`selected = projected + intentionallySkipped + omittedAfterReadFailure + omittedAfterProjectionFailure`.
`readSuccessful` includes a readable source whose typed projection failed. Dependencies and failed
selection candidates are separate. A completed typed projection can yield zero rows. One Generator
can yield many rows or canonical source sets. Row duplicates and row-level policy exclusions count
in `intentionallySkippedRows`, independently of source-Record skips.

`sourceEntries` has separate physical Archive Entry units and uses `failedRequiredReads` and
`failedProjections`, because a failed operation can retain partial rows. Required operations such
as directory opening, entity-link reading and SOC projection share one physical input count while
their exact failed operation identities remain available. A required streaming read completing
does not assert that every byte or integrity checksum of that file was validated.
Aggregated Entry failures use `countUnit: "entryOperations"`; two failed required operations on one
file do not become two physical inputs in `sourceEntries`.
Kinds selected from Records can also consume Archive Entries as dependencies; their primary source
mode does not exclude XML Loadout, vehicle implementation or patch evidence in `sourceEntries`.
Byte-read operations distinguish `xmlLoadoutBytes`, `vehicleImplementationBytes` and
`vehiclePatchBytes`; intrinsic document projections have their own matching operations. Paints
reuse actual vehicle inputs without inheriting unrelated XML Loadout failures.
An explicitly named patch carries the selected XML Modification name and the requesting Record
value as separate selectors. The selected XML patch filename supplies the path payload and its
conversion.

`sourceInventoryComplete` establishes that the actual selection of required inputs was completely
enumerated. A known truncated walk or an unreadable source hiding configured inputs sets it false
and adds a blocking `incompleteInventory` diagnostic. In that case observed source counts are lower
bounds; they do not claim to count unknown inputs. A missing inventory observation is an accounting
error rather than a successful empty selection.
A root Archive actually recovered by directory scanning cannot establish complete source
membership, even if its recovered DCB is readable. Its global `incompleteInventory` diagnostic
uses `recovered_archive_directory` or `archive_recovery_limit`; every potentially affected kind's
inventory and stages stay incomplete. Observed counts remain lower bounds, with no fabricated
Entry-zero/Record failure or estimated lost count. Merely enabling recovery on a healthy directory
does not change health.
`fieldInventoryComplete` establishes that owned materialized fields were inspected within the
canonical depth/value budgets. An absent optional field, explicit empty value or dangling target
with retained identity is represented distinctly from an unreadable field. The field inspection
does not follow References into other owners. A retained row can have a failed field or derived
operation; it still counts as written while the relevant stage and diagnostics remain incomplete.

Source warning diagnostics retain the established warning code and producer count. Diagnostic
examples contain technical identity and field metadata rather than arbitrary game values or raw
error messages. The eight-example limit does not truncate `failedRecords` or unresolved target
identities.

Resolved links can include `targetRecordIndex`, the physical target index in this source Build.
It is null when the target is unavailable or only raw pointer coordinates are known. This index
disambiguates file-local provenance; it is not a persistent domain identity across Builds.

For import removal decisions, require an understood schema version, the intended Channel/Build,
unfiltered `completeness` for the kind and complete catalog-wide export health, including global
blocking diagnostics. Healthy-looking per-kind fields never override an incomplete overall
result. Unreadable metadata with unknown kind invalidates every potential kind's source inventory
and read status, while its failed identity and counters remain global rather than guessed per kind.
Preserve existing rows when a diagnostic artifact, failed source, incomplete dependency or restricted selection cannot prove
absence. See [ADR-0347](adr/0347-catalog-health-measures-production-and-gates-publication.md).
