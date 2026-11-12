# Catalog mission location vocabulary

The catalog v3 keeps six physical Record families as definition tables. Each row has the
[canonical Record header](catalog-record-headers.md) with the original GUID and physical
Record index. The exact Record type selects a family at any path. Rows sort by GUID and
physical index.

| Catalog table | Original Record type | Original typed field |
| --- | --- | --- |
| `definitions.locationClasses` | `MissionLocationClass` | `scopesVisibleTo`, an ordered Enum array of `ContractScope` |
| `definitions.locationFunctions` | `MissionLocationFunction` | The Record identity |
| `definitions.locationSizes` | `MissionLocationSize` | The Record identity |
| `definitions.locationRegistries.classes` | `MissionLocationClasses` | `locationClasses`, an ordered Reference array to `MissionLocationClass` |
| `definitions.locationRegistries.functions` | `MissionLocationFunctions` | `locationFunctions`, an ordered Reference array to `MissionLocationFunction` |
| `definitions.locationRegistries.sizes` | `MissionLocationSizes` | `locationSizes`, an ordered Reference array to `MissionLocationSize` |

All three registry tables export the original array as `members`. The catalog retains
each registry Record even when its array is empty. An individual dictionary definition
can have zero own payload fields. Additional authored members stay in the owner-local
`authoredFields` arena.

Scope labels and registry members retain their source order and repetitions. The array
and each position have independent source evidence and a `containsEncryptedSource` marker.
A null position stays null. A dangling or incompatible target retains its original GUID
and link state. Export health identifies targets excluded by a Filter. Table availability
is based on the source declaration, independently of the count of emitted rows.

Mission location filters that reference these definitions join to their original GUID
and physical Record identity when available. The registry membership and visible scopes
describe configuration stored in the Archive.
