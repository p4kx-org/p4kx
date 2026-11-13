# Vehicle armour in catalog schema v3

`configurations.vehicleArmor` contains one row for each selected physical
`SCItemVehicleArmorParams` component. The `ownerRecordIndex` and
`componentOrdinal` identify the original component occurrence. The
`ownerGuid` also carries the logical Entity identity. A resolved loadout
attachment joins by `sourceConstraints.attachedItem.targetRecordIndex`.
The port path identifies each placement and carries its original visibility.

| Original source | Exported facts |
|---|---|
| `signalInfrared`, `signalElectromagnetic`, `signalCrossSection` | The three stored Single signature multipliers |
| `damageMultiplier` | Original StrongPointer to the `DamageBase` family and its concrete subtype |
| `armorPenetrationResistance` | `basePenetrationReduction` and the original `DamageInfo` absorption values |
| `armorDeflection` | The original `DamageInfo` deflection values and the inline `BezierCurve` |

DamageInfo publishes its original physical, energy, distortion, thermal,
biochemical and stun words. `DamageParams` retains its linked damage Macro
and original `damageTotal`. The unknown alternative preserves the original
concrete type and authored value arena and records an informational projection
diagnostic at that field. The curve preserves every authored
`Vec2` point in order and the original `useLUT` value under `useLut`.

Each field retains its direct source state, exact contributors and encryption
marker. The numeric cells carry stored IEEE bits, including nonfinite values.
Unresolved authored links retain their original GUID and disposition. A
malformed curve declaration exposes its original failure in diagnostic
publication. Suit armour and clothing use their separate component contracts.
