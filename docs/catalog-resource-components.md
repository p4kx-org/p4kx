# Resource components in catalog schema v3

`configurations.resourceComponents` exports one row for each selected
`ItemResourceComponentParams` occurrence. The owning DataForge Record index
and its original `Components` slot identify the row. The source owner GUID
remains available for logical joins. The same Record can own several
components. Ship loadout ports expose their selected physical target index.

The row contains the authored network, relay, room and wireless flags,
`defaultPriority`, `repairRestoreRatio`, and the self-repair parameters
`maxRepairCount`, `timeToRepair` and `healthRatio`. The named control blocks
`filterParams`, `controlParameters`, `controlBlocks`, `functionalityModifiers`
and `powerPlantOverride` retain their ordered original field/value arenas.
These retain, among other data, the relay's declared allowed-resources bits,
control conditions and outputs, and the power override's additive modifier.

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
