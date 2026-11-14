# Loose planet JSON uses original entry coordinates

Planet resources, climate assets and ecosystem definitions are authored as loose
JSON Archive Entries. Use the existing entry token walker shared with shop
inventories. Keep the original tokens, field positions, duplicate members and
physical Entry identity through typed projection. This preserves the evidence
behind missing, explicit null, invalid and present fields.

The source length and platform representation bound the read. Verify checksums
and the decoded end, use fallible allocation for the materialized token tree,
and check cancellation during reads and traversal. Report read or projection
failure against the original Entry. Diagnostic publication can retain other
readable definitions.

A tint palette path in the DataForge Record namespace is a Record reference
resolved by the original path and exact TintPaletteTree type. Archive-relative
paths use exact directory evidence and the applicable Data root prefix. Retain
the authored extensionless path, observed candidates and lookup state.

Resolve the original climate `entityClass`, `harvestableResource` and
`harvestableSetupRecord` declarations against their respective families.
The last identifies a DataForge `HarvestableSetup` header in the verified PTU
sample. The EntXML `SubHarvestableMultiConfigRecord` and Child `ContextPreset`
declarations identify separate source families. Emit four independently
sourced shading-class GUID selectors so the shading-class family can be
joined without making the climate definition depend on that projection.

The bounded PTU review covered original `.climasset`, `.geoasset`, `.creature`,
`.resource`, `.eco`, `.geology` and `.shadingclass` definitions, the Child and
HCD/EntXML placement declarations, and the exact known harvestable setup
headers. These establish authored definition relations. A per-body
climate/geology/fauna assignment remains unestablished in those sources.
Emit a per-body relation when an original assignment edge and its route are
both available. Preserve readable definitions independently of that evidence.
