# PlanetTech fauna definitions

`definitions.creatures` in schema v3 contains one row for each selected original
`.creature` Entry below `Data/Libs/Planets/creatures/`. It preserves the original
Entry identity and the decoded source field states. The Catalog Filter selects
Entry paths, and rows follow physical Archive order.

| Source | Export | Meaning |
| --- | --- | --- |
| `guid` | `guid` and `guidRaw` | Canonical UUID and original decoded spelling. |
| `entityClass` | `entityClass`, `entityClassRaw` and `entity` | Authored Entity GUID and exact DataForge Record lookup. |
| `needVerticalOffset` | `needVerticalOffset` | Authored Boolean. |
| `slopeTolerance` | `slopeTolerance` | Exact authored JSON number token as a decimal string. |

The `entity` lookup checks the original GUID and exact `EntityClassDefinition`
type. Candidate headers preserve their physical indexes and original coordinates.
Duplicate candidates and wrong-family, missing, filtered or unreadable targets
retain the authored reference. The all-zero GUID and empty string remain
distinguishable through their original cells. Readable siblings remain available
in diagnostic publication if an Entry fails its read or its JSON projection.

These definitions describe the source fauna configuration. A body assignment
requires a separate declared association. The resolved Entity alone provides
no evidence of placement, encounter probability or runtime population.
