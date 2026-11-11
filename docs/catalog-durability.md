# Item durability, degradation and distortion

Catalog schema 3 exports the Health, Degradation and Distortion components of selected Items in
`configurations.durability`, `configurations.degradation` and `configurations.distortion`. Each table has
an availability state and ordered rows. A row uses `ownerGuid`, `ownerRecordIndex` and `componentOrdinal`.
These values identify the physical Record in the source Build and the original slot in `Components`.
Items with Health alone are included. Repeated components retain their original positions.

| Table | Source component | Exported fields |
| --- | --- | --- |
| durability | `SHealthComponentParams` | Health, damage cap, repair and salvage flags, salvage modifier, propagation, detachment, destruction and explosion delays, and damage-map, geometry and bone identifiers |
| degradation | `SDegradationParams` | `stopDegradingIfDestroyed` and the ordered accumulators, including inherited controls, wear, dirt and wetness variants |
| distortion | `SDistortionParams` | Decay delay and rate, maximum, warning and recovery ratios, and power change at maximum distortion |

The Health source spells its repair and salvage fields `IsRepairable`, `IsSalvagable`,
`SalvageDamageModifier` and `MirrorSalvageHit`. The catalog exposes them as `repairable`, `salvageable`,
`salvageDamageModifier` and `mirrorSalvageHit`. `DamageResistances` distinguishes the fieldless base
from the populated resistance set. The set carries `IgnoreMeleeDamage` and the physical, energy,
distortion, thermal, biochemical and stun entries with their authored `Multiplier`, `Threshold`,
`DamageCap` and `DamageCapLimit`.

## Health source fields

| Source group | Fields | Catalog representation |
| --- | --- | --- |
| Scalars and identifiers | `Health`, `DamageCap`, `SerializedDamageMapPath`, `DestroyedGeometryTag`, `ExplodedGeometryTag`, `DebrisGeometryTag`, `ExplosionBone`, `ExplosionDelayTime`, `ExplosionRandomDelayRange` | Exact stored numbers and original strings |
| Repair and salvage | `IsRepairable`, `IsSalvagable`, `SalvageDamageModifier`, `MirrorSalvageHit` | Independent flags and modifier |
| Destruction and propagation | `ClientOnly`, `UnlockInteractionsOnDeath`, `PushDamageUpPartsHeirarchyWhenDead`, `DetachFromItemPortOnDeath`, `DetachFromEntityOnDeath`, `DestroySelfOnDeath`, `DestroyChildrenOnDeath`, `PropagateExplosionDamageToChildren`, `UseDirtShaderForDamage`, `UpdateObservableStatus`, `DisableMaterialDamage`, `alwaysPropagateHitToParent`, `duplicateHitToParent`, `sendOnDestructionEventToMasterRootContainer`, `applyExplosionFalloffFromOrigin` | Independent stored flags, including authored false |
| Resistance | `DamageResistances` | Fieldless base, inline six-channel resistance, or a link to a resistance macro |
| Initialization | `InitialDamage` | Signed seed and hit counts, damage/radius/variation scales, bounding-box coordinates and damage-map channels, with base, fixed and fraction damage alternatives |
| Hit handling | `hitManipulationParams` | Separate `ai` and `player` values with their own `damageMultiplier` |
| Health thresholds | `HealthLevelStates` | Ordered geometry tags with separate `effectTrigger` and `effectTag` Record references |
| Destruction explosion | `DeathExplosionParams` | Fieldless base, inline explosion parameters, or a Record reference |
| Gas | `DeathGasComposition` | Ordered `Gas` Record references and exact `Mass` values |
| Interaction locks | `InteractionLocks` | Ordered weak `Interaction` pointers and authored minimum and maximum health ratios |
| Sequencer | `sequencerTasks` | Ordered base or apply-damage-map tasks with original `damageMapPath` |
| Maelstrom | `maelstromEvents` | Ordered base, destroy or interaction-state-machine events with original event and group tags and the state variant's weak pointer |

The base classes `SBaseHealthComponentParams` and `DataForgeComponentParams` declare no additional
fields in the surveyed source. Initial damage keeps `RandomSeed`, `MinHitCount` and `MaxHitCount`
as signed Int32 and retains each Single's original bits. A fixed damage value preserves the six
authored damage channels or the authored macro and total. A fraction preserves the damage macro
link and the original `HealthFraction`.

A Record reference preserves its stored GUID and, when resolvable, the physical target Record
index, path and type. A weak pointer preserves the exact Record owner and the original instance
location for an instance target. Unresolved Record references keep their stored GUID and a typed
status. A readable target with an incompatible declared type carries the actual target header
and the status `unexpectedTargetType`. The link's source uses include the caller declaration and
the target types consulted. Empty arrays, explicit null slots, repeated targets and original
array order survive.
Unsupported variants retain their type evidence in diagnostic output. The durability rows carry
the links to separately owned gas, tag, interaction and other definitions.

## Degradation and Distortion source fields

`SDegradationParams` retains `StopDegradingIfDestroyed` and `accumulators`. Each accumulator
carries `InitialAccumulationRatio`, `AccumulateWhenUnstreamed`, `AccumulateOnlyAfterTractorBeam`,
`AccumulateOnlyWhenAttached`, `StopAccumulationWhenAttached`, `AccumulationEventThreshold`,
`PortTags` and `RequiredPortTags`. The dirt variant also carries `InitialDirtRatio`. The base
and wetness variants have the shared fields.

The wear variant adds `MaxLifetimeHours`, `UseAsTimer`, `InitialAgeRatio`, `InitialUsageRatio`,
`AtmosphereMultiplier`, `HeatMultipliers`, `FunctionalityMultiplier`, `DamageConversionRate`,
`EffectCurve`, `degradeFromParentParams` and `DegradationPercentageRTPC`. Each curve preserves
the LUT flag and original point order and coordinates. Parent degradation keeps the stored
resource and delta kinds, one-at-a-time flag and conversion rate. RTPC retains its authored
identifier.

`SDistortionParams` retains `DecayDelay`, `DecayRate`, `Maximum`, `WarningRatio`, `RecoveryRatio`,
`PowerRatioAtMaxDistortion` and `PowerChangeOnlyAtMaxDistortion`.

## Source states and legacy selection

Every source-backed value includes its own provenance. Single fields preserve their IEEE-754 `rawBits`.
Finite fields also expose the finite decimal value. NaN and infinity remain present with a null
decimal value. False and zero remain present. An undeclared field is absent. A failed read is
unreadable and retains its attempted evidence in provenance without contributing a value.

The existing specialist stats gate uses the first original Health component for its finite health
value and variant grouping. The separate durability table follows all observed Health components.
Vehicle hull and armour data have their own source families.

## Verification

The opt-in test `item_durability_matches_original_health_only_shield_and_armour_fields` independently
reads the original selected components from the owner's Archive. It checks the values against
their exact Record and original field positions and prints only the number of fields checked
per representative category. Set `P4KX_DATA_P4K` and run this test.

```powershell
cargo test -p p4kx-export --test real_archive item_durability_matches_original_health_only_shield_and_armour_fields -- --ignored --nocapture
```
