# An unknown location condition fails the Record, and the `other` condition is deprecated

Decided for issue #1011. Evidence: LIVE 4.10 (2026-10-31), every `matchConditions` array of a
location, organisation or mission item property. Amends ADR-0266 and ADR-0324 and applies
ADR-0054. The owner's rule from #812, repeated in ADR-0320 and ADR-0324, is that a struct the
catalog does not know fails the Record instead of hiding behind an escape hatch.

## Decision

- **The reader is strict.** A `matchConditions` element of a location query
  (`contractLocationQueries`), an organisation value or a mission item value whose struct is not one
  of `tagSearch`, `excludeNearbyLocations`, `excludeDistantLocations`, `excludeProperty`,
  `propertyTags`, `specificOrganizations` and `specificItems` fails the Record with
  `TemplateError::UnknownStruct`. The field of the error is `location condition` for a query,
  `organization condition` and `item condition` for the other two. One rule, `Reader::conditions`,
  serves all three; the spawn readers return a typed `SpawnError` that carries either the
  identifier error or the unknown struct.
- **The schema keeps the `other` condition, marked `deprecated`.** The variant shipped in 0.4.2,
  so the schema still describes it: `condition: "other"` with `structName`. The catalog never
  writes it. It is a schema-only definition (`condition_schema::with_deprecated_other`); no Rust
  type models it and no reader can construct it.
- **The removal waits for the next `schemaVersion` bump.** `schemaVersion` stays 2, the contract is
  unchanged and ADR-0054 is not amended.

## Evidence

No catalog of LIVE 4.10 holds a condition of an unknown struct, so none contains `other`. A later
game version with such a condition fails the Records that hold it until the catalog reads the
struct; how a new condition is then added is decided at that time.

## Consequences

- A location query with an unknown condition cannot reach a catalog. On LIVE no Record fails
  through this rule.
- A reader that validates against the schema keeps accepting `other`, and sees it flagged as
  deprecated.
