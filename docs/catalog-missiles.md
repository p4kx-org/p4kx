# Missile configurations

`configurations.missileConfigurations` has one row per original
`Components[SCItemMissileParams]` occurrence on a selected item. The row uses `ownerGuid`,
`ownerRecordIndex` and `componentOrdinal`. Use the same physical owner and ordinal to join its
`configurations.explosives` missile row. That row carries the inherited `SExplosiveOrdnanceParams`
fields for launch requirements, lifetime, arming, collision, proximity, explosion, emissions,
cluster and audio. The explosion payload uses the common authored explosion representation.

| Catalog field | Source declaration | Contents |
| --- | --- | --- |
| `guidance` | `GCSParams`, `SGuidanceAndControlParams` | Dumb-fire inputs, linear speed, boost duration, terminal engagement time and angle, fuel tank size, PID terms and separate boost, intercept and terminal phases |
| `targeting` | `targetingParams`, `STargetingParams` | Original `ESignatureType` label, tracking and lock thresholds and rates, angle, limits, resolution, resilience, dumb firing and maximum misses |
| `shake` | `shakeParams`, StrongPointer to `SShakeParams` | `Ang3` angle and `Vec3` position offsets, randomness, range, duration and frequency |

Each phase retains `angularSpeed`, `maxRotationAccel`, `pidAggression` and independent
`maxLinearAccelerationPositive` and `maxLinearAccelerationNegative` vectors. The vectors use
three independently sourced `x`, `y` and `z` cells. The sign is preserved exactly as authored.
`linearSpeed` keeps the established metres-per-second meaning. Other controls retain their
stored representation and units. No range, turning radius, achieved speed, countermeasure
effectiveness or hit chance is calculated.

The original signal vocabulary remains open. A new label in `ESignatureType` remains the
authored label. A different declaration is reported as unsupported and retains its original
field in `additionalFields` in diagnostic publication. An absent field is distinct from an
explicit null or an unreadable value. Zero, false and exact floating-point bits are retained.
Every typed value and retained original field has its own source provenance.

The surveyed source declarations contain no `dynamicLaunchZoneRecord` or declared launch-zone
type. An additional source member of the targeting Class is retained in the original
`additionalFields` arena with its declaration and payload, including Record references and
their original error facts. A typed launch-zone definition requires an observed source
declaration and target.
