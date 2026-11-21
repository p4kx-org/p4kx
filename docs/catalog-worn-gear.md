# Worn gear and item radar in P4KX catalog v3

Each row in `configurations.suitArmor`, `configurations.clothing` and
`configurations.itemRadars` describes a physical component of one
`EntityClassDefinition` record. The key within a build is
`(ownerRecordIndex.value, componentOrdinal.value)`. `ownerGuid` records the
authored GUID. GUIDs can occur on several physical records.

| Table | DataForge component | Canonical fields |
| --- | --- | --- |
| `configurations.suitArmor` | `SCItemSuitArmorParams` | `integrityMilestoneToBreak`, `damageResistance` |
| `configurations.clothing` | `SCItemClothingParams` | `temperatureResistance.value.{minResistance,maxResistance}`, `radiationResistance.value.{maximumRadiationCapacity,radiationDissipationRate}`, `flight.value.gForceResistance` |
| `configurations.itemRadars` | `SCItemRadarComponentParams` | `aimAssist.value.{distanceMinAssignment,distanceMaxAssignment,outsideRangeBufferDistance}`, `pingProperties.value.cooldownTime` |

The item radar table describes devices. `configurations.radarOptions` describes
the separate game-mode radar configuration. The original numbers retain IEEE-754
Single `rawBits` and the nullable decimal `finite` projection. The device
radar declarations carry no unit metadata. The wire stores the author's numbers
without conversion.

`definitions.damageResistanceMacros` holds one row per physical
`DamageResistanceMacro` record. A suit's present `damageResistance.value`
joins by `targetRecordIndex.value` to the macro's `source.recordIndex.value`.
A filtered catalog includes the compatible resolved macros needed by its
selected suits as well as independently selected macro records. DataForge
resolves a stored Reference GUID to the first matching physical record in
definition order. The join uses that record index, even when several records
have the same GUID. Null, dangling and unexpected-family references carry
their own original states.

The macro's `damageResistance.value` contains `ignoreMeleeDamage` and its
`physical`, `energy`, `distortion`, `thermal`, `biochemical` and `stun`
channels. Each channel has `multiplier`, `threshold`, `damageCap` and
`damageCapLimit`. The multiplier is stored as authored. The tables describe
static source values. They do not calculate damage taken, effective protection,
radar range or current game state.

`additionalFields` retains authored members outside the typed fields. Each
typed value and source-arena atom has its own provenance and
`containsEncryptedSource` marker. For a resolved macro, consult the Reference
and the macro cells when carrying provenance along a consumer-side join.

## Shared device radar settings

`definitions.radarSystemSharedParams` holds one row per physical
`RadarSystemSharedParams` Record. The `source` header identifies its original
Record and `authoredFields` keeps the complete owned parameter tree. The
`sharedParams` Reference in a device's `additionalFields` joins to
`source.recordIndex.value` using the same DataForge input. Filtered catalogs
include the compatible target selected by each included radar device.

Use `ItemRadar::shared_params` for the resolved original link. Use
`equipment::component_definitions::with_radar_system_shared_params` with its
physical Record index to read that definition and its provenance. Query by
GUID to retrieve all physical definitions with that GUID. The original
unresolved and unsupported fields, including experimental parameters, stay
available in `additionalFields`.

Shared device settings describe the authored RadarSystem configuration.
Game-mode options are in `configurations.radarOptions`. The separate XML
slot declarations and their binding status are in `configurations.radarAudio`.
