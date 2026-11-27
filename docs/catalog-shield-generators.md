# Shield generator configurations

Schema v3 publishes one row in `configurations.shieldGenerators.rows` for each selected physical
`Components[SCItemShieldGeneratorParams]` occurrence. Join it to its item through
`ownerGuid.value` and use `ownerRecordIndex.value` with `componentOrdinal.value` to distinguish
original Records and repeated components. The table's `availability` describes whether the source family is declared.

| Field | Original member |
| --- | --- |
| `maxHealth` | `MaxShieldHealth` |
| `maxRegen` | `MaxShieldRegen`, in hit points per second |
| `decayRatio` | `DecayRatio` |
| `downedRegenDelay`, `damagedRegenDelay` | `DownedRegenDelay`, `DamagedRegenDelay`, in seconds |
| `reservePoolInitialHealthRatio`, `reservePoolMaxHealthRatio` | `ReservePoolInitialHealthRatio`, `ReservePoolMaxHealthRatio` |
| `reservePoolRegenRateRatio`, `reservePoolDrainRateRatio` | `ReservePoolRegenRateRatio`, `ReservePoolDrainRateRatio` |
| `electricalChargeDamageResistance` | `ElectricalChargeDamageResistance` |
| `stunParams` | `stunParams` with `minAlphaDamageRatio`, `maxAlphaDamageRatio`, `minStunTime`, `maxStunTime` |
| `shieldResistance`, `shieldAbsorption` | `ShieldResistance`, `ShieldAbsorption`, with `Min` and `Max` in every authored slot |

Numbers carry their original width and value. Floating-point cells have exact `rawBits` and
a `finite` decimal projection. Signed zero and nonfinite source bits survive. The output
retains the stored units and ratios. The resistance and absorption slot identity is its
zero-based position in the original array. The source evidence leaves damage labels unresolved.
Every authored slot is kept, including longer arrays and duplicate values.

Each source-backed value has its own state and encryption receipt. `absent` means the member
was undeclared and `null` means an explicit source null. A readable empty array is `present`
with `value: []`. An unreadable or invalid member keeps its diagnostic state. Each structure
reports its actual `sourceType`. The reader checks the original Class and array declarations.

`additionalFields` on the generator, stun parameters and range slots retains the ordered
original members outside the typed projection. It uses the same `roots` and arena-local
`values` representation as other schema-v3 authored trees. Original flags, references,
additional members and invalid typed members retain their own source observations. Consumers
can use the stored inputs to model shield behavior in their own domain.
