# Ammunition definitions

`definitions.ammoDefinitions` contains one row for each selected physical
`AmmoParams` Record. The ordinary Record filter applies at every path,
including definitions without a referring weapon. The canonical header
retains the GUID, physical `recordIndex`, source type and path. Rows sort
by GUID and then physical index, so equal GUIDs do not merge physical
definitions. The table's `availability` describes the source family
independently of the number of selected rows.

## Authored fields

| Source | Schema v3 projection |
| --- | --- |
| `AmmoParams` | Spawn type, size, category and UI icon. Hit points, lifetime, showtime, inherited velocity, bullet type, speed, impulse scale, hit/removal/convergence flags, the resource Reference and audio/particle fields |
| `AmmoParams.projectileParams` | Shared detonation and proximity configurations beside the `kind.type` union `base`, `bullet`, `tachyon` or `countermeasure` |
| `BulletProjectileParams` | Per-channel damage and falloff, impact radii, ignition and keep-alive flags, impulse falloff, penetration, pierceability and electron settings. Authored visual, extra-projectile and ordered hit-behavior inputs |
| `TachyonProjectileParams` | Hit type/effect, per-channel damage, full- and zero-damage ranges, penetration and ordered beam effects |
| `CounterMeasureProjectileParams` | `typeParams` keeps `base`, `chaff` and `flare` distinct, including their applicable start/end signature profiles and trail/explosion audio |
| `physicsControllerParams.PhysType` | Shared authored physics fields and the concrete `base` or `particle` branch, with particle dimensions, pierceability, drag and collision/gravity flags |
| `AmmoParams.conversionRate` | `base` has no quantity. `standard` retains `standardCargoUnits` as a Single, while `centi` and `micro` retain the signed `centiScu` and `microScu` quantities in their respective source units |

The original struct selects each branch. The declarations for
`projectileParams`, `conversionRate` and `physicsControllerParams` must
hold a scalar StrongPointer. The source receipts include the declared
storage used to accept or reject each field. Damage channels remain separate.
The export preserves ordered arrays, original number widths, and the units
specified by the source, such as degrees in `spreadDegrees` and damage
per metre in `damageDropPerMeter`. It keeps the three cargo-unit scales
distinct. Distance, timing, mass and other inputs stay in the original
representation. The size is UInt8, bullet type is Int32 and shots per audio loop is UInt32. The export computes no range, damage sum, penetration
probability or damage per second.

Source-backed fields use the [catalog provenance contract](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md)
through `containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`.
The owning carrier records the concrete type that selects each typed branch.
Original Singles expose `rawBits` and a nullable `finite` decimal. Stored
zero and false stay present. Absent, null, unreadable, unsupported and
invalid inputs keep distinct states. Record References preserve actual
target identity and resolution state separately from whether the target is
included by the export filter.

## Existing weapon view

The existing `items[].stats.weapon.ammo` still shows the original primary
`ammoParamsRecord` selection and its established speed, lifetime, damage
and detonation subset. The weapon's own container takes precedence over
the one-hop `ammoContainerRecord` magazine fallback. `ammoCapacity` is the
selected container's `maxAmmoCount`. Missing, wrong-type or unreadable
ammunition leaves the legacy ammo value null. An admitted physical Ammo
Record is materialized once for both the selected definition and weapon
dependencies in a catalog scan. A filtered-out definition can still supply
the legacy view of a selected weapon.

## Physical containers and slots

`configurations.ammoContainers` contains one row for each physical
`SAmmoContainerComponentParams` on a selected Item. The rows follow
`ownerRecordIndex` and `componentOrdinal`, which together identify the
original component occurrence. Repeated components keep separate rows.

The container keeps the signed `initialAmmoCount` and `maxAmmoCount`, and
the unsigned `maxRestockCount`. The warning percentage stays in its
authored Single representation. The original enum, despawn and repool
flags, empty geometry tag, fragment and blend time remain separate fields.
Both attachment arrays preserve their original entries and order.

`declarations.ammoBindings` carries a `containerSlots` declaration for each
container occurrence. `primaryAmmo` and `secondaryAmmo` retain the original
`ammoParamsRecord` and `secondaryAmmoParamsRecord` References separately.
Join a resolved `targetRecordIndex` to the physical index in
`definitions.ammoDefinitions` when the definition is selected. The binding
retains its Reference status when the definition is excluded by the filter.
Each slot requires a scalar Reference declared for `AmmoParams`. The source
diagnostic preserves an incompatible declaration independently of a
correctly declared Reference with a missing or differently typed target.
