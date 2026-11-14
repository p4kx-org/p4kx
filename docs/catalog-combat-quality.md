# Crafting quality calculation

This client contract defines `conservativeV1` quality evaluation against [Catalog v3
Blueprint data](catalog-blueprints.md). The
[shared conformance vectors](examples/catalog-combat-quality-v1.json) exercise arithmetic and
failure states with synthetic inputs for Basetool and VerseKit.

## Original identity and client selection

Start with the physical equipment Record and component ordinal. A weapon result also needs its
original action occurrence and selected physical ammunition Record. Use
`configurations.weaponActions.rows[].actionNodes[].sourceValueOrdinal.value` within the owning
Record. Match it with `actionSourceValueOrdinal.value` in
`projections.weaponProjectileAmmoSelections.rows[]` on the same `ownerRecordIndex.value` and
`componentOrdinal.value`. A present selection with `selection.value.type = "selected"` supplies
`selection.value.ammoRecordIndex.value`, which joins
`definitions.ammoDefinitions.rows[].recordIndex.value`. A filtered target can have a selected
index and no emitted definition body. Use these fields within their owning input and Build.
[Projectile ammunition selection](catalog-ammo-definitions.md#projectile-ammunition-selection)
defines the admitted routes and evidence states.

A factory evaluation uses the original input. A crafted evaluation requires a selected recipe
tier and its active cost occurrences. The tier's `blueprint.inputRef` and
`blueprint.recordIndex` identify the physical `definitions.blueprints.rows[]` owner.
The tier's `source.valueIndex` and a selection cost's `source.valueIndex` distinguish the
original tier and slot occurrences in that owner's `source.values` arena. Retain each active
option cost's own source position. The `mandatory`, `optional` and `meaning.options`
references describe available costs. A selected branch must satisfy its declared constraints.

For a named slot, read the exact string through `costs[].meaning.debugName.valueIndex`.
The referenced original `debugName` field is under `nameInfo` on that selection. Read
the exact `sourceText` from the original String payload. Match
override keys case-sensitively. An omitted override uses 500 for each active slot. An explicit
quality must be a JSON number with integer value in the inclusive 0–1000 domain. A null, string,
Boolean, fraction or out-of-range number returns `invalidQuality`. An override with no active
matching slot returns `unknownSlotKey`. An override matching multiple active slots with the
same `debugName` returns `ambiguousSlotKey`. Distinct original slots still receive their defaults
independently when the duplicated key is omitted. A selected slot without a readable name can use
the default. Exact original identity allows the caller to expose the unresolved key to its user.

## Eligibility and source evidence

Read each active `costs[].context.modifiers[]`. The modifier's `source.valueIndex` points to
the original occurrence with `gameplayPropertyRecord` and `valueRanges`. Resolve the Reference
target by its physical `targetRecordIndex.value` from the original arena and join
`definitions.gameplayProperties.rows[].recordIndex.value`. Keep the nominal `propertyGuid`
and exact original property identity. An unavailable physical binding returns `unresolvedProperty`.

The Archive describes the gameplay property and its display transformation. Use the bound
`className.value` with the reviewed [client mapping](basetool-catalog.md#item-stats). The mapping
assigns property identities to applicable scalar inputs as described in
[ADR-0072](adr/0072-gameplay-properties-have-no-archive-link-to-a-stat-and-armour-stats-follow-the-components.md). Any client
`betterWhen` metadata is presentation information. It does not affect which range or operation
applies. An unverified identity or input mapping yields `unresolvedProperty` or
`unsupportedTarget`, as appropriate.

For each scalar, count eligible original modifier occurrences across the selected tier, including
parent and active option contexts. If the active branch can affect the target and remains unknown,
return `selectionRequired` for that target. Two or more eligible modifier occurrences targeting
the same scalar return `unsupportedOverlap`, including one slot with multiple matching
modifiers and a pair with identical or neutral effects. The original order and occurrences stay
available for diagnosis. There is no verified inter-slot aggregation or mixed-kind ordering
rule in this policy. A single admitted modifier may affect several scalars when the mapping
identifies each input. Return `unchanged` after establishing that no eligible modifier applies.
Unsupported or incompletely inspected relevant inputs keep their explicit states. Factory
and independently available crafted metrics remain usable.

## Curves and numeric operations

Reconcile all original ordered `valueRanges` occurrences with the typed
`ranges[].source.valueIndex` cells before evaluating a modifier. Typed rows cover two concrete
range classes. Keep the position of every other original range. An unknown or skipped
range yields `unsupportedRange`. Invalid source bounds, unavailable or nonfinite endpoints
yield `invalidRange`. Decode `startQuality.value` and `endQuality.value` from signed
decimal integer text. Decode each finite `atStart.value` and `atEnd.value` as an authored
IEEE 754 Single before promoting it to binary64. When available, the exact original
arena numeric word is the input. A shortest-decimal Single projection can recover the
same binary32 by rounding to nearest, ties-to-even before promotion.

Use closed integer intervals `[lo, hi]` within 0–1000, in authored order. Zero matching ranges
gives `missingRange`. More than one match gives `ambiguousRange`, including a shared inclusive
endpoint. Preserve the exact selected endpoint value at either boundary. A zero-width range
with conflicting endpoint values gives `ambiguousEndpoint`. Equal endpoint values define
a constant band. Adjacent integer bands can change abruptly at the next integer, such as
499 to 500. The separate [resource quality quantization](basetool-catalog.md#quality-quantizations)
table describes input mapping bands.

- `CraftingGameplayPropertyModifierValueRange_Linear` uses
  `f = atStart + ((q - lo) / (hi - lo)) × (atEnd - atStart)` for an interior point.
  Apply `original × f` using ordinary binary64 operations, with no display rounding or
  fused multiply-add. The original type identifies the operation even for zero or negative factors.
- `CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive` uses endpoints with
  exact integer value. For an interior point compute the exact integer numerator
  `atStart × (hi - lo) + (q - lo) × (atEnd - atStart)` and require zero remainder on division
  by `hi - lo`. Use arbitrary-precision integers for that intermediate calculation where needed.
  The interpolated integer must convert exactly to binary64. A conversion that changes the
  integer returns `unsupportedNumeric`. A fractional endpoint or fractional interior result
  returns `unsupportedIntegerRounding`. Apply `original + summand` when supported.
  Any separate estimate that chooses a rounding or stacking policy needs its own policy identifier.

## Inputs for combat evaluation

| Reviewed property mapping | Source-backed calculation input |
| --- | --- |
| `GPP_Weapon_Damage`, applicable simple projectile | `definitions.ammoDefinitions.rows[].projectileParams.value.kind.value.damage.value.value.<channel>.value.finite`, with `kind.type = "bullet"` and `damage.value.type = "info"` |
| `GPP_Weapon_FireRate`, applicable direct shot action | `configurations.weaponActions.rows[].actionNodes[].kind.value.fireRate.value` for a supported single or rapid action |
| `GPP_Shield_MaxHealth`, applicable shield generator | `configurations.shieldGenerators.rows[].maxHealth.value.finite` |

The reviewed fire-rate mapping covers personal weapons. The vectors supply synthetic,
explicitly admitted mappings to exercise rate, damage and shielding arithmetic.
An actual ship rate or beam property mapping requires applicable source evidence.

For a supported simple projectile action, adjust the mapped original inputs first. Calculate
each channel's nominal direct-hit alpha as
`(adjustedDamagePerProjectile × pelletCount) × launcherDamageMultiplier`, using
`kind.value.launchParams.value.pelletCount.value` and
`kind.value.launchParams.value.damageMultiplier.value.finite`. Nominal DPS for that channel
is `alpha × (adjustedRPM / 60)`. Retain `ammoCost` as a separate consumption input.
Use complete declared channel coverage for a total. `DamageBase` supplies no explicit channel
vector. `DamageParams` supplies a macro and `damageTotal` that needs its own channel model.
Keep unavailable channels distinguishable from a measured zero. The supported model needs
finite nonnegative channels and launcher multiplier, a positive integral projectile count and a
positive finite rate. A result outside its input's supported domain gives `invalidTargetValue`.
A non-selected ammo status supplies no eligible ammunition inputs and the fixture reports
`unresolvedAmmo`.

Shield max health is independently evaluated from its own base value. The additive shield
vectors exercise the generic integer operation. The projectile policy marks a beam or
charged wrapper as `unsupportedAction`. Preserve independently sourced beam fields such as
damage per second and full- and zero-damage ranges. Projectile travel distance, burst cycle,
charge, reload and sustained DPS use separate models with their own source evidence.

For an adjusted result retain the actual source uses of its input, selection and modifier
occurrences, including evidence that establishes eligibility. Keep encryption and completeness
per datum as described by the
[provenance contract](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md).
The client quality override and algorithm constants are client calculation parameters.

## Shared fixture

`fixtureVersion: 1` defines the JSON layout independently of Catalog schema versions.
`policy: "conservativeV1"` identifies the client rules tested. Each case has `profile`,
`input`, `slots`, `qualityOverrides` and `expected`. These are prepared inputs after Catalog
joins. `slots[].slotId` is an opaque synthetic occurrence identity. `propertyMappings` supplies
an explicit synthetic scalar for each property and example presentation direction.
`selectionStatus: "required"` marks an unresolved branch for the candidate targets in that case.
Expected nulls accompany explicit request, action, or per-target statuses. The fixture uses a
complete synthetic damage vector for its two channels.

Compare statuses, identities, integers and exact stored endpoints by equality. Other scalars
permit absolute error up to `max(tolerance.absolute, tolerance.relative × abs(expected))`.
Fixture endpoint decimals are exactly representable Singles. The dataset tests client
arithmetic. Source identity joins, original receipts and actual game property applicability
are separate Catalog validations.
