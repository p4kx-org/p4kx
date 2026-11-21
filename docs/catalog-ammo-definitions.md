# Ammunition definitions

`definitions.ammoDefinitions` contains one row for each selected physical
`AmmoParams` Record and each required primary Ammo dependency. The Record
filter selects definitions independently of their use by weapons. The canonical header
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
| `physicsControllerParams.PhysType` | Shared authored physics fields and the concrete `base`, `particle` or `rigid` branch. Particle retains dimensions, pierceability, drag and collision/gravity flags. Rigid retains remaining original members in its ordered `kind.additionalFields` arena with per-member provenance |
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

## Reading the original primary ammunition

The physical weapon's own container takes precedence over the one-hop
`ammoContainerRecord` magazine fallback. Join the magazine resolution
to the chosen container to read `maxAmmoCount` and its `primaryAmmo`
Reference. The binding retains the target status for missing or wrong-type ammunition.
A readable physical Ammo Record appears once even when multiple weapons share it.
The record supplies its original speed, lifetime, damage and detonation fields.
Filtered weapon selections can retain the exact container and ammunition physical
dependencies without selecting additional Items.

## Physical containers and slots

Ammo definitions and container configuration rows state `selection: selected` for ordinary
catalog selection or `selection: dependency` for a physical row retained by an admitted
weapon's original primary route. Dependencies preserve the same owner keys, fields and
provenance and do not select their owning Item. A row that is also selected normally appears
once. Omitted, unreadable and missing source endpoints retain their original decision states.

The installed source check selects one physical weapon owner per Build through its record path.
It verifies the primary container and dependent Ammo definition by their original Record and slot
coordinates, then compares the authored damage channels and nominal flight-distance inputs.

`configurations.ammoContainers` contains one row for each physical
`SAmmoContainerComponentParams` on a selected Item or a consulted linked
primary magazine slot. The rows follow
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
`definitions.ammoDefinitions` when the definition is selected or retained as a dependency. The binding
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
The same target body supplies the canonical container configuration and its bindings. Read
`maxAmmoCount` from that exact physical slot and follow the primary binding to the Ammo definition.

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

