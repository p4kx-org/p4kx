# Authored mission property sets

Catalog schema 3 exports `missionPropertySets` for the property overrides of selected
`ContractGenerator` Records. The `propertySetGuids` of a contract row reference these canonical
sets in handler, contract, sub-contract order. A parent row has no sub-contract link. All links
join a set in the same catalog; shared handler or contract declarations are emitted once even
when many rows refer to them.

The source arrays are `contractParams.propertyOverrides` of a handler and
`paramOverrides.propertyOverrides` of a contract or sub-contract. A sub-contract with no
`paramOverrides` block uses its directly authored `propertyOverrides`. Empty arrays remain
explicit empty sets. Missing arrays create no link. Present malformed arrays or unreadable
declarations fail their original Generator Record and emit neither its contract rows nor
partial sets. Development leftovers follow the existing contract selection rules.

| Set field | Meaning |
| --- | --- |
| `guid` | Catalog identifier for the owning Record GUID and exact authored array position |
| `recordGuid` | Original `ContractGenerator` Record GUID |
| `recordPath` | Original Record path without `.xml` |
| `sourceType` | Original owning source struct name |
| `sourceLocation` | Canonical authored array position, including field names and zero-based indices |
| `level` | `handler`, `contract` or `subContract`; the source level, without computing precedence |
| `properties` | Nullable declarations in source order, retaining null pointer elements |

Each non-null declaration contains `variable` (exact `missionVariableName`), `token` (exact
`extendedTextToken`), `valueType` (concrete source struct without `MissionPropertyValue_`) and
`value` (the catalog's typed `PropertyValue`). Empty variable or token strings remain empty;
missing or null strings remain null. An absent/null value pointer gives null for both
`valueType` and `value`. This raw-source convention is specific to the new kind; existing
template and broker fields retain their documented normalization.

Values use the same complete union as template and broker properties, including
[`scopedLocation`](scoped-locations.md). An unsupported concrete type is a source failure,
never a free-form JSON escape hatch. All six scoped filter families and their weak references
remain in their original owner even when contract rows inherit the declaration. Spawn and
location values retain their typed links to the existing canonical spawn/query kinds.

The set GUID is independent of the payload: it hashes
`["missionPropertySet", recordGuid, sourceLocation]` with the mapping specified in ADR-0345.
Payload edits at the same authored position keep the GUID. Moving an array or changing the
owning Record changes it. Equal content at different source positions does not share identity.
Sets are ordered by GUID text; declarations and contract references keep authored order.

These are authored configuration values. The exporter does not merge overrides, evaluate
filters, choose a runtime result or replace the established derived text/spawn fields.

The synthetic Archive checks run with
`cargo test -p p4kx-export --test it --locked mission_property_sets`. They exercise complete
filter payloads on all three Generator levels, ordered joins, fanout deduplication, identity
stability, empty and null source data, filters, weak-owner boundaries and strict source failures.

The ignored `generator_property_sets_match_raw_sources_and_contract_joins` real-Archive test
independently traverses raw Generator structs. It compares original scope locations, complete
scoped filter payloads and reference identities, then verifies ordered joins for every emitted
contract row. It keeps the Archive and any observed values outside the repository.
