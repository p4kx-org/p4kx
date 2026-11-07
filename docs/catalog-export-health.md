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
| `failedRecords` | Exact failed source indices, GUIDs, paths, roles, stages and stable categories |
| `failedEntries` | Exact failed Archive Entry identities and required operations, including nested source paths |
| `unresolvedReferences` | Unsampled unresolved target GUIDs or raw instance coordinates with owner and field |
| `diagnostics` | Full counts with explicit units and at most eight deterministic examples per aggregate |

Source indices identify physical entries in this Build's Record table. Two entries can share a
GUID and still be separate selected or failed sources. Compare GUID/path semantics across Builds;
do not persist an index as a cross-Build identity.

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
