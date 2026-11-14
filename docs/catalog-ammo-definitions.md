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
| `AmmoParams.displayName` | Public localized name with the original locale and independent translation receipts |
| `AmmoParams.geometryResourceParams` | Separate geometry and material node arenas, authored model and material paths, model and visibility tags, palettes, stored rendering flags and soft-body settings |
| `AmmoParams.geometryTransformParams` | Authored initial/random angles, rotation rate, random rotation and scale range |
| `AmmoParams.lightPoolParams` | Authored flare asset name and scale, RGB, radius, attenuation and multipliers, flags and animation inputs |
| `AmmoParams.radarObjectParams` | `radarParams` carries original contact, signature and audio parameters, typed scan values, and ordered Reference slots |
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

The sourced `geometry` and `material` values index their respective `geometryNodes` and
`materialNodes` arrays. Indices follow breadth-first discovery. The ordered `subGeometry` and
`materialVariants` entries retain each original slot, including duplicates and empty arrays.
Asset paths preserve original spelling and case. Numeric geometry and simulation inputs keep
their stored widths and raw float words. The authored `enforceCosseratStrechShear` field is
exported as `enforceCosseratStretchShear`.

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

A `weaponMagazineSlots` declaration identifies each physical
`SCItemWeaponComponentParams` by the same owner and component ordinal.
Its `ammoContainerRecord` retains the authored primary magazine
Reference. Each original `secondaryAmmoContainers` element carries its own
magazine Reference and authored condition in stored order, including repeated
targets. The physical target index identifies the magazine Item independently
of the export filter. Its authored containers have their own component
ordinals in `configurations.ammoContainers`.

`shouldIgnorePrimaryAmmoContainer` retains the original Bool, including false.
Each available condition carries its exact StrongPointer declaration and
original subtree in a local `roots` and `values` arena. Condition values keep
their original provenance and text classifications. This declaration records
the source slots and condition inputs for consumers to interpret.

`projections.weaponMagazineResolutions` records the primary route actually
consulted by the existing weapon view. The row identifies the first
applicable weapon component by physical owner and slot. An own container
records its original component ordinal. A linked route records the actual
magazine Record index and, when selected, that container's component ordinal.
The same target body supplies the legacy capacity and ammunition.

A readable target with no container, a target whose component selection
is indeterminate, and an unreadable target body have distinct outcomes.
The unreadable result retains the original magazine header identity and
the source-read failure code. The consulted dependency is visible in the
health report even if the catalog selection excludes its own Item row.

## Authored weapon actions and launchers

`configurations.weaponActions` contains one row for each physical
`SCItemWeaponComponentParams` on a selected Item. Join the row to
`declarations.ammoBindings` by `ownerRecordIndex` and `componentOrdinal`.
The sourced `fireActions` array preserves the original slots, including
null entries. The separately sourced `aimAction` keeps its own slot.
Each admitted slot indexes its row's `actionNodes` array. Ordered child
slots in sequences, parallel actions and conditional actions use the
same row-local indices. Sequence entries retain their original delay,
unit and repetition count. Conditional entries retain unevaluated
conditions and the separately declared default action. A charged action
retains its own timing, flags, modifier and child action. Toggle actions
retain their separate child slot. The `authored` arenas on typed actions, launchers and entries retain
the remaining original fields in source order. A typed field that cannot
represent its original value keeps that value in the arena, alongside
the typed field's source state.
Action and launcher slots validate the concrete type's physical parent
chain against the field's declared base. An unrelated concrete type
produces an `unsupported` cell. A node retains its concrete `sourceType`
and original `sourceValueOrdinal` as a decimal string scoped to the
owning Record.

Single, rapid and burst actions expose their stored per-shot inputs and
owned `launchParams`. Rapid and burst inputs retain their own spin and
count fields. Beam actions retain `heatCostPerSecond` separately from
`heatPerSecond`. The typed beam body exposes both original range endpoints and the original
hit label. The damage-per-second pointer exposes the exact concrete damage
type. A channel body retains the six stored damage words with their separate
origins. A parameters body retains the original macro reference and total.
Other admitted damage bodies and unprojected members remain in the authored
arena. The catalog retains these inputs for consumers to evaluate.

A projectile launcher's `heatCost` and the action's
`heatPerShot` remain independent Single values. The other admitted action
declarations retain their original fields and local value links under the
`authored` alternative. Other admitted launchers retain their concrete
`sourceType` and original field arena under the same alternative. The
legacy `items[].stats.weapon.fireActions` list
retains its existing flattening and precedence.

The row's `operatingConfiguration` keeps the component's original
`proceduralAnimationRecord` and `actorProceduralRecoilConfig` References.
`connectionParams` distinguishes the no-power, underpower, overpower,
overclock, heat and locked-overheat stat blocks. Each block keeps its
original fields and exact numeric representations. The other connection
members, such as ranged heat states and overheat effects, remain in
`additionalFields` in source order. `weaponDegradationModifier` and
the aim action's `aimModifier` expose their own `weaponStats` blocks.
The typed aim action also retains its geometry settings and
`hasReloadModesOnUI` flag. These fields describe stored configuration.
The flag records UI availability for that action.

A projectile launcher's `projectileType` is the stored label of the
declared `EProjectileType` Enum. A readable different declaration is
`unsupported`. A failed declaration or label read is `unreadable`. The ammunition
Reference receipts stay in the authored container slots. Use their physical
target indices to join selected Ammo definitions. The launcher label and
the owning component relationship provide source inputs for selection. The
existing legacy primary resolution remains a separate projection.
