# Resource components in catalog schema v3

`configurations.resourceComponents` exports one row for each selected
`ItemResourceComponentParams` occurrence. The owning DataForge Record index
and its original `Components` slot identify the row. The source owner GUID
remains available for logical joins. The same Record can own several
components. Ship loadout ports expose their selected physical target index.

The row contains the authored network, relay, room and wireless flags,
`defaultPriority`, `repairRestoreRatio`, and the self-repair parameters
`maxRepairCount`, `timeToRepair` and `healthRatio`. The named control blocks
`filterParams`, `controlBlocks`, `functionalityModifiers` and
`powerPlantOverride` retain their ordered original field/value arenas.
These retain the relay's declared allowed-resources bits, control conditions
and outputs, and the power override's additive modifier.

The ordered `controlParameters` cells distinguish the original network
parameter and external-port classes. An external-port cell keeps its
`parameter`, `name` and `portName` as authored. The port name is scoped to
that declaration. Its actual installation, if known, is described by the
corresponding loadout placement. New admitted parameter subtypes keep their
actual type, array slot and original member arena.

## States and deltas

Each sourced `states` slot retains its original position, name, linked
interaction state pointers, electromagnetic and infrared signatures, and
power ranges. Signatures contain `nominalSignature`, `decayRate` and the
original functionality curve when stored. Each `low`, `medium` and `high`
range contains `start`, `modifier` and `registerRange`.

| Delta kind | Fields under the original delta |
|---|---|
| `consumption` | `consumption`, `consumptionComposition`, `minimumConsumptionFraction` |
| `generation` | `generation`, `composition`, `noOverGeneration` |
| `conversion` | `consumption`, `generation`, `consumptionComposition`, `generatedComposition`, `minimumConsumptionFraction`, `noOverGeneration` |
| `storage` | `consumption`, `generation`, `consumptionComposition`, `discharge` |
| `networkReflection` | `resource`, `binaryEvaluation` |
| `unknownDelta` | Original source type, slot and complete authored member arena |

Typed cells and the additional member arena cover the original delta together.
The arena retains dynamic amount, composition, resource and conversion
overrides and functionality/transfer modifiers with their actual subtypes
and source positions. The source position preserves ordered duplicates and
null slots. A conversion retains both sides in the same original occurrence.
Composition ratios and original ResourceType references remain ordered.
A dangling target retains its GUID and its resolution status.

Each side retains its resource label, concrete amount-per-second unit and
original named member. The current declarations use `centiResourceUnits`,
`microResourceUnits` and `units` for signed Int32 amounts and
`standardResourceUnits` for Single amounts. The typed numeric cell preserves
the actual stored width. Unknown unit types retain all original members in
their arena. Every sourced field, slot and raw arena member keeps its direct
encryption origin and source references. Singles retain their IEEE bits,
including signed zero and nonfinite values.

## Power and heat connections

`configurations.resourceConnections` contains one row for each selected
`EntityComponentPowerConnection` or `EntityComponentHeatConnection`. Join
by `ownerRecordIndex` and use `componentOrdinal` to distinguish repeated
physical components. The row's `kind` selects `power` or `heat`.
Both forms preserve the source names and keep unprojected fields in the
original member arena.

| Form | Original fields |
|---|---|
| Power supply | `PowerBase`, `PowerDraw`, `TimeToReachDrawRequest`, `SafeguardPriority`, `DisplayedInPoweredItemList`, `IsThrottleable` |
| Power performance | `IsOverclockable`, `OverclockThresholdMin`, `OverclockThresholdMax`, `OverpowerPerformance`, `OverclockPerformance` |
| Power signatures and warnings | `PowerToEM`, `DecayRateOfEM`, `WarningDelayTime`, `WarningDisplayTime` |
| Power misfire blocks | `MisfireItemTypeLocID`, `MisfireGenerationParams`, `MisfireEvents` |
| Heat transfer | `ThermalEnergyBase`, `ThermalEnergyDraw`, `ThermalConductivity`, `SpecificHeatCapacity`, `Mass`, `SurfaceArea` |
| Heat performance | `TemperatureToIR`, `StartIRTemperature`, `OverpowerHeat`, `OverclockThresholdMinHeat`, `OverclockThresholdMaxHeat` |
| Heat operating bounds | `StartCoolingTemperature`, `MaxCoolingRate`, `MaxTemperature`, `OverheatTemperature`, `RecoveryTemperature`, `MinTemperature`, `MisfireMinTemperature`, `MisfireMaxTemperature` |

The three power misfire blocks retain their exact original Locale key or
owned value arena. The Locale key uses the protected game-text domain.
Power and heat rows supply independent component parameters. External
resource parameter ports use their authored declarations, and ship-level
topology uses the original hardpoint declarations.

## Previous convenience values

To reproduce the compact flat view, select the first original resource
component of the item. Visit present states and deltas in original slot
order, placing each delta's generation side before its consumption side.
An existing side produces a row even if the amount or resource label is
absent. Earlier text projection treated an empty label as absent.

For the old power-plant scalar select the first generation side with
resource label Power, and for the cooler scalar select Coolant, both
case-insensitively. The original convenience amount selected the first
numeric unit member in physical field order that yielded a finite f32.
Use original source-use coordinates to order the named amount alongside
raw additional members when a unit has several. The old conversion
accepted stored integers representable by i64 and finite f32 results from
stored Double values. Current typed cells preserve the original amount
instead of narrowing it. Variant identity continues to use the exact
private compact bytes.

## Global resource-network rules

`configurations.resourceNetworkParameters` owns a separate row for each
selected physical `GameMode` Record. Its `source` header and `gameModeId`
identify the original owner. The original StrongPointer `params` selects
the concrete mode parameters. The nested Class `resourceNetworkParams`
contains the authored `ResourceNetworkParams`. Modes with a null pointer
or a concrete type without a rules declaration retain those source states.
The regular catalog Record filter applies to the owning GameMode.

| Scope | Original settings |
|---|---|
| Resource network and repairs | `enableItemResourceNetwork`, `enableSalvageItemRepair`, `doorsIgnoreRN` |
| Misfire and armour flags | `enableMisfires`, `enableCriticalMisfires`, `enableExperimentalArmorPenetration` |
| Destruction flags | `shipPassDestroyedPartDamageToComponent`, `shipPassDestroyedPartDamageToChild` |
| Destruction Singles | `shipDestructionBufferMultiplier`, `shipDestructionHealthThreshold` |

Each value keeps its original data and direct origin. Additional members
of the rule Class retain their original storage and source coordinates.
Health inspection covers the original rule subtree and the required
selectors. Component and loadout rows retain their independent source
scope.
