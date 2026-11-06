# Mission property sets preserve Generator source declarations

The catalog stores authored Generator property arrays once in `missionPropertySets` and links
them from derived contracts through ordered `propertySetGuids`. A Generator can produce many
contract rows; copying its declarations into every row multiplies the payload, while exporting
only effective text or spawn projections loses authored values such as scoped-location filters.

A set belongs to its original `ContractGenerator` Record GUID and the canonical location of its
`propertyOverrides` array. Its GUID is the first 16 bytes of SHA-256 over the canonical JSON tuple
`["missionPropertySet", recordGuid, sourceLocation]`, using the catalog's stored-byte UUID mapping.
The identity therefore survives a payload change at the same authored position, while equal
payloads at different positions remain distinct. Moving the declaration changes its identity.
The kind is sorted by GUID text, and a shared source set is parsed and emitted once per build.

Links preserve handler, contract and sub-contract order without choosing runtime precedence.
Sub-contracts with no `paramOverrides` block retain their directly authored declarations. Sets
retain the owner GUID, path and struct, authoring level, exact source location, and ordered
nullable declarations. Variable and token strings are source identifiers: empty stays empty,
and absent or null stays null. Values reuse the complete typed `PropertyValue` union; templates
and broker entries keep their existing fields and normalization semantics.

Unknown or unreadable present values fail the original Generator instead of yielding null or
partial sets. Weak links are resolved only in that original owner, before fanout into contract
rows. Every authored set of a selected Generator Record is retained, including scopes without row
IDs or derived contract rows and scopes whose rows are development leftovers or duplicates.
Global Record selection bounds the source kind; row selection controls only derived contracts
and their links. No runtime choice or final location is inferred.
