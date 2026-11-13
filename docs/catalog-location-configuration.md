# Mission location configuration

The `missionLocationTemplates` row identifies the physical `MissionLocationTemplate` Record.
Its sourced `configuration` is read from the declared inline `MissionLocationData`.
Locations with the authored `disabled` flag retain their configuration.

| Original field | Catalog field | Stored values |
| --- | --- | --- |
| `locationData.missionLimits` | `configuration.value.missionLimits` | Ordered `LocationMissionLimit` Class occurrences with the Tag Reference, `autoConsumeIfPlayerAtLocation` Bool and `maxMissionInstances` Int32. |
| `locationData.defaultSecurityNetworkManifest` | `configuration.value.defaultSecurityNetworkManifest` | The authored Reference to `SecurityNetworkManifest`. The physical target identity or dangling GUID remains available. |

A present empty array retains its state and array value. Every limit occurrence and member,
including the original numeric value, carries its own source marker. These fields describe
stored declarations. Their presence is independent of whether a location matches a query.
