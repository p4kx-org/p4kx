# An unknown location condition fails the Record, and the `other` variant is removed

Decided for issue #1011. Evidence: LIVE 4.10 (2026-10-31), every `matchConditions` array of a
location, organisation or mission item property. Amends ADR-0266 and ADR-0324 and applies
ADR-0054. The owner's rule from #812, repeated in ADR-0320 and ADR-0324, is that a struct the
catalog does not know fails the Record instead of hiding behind an escape hatch.

## Decision

- **`LocationCondition::Other` is removed.** A `matchConditions` element of a location query
  (`contractLocationQueries`), an organisation value or a mission item value whose struct is not one
  of `tagSearch`, `excludeNearbyLocations`, `excludeDistantLocations`, `excludeProperty`,
  `propertyTags`, `specificOrganizations` and `specificItems` fails the Record with
  `TemplateError::UnknownStruct`. The field of the error is `location condition` for a query,
  `organization condition` and `item condition` for the other two, as before.
- **One reader for all three.** `Reader::conditions` names the field in the error, so the query and
  the two values share the one rule. The spawn readers return `SpawnError`, which carries either
  the identifier error or the unknown struct; no reader hides a failure in a variant.
- **The schema and `docs/basetool-catalog.md` drop the variant.** `schemaVersion` stays 2.

## ADR-0054 and the release

`LocationCondition::Other` and the kind `contractLocationQueries` were first released in 0.4.2
(commit 7834af0d, 2026-10-30, `git tag --contains` names only `v0.4.2`). ADR-0054 calls removing a
field or restructuring a kind breaking and lists the guarantees a consumer relies on: no field
changes its type, meaning or nullability, a new kind never changes the selection or ordering of an
existing kind, and `completeness` keeps its meaning. The removal touches none of them:

- No Record of LIVE holds a condition of an unknown struct, so no catalog the tool has written
  contains the `other` variant. The removal changes no value of any catalog written from LIVE and
  no consumer has read an `other` condition.
- The schema narrows by one `oneOf` branch that no data uses. A reader that validates a catalog
  against the new schema accepts every catalog of 0.4.2 from LIVE; a reader that validates against
  the old schema accepts every catalog of the new build, because the variants that remain are
  unchanged.
- An Archive that does hold an unknown condition (a later game version) now reports the Record
  as a failure of the build instead of listing the query with an `other` condition. That is the
  behaviour of every other union since ADR-0320, and the failure names the struct, so the schema
  can be extended with the new variant in an additive change (ADR-0054, a new value of a closed set).

This is the same argument as in ADR-0320 and ADR-0324, with one difference: those two removed
variants before any release, and this one removes a variant that a release contains but no data
ever used. Raising the schema version for a branch of a union that has never occurred would force
every Basetool release to adapt for nothing, which ADR-0054 set out to avoid. Version 2 therefore
stays; if a catalog of an Archive with such a condition ever existed, it would have to be treated as
a breaking change and the version raised.

## Consequences

- A location query with an unknown condition cannot reach a catalog. On LIVE no Record fails
  through this rule, and the real-Archive catalog test still reports zero failed Records.
- A new condition struct of the game fails the Records that hold it until the catalog reads it,
  which is the signal to add the variant.
