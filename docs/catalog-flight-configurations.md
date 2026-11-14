# Flight-control configurations

Catalog v3 publishes the original physical component occurrences as
`configurations.flightControllers`, `configurations.ifcs` and
`configurations.thrusters`. The key is `ownerRecordIndex` plus
`componentOrdinal`, with the nominal `ownerGuid` also available. Entries
selected by their original Record path are independent of item stat blocks.
Source declaration availability stays separate from the number of emitted
rows. The tables retain immediate per-value states and original source
references. `additionalFields` retains other original member occurrences.

| Original component | Contract |
| --- | --- |
| `SCItemFlightControllerParams` | Original operational flags, warning limits, weak interaction selectors, HUD and effect references, collision and recall configurations. |
| `IFCSParams` | Stored speed, acceleration, angular axes, jerk, proximity, authored movement limits, two separately stored afterburner blocks, atmospheric configurations, no-fuel behavior, fuel and thruster-efficiency fields. |
| `SCItemThrusterParams` | Both original thrust capacities, original role and animation labels, fuel burn, gimbal and ordered interaction overrides, signature and integrity curves, and backwash values. |

The `ifcs` row uses `limits`, `precision`, `dynamics`, `environment` and
`systems`. The direct original names can be located within those groups.
Named nested configuration members retain their original typed values or
an independent original field arena for complex subtrees such as HUD
messages, atmosphere effects and rotation modifiers. The arena preserves
ordered Original values, raw localized keys in the protected text domain,
and authored references with their resolution states. Vectors have original
named coordinates. Authored curve knots retain their order and their exact
source numbers. Single precision numbers expose finite values and raw bits
without converting units, evaluating curves or selecting between separate
legacy and current values. Fuel members with a stored resource-unit type
retain that unit identity and unconverted numeric payload.

`placements.installedConfigurations` links the exact physical configurations
to the existing resolved Loadout position. Empty `portOrdinals` refers to the
vehicle itself. The same component placed twice yields two rows. An invisible
port can carry a row. A docked vehicle has its own root and does not contribute
its child configurations to the host. Missing or ambiguous selected port
targets do not fabricate placements. Positions are scoped to the exported
Loadout and Build. The stored values support inspection of configuration,
and movement predictions require independent runtime evaluation.
