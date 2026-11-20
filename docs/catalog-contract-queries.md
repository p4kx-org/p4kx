# Selective canonical Contract queries

The archive-bound `CatalogQuerySession` and its reusable `ParsedCatalogQueries` view expose
`with_contracts(selection, cancel, callback)`. One-shot `with_contracts` and
`with_contracts_from` wrappers are available from `p4kx_export::catalog_queries`. The `_from`
form checks that the decoded Entry belongs to the opened Archive. Publish consumer-owned cache
changes only after the outer query returns successfully; cancellation and Archive freshness are
checked by the same session contract as other domain queries.

## Root selection and dependency scope

`CatalogRecordSelection::Guid` selects every admitted physical `ContractGenerator` Record with
that owner GUID. `RecordIndex` selects one physical owner. These are Generator identities;
they are not the logical contract `id` or sub-contract `id`. `All` returns the canonical complete
Contract and mission-title projections in ordinary Catalog order. Raw source-only and development
owners remain individually accessible through `with_contract_generators`.

`with_contract(ContractKey::new(contract_guid, sub_contract_guid), cancel, callback)` selects
one exact logical identity instead. The one-shot and `_from` forms are exported alongside
`with_contracts`. `None` selects only the parent row; `Some(sub_guid)` selects only that
sub-contract under the supplied parent. The native input key is separate from the sourced output
identity and the physical Generator GUID. The result contains zero or one canonical row and its
paired mission-title row. Every physical owner of the exact pair remains eligible until the shared
whole-owner validation and global duplicate selection finish; an invalid earlier candidate cannot
hide a later valid owner. Shadow coverage is restricted to that exact logical pair.

A narrow query reads the source dependency closure required by the selected roots:

- Forward-linked Contract templates, mission types, organisations, rewards, mission items,
  localities, brokers, faction names and starmap name/type facets use the existing canonical
  constructors and duplicate-candidate policies. Every physical candidate under a required nominal
  GUID remains eligible; mission-item text identity retains its exact physical Record index.
- A source-only survey of Generator owners finds relevant outside-root intro grants, logical-ID
  contenders and property-set aliases. It does not run their unrelated sibling projectors or the
  projectors of unrelated Generator owners.
- Related Generator owners pass the shared whole-Generator Contract transaction before they can
  supply a positive grant or a duplicate contender. An invalid unrelated entry in that owner
  prevents its entire derived contribution, as in the canonical producer.
- Canonical property alias handling and global logical duplicate ordering run before root output
  filtering. `coverage.shadowedCandidates` identifies selected rows replaced by a winning outside
  owner. Dependency-only rows do not become selected output rows.

This scope is independent of the complete producer's existing path Filter. A filtered producer
run is not used as the narrow query implementation. It could omit a needed outside intro and
produce a misleading empty join.

## Result and typed traversal

`ContractSnapshot.rows` contains canonical `ContractEntry` rows. `mission_titles` contains the
paired titles and descriptions with the same physical `ContractOccurrence`. `generators` and
`dependency_generators` contain original source definitions in the logical result's closure;
source-only owners without an admitted logical row are queried with `with_contract_generators`.

`mission_property_sets`, `ship_spawns`, `npc_spawns`, `entity_spawns` and `location_queries`
retain canonical typed dependency data and their stable identifiers. Owner selections retain their selected physical owners' authored property sets, including source
declarations not selected by a runtime layering decision. Exact logical-key selections retain only
the effective property sets linked by their returned row. Their reverse alias and intro selection
uses that row's own effective set links and requirements, so another row's intro provider does not
become a validation dependency. Canonical conflicting aliases stay quarantined with explicit diagnostics.
Spawn tables may include other declarations in an admitted complete source transaction. Consumers
join them using the ordinary pool and query GUIDs; they need not recompute stable identifiers.

All exported nested data has a public typed view: layered facts, cooldowns, rewards, hauling
alternatives, scoped-location filters, investigation clues and names, owner-local references,
property options, spawn groups and title token outcomes. `MissionPropertyText.published()` returns
the protected text cell after the canonical publication step. `ContractOccurrence` exposes its
physical owner and original arena value references; `GeneratorDefinition.source()` exposes the
shared original ordered typed arena. A consumer can follow the derived row to its exact source
without serializing either value.

`public_names` and `game_texts` contain referenced canonical text rows. Per-datum source origins
and encryption markers retain the same meaning as Catalog v3. All field source-use references
resolve through the snapshot's own `source_inventory`. Requested language policy is preserved;
`available_languages` lists only payloads actually available and read. A selection without an
admitted logical contract does not open localization or world/media payloads. Contract place
joins use only starmap name/type facets; container placement, SOC scenes and world coordinates
are separate domain queries.

## Coverage and cache updates

The result separates selected-owner coverage, reverse-survey scope, related Generator owners,
forward candidates, canonical projection failures, shadowed candidates and field diagnostics.
The absence of a reverse candidate is established only by the actual source survey. An unreadable
header/body is an explicit gap, and the affected intro join has incomplete source coverage rather
than a complete empty value. Source-only positive observations never bypass canonical Generator
failure. Per-field `SourceValue` state continues to distinguish absence, null, unsupported,
unreadable and invalid values.

