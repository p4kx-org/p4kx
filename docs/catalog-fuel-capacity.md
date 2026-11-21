# Fuel and resource capacity in catalog schema v3

The catalog exports each physical component once. Join a row by
`ownerRecordIndex` and `componentOrdinal`. Its `ownerGuid` retains the
original Entity GUID. Component rows follow physical Record and slot order.

| Dataset | Original component | Projected fields |
|---|---|---|
| `configurations.fuelTanks` | `SCItemFuelTankParams` | Hydrogen and quantum maximum flow multipliers, open/closed/pumping state links, flow loop audio triggers, fill/drain rate and fill level RTPC declarations |
| `configurations.fuelIntakes` | `SCItemFuelIntakeParams` | `fuelPushRate` and `minimumRate` |
| `configurations.resourceContainers` | `ResourceContainer` | Capacity, inclusive and exclusive resource and group links, ordered default composition, fill factor, immutable flag, mutability level, random quality flag and visibility flag |

Every typed field carries its original source status and encryption marker.
Singles retain their IEEE bits and expose a finite decimal when available.
The ordered composition retains the source `entry` link and `weight` at each
slot. Dangling entries retain the authored GUID and its resolution status.
Additional declared members, including quality fields when present, appear in
`additionalFields` with their own physical positions and source evidence.

## Stored capacity units

The `capacity` cell reads the original StrongPointer to `SBaseCargoUnit`.
The concrete unit controls the shape of its present value.

| `kind` | Original concrete type | Stored amount |
|---|---|---|
| `base` | `SBaseCargoUnit` | Original base type and additional members |
| `centiCargoUnit` | `SCentiCargoUnit` | Signed Int32 `centiSCU` |
| `microCargoUnit` | `SMicroCargoUnit` | Signed Int32 `microSCU` |
| `standardCargoUnit` | `SStandardCargoUnit` | Exact Single `standardCargoUnits` |
| `unknownUnit` | Another admitted subtype | Original concrete type and authored member arena |

The numeric amount retains its original scale and sign. A stored zero remains
present. Absent declarations, null pointers and unreadable fields have their
own states. Ship inventory `cargoScu` and quantum-drive fuel requirements
keep their existing domain and source.

## Loadout instances

Each resolved `loadouts[].ports[]` attachment exposes its physical
`sourceConstraints.attachedItem.targetRecordIndex` alongside `targetGuid`.
Join that index to the owner index in the configuration tables. The existing
recursive port path and parent maintain separate instances even when several
ports select the same configuration. A filter can leave a selected attachment
outside the configuration tables. The resolver stops at docked vehicles.

## Commodity authoring

`definitions.commodities.rows[].entity.value.resourceContainers` keeps the
authored container slots as links. Join `ownerRecordIndex` and
`sourcePosition` to `ownerRecordIndex` and `componentOrdinal` in
`configurations.resourceContainers.rows`. This v3 draft shape places the
original `defaultComposition` under the canonical configuration row.
Commodity presentation still selects the first non-null entry in the first
matching container and retains that selection if its target is dangling.
