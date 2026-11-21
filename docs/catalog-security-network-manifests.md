# Catalog security network manifests

`definitions.securityNetworkManifests` contains the physical DataForge Records declared as
`SecurityNetworkManifest`. Each row has the [canonical Record header](catalog-record-headers.md).
The table reports source availability, while completeness and export health account for filtering
and read failures. Physical rows are ordered by GUID and Record index.

| Original field | Authored value |
| --- | --- |
| `defaultProtocol` | Owned security protocol with `permissionsWithClearance`, `permissionsWithoutClearance`, `acceptedClearance` and the physical `ownerFaction` Reference. |
| `roomProtocols` | Ordered protocol overrides. Each carries its original `roomIdentifiers.tags` Reference array and its own protocol. |
| `variables` | Ordered variables with a Tag Reference and a nullable owned value. The value discriminates `bool` and signed Int32 `int`. |
| `hostilityRules` | Ordered rules with their Tag, six Boolean hostility and trespass settings, and separate `factionOverride` and `factionToOverride` References. |
| `variableEffects` | Ordered effect groups. Each group contains the authored trigger variable and a nullable-slot effects array. |
| `isNeutralTerritory`, `teleportPlayerOut` | Original Boolean flags. |

The `access` and `trespass` cells keep their authored `ESecurityNetworkPermissionValue` labels.
`acceptedClearance.tokens` keeps the ordered Reference slots to `SecurityClearanceToken`.
`factionOverride` targets `Faction_LEGACY`. `ownerFaction` and `factionToOverride` target
`Faction`. Those Reference identities remain distinct.

An effect has kind `setInteractionState`, `setHostilityRules`, or `authored`. The first retains
the original weak pointer to `SInteractionState` as a Record or owner-local instance target.
The second retains the original owned hostility rule. Unknown value and effect variants use
kind `authored`, the original source type, and their complete `additionalFields` arena. Extra
or invalidly typed fields on known variants also remain in the owner-local arena.

Every sourced value, array slot, and Reference identity has its own `containsEncryptedSource`,
`sourceUsesRefs` and `sourceSetComplete`. Consumers can resolve current faction and manifest
links through the catalog's physical identities and inspect filter exclusions in export health.
The data describes authored policies and preserves every original occurrence.

## Joining an objective override

A contract template retains the authored override in its canonical source arena. Start from
`configurations.contractTemplates.rows[].objectives[].source` and follow the owner-local
`valueIndex` into the matching physical template row's `source.values`. Follow the
`objectiveHandler` and then `securityManifestToOverride` field references. A present
`recordReference` payload carries the manifest GUID and physical Record index. Join those
sourced values to `definitions.securityNetworkManifests.rows[].guid` and `recordIndex`
within the same catalog build.

An explicit null remains a null source cell. A dangling reference keeps its authored GUID in
`originalError`. A filter exclusion keeps the physical link on the objective and is recorded in
`exportHealth.unresolvedReferences`.
