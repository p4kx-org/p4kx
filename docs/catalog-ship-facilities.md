# Ship facility configurations in catalog schema v3

The `configurations.facilities` table contains original facility components
of selected Entity Records. The `seat` kind identifies `SCItemSeatParams`.
Use `ownerRecordIndex` and `componentOrdinal` to distinguish physical
components. `ownerGuid` supplies the authored Entity identity for joins.
Rows use the same source carriers and encryption markers as other component
tables.

| Original source | Catalog field |
| --- | --- |
| `SCItemSeatParams.seatType` | `seatType`, original `SeatTypes` option |
| `availableOperatorModes` | `availableOperatorModes`, a sourced `OperatorModeAvailabilityParams` Reference with resolution status |
| `controlTemplate`, `transitionTemplate` | Original strings |
| `minYaw`, `maxYaw`, `minPitch`, `maxPitch`, `setYawPitchLimits` | Authored Singles and flag |
| `actorAttachment` | Inline `SCSeatActorAttachmentParams` with `type`, `boneName` and `stance` |
| `ejection` | Original nullable StrongPointer to `SCItemSeatEjectParams` |

The ejection value carries `maxLinearVelocity`, `maxLinearAcceleration`,
`maxAngularVelocity`, `maxAngularAcceleration`, `ejectionLoopTime` and
`ejectionInteraction`. The interaction retains the original weak target and
its physical owner or instance path. `offsetAngles` retains the original
`Deg3` coordinates `x`, `y` and `z`. Singles carry their finite value when
available and the exact IEEE bits in `rawBits`.

Each owned source block has its own `additionalFields` arena. The seat
arena includes authored animation databases and contexts, transition flags,
camera and interaction declarations, head adjustment references and
ADS audio triggers. Member order, source types, array slots and references
are retained in the arena. A missing field, null pointer, unreadable field
and present zero or false have distinct source states.

## Medical facilities and tier definitions

The `medBed` facility kind retains the physical `MedBedComponentParams` owner and
component ordinal. The original `MedBedTier` label, treatment and delay Singles,
respawn and invulnerability flags, and the optional signed `respawnRangeOverride`
are separate source values. Authored interaction, point, use-channel and surgery
state WeakPointers retain their actual record or owned-instance target. Surgery
labels use the protected game-text domain.

The sourced `medicalItemTierConfig` Reference can join to the physical
`definitions.medicalItemTierConfigs` row by Record index and GUID. Each definition
contains the ordered `medBedConfigs` and `statTypesToHeal` slots. Drug efficacy
and available drug entries retain their concrete source types and original members
in the authored value arena. The separate resource consumption block retains its
`bedResource` Reference and surgery T1–T3 and cloning amounts.

Regeneration and resource costs use the shared cargo-unit representation. Stored
values retain their own source units, exact Single bits and signed integer amounts.
The ranges and timing fields retain the original scalar values.

## Cargo grid facilities

The `cargoGrid` kind carries the physical `SCItemCargoGridParams` component.
`dimensions` retains the original `Vec3` Single coordinates and their IEEE bits.
`crateGenPercentageOnDestroy`, `crateMaxOnDestroy`, `invisible`, `miningOnly`,
`transformDependant` and `minVolatilePowerToExplode` retain their source values.
`damageConfiguration` carries the original `CommodityDamageConfiguration`
Reference with target identity and resolution status. The component's
`additionalFields` arena retains other authored members.

`capacity` links the grid to the sole item-storage component on the same physical Entity.
`storageComponentOrdinal` gives that component's original slot, and
`containerRecordIndex` addresses its unique `InventoryContainer` Record in
`definitions.inventoryContainers`. Both are sourced values or `null` when the identity
cannot be established. For `status: available`, `scu.value` is capacity in standard cargo
units, rounded to four decimals. Open containers use the product of their positive finite
`interiorDimensions` in metres divided by 1.953125. Closed containers use the stored
nonnegative finite amount in standard, hundredth or millionth SCU. The grid's own
`dimensions` are a separate authored field. The derived value's source uses identify the
component and Reference selectors, the container type and the actual dimension or unit
operands. [ADR-0395](adr/0395-cargo-grid-capacity-uses-physical-inventory-bindings.md)