A consumer may refresh the selected roots and their joined dependencies after the complete query
returns successfully. It must not interpret a coverage gap as proof that cached rows were removed.
It must also preserve globally shadowed-row information rather than resurrecting a selected loser
as a local winner. Query results are independent inventories; retain each result's inventory with
its values instead of mixing source-use references from different callbacks.

The SDK adds no Catalog wire fields or schema-version change. Public visibility and documentation
can regenerate equivalent schema annotations; Catalog schema v3 remains the emitted contract.

See [ADR-0401](adr/0401-contract-sdk-selection-retains-global-join-dependencies.md).

## Logical lookup resource policy

Finding a logical pair requires an archive-wide source-only Generator survey because the pair is
nested inside its physical owner. This is not a full Catalog projection or a global MissionIndex.
The logical planner retains trees only for exact matching owners and keeps compact grant-tag and
authored-set identity summaries for the remaining owners. Once the target's dependencies are known,
it materializes only related outside owners again for their canonical whole-owner transaction.
The decoded DataForge Entry and parsed layout/reference indexes are reused; this trades extra tree
materialization for avoiding retention of every Generator tree. Source-use evidence and survey
coverage are still retained and contribute to memory use.

Whole-owner validation may return other source declarations and spawn pools from an admitted
transaction. The exact-key row and title projection remains singular; extra source closure data
is identified by its original physical owner and stable pool or set identity. A readable exact
identity remains a validation candidate when a later survey field fails. The original failure is
reported, and uncertain reverse scope never proves complete absence or bypasses source validation.

## Canonical dependency preparation

Forward dependencies are requested by the shared typed Contract interpreter at the exact lookup
that needs them. Original raw references and unused tokens remain available in the source arena;
they do not activate their target bodies. A dependency key includes its canonical family and
nominal GUID, or the original physical Record index for a static mission item. Each nominal lookup
prepares every eligible physical candidate in original order before absence, fallback or duplicate
selection is interpreted. Starmap candidates use the same PU path and leftover admission as the
complete producer. Reward lookup retains its canonical last-usable candidate policy.

The query caches one GUID header index and every required body read, including failures. A private
typed preparation request suspends the current whole-Generator transaction before source-error
classification. It does not become a public diagnostic or a missing-value fallback. Once the
requested group is prepared as proven, absent or failed, the same owner is retried. Every retry
requires a previously unknown key; repeating an already prepared request is an accounting error.
There is no fixed retry quota or truncated successful result.

Probe texts, rows, pools and intro grants stay private. Only the final complete owner transaction
contributes derived rows and grants. A failed owner uses a private text registry as well; partial
titles cannot leak into the result's text definitions. Published place names are replayed only for
the actual Place lookups of the final owner attempt. Required body errors retain their original
physical identity in forward coverage and canonical failure diagnostics. The complete producer
uses its existing fully prepared inputs and does not perform the selective preparation protocol.

## Opt-in installed source witness

The ignored test
`real_archive_contract_sdk::selected_contract_sdk_matches_original_owner_and_logical_slots`
checks one explicitly selected physical Generator and two parent/sub-contract identities selected
from its original source. Set `P4KX_DATA_P4K` and `P4KX_CONTRACT_SDK_SELECTOR_FILE`; the latter names
a private JSON object with `recordIndex`, `branch`, `version`, `changeNumber` and `buildId`.
All four Build strings must match the opened Archive. Missing selectors and insufficient source
features are errors rather than a skipped or empty successful witness. The selected source must
exercise a parent, a sub-contract, an effective property-set link and a direct Record reference.

The witness decodes one DataForge Entry, shares one parsed SDK session across the three queries,
and opens an independent original view over those same bytes. It checks exact original handler,
contract and sub-contract value positions, logical IDs, direct source fields and reference targets,
property-set source locations and typed Boolean values when present. Source-use references and
per-cell encryption markers are checked against the independently observed physical Entry. A
shadowed owner must be explicitly covered; a logical query's actual winner is compared to its own
original physical body. Known source-coverage gaps fail this focused acceptance run.

The requested language list is empty. With an existing selector, the test emits one aggregate success line of at most 256
bytes and writes no files. It does not build or serialize a Catalog and does not read localization,
world placements or media payloads. Canonical reverse surveys still examine Generator sources and
the needed closure can include outside owners. This source witness does not establish an all-data
parity result or a memory guarantee for a machine with 16 GiB total system RAM.

If no suitable private selector exists, set
`P4KX_CONTRACT_SDK_DISCOVER_SELECTOR_FILE` instead to a new absolute private file path. The same
ignored test then examines Generator headers and one original candidate tree at a time until the
first source owner supplies all required witness features. It retains that exact tree and continues
the SDK comparisons using the same decoded Entry and parsed session. There is no separate full
payload pass. Discovery writes at most 4 KiB of private selector JSON with the full Build identity
and optional typed `discovery` counters for inspected headers, candidate bodies and source gaps.
The file is created without replacing an existing path and is immediately re-read through the
strict ordinary selector reader. Discovery records source selection; only the subsequent completed
SDK check establishes an acceptance result. Setting both selector modes, setting neither mode,
or finding no suitable source owner is an error.
