# Mission location configuration

The `missionLocationTemplates` row identifies the physical `MissionLocationTemplate` Record.
Its sourced `configuration` is read from the declared inline `MissionLocationData`.
Locations with the authored `disabled` flag retain their configuration.

| Original field | Catalog field | Stored values |
| --- | --- | --- |
| `locationData.autoSpawnSettings` | `configuration.value.autoSpawnSettings` | Ordered `AutoSpawnSettings` Class occurrences with their authored tag lists, spawn limits and delays, inventory, faction override and controls. |
| `locationData.missionLimits` | `configuration.value.missionLimits` | Ordered `LocationMissionLimit` Class occurrences with the Tag Reference, `autoConsumeIfPlayerAtLocation` Bool and `maxMissionInstances` Int32. |
| `locationData.defaultSecurityNetworkManifest` | `configuration.value.defaultSecurityNetworkManifest` | The authored Reference to `SecurityNetworkManifest`. The physical target identity or dangling GUID remains available. |

A present empty array retains its state and array value. Every limit occurrence and member,
including the original numeric value, carries its own source marker. These fields describe
stored declarations. Their presence is independent of whether a location matches a query.

The auto-spawn rows use the same typed TagList, inventory and exact Single delay representations
as the [independent NPC definitions](catalog-npc-spawn-definitions.md). The location owns each
occurrence. The `factionOverride` Reference remains an authored selector.
