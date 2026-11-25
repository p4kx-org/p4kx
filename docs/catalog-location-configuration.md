# Mission location configuration

The `missionLocationTemplates` row identifies the physical `MissionLocationTemplate` Record.
Its sourced `configuration` is read from the declared inline `MissionLocationData`.
Locations with the authored `disabled` flag retain their configuration.

## Source identity, flags, cluster link and names

Each row flattens the eight [canonical Record header fields](catalog-record-headers.md).
The original header identity, normalized class/path, physical Record index, stored filename
and metadata tag each have their own sourced carrier. GUID ordering and physical ties retain
their existing meaning.

| Original field | Catalog field | Source form |
| --- | --- | --- |
| `locationData.disabled` | `disabled` | `SourcedValue<bool>`; present false differs from absent, null and invalid. |
| `locationData.isSecurityNetworkHost` | `isSecurityNetworkHost` | Independent sourced Bool with the same state distinctions. |
| `locationData.entityClusterMember` | `entityClusterMember` | Sourced Record reference with separately sourced GUID, target index, path and type plus the closed lookup status. Dangling/incompatible GUIDs remain available. |
| `locationData.stringVariants.variants` | `names` | Sourced ordered collection of sourced slots. Present empty, absent, null and unreadable members remain distinct. |

The Reader preserves the original first-readable-Struct choice for locationData and
stringVariants, including scalar singleton normalization. Earlier unreadable candidates remain
actual selectors and leave the affected per-value contribution sets incomplete. Intrinsic
header facts retain their independent source scope. A malformed field remains explicit in
export health; a known false flag or present empty collection is not a missing value.

A present name slot contains the existing Tag operand and protected `displayName` GameTextRef.
Join its `value` to `texts.gameTexts[].id.value`, then use the requested available language's
source/display form. The name key, canonical text identity and occurrence counting do not
change. Null/unreadable slots have no invented name. Repeated slots preserve physical source
positions even when their text reference is shared. The existing classification and
configuration projections remain independent of these additional source cells.

The SDK's `source()`, sourced Bool/cluster getters and `names()` expose this same form, with
the callback's own SourceInventory. There is no second `entityClusterMemberGuid` wire value.

## Location-owned configuration

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

### Location entity declarations

`definitions.locationEntityDeclarations` publishes each selected physical
`LocationEntityDeclaration` with the canonical Record header. The table reports whether the
DataForge declares the source family independently of the filter. Duplicate GUIDs retain separate
rows in physical Record order within each GUID.

| Catalog field | Original source |
| --- | --- |
| `entityTags.value.tags` | Ordered `entityTags.tags` References, including repeats, empty arrays, null cells and unresolved GUIDs. |
| `resourceSlot` | The authored `LocationResourceSlot` Reference with its compatible target header, if available. |
| `type` | The sourced StrongPointer to the concrete location entity type. A `staticEntity` retains any additional authored fields. A `deliveryTarget` exposes the original `DeliveryObjectiveType` option and protected `targetDescription` GameTextRef. Another concrete type uses `original` and retains its authored members. |
| `authoredFields` | Original members outside the typed fields. |

Static mission-module entity declarations, AutoSpawns entity-declaration searches, Starmap amenity
requirements and hauling targets use the same nominal Record type. Join a resolved link by GUID
and original physical Record index within the same DataForge. A source reference can resolve to
a Record whose row is outside the current Catalog or SDK selection. A retained link carries the caller
and applicable target-header evidence. Target payload fields carry their own evidence on the
definition row.

A physical ContractGenerator definition and each Template or Broker context publish
`haulingTargetBindings` for their authored hauling occurrences. Each binding points to the physical
`order` and its independent `pickUpTargetTypes` and `dropOffTargetTypes` array cells through the
owner-local value inventory. The two target arrays preserve stored direction, order, repeats, and
missing and unreadable states. This links override occurrences and orders that only specify targets
while retaining one canonical copy of the original array and its per-slot origins. Use each target
Reference's physical header to join to the selected location declaration table.

## Resource slot definitions

`definitions.locationResourceSlots` contains selected physical `LocationResourceSlot` Records.
Follow the sourced GUID and Record index of a mission module's `moduleType.value.requiredResourceSlot`
to the matching row from the same build. The original link retains its own state and provenance.

| Original field | Catalog field | Stored value |
| --- | --- | --- |
| `numberOfResourceSlotsPerEntity` | `numberOfResourceSlotsPerEntity` | Signed Int32 count with its individual origin, including zero. |
| `resourceSlotTag` | `resourceSlotTag` | Original Tag Reference where declared. |
| `autoConsumeIfPlayerAtLocation` | `autoConsumeIfPlayerAtLocation` | Original Bool for player-at-location consumption, including explicit false. |

Each member distinguishes absent, null, unreadable and incompatible source states independently.
An older layout that lacks the consumption flag reports it as absent. Additional fields
and incompatible stored values remain in the row's `authoredFields` arena.
`completeness.locationResourceSlots` describes selection and the table's `availability` describes
the source declaration. Locating instances in the world requires placement data.

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

For installed verification, set the standard `P4KX_DATA_P4K` and Archive key inputs and run

    cargo test --locked --release -p p4kx-export --test real_archive -- real_archive_location_configuration::mission_location_configurations_match_original_selected_owners --exact --ignored

The test reads original location owners, selects populated spawn and limit arrays and checks a
disabled owner when the source has one. Set `P4KX_LOCATION_CONFIGURATION_FILTER` to scope Record
paths or `P4KX_EXPECT_DISABLED_LOCATION_CONFIGURATION=1` to require a disabled owner with
populated configuration. It runs a filtered diagnostic catalog for each selected owner and
compares original order, exact Single bits, physical Record links and individual source origins.
It also checks populated module inputs and properties when present. Set
`P4KX_EXPECT_POPULATED_LOCATION_MODULES=1` to require a populated embedded module owner.
