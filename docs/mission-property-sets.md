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
partial sets. Every authored set in a selected Generator Record is retained independently of
whether its scope produces a contract row. Missing IDs, debug rows and duplicate rows do not
discard source arrays. Existing development-leftover and duplicate rules apply to derived
contract rows and their links; the global Record Filter bounds the source kind.

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
[`scopedLocation`](scoped-locations.md), [`autoSpawns`, investigation and complete typed
options](mission-property-values.md). An unsupported concrete type is a source failure,
never a free-form JSON escape hatch. All six scoped filter families and their weak references
remain in their original owner even when contract rows inherit the declaration. Spawn and
location values retain their typed links to the existing canonical spawn/query kinds.

The set GUID is independent of the payload: it hashes
`["missionPropertySet", recordGuid, sourceLocation]` with the mapping specified in ADR-0345.
Payload edits at the same authored position keep the GUID. Moving an array or changing the
owning Record changes it. Equal content at different source positions does not share identity.
Sets are ordered by GUID text; declarations and contract references keep authored order.

Physical Record-table aliases can share a Record GUID and authored position. Identical complete
set payloads remain idempotent. Incompatible payloads permanently quarantine that canonical set
for this export; comparison preserves IEEE floating-point bits, including signed zero and NaN
payloads. Normal publication refuses the result. An explicit diagnostic export omits the
ambiguous set and every contract link to it, while retaining other compatible sets and contract
rows. `ambiguousIdentity` health failures preserve both exact physical owner indices, and the
`ambiguous_mission_property_set` diagnostic identifies the authored positions. Successful
canonical reads and retained typed source projections stay successful; failed canonical joins
are separate derived operations. Record-table indices are provenance within this Build, not
replacement domain identifiers.

These are authored configuration values. The exporter does not merge overrides, evaluate
filters, choose a runtime result or replace the established derived text/spawn fields.

The synthetic Archive checks run with
`cargo test -p p4kx-export --test it --locked mission_property_sets`. They exercise complete
filter payloads on all three Generator levels, ordered joins, fanout deduplication, identity
stability, empty and null source data, filters, weak-owner boundaries and strict source failures.

The ignored `generator_property_sets_match_raw_sources_and_contract_joins` real-Archive test
independently traverses raw Generator structs. It compares original scope locations, complete
scoped filter payloads and reference identities, then verifies ordered joins for every emitted
contract row. Every eligible physical Generator is independently decoded before production
health is considered. An actual independent read failure requires exact source-read evidence;
production projection failures cannot waive checks of readable authored arrays. Retained field
failures do not count as whole omitted sources. Synthetic mutations verify these boundaries
while another Generator's rows and sets remain present.

Set `P4KX_MIN_SCOPED_LOCATION_VALUES=1` for a PTU check that must exercise scoped-location data.
The default minimum is zero for sources without these values. A run that misses the explicit
coverage requirement fails. Source values and identities stay in memory: failures contain only
typed static stage and category discriminants, including array, value, join, health and coverage
failures. Underlying errors are discarded without formatting or retaining an error chain;
synthetic sentinel mutations check both Display and Debug. Successful output contains only
verification counts, and an Archive change prevents a successful result.
