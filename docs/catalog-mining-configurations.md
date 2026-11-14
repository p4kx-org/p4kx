# Mining lasers and attachable modifiers in P4KX catalog v3

`configurations.miningLasers` and `configurations.attachableModifiers` describe
physical entity components. Within a build join by `ownerRecordIndex.value`
and `componentOrdinal.value`. `ownerGuid` retains the original owner GUID.

## Mining lasers

A mining laser row carries the original throttle fields, `globalParams` record
reference, `miningLaserModifiers`, and `filterParams` from
`SEntityComponentMiningLaserParams`. The modifier fields retain their concrete
`FloatModifier` type and the authored `value` when present. For an original
`FloatModifierMultiplicative` the value is the stored percent. An unset pointer
keeps its original state. The firing and extraction actions are independently
available from the same entity's `configurations.weaponActions`.

`definitions.miningLaserGlobals` holds `MiningLaserGlobalParams` records.
Join the laser's present `globalParams.value.targetRecordIndex.value` to
`source.recordIndex.value` inside the same build. A filtered catalog includes the
target of a selected compatible reference. These definitions carry throttle
change and acceleration settings, plus three original audio RTPC identifiers.
The separate `miningGlobalParams` dataset describes the physical mining process.

## Attachable modifiers

Rows for `EntityComponentAttachableModifierParams` retain the authored
`activationMethod`, `charges`, `canInterrupt` and `isInterruptible`.
`modifiers.value` is the ordered physical slot array. Each slot retains its
own state and provenance. Present slots identify their concrete branch.

| Branch | Original type | Typed fields |
| --- | --- | --- |
| `miningLaser` | `ItemMiningModifierParams` | Lifetime and laser modifier |
| `mineableRock` | `ItemMineableRockModifierParams` | Lifetime and rock modifier |
| `miningFilter` | `MiningFilterItemModifierParams` | Lifetime and filter modifier |
| `weapon` | `ItemWeaponModifiersParams` | Lifetime, action index, damage, salvage and tractor modifiers |
| `authored` | Other valid types | Concrete name and the entire original source arena |

For a `weapon` effect, read `weaponModifier.value.weaponStats.value` for the
damage multiplier and the salvage and tractor groups. The original
`fireActionIndex` belongs to the attachment's applicable action layout.
The catalog retains the authored index and leaves runtime application to
consumers with matching loadout information.

`additionalFields` preserves other original fields, including complete
nested structures and traversing modifier declarations. Typed scalar values
preserve their original Single bits, and every value has its own provenance and
`containsEncryptedSource` marker. Original record lookups and references
remain scoped to the selected build.
