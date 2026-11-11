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

Wear fields include lifetime, initial age and usage, heat, functionality and effect curves, parent
resource scaling and the audio RTPC identifier. Bezier points retain authored positions and
coordinates. Degradation retains the original accumulator array slots, including explicit nulls.
Unsupported accumulator variants carry their source-type evidence in diagnostic output.

Every source-backed value includes its own provenance. Single fields preserve their IEEE-754 `rawBits`.
Finite fields also expose the finite decimal value. NaN and infinity remain present with a null
decimal value. False and zero remain present. An undeclared field is absent. A failed read is
unreadable and retains its attempted evidence in provenance without contributing a value.

The existing specialist stats gate uses the first original Health component for its finite health
value and variant grouping. The separate durability table follows all observed Health components.
Vehicle hull and armour data have their own source families.
