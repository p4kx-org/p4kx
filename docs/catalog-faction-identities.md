# Faction identity definitions

`definitions.factionIdentities` contains one row per physical `FactionIdentity` Record selected by the ordinary Record filter. GUID, normalized source path and original Record index identify the owner. Separate identities can refer to the same Faction.

| Source declaration | Catalog field |
| --- | --- |
| `name`, `description` Locale | `name`, `description` protected `GameTextRef` values with the original keys and observed translations in the shared game text table |
| `factionTag` Reference<Tag> | `factionTag` original Tag link |
| `factionRef` Reference<Faction> | `factionRef` original Faction link |
| `securityNetworkRef` Reference<SecurityNetworkManifest> | `securityNetworkRef` original security manifest link |
| `encounterPresetWeights` Class<EncounterPresetWeight>[] | `encounterPresetWeights` ordered entries with the authored `EncounterPreset` enum token and exact `Single` weight |
| `lootPoolThemeTags` Reference<Tag>[] | `lootPoolThemeTags` ordered original Tag links |

The source carries the encounter preset as an enum. An EncounterIndex can group spawn sets by the same authored enum token. NPC archetype branches and AutoSpawnData carry separate physical FactionIdentity references. A Faction supplies faction configuration, and a FactionReputation supplies reputation configuration. Consumers can join those definitions through their respective original references.

Every sourced field and list element carries its immediate provenance. Empty arrays, null, invalid and unsupported data retain separate states. A resolved original target retains its identity when excluded from the catalog, and a dangling Reference retains its declared GUID. Extra authored declarations remain in `authoredFields`.
