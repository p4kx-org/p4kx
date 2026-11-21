# Jump-drive item configurations

`configurations.jumpDrives` contains one row per physical `SCItemJumpDriveParams` component. Join `ownerGuid` to the item and use `ownerRecordIndex` with `componentOrdinal` to distinguish physical occurrences. Source values keep their original storage and units. Each value uses the catalog's sourced-value contract.

| Catalog field | DataForge field |
| --- | --- |
| `alignmentRate`, `alignmentDecayRate` | `alignmentRate`, `alignmentDecayRate` |
| `tuningRate`, `tuningDecayRate` | `tuningRate`, `tuningDecayRate` |
| `fuelUsageEfficiencyMultiplier` | `fuelUsageEfficiencyMultiplier` |
| `idleState`, `transitingState` | Weak pointers to `ItemResourceState` |
| `flightTuning` | `flightTuning`, a Record Reference to `JumpDriveFlightParams` |
| `tunnelForces` | `tunnelForces`, a Record Reference to `JumpTunnelForcesParams` |
| `effects` | `jumpDriveEffectParams` |

Join a present reference's `targetRecordIndex.value` to `source.recordIndex.value` in
`configurations.jumpDriveFlightTuning.rows` or `configurations.jumpTunnelForces.rows`. The
owner-local `fields` arena preserves the source order and storage of every definition field.
This includes flight rotation, linear, steering, afterburner and turbulence groups, and
angular and linear tunnel limits, decay, force switches and the original correction curve.
A filtered export includes definitions selected directly by the filter or linked by selected devices.

The effects group retains `exitParams`, `openingParams`, `travelParams` and `wakeParams` as
`exit`, `opening`, `travel` and `wake`. Ranges contain the original minimum and maximum.
The original declarations `projectileEffect`, `tunnelResponseEffect` and `jumpDriveParticles`
retain their complete nested source values in the shared field arena. This includes authored array
order, links, and particle paths. Additional fields in each group retain their original typed
values and storage metadata.

These rows describe a device's stored configuration. World endpoints and routes, quantum-drive
travel modes and the shared resource and durability tables have their own owners.
