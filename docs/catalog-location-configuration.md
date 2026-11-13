# Mission location configuration

The `missionLocationTemplates` row identifies the physical `MissionLocationTemplate` Record.
Its sourced `configuration` is read from the declared inline `MissionLocationData`.
Locations with the authored `disabled` flag retain their configuration.

| Original field | Catalog field | Stored values |
| --- | --- | --- |
| `locationData.autoSpawnSettings` | `configuration.value.autoSpawnSettings` | Ordered `AutoSpawnSettings` Class occurrences with their authored tag lists, spawn limits and delays, inventory, faction override and controls. |
| `locationData.missionModules` | `configuration.value.missionModules` | Ordered `SMissionLocationModule` classes. Each retains its persistence flag, module selector, ordered initialization inputs and original property occurrences. |
| `locationData.missionLimits` | `configuration.value.missionLimits` | Ordered `LocationMissionLimit` Class occurrences with the Tag Reference, `autoConsumeIfPlayerAtLocation` Bool and `maxMissionInstances` Int32. |
| `locationData.defaultSecurityNetworkManifest` | `configuration.value.defaultSecurityNetworkManifest` | The authored Reference to `SecurityNetworkManifest`. The physical target identity or dangling GUID remains available. Join to [security network manifests](catalog-security-network-manifests.md). |

A present empty array retains its state and array value. Every limit occurrence and member,
including the original numeric value, carries its own source marker. These fields describe
stored declarations. Their presence is independent of whether a location matches a query.

The auto-spawn rows use the same typed TagList, inventory and exact Single delay representations
as the [independent NPC definitions](catalog-npc-spawn-definitions.md). The location owns each
occurrence. The `factionOverride` Reference remains an authored selector.

Module input pointers carry their original concrete type and name. Boolean, Int32, Single,
Activity String and TagSearchTerm payloads retain distinct tagged variants and their
authored values. An additional concrete input keeps its authored fields and is reported in
export health. The original positive and negative Tag arrays preserve their order.

Module properties preserve the original variable name and extended text token. The concrete
property value points to an owner-local inventory of its exact source fields and nested
values. The inventory retains original references, nulls and errors. The location owns the
module and each property occurrence.

## Physical module declarations

`definitions.missionModuleDeclarations` contains the selected `ModuleDeclaration` Records.
Each row has the original GUID, Record index and path. `completeness.missionModuleDeclarations`
identifies filtered output, while `availability` describes the declared source type.

| Original field | Catalog field | Stored values |
| --- | --- | --- |
| `module` | `module` | Authored module name. |
| `moduleType` | `moduleType` | Original concrete type and payload, including required module and resource slot References where declared. |
| `locationEntities` | `locationEntities` | Ordered owned slots, declared initialization and optionality flags, acquisition strategies and constraints. |
| `properties` | `properties` | Original property declarations and concrete value arenas with per-value provenance, shared with embedded location modules. |
| `spawnedEntities` | `spawnedEntities` | Ordered owned slots with debug name and immediate-spawn flag. The NPC variant includes its original property-name selector. |
| `declaredVariables` | `declaredVariables` | Ordered owned variables with original weak pointers to the declared location or spawn slots. |

The `crewInheritsLocationFaction` Bool is owned by a `SpawnDescription_Ship`. It remains
with that concrete ship payload in the module property's `authoredFields` arena. Property
selectors, module names and weak pointer slots preserve the authored binding semantics.

### Joining an objective module

Start at the objective's `source` in `configurations.contractTemplates.rows[].objectives[]`.
Follow its `valueIndex` through the corresponding physical template row's `source.values`
to `objectiveHandler` and then `moduleDeclaration`. This inherited Reference is used by
handlers such as `ObjectiveHandler_Local` and `ObjectiveHandler_PlayerAttached`.
Join the `recordReference` payload's sourced GUID and Record index to the physical row in
`definitions.missionModuleDeclarations` in the same build. An excluded target stays identifiable
in the original cell and has an occurrence in `exportHealth.unresolvedReferences`. Missing
targets keep their authored GUID in `originalError`. Null links remain explicit nulls.

Owned pointers preserve null slots and their original array order. Unrecognized concrete
variants retain source fields and report their status. Repeated and cyclic Record References
retain their identities as links.
