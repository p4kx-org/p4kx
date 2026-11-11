# Ship insurance is a field of the catalog ship, read from its own Record

Decided for issue #323. Builds on ADR-0002, ADR-0054 and ADR-0065.

## Decision

- **The insurance data is the field `insurance` of every entry of `ships`,** not a kind of its
  own. The issue names a `ShipInsuranceRecord` with an `allShips` list; the Archive has no such
  type any more (LIVE). The data sits in the ship's own `EntityClassDefinition`, in the
  `SEntityInsuranceProperties` of its `StaticEntityClassData`, so the ship's `recordPath` is the
  source path and no second Record, link or ordering is needed.
- **The object holds the raw facts under their data names:** `restoreOnClaim`,
  `shipEntityClassName`, `baseWaitTimeMinutes`, `mandatoryWaitTimeMinutes` and `expediteFee` (the
  Record's `baseExpeditingFee`), each `null` when the Record states none. The object is `null`
  when the Record has no `SEntityInsuranceProperties`. What the names mean (claim time, shortest
  claim time, restore with the Loadout) is inferred from them; the Archive states no meaning.
- **`displayParams` stay out:** they repeat the ship's name, role, career, crew size and
  manufacturer, mostly as placeholders, and are game text.
- **No claim fee is exported.** The Records hold multipliers (`itemRecovery`) and one fee in
  aUEC per ship (`baseExpeditingFee`, exported as `expediteFee`), and the `EntityClassDefinition`
  Records of ships and items hold no field named like a price. A formula over the exported
  facts (the ship's fee, wait times and base data, and the multipliers) was tried against two
  in-game claim fees (#778); none reproduces both, so none is documented, and ADR-0065 forbids
  exporting a value that the data does not hold.
- **The recovery multipliers are the object `itemRecovery`,** not a kind: the
  `ItemRecoveryConfigurationParams` Record under `entitlementpolicies/`, its `economyParams`
  under their data names, with the override groups and the GUIDs of their entity classes. The
  Record is selected by a `KindRule` like the crafting global parameters, honours the Filter, and
  with several readable Records the smallest record path wins. A Record that cannot be read is
  left out and listed in the failures of the build, so `null` with a failure differs from
  `null` without one.
- **It is part of the unreleased `schemaVersion` 2,** with no bump.

## Consequences

- On LIVE all 1,066 catalog ships carry `insurance` with a fee.
- The Record is optional: an Archive without it gives `itemRecovery` `null`, and the choice
  follows ADR-0065, which exports what a Record states and nothing it does not.

## Catalog schema 3 recovery

The selected physical `ItemRecoveryConfigurationParams` Record supplies three configuration
tables under ADR-0356. The economy row retains the canonical Record header. Override and
policy rows use that Record's GUID and physical index. This keeps the original selection
rule while preserving each authored array, repeated condition and independent field.

The economy separates fields declared directly in older sources from the nested cooldown,
delivery, repair and escalation groups. Shared names keep their source-specific meanings.
The catalog stores numeric words and authored references under ADR-0357. Notification locale
fields use the protected game-text catalog with the same per-value provenance. Ship insurance
continues to originate in each ship's own Record.
