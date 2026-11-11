# Explosive components

`configurations.explosives` contains one row for each supported original component on a selected
Item. `ownerGuid`, `ownerRecordIndex` and `componentOrdinal` identify the physical Record and
the authored position in its `Components` array. Rows follow that physical order. The sourced
`sourceType` keeps the original concrete name and `configuration` identifies the typed shape.
A table can be available with zero selected rows when its source family is declared.

The same Item component scan also emits `configurations.durability`. To find an Item's
`DeathExplosionParams`, join the Health rows by `ownerRecordIndex` and keep each Health slot's
`componentOrdinal`. An Item can have more than one Health component. The device table leaves
that authored ownership intact.

## Sensor mine declarations

| Original owner | Fields and storage |
| --- | --- |
| `SSensorMineComponentParams` | `FriendlyFire` Bool, `LaserVFX` Class `GlobalResourceParticle`, `TriggerType` StrongPointer `SSensorMineTriggerType` |
| `SSensorMineTriggerType` | Fieldless base |
| `SSensorMineLaserTrigger` | `LaserLength` and `LaserHitOffset` Single, `MineFront` and `LaserOffset` Class `Vec3` |
| `SSensorMineProximityTrigger` | `MaxRaysPerMine` Int32, `Radius`, `WarningRadius`, `AngleVertical`, `AngleHorizontal` and `GuideLaserLength` Single, `Front`, `Up` and `Center` Class `Vec3` |

`laserVfx.path` retains the authored `GlobalResourceParticle.path` string. Vector coordinates
retain their three authored Single values. Null, unsupported and unreadable trigger slots keep
separate states. An authored value stored through a different declaration has invalid state and
a diagnostic, even if its decoded struct has the expected name.

## Ordnance declarations

`SExplosiveOrdnanceParams` supplies the inherited fields in the base, bomb, space mine and missile
configurations. `SCItemSpaceMineParams` adds no fields. Missile guidance and targeting settings
have their own source scope.

| Original owner | Fields and storage |
| --- | --- |
| `SExplosiveOrdnanceParams` | `requiresLauncher` and `enableLifetime` Bool, `maxArmableOverride` Int32, `maxLifetime`, `armTime`, `igniteTime`, `collisionDelayTime`, `explosionSafetyDistance` and `projectileProximity` Single |
| `SExplosiveOrdnanceParams` | `audioParams` Class `SOrdnanceAudioParams`, `explosionParams` Class `ExplosionParams`, `clusterParams` StrongPointer `SOrdnanceClusterParams`, `emissionsParams` Class array `SOrdnanceEmissionsParams` |
| `SOrdnanceAudioParams` | `releaseAudioTrigger`, `engineIgniteAudioTrigger`, `trailAudioTrigger` and `trailStopAudioTrigger` Class `GlobalResourceAudio`, each with the original `audioTrigger` String |
| `SOrdnanceClusterParams` | `launchDetachTime`, `impactDetachTime`, `detachAngleInitial`, `detachAngleIncrement`, `detachDelay` and `detachSpeed` Single, `detachAngleRelativeToGravity` Bool and `detachAngleResetCount` Int32 |
| `SOrdnanceEmissionsParams` | `active` Bool, `minValue`, `maxValue`, `riseRate` and `decayRate` Single |
| `SCItemBombParams` | `dragAreaRadius`, `centreOfPressureOffsetY`, `maximumDropAngleFromFlatFlight` and `maxAltitudeForAudioRtpc` Single, `altitudeRtpc` Class `AudioRtpc` with its original `rtpc` String |

An empty emission array and each repeated occurrence keep their original states and positions.
Inline explosion and damage values use the [shared explosion reader](catalog-durability.md#explosion-definitions).
Ammo projectile detonation fields remain in the projectile's original scope. The device table
keeps source numeric widths, raw Single bits and units as authored.

## Installed Archive verification

Run the ignored check against a configured installed Archive to compare the emitted device
slot set with the original Item Components arrays. It checks the physical owner, concrete type,
original source-use coordinates, sensor-trigger states and scalar bits. Output contains only
counts. Set P4KX_EXPECT_EXPLOSIVE_COMPONENTS to require at least one original device slot.

~~~powershell
cargo test -p p4kx-export --test real_archive explosive_components_match_actual_item_slots_and_sensor_source_fields -- --ignored --nocapture
~~~
