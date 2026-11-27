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
own states. Ship inventory `projections.shipBaseFacts.rows[].cargo.scu` and quantum-drive fuel requirements
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

## Installed SDK parity

The ignored `installed_resource_supply_sdk_matches_one_complete_catalog` test
compares all five resource-supply configuration tables, including
`resourceComponents` and `resourceConnections`, with a completed Normal
Catalog from the same Archive. It compares the ordered rows and availability
and resolves their recursive source references. Configure the installed
Archive and key file, the retained measurement directory and the exact
producer revision.

```text
cargo test --locked -p p4kx-export --test real_archive real_archive_resource_supply_sdk::installed_resource_supply_sdk_matches_one_complete_catalog -- --ignored --exact --nocapture --test-threads=1
```

The environment variables are `P4KX_DATA_P4K`, `P4KX_KEY_FILE`,
`P4KX_CATALOG_MEASUREMENT_DIR` and `P4KX_RESOURCE_SUPPLY_PRODUCER_REVISION`.

On PTU `13260598`, a Windows AMD64 debug replay matched a retained complete
Normal export from producer `300226ceeeaf522b7d9af790e8fa3698752cf575`. It checked
372 tanks, 159 intakes, 2,095 containers, 4,407 components and six connection
rows, with 964,078 contributing source uses linked to one physical source.
The full-frame batch replay took 4,409.613 seconds, peaked at 5,447,360,512 bytes
of private commit and 5,296,955,392 bytes of working set, and left at least
33,363,488,768 bytes of system RAM available. Recorded scratch usage was zero.
The 2 GB SDK target is measured on standalone individual queries.