The other status values identify unreadable or ambiguous owner components, missing storage,
a missing, unresolved or ambiguous container Reference, a container outside the selected
Records or otherwise unavailable, or missing, unreadable, unsupported or invalid capacity.
The sourced `scu` follows the corresponding absence or failure state.

For a Loadout, join each occupied port's facility configuration to its Cargo Grid and count
each physical port occurrence. A readable `miningOnly.value` of `false` identifies capacity
to include in a commodity total. A value of `true` identifies mining-only capacity. Report
the capacity or classification as unknown when its corresponding value is unavailable.
`invisible` remains a separate authored flag. A complete commodity total requires a classification
and readable capacity for each eligible grid. A Loadout attachment without a matching facility
remains unclassified. ShipBase and `projections.shipBaseFacts.rows[].cargo.scu` retain readable
capacity from both flag values. [Ship base data](ship-base-data.md#cargo)

An occupied Loadout attachment with a `ResourceContainer` component supplies its own
capacity and resource restrictions in `configurations.resourceContainers.rows`. This is
the authored storage used by the mining pods inspected in the installed Builds. Join by
`loadouts[].ports[].sourceConstraints.attachedItem.targetRecordIndex.value` and
`ownerRecordIndex.value` to follow each occupied port. Such a pod is separate from the
inventory cargo sum. An empty Cargo Grid port contributes no capacity.
[Resource container and Loadout contract](catalog-fuel-capacity.md#loadout-instances)

## Logout and outfit storage facilities

The `logout` kind identifies `LogoutComponentParams`. Its ordered
`logoutInteractions` retains original StrongPointer slots, including null
slots. Each populated slot carries its logout and respawn interaction and
point WeakPointers with their target coordinates. The component declares a
logout capability. A bed role needs an independently authored source.

The `outfitHanger` kind identifies `SEntityComponentOutfitHangerParams`.
It retains the equip, hang, swap and disguise interaction WeakPointers,
ordered `itemPortTypeSubtypes`, the concrete animated swap and hang
payloads and `destroyPlayerItems`. This component's restrictions and the
owner's individual port restrictions are separate declarations.

For personal-weapon holders, `configurations.itemPorts` exposes the original
owner and port ordinal and the accepted type `WeaponPersonal` in
`ports.value[].value.restrictions.types` where declared. The same port row
retains subtype slots, size bounds, tags and rules. This is a port
capability. Physical port counts come from the authored port slots.

## Ship Loadout placements

`placements.facilityInstances` links physical facility and inventory configurations to their
resolved ship contexts. Each row carries the root ship GUID and physical Record index.
For a `loadout` source, match `position.value.loadoutRecordIndex` to
`loadouts[].sourceRecordIndex.value`, then follow the ordered
`position.value.portOrdinals` through the nested `ports` arrays. An empty path addresses
the root. Attached rows carry the selected physical Entity Record and the original
port coordinates when a port definition is available. Each path retains the observed port
and attachment contributors from the root to the component's owner.

Each occupied port with a linked configuration has one row. Its `configurations`
array follows original component order. Join each `configurations[].owner` by Record index and component
ordinal. The `facility` kind addresses `configurations.facilities`. The `inventory`
kind addresses `configurations.inventoryConfigurations`, whose stored selector leads
to `definitions.inventoryContainers`. Separate port paths retain separate rows even
where they share an Entity. Docked vehicles retain their own root contexts. The
catalog selection determines which configuration rows can be joined.
Failed attachment resolution stays visible in the published Loadout.
A Loadout path identifies a contextual occurrence.

## SOC room placements

The `interior` source in `placements.facilityInstances` joins an original
`EntityClassGUID` from a streamed SOC to a uniquely identified physical
DataForge Record. The resulting row links that Record's published facility and
storage components. Entity slots that supply their own geometry or have an
invalid transform can still carry a class and its configuration links.

For a `vehicle` room, join `source.room.owner` and `objectContainerOrdinal` to the
physical component and array slot in `declarations.roomAssociations`. For an `item`
room, the owner identifies the scalar `objectContainer` in
`declarations.itemRoomAssociations`. That declaration appears once per original
component. `source.room.loadoutPosition` uses the same physical root and ordered port path
for each item-room occurrence. Its value is null for a room declared directly on the ship.
The sourced `source.position` identifies the
actual containing SOCpak, descriptor and SOC Entry, the descriptor's original OC
node, the CrCh chunk-table index and the Entity's original direct element slot.
Repeated OC declarations keep separate positions even when they use one SOC Entry.

`source.entityAttributes` retains authored names and values in source order.
Binary XML provenance locates the original chunk, node and attribute-table
indexes. Text XML provenance locates the original lexical byte span within the
decoded SOC Entry. `localTransform` is a row-major 3x4 transform in the containing
room's coordinates, with a distinct invalid state for malformed or ambiguous inputs.
The original attributes are available in either case.

`placements.facilityInteriorCoverage` records opened `containers` and findings
for each inspected room slot. A facility joins its containing occurrence using
the `source.room` key and `source.position.value.containerOccurrenceOrdinal`.
The root occurrence has a null `parentOrdinal` and `child`. Each nested occurrence
references its parent and retains the original Child descriptor node, raw
attributes, selected container Entry and local matrix. The lookup searches the
current container, enclosing containers and then the main Archive. Repeated
sibling declarations remain separate. A cycle reports the original Child
occurrence while leaving sibling routes available.

`rootRoomTransform` expresses a nested entity in the root room's coordinates
using the ordered Child matrices and the entity-local matrix. It is null for a
direct entity. An invalid Child matrix leaves the local entity and its original
attributes available and marks the composed matrix invalid. The attached item's
resolved Loadout path identifies the room occurrence.

`shipFrameTransform` is a sourced 3x4 matrix from the Entity's local space to
the ship frame. For a direct vehicle room with no named helper, it composes
the complete stored `Offset.Position` in metres and `Offset.Rotation` as
X/Y/Z degrees with the Child chain and Entity matrix. A scalar room declared
on the ship uses the ship Entity's origin. A named helper on the direct
ship model uses its original node and parent matrices. It uses the
authored offset when the helper is within Assembly's 5 cm origin
tolerance. A missing or ambiguous helper yields `absent` or `invalid`
respectively. Character-definition indirection and attached item placement
yield `unsupported`. The helper lookup covers model nodes and their ancestors.
An omitted offset, malformed coordinate or unreadable input has its own
source state. The authored offset remains in the original declaration.

Findings identify the affected `containerOrdinal`, the original stream name
when applicable and the Child source for an unresolved edge. Missing,
ambiguous or unreadable source paths have separate codes. A descriptor with
repaired string-table bytes is unreadable for selection. A repaired SOC
entity chunk records `lossyEntityChunk`. Its usable slots retain their original
coordinates and incomplete source sets.

## Installed placement checks

Set `P4KX_DATA_P4K` to the installed Archive and run the two ignored tests.

```text
cargo test --locked -p p4kx-export --test real_archive ship_facilities_match_original_placements -- --ignored --nocapture --test-threads=1
```

The tests select a small ship and a ship with multiple physical rooms from
original room and facility declarations. They report the selection scope and
trace exported placement and configuration rows back to original Entries,
Entity slots, transforms and component ordinals. Nested facility evidence is
checked when the bounded sample contains it. A publication refusal caused only
by malformed ShopInventory inputs is reported separately. In that case the
tests verify placement rows in the diagnostic export.

## Installed cargo capacity checks

Set `P4KX_DATA_P4K` to the installed Archive and use the configured Archive key. Run the
source census for the exact Cargo Grid component and its descendants. It reports declaration
and instance counts. The strict flagged-grid witness accepts `P4KX_CATALOG_CARGO_GRID_FILTER`
and exact suffixless Item paths in `P4KX_CATALOG_CARGO_MINING_ITEM` and
`P4KX_CATALOG_CARGO_CONTROL_ITEM`. The filter must admit both Items and their inventory
container Records when the Build contains both flag variants.

For the separate mining-pod source, set `P4KX_CATALOG_MINING_SHIP` and
`P4KX_CATALOG_MINING_POD` to the original suffixless Record paths.
`P4KX_CATALOG_MINING_FILTER` can select both in one scoped catalog.
`P4KX_CATALOG_MINING_CONTROL_SHIP` selects an ordinary Cargo Grid ship to check its
legacy capacity against original container operands. The test streams each scoped
catalog into the verifier.

```text
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_cargo_grid_capacity::installed_grid_flag_source_census
cargo test --release --locked -p p4kx-export --test real_archive -- --ignored --exact --nocapture real_archive_mining_storage::installed_mining_storage_stays_outside_ship_cargo_scu
```
