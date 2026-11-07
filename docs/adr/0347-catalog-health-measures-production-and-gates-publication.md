# ADR-0347: Measure catalog health at production boundaries and gate publication

Status: accepted

## Context

Filter completeness describes the selected scope. It cannot tell a consumer whether an absent row
was successfully absent, intentionally excluded, unreadable or unsupported. A console summary
does not survive transferring the catalog file. One Generator can emit many contract rows, several
kinds can use the same source, and a ship can remain present while its loadout fails.

The canonical reader can return a readable Record tree containing `Value::Error` leaves. Optional
mapping helpers may turn those leaves into nulls. Counting only thrown Record errors therefore
does not establish successful field reading.

## Decision

Version 3 carries required `exportHealth`, independently of the unchanged `completeness` selection
contract. The library defaults to `CatalogPublicationMode::Normal`. Incomplete health refuses both
in-memory publication and streaming publication before the output creation callback. Explicit
`DiagnosticPartial` permits the diagnostic artifact and records that mode inside it. CLI and GUI
must display the incomplete result and the explicit mode; a GUI choice applies to one job.

Production readers record source selection, canonical reading and typed projection where each
operation occurs. An immutable ledger retains these observations before compatibility-summary
deduplication or row aggregation. No source counter is reconstructed from emitted row counts.

Source identity is the physical Record-table index within the identified source Build, with its
GUID and readable path retained as metadata. GUIDs alone are not unique within a DataForge. The
same physical source in several kinds counts once in the overall unique-source counter and once
per participating kind in `selectedKindRecords`. Repeated identical observations of one
index/kind/role are deduplicated; contradictory observations refuse publication as an accounting
error. Physical indices are not persistent identities across Builds.

For selected Record inputs, the reconciliation is:

```text
selected = projected + intentionallySkipped
         + omittedAfterReadFailure + omittedAfterProjectionFailure
```

Successful projection includes an explicitly completed zero-row projection. `readSuccessful`
includes a readable source whose projection subsequently failed. Dependencies and uncertain
selection candidates are separate roles and never inflate omitted selected Records. A retained
ship with a failed loadout keeps its written ship row and zero omitted ship Records; the failed
secondary operation is counted separately. Row fan-out and intentionally skipped rows have their
own row units. Archive Entry readers identify their different source mode.

Each successful canonical read has a bounded, cancellable field inspection before optional typed
helpers. It walks owned structs and arrays only, never follows Reference graphs, and applies the
canonical depth and value limits. Immutable inspection proofs can be reused for another explicit
role or kind by physical source index, without another decode or field walk. A retained row with
a failed field remains projected and written while health explains the field failure and refuses
normal publication. Inspection limits are explicit incomplete evidence, not successful absence.

Diagnostics distinguish optional absence, explicit empty values, dangling References, outside-owner
weak pointers, unsupported values, malformed values and actual read failures. Represented dangling
identities remain available without pretending that the target was resolved. Stable source warning
codes retain their actual producer counts and explicit units. Every existing failure or warning
that makes the summary incomplete has an artifact explanation.

Diagnostic aggregates retain full counts and at most eight deterministic technical examples.
Exact failed source identities and unresolved target identities are not sampled. Diagnostics carry
field locations, struct names and link coordinates where available, without copying arbitrary raw
values or error-display strings. The normal catalog payload retains its authored source values.

## Consequences

Consumers require both an unfiltered selection and healthy export before treating missing rows as
removals. Diagnostic artifacts support investigation and retention decisions but cannot establish
deletion. Existing summary row counts and Filter completeness keep their units and meanings.

Every producer records its actual inventory enumeration, including a successful empty one. A
known truncated walk or an unreadable source hiding configured inputs records an incomplete
inventory and a blocking diagnostic; observed source counts are lower bounds while that flag is
false. A missing inventory observation, missing canonical inspection proofs and inconsistent
counters are accounting errors even in diagnostic mode. Cancellation, changed-Archive checks and
atomic file commit remain independent publication protections.
An evidence lock poisoned by an unwinding worker is an accounting failure; no captured state is
recovered from that lock. A sound previously captured fatal error retains its original type.

Unreadable selection metadata with unknown kind conservatively invalidates every potential kind's
input inventory and stage completeness. The actual failed physical identity and candidate counts
remain global; they are not copied into invented selected sources. A failed global dependency does
not by itself imply unknown selection membership. Consumers check overall health and its global
diagnostics before any removal decision, even when a particular kind has no local failures.

Actual root Archive recovery likewise cannot establish complete Entry membership. Typed
archive-wide recovery and scan-limit markers produce one global blocking inventory diagnostic,
without assigning the marker's storage position to an affected Entry or Record. All potentially
affected inventories stay unknown and counts remain observed lower bounds. An enabled recovery
option with an intact normally read directory creates no uncertainty.

Required dependency-file byte reads and intrinsic document parsing are distinct Entry operations.
Completing a byte read does not assert that a parser ran. Per-Entry aggregation retains all required
operation outcomes while counting the physical file once. Context-dependent application failures
remain derived failures of the actual root rather than conflicting facts about the file's bytes.
Byte-read operations retain their actual caller purpose, so paint derivation reuses vehicle
implementation and patch inputs without claiming unrelated XML Loadout dependencies.

Resolved source links retain the actual physical target Record index when known. Canonical weak
ownership compares that target index with the owning tree index and canonical location; GUID
equality cannot establish ownership in the presence of aliases. The indices describe this Build's
provenance and do not change Record JSON/XML or persistent domain identities.
