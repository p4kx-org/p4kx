# Contract pickup places retain every eligible source location

## Decision

Catalog schema 3 retains every resolved non-star pickup place selected by the existing contract
prerequisite rules. There is no separate five-place publication cap. This supersedes only the
pickup-list length policy in ADR-0068 and ADR-0273; their selection and fallback decisions remain.
The schema version remains 3 and the field shape stays unchanged.

The existing stable order is preserved: star systems first, then planets and moons, then other
objects, with authored order inside each group. Each selected GUID appears once. Missing starmap
objects and stars remain excluded. A sub-contract's own locations still replace inherited ones;
a contract with no resolved eligible pickup still uses its existing broker-location fallback.

`offeredAt` remains the authored declaration view: locality references are not expanded, stars
and missing object references stay present, and fallback is used only when no place was stated.
It is not a second copy of the ranked pickup projection. Origin capture, physical identity and
per-datum encryption policy are unchanged; matching GUIDs or paths do not establish new authority.

## Rationale

A display-size convention must not discard eligible source facts from the normal catalog.
The ranking already materializes the complete candidate vector before publication, so removing
its final iterator cut does not change candidate reading, sorting or deduplication. It only
retains the previously omitted output entries and their existing description path.

## Validation

The tests-first synthetic case verifies seven actual original locality references in the
canonical Record tree before comparing the exported pickup GUIDs. Root's baseline compiles and
fails at seven original references versus five exported entries; it is a semantic failure.
After the fix this same oracle must pass unchanged. Existing ranking, duplicate/star omission,
sub-contract inheritance, empty selection and broker-fallback guards remain required. The two
older five-place assertions change only their obsolete cardinality contract to seven.

Generated schema descriptions, normal/diagnostic catalog paths and relevant contract consumers
must be checked after implementation. No installed-Build result or runtime GREEN is established
by this decision.
