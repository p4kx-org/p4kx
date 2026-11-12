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

## Ship Loadout placements

`placements.facilityInstances` links physical facility and inventory configurations to their
resolved ship contexts. Each row carries the root ship GUID and physical Record index.
The `loadout` source uses `position.value.loadoutOrdinal` and ordered
`position.value.portOrdinals` to navigate the same published Loadout. An empty port path
addresses the root. Attached rows carry the selected physical Entity Record and the original
port coordinates when a port definition is available. Each path retains the observed port
and attachment contributors from the root to the component's owner.

Join `configuration.owner` by Record index and component ordinal. The `facility` kind
addresses `configurations.facilities`. The `inventory` kind addresses
`configurations.inventoryConfigurations`, whose stored selector leads to
`definitions.inventoryContainers`. Components at separate port paths remain separate
instances even where they share an owner. The resolver keeps docked vehicles in their own
root contexts. The catalog selection determines which configuration rows can be joined.
Failed attachment resolution stays visible in the published Loadout.
A loadout path identifies a contextual occurrence. Spatial transforms and placements found
only in interior object containers require their own source observations.
