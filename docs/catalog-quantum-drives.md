# Quantum drive configurations

`configurations.quantumDrives.rows` contains one row per physical
`Components[SCItemQuantumDriveParams]` occurrence on a selected item. Join `ownerGuid.value`
to the item GUID. `ownerRecordIndex.value` and `componentOrdinal.value` identify the original
slot even when an owner has multiple drives. `sourceType` retains the concrete source name.

| Row field | Original field | Stored meaning |
| --- | --- | --- |
| `fuelRequirement` | `quantumFuelRequirement` | Raw fuel requirement |
| `jumpRange` | `jumpRange` | Metres |
| `disconnectRange` | `disconnectRange` | Metres |
| `standard` | `params` | Independently declared parameter Class |
| `spline` | `splineJumpParams` | Independently declared parameter Class |
| `heat` | `heatParams` | Authored thermal energy inputs |
| `boost` | `quantumBoostParams` | Authored boost inputs when the Build provides them |

A parameter Class retains its actual `sourceType`, drive speed, cooldown, spool, the two
acceleration stages and engage speed. It also retains calibration rate, minimum and maximum
calibration requirements, process and warning angles, calibration delay, interdiction effect time,
all VFX and shader thresholds, and independent `idleState`, `travellingState` and `effectTags`
subtrees. The parameter keys match the authored field names in lower camel case. `driveSpeed` and
`engageSpeed` are metres per second. `cooldownTime`, `spoolUpTime` and
`calibrationDelayInSeconds` are seconds. Other quantities retain their original representation
and units.

The heat block stores `preRampUpThermalEnergyDraw`, `rampUpThermalEnergyDraw`,
`inFlightThermalEnergyDraw`, `rampDownThermalEnergyDraw` and `postRampDownThermalEnergyDraw`.
These are energy inputs as authored. The boost block stores `maxBoostSpeed`, `timeToMaxBoostSpeed`,
`boostUseTime`, `boostRechargeTime`, `stopTime`, `minJumpDistance`, `ifcsHandoverDownTime` and
`ifcsHandoverRespoolTime` where declared. Each block also preserves its other original members
and type identity.

Cells use the shared v3 source states. An absent block carries its own declaration survey. A
present empty Class has a source type and absent fields. A null or unreadable occurrence has an
explicit state and null typed value. A mismatched declaration produces a structured projection
diagnostic and the original value remains in `additionalFields` in diagnostic publication.
Additional readable fields retain their original declarations, order and payloads in the owner-local
arena. The state and effect subtrees use the same arena so References, ordered elements and
source metadata remain available across underlying declarations.

Numeric cells preserve their original type and exact representation. A floating-point value carries
`rawBits` and a nullable finite decimal projection. Zero, signed zero, `f32::MAX`, infinities and NaN
payloads retain their source bits. The schema stores the two modes independently. Fuel, acceleration,
travel time and efficiency calculations belong to consumers.

For consumers migrating from the draft flat `items[].stats.quantumDrive`, the three root values
map to the same-named row cells. The former `driveSpeed`, `cooldownTime`, `spoolUpTime`,
`stageOneAccelRate`, `stageTwoAccelRate` and `engageSpeed` map to `standard.value`. Select the
actual component slot before presenting an item-level value. A missing standard mode remains absent.