On PTU 13189690 the Player animation component's authored `animDatabase3P` leads to
`AnimDB/SubADBs/SubADB@File` and ordered `FragmentID@Name` declarations. Two resolved
sub-databases contain 323 options in the selected reload fragment groups. The source
carries option blend-out, layer blend start, exit and duration attributes, and optionally
animation playback speed. The shared CryXML animation database reader keeps each original
position, tag condition and alternative. The Player exporter reads that structure through
the shared reader. The Catalog currently exports controller locators. Combining weapon
motion tag scope with those options and exporting the resulting candidates is tracked
in [#1053](https://github.com/p4kx-org/p4kx/issues/1053). The authored blend and playback
values do not specify a total elapsed reload time.

A projectile launcher's `heatCost` and the action's
`heatPerShot` remain independent Single values. The other admitted action
declarations retain their original fields and local value links under the
`authored` alternative. Other admitted launchers retain their concrete
`sourceType` and original field arena under the same alternative. Follow the
authored action indices to retrieve the inputs of the selected action and its children.

### Original action field inventory

Paths below start at each `actionNodes[]` element of the physical
`weaponActions` row. Sourced cells carry a value, its state and its
own provenance. Where present, read their payload through `.value`.

- Across action families, `recoil`, `recoilGrappling` and
  `recoilInterval` are siblings of `kind`. The first two retain
  their referenced Record identities. Join the target definitions
  through their original Record indices.
- `kind.value` keeps the direct fields of single, rapid and burst
  actions. All three retain `fireRate`, `heatPerShot`, `wearPerShot`
  and `launchParams`,
  plus `shotCount` and `cooldownTime` for burst or `fireDuringSpinUp`,
  `spinUpTime`, `spinDownTime`, `spinParam` and
  `audioLoopTimeBetweenFirstShots` for rapid.
- `kind.value.launchParams.value.type` identifies the launcher variant.
  Projectile inputs keep `fireHelper`, `muzzleHelper`,
  `ammoCost`, `heatCost`, `pelletCount`, `damageMultiplier`, `soundRadius`,
  `projectileType` and `spreadParams`. Its typed spread exposes `min`,
  `max`, `firstAttack`, `attack` and `decay`. Missile inputs keep
  `soundRadius`, `detachVelocityRight`, `detachVelocityForward`,
  `detachVelocityUp`, `detachVelocityRoll`, `canFireWithoutLock`,
  `showHitIndicator` and `igniteOnPylon`. Throwable inputs keep
  `throwMode` and `soundRadius`. The base and dummy variants keep their
  original fields in `authored`. Other concrete launchers retain their
  `sourceType` and use `original`.
- Beam inputs in `kind.value` also keep `wearPerSecond`,
  `minEnergyDraw`, `maxEnergyDraw`, `chargeUpTime`, `chargeDownTime` and
  `fireDuringChargeUp`. Under `damagePerSecond.value.value`, channel
  fields such as `DamageEnergy` use the labels `physical`, `energy`,
  `distortion`, `thermal`, `biochemical` and `stun`. The parameter
  alternative uses `damageMacro` and `damageTotal`.
- Tractor beam actions use `kind.type = "tractorBeam"`. Their `kind.value`
  keeps the stored Singles `minForce`, `maxForce`, `minDistance`,
  `maxDistance`, `fullStrengthDistance`, `maxVolume` and `maxAngle`.
  Each value includes its exact IEEE bits and source. Other stored members
  remain in `authored`. Tractor beam actions supply no projectile damage
  or DPS inputs.
- `kind.value` keeps `mode` and `sequenceEntries` for sequences,
  `weaponActions` for parallel actions, `conditionalWeaponActions`
  and `defaultWeaponAction` for both conditional kinds, and
  `weaponAction` for toggle and charged actions. Each sequence entry
  keeps `delay`, `unit`, `repetitions` and `weaponAction`. Each conditional
  entry keeps `weaponAction` and the complete original `condition.value`
  arena. The charged action exposes `chargeTime`, `overchargeTime`,
  `overchargedTime`, `cooldownTime`, `maxGlow`, `interpolateChargeBonus`,
  `fireAutomaticallyOnFullCharge`, `fireOnlyOnFullCharge`,
  `chargeAutomatically` and the complete `maxChargeModifier.value` arena.
- Aim inputs expose `hasReloadModesOnUI`, `aimGeometryTags` and
  `aimModifier`. The latter retains its nested `weaponStats`. Each of these
  original bodies records remaining members in its own `additionalFields`.

For each admitted body, a direct typed cell covers its first original
occurrence when its state is `present` or `null`. Remaining occurrences,
including duplicate names, inherited fields, and values rejected by a
typed cell, have explicit roots under the body's `authored`,
`additionalFields` or `original` arena. When `kind.type` is `authored`,
`kind.value` is the arena itself. Read its `roots[].name` and follow the
local `valueIndex` into `values` for declaration, state, value and provenance.
Original arrays keep their slot order. References carry their identity,
while owned structs retain their descendant fields. A rejected action or
launcher slot declares its source state and does not produce a typed node.

### Component operating configuration

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

Each `actionNodes` entry exposes the original `recoil` and `recoilGrappling`
Record References, and `recoilInterval` where stored. A resolved reference's
`targetRecordIndex` joins `definitions.weaponActionRecoilDefinitions` by
`source.recordIndex`. Multiple actions can share one physical definition.
The definition retains the authored hand, aim, body and head settings,
including ordered curves in the original field arenas. Reference states
and the health report cover unavailable targets independently of their
physical identity.

Follow `operatingConfiguration.proceduralAnimationRecord` to
`definitions.weaponProceduralAnimations` and
`actorProceduralRecoilConfig` to `definitions.actorProceduralRecoilConfigs`.
Each definition keeps the original setup order, actor-state filter
labels and nested references. Those references join the physical rows
in `definitions.weaponProceduralClips` or
`definitions.actorProceduralRecoilModifiers`. The target bodies retain
the original typed value storage and payload as an ordered arena. A
selected weapon's admitted links can include definitions independently
of the item path filter. The shared tables keep one row per physical
target, including when multiple setup slots reference the same Record.

`configurations.entityAnimationControllers` exports the original
`SAnimationControllerParams.AnimationController` and `AnimationDatabase`
file locators. These belong to their own component, which can share the
physical Entity with the weapon. Join by `ownerRecordIndex` and use
`componentOrdinal` to distinguish multiple controllers. The locators
identify the authored Mannequin inputs. The reload UI flag and the
magazine's ammunition-count blend time remain separate values.

In two PTU witnesses the controller's `ControllerDef/Fragments@filename`
and the database's `AnimDB@FragDef` resolve to the same physical Fragment
definition. Each paired controller declares four reload-named fragments.
The paired weapon databases contain zero corresponding options and zero
sub-databases. These observations establish the declared fragment
relationship for the two witnesses. A concrete reload animation, event
or elapsed time requires a bound Fragment option and its applicable
animation inputs.

### Player Mannequin branches and options

When the selected item definitions contain personal weapon attachments,
`definitions.items.rows[].components.value[].value.value.weaponMotion` carries the original
`AttachDef.Type` and `AttachDef.Tags`. The attachment keeps its physical
component ordinal. `definitions.weaponAnimationSources` contains the
Player Entity candidates and their ordered animation component locators.
The controller, AnimDB, TagDefinition, and import rows identify the
verified archive reads. A physical file is identified by `inputRef` and
`entryIndex`. Text XML coordinates are byte spans. CryXml coordinates use
the original node and attribute indices.

The database rows retain ordered `SubADB` and `FragmentID` selectors,
fragment options, and nested elements with original source coordinates.
`Tags`, `FragTags`, `BlendOutDuration`, `Blend` attributes `Duration`,
`StartTime` and `ExitTime`, and `Animation` attributes `name`, `speed` and
`flags` retain their original spellings. `ProcLayer` and other nested
elements retain attributes, sibling ordinals and parent pointers.
Timing interpretation and units depend on the owning Mannequin format.

Each fragment selector links to its same-name options by original position.
Its state records whether the parent and target resolve the name to the
same physical `FragDef` declaration. The source rows distinguish missing
groups, invalid paths, ambiguous targets, incomplete scopes, and cycles.
A weapon tag candidate requires at least one common authored attachment
and option tag with a unique declaration in the target's `TagDef`. Each
match gives the two zero-based token indices and the original declaration.
The `weaponPlayerAnimationCandidates` projection links the weapon
attachment, Player component, branch and option. `playback` is `unverified`.
Runtime fragment or animation selection, and elapsed reload time, require
separate evidence.

The typed Player graph SDK follows these same physical source relationships directly.
It reads the reached graph even when a filtered Catalog admits no personal-weapon
attachments. [Player animation SDK](catalog-sdk.md#player-weapon-animation-candidates)

Each source field and candidate state carries per-value provenance. A
candidate state's provenance includes the weapon attachment and the scoped
option selection.

A projectile launcher's `projectileType` is the stored label of the
declared `EProjectileType` Enum. A readable different declaration is
`unsupported`. A failed declaration or label read is `unreadable`. The ammunition
Reference receipts stay in the authored container slots. Use their physical
target indices to join selected Ammo definitions. The launcher label and
the owning component relationship provide source inputs for selection. The
existing legacy primary resolution remains a separate projection.

## Projectile ammunition selection

`projections.weaponProjectileAmmoSelections` keys each readable direct
projectile launcher by physical owner Record, original weapon component
ordinal, and the action's original `sourceValueOrdinal`. The projection
accepts single, rapid, and burst actions in the weapon's own `fireActions`.
`Primary` and `Secondary` from `EProjectileType` select the corresponding
`AmmoParams` Reference in the on-item container. A selected result requires
one physical weapon and one compatible on-item container, a complete
component survey, an explicit false ignore flag, and no declared external
or conditional magazine candidate. Every proved primary or secondary route
reads its exact physical Ammo Record. A readable definition outside the
independent export selection is retained as a dependency. An unreadable
target keeps its physical index and reports the body failure.

Use the `selection` source uses for the whole decision, including the
component survey and the selected role. Repeated ownership, unreadable
components or action slots, a competing magazine route, and missing or
dangling ammo retain distinct outcomes. The authored declarations and
action tree carry those source values and their independent states.

## Linked primary magazines

A direct projectile launcher can also select ammo through the weapon's
original primary magazine Reference. The caller needs one admitted physical
weapon, no own ammo-container family member, an explicit false ignore flag and
no secondary magazine slots. The exact Reference must identify one other
`EntityClassDefinition` header, whose complete Components array has one
admitted ammo container. `selectedLinkedMagazine` retains the magazine Record
index, container component ordinal, ammo role, and Ammo Record index. Target
selection and source evidence remain available when the export filter excludes
the magazine or the Ammo definition.

An unreadable target body, incomplete component survey, competing magazine
or container, duplicate target GUID, self link, dangling Reference and
wrong target type each keep an explicit outcome. The linkage is one hop.
Conditions and competing routes remain available in the authored declarations.

## Installed archive verification

The opt-in ammunition check selects the standard physical ammunition Record directory
`libs/foundry/records/ammoparams/`. It compares every selected header and source-read outcome,
then samples original projectile, countermeasure, conversion and physics combinations.
A build can add additional Record directories. Set `P4KX_AMMO_ORACLE_FILTER` to check an
alternate measured scope. The test prints how many other physical `AmmoParams` Records are
outside that filter. Set `P4KX_EXPECT_ALL_AMMO_RECORDS=1` to require that the checked scope
covers every `AmmoParams` Record in the configured build.

~~~powershell
cargo test -p p4kx-export --test real_archive ammunition_definitions_match_original_physical_records_and_sampled_variant_values -- --ignored --nocapture
~~~

The weapon-action check verifies the ordered original slots, typed values,
and complete authored value trees of the P8-SC, Volt and Revenant. It requires
all three in PTU change `13189690` and checks the named owners found in other
builds. Set `P4KX_EXPECT_ALL_WEAPON_ACTION_WITNESSES=1` to require all three
on another build. The receipt identifies the Build and counts original
owners, nodes and compared values.

~~~powershell
cargo test -p p4kx-export --test real_archive representative_weapon_actions_match_original_slots_and_inputs -- --ignored --nocapture
~~~
