# Values by Crafting quality

`formulaVersion: 1` governs consumer calculations against Catalog
`schemaVersion: 3`. P4KX supplies authored inputs, Blueprint positions and derived factory
weapon values. Consumers select equipment, modes, branches and qualities, apply the formula
and aggregate their selected loadouts. The versioned fixtures describe synthetic builds.
Measured Crafting results can warrant a new version of the formula contract.
[Original formula agreement](https://github.com/krt-profit/basetool/issues/2821#issuecomment-6878138700)

## Quality key and the active recipe

For one crafted placement, use an array of quality entries. Each entry has this shape:

```json
{
  "blueprintGuid": "00000000-0000-4000-8000-000000000101",
  "tierIndex": 2,
  "slotPath": [
    {"type": "optional", "index": 0},
    {"type": "option", "index": 2}
  ],
  "quality": 750
}
```

Join the selected Blueprint via `definitions.blueprints.rows[]`; the tier's
`blueprint.inputRef` and `blueprint.recordIndex` identify the physical owner. If the external
Blueprint GUID has multiple eligible physical targets, require an explicit resolution against
the current Catalog. Treat Build, selected Blueprint, tier and path as the identity.
`tierIndex` and every path `index` are zero-based unsigned 32-bit integers from the
original source arrays. The indices survive a skipped sibling in diagnostic output. A first
step is `{"type":"mandatory"}` or `{"type":"optional","index":N}`. Each further step is
`{"type":"option","index":N}`. Read the original tier index from `tierIndex.value`. Catalog cost rows publish a sourced
`position` cell with `value.type`, optional or option `value.index`, and for options a
`value.parent` reference. Find that parent by
matching its `source.valueIndex` within the same tier and Blueprint. Walk to the root and
reverse to obtain the exchange path. Arena `valueIndex` strings are Catalog-local references.
Quality paths are independent of language and duplicate or missing debug names.
Reload and resolve the path against the new Blueprint when the Build changes.

Use the declared mandatory cost and the active optional and option branches. Validate the
branch against the authored selection constraints. Every active position with a modifier
context owns its input quality, including a parent with its own context and a chosen alternative
with a distinct context. Omitted quality means 500 per active position. A supplied quality is
an integer from 0 through 1000. Invalid values return `invalidQuality`, duplicate entries return `duplicateSlot`, and
unknown, malformed or inactive positions return `unknownSlot`. Preserve repeated occurrences of properties and
slots. Use `selectionRequired` for an unresolved branch that can affect the requested result.
Factory equipment uses the unmodified base inputs and evaluates without a recipe.

## Modifier inputs and deterministic arithmetic

Resolve each active `costs[].context.modifiers[]` through its original
`gameplayPropertyRecord` Reference. Join the physical `targetRecordIndex.value` from its source
arena to `definitions.gameplayProperties.rows[].recordIndex.value`. Use the reviewed
[property mapping](catalog.md#item-stats) and its item applicability. In particular,
shield max health maps to the selected shield generator, projectile damage follows selected
direct-hit `DamageInfo` channels, and the reviewed weapon fire-rate map currently demonstrates
personal weapon applicability. A consumer needs evidence to extend that map to other equipment.
`betterWhen` controls presentation. Each original cost and modifier contributes once.

Read all original `valueRanges` occurrences in order. The typed ranges use `kind`, two
signed decimal quality bounds and binary32 endpoint projections. Recover finite authored
Single endpoints before promoting them to binary64. For an integer endpoint, read its
exact original word in the Blueprint arena and convert it to binary64 only when the
conversion is exact. This preserves additive endpoints such as 16,777,217 when the typed
binary32 projection rounds them. Map recognized linear ranges to ordered linear segments.
A stepped consumer segment has explicit step shape. An unfamiliar source shape or an
incomplete enumeration of the modifier or its ranges yields an unavailable result.
A modifier that cannot be bound to an applicable scalar retains its unresolved state.
Use the segment or band parameters only after resolving the original modifier's operation.

For a modifier with segments, examine them in their provided order. Skip a segment with either
quality bound missing. Choose the first segment whose upper bound is at least q, or the final
entry if reached with readable bounds. At a shared endpoint the earlier segment wins. For a
step take the first available value, preferring start over end. For a linear segment both
endpoints are required. Compute `t = clamp((q - a) / (b - a), 0, 1)`, with t = 0 when
a equals b, then calculate `start + (end - start) * t`. A gap selects the next eligible segment
and clamps its interpolation. When there are no segments, apply the same linear rule to the
explicit band and require both bounds and both endpoints. A selected segment missing a required
value, an exhausted list, or an incomplete band produces `unsupportedRange`.

Group eligible modifiers by bound physical Property. Visit active costs in their original
depth-first order and modifiers in authored order. Use ordinary binary64 operations without
fused multiply-add or display rounding.

- For `MULTIPLIER` start at 1. For each contribution v add `(v - 1)` to the running
  factor and apply `base * factor` once. Two factors of 1.10 produce 1.20.
- For `ADDITIVE` round each v to the nearest integer with ties to even, then sum with exact
  integer arithmetic. Convert the final sum to binary64 only if exactly representable and
  add it to the base once. For example, two contributions of 0.5 sum to 0, two of 1.5
  sum to 4, and two of -1.5 sum to -4. Keep explicit zero values.
- Mixed kinds on the same Property and different Properties targeting the same scalar have
  explicit unsupported results. An unresolved branch, binding or unknown modifier keeps
  its affected target unavailable. An unaffected scalar keeps its original value. Reject
  nonfinite inputs and arithmetic and results outside the metric's domain.

For every adjusted result retain its actual source uses for the base input, eligible recipe
positions, original modifiers and ranges. Use the per-value provenance rules in
[ADR-0357](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md). A caller's
quality and the contract constants are calculation parameters. A default quality of 500
still evaluates the modifier. Factory and crafted profiles can give different values at Q500.

## Weapon projection

`projections.weaponCombatBases.rows[]` has one row per top-level
`configurations.weaponActions.rows[].fireActions.value[]` slot. Join by `ownerRecordIndex`
and `componentOrdinal`, then original `modeIndex.value`. `actionSourceValueOrdinal`
and `actionSourceType` identify the readable original action node.
`ammunitionRecordIndex.value` identifies a proved physical selection. Read its body from
`definitions.ammoDefinitions.rows[].recordIndex.value`. The ammo selection and its reason
are in `projections.weaponProjectileAmmoSelections` joined by component and action ordinal.
A filtered Catalog can keep the selected coordinate without its body. See the
[ammunition selection contract](catalog-ammo-definitions.md#projectile-ammunition-selection).

Each row specifies `formulaVersion` and `units`. The model is a sourced tagged outcome.
`nominalBullet` covers direct single and rapid projectile actions with unambiguous selected
Bullet ammunition. The model's `values` retains fire rate in discharges per minute,
projectiles per discharge, and the launcher multiplier. `nominalFlightDistance` in metres
is original ammo `speed` in metres per second times `lifetime` in seconds. Inspect these
two original fields on the selected Ammo definition. This metric represents the nominal
constant-speed flight distance. Its calculation is independent of the damage payload and RPM.

With a readable explicit `DamageInfo` payload, `model.value.values.damage.value` contains sourced
per-channel `alpha` and `nominalDps`. The raw direct channel inputs are under
`projectileParams.value.kind.value.damage.value.value` on the linked Ammo definition, where
`kind.type` is `bullet` and the damage type is `info`. Apply quality to the mapped original
inputs first for the crafted calculation. Per channel, with direct-hit damage d, projectile
count n, launcher multiplier m and rate r in discharges per minute, use:

```text
alpha = (d * n) * m
nominalDps = (alpha * r) / 60
nominalFlightDistance = speed * lifetime
```

Each channel and m requires a finite nonnegative value. The projectile count is a positive
integer and RPM is positive. Speed and lifetime are finite and nonnegative. An RPM error
leaves supported alpha and distance available. Keep absent, null, unreadable, unsupported and
invalid states, and explicit zero, distinct. A complete total requires the declared channels
to be available. `DamageBase` and macro-backed `DamageParams` leave the direct channel
result unsupported. Ammo cost is a separate input from the owned launcher.
Nominal firing DPS excludes reload and transient timing such as spin-up, cooldown and charge.

The projection records Burst, Charge, Beam, Conditional and other unsupported actions and
unavailable action or ammo selection states. Join a readable Beam mode back to its authored
`kind.value.damagePerSecond`, `kind.value.fullDamageRange` and `kind.value.zeroDamageRange`. These fields remain
independently usable. Projectile `damageDropParams` likewise remains on the Ammo definition.
These authored values stay distinct from modeled nominal flight distance. A mode with
conditional ammunition or without a unique selection yields no assumed alpha or DPS.
Its result carries the source evidence for the diagnostic outcome. A present array retains
slots with unavailable actions. An unavailable whole action array has no addressable modes.

## Ship scenario

Each equipment comparison has a factory and a crafted loadout. Use the resolved
`loadouts[].ports` tree and the authored port path for the factory loadout. Use the physical
attachment target at `sourceConstraints.attachedItem.targetRecordIndex.value` when available. Each occupied
physical port contributes independently, including identical items installed in two ports.
Specify one selected action per weapon placement and an explicit control group, either
`pilot` or an identified turret. Sum pilot attack values under simultaneous trigger and
continuous nominal fire. Sum each turret separately by its own identifier. Group assignments
are scenario inputs. Sum compatible modeled alpha and DPS per damage channel and as
complete totals when declared channel coverage permits it. Accumulate contributions in
the scenario placement order and sum each complete channel vector in channel-name order. Ship shield capacity is the sum
of `maxHealth` for installed shield generator placements. Treat each placement independently
and use its own qualities. Range is a per-mode metric. Separate factory and crafted
configurations can select different parts. Expose missing contributions as partial or
unavailable; a known subtotal can still be displayed with its state.

## Fixtures and importer changes

[Catalog combat quality v1 fixtures](examples/catalog-combat-quality-v1.json) have independent
`fixtureVersion`, `formulaVersion` and `catalogSchemaVersion`. Each case carries Build identity
(`branch`, `version`, `changeNumber`, `buildId`), synthetic part and Blueprint GUIDs, tier
and slot positions, qualities and expected factory/crafted values. The fixtures use
normalized input objects after the Catalog joins. A crafted placement's explicit `qualities`
array replaces that part's fixture-level array. Comparisons require equal statuses and exact
integers. Numeric comparisons use the fixture's absolute or relative tolerance, whichever
limit is larger. The P4KX reference parity test reads the published fixture unchanged.

For a v2 importer, replace nested Blueprint recipe data with the physical
`definitions.blueprints` and `configurations.blueprintTiers` joins. Read costs and modifiers
from the tier and source arena. Derive saved quality keys from original positions.
The new weapon factory projection is an additional v3 dataset. Join its physical owner and
mode to the typed action and ammo rows for authored inputs and definitive source uses.
v3 uses per-value encryption and completeness markers so each result can be filtered
independently. See the [v3 catalog layout](catalog.md), [Blueprint contract](catalog-blueprints.md)
and [formula and projection ADR](adr/0388-versioned-crafting-quality-and-weapon-base-values.md).
