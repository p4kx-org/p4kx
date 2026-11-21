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

`placements.installedConfigurations` links physical configurations to their
resolved Loadout positions. Match `source.position.value.loadoutRecordIndex` to
`loadouts[].sourceRecordIndex.value` and follow `source.position.value.portOrdinals`
through the nested `ports` arrays. An empty path addresses the vehicle itself.
The same component placed twice yields two rows. An invisible
port can carry a row. A docked vehicle has its own root and does not contribute
its child configurations to the host. Missing or ambiguous selected port
targets do not fabricate placements. Positions are scoped to the exported
Loadout and Build. The stored values support inspection of configuration,
and movement predictions require independent runtime evaluation.

## Checking an installed archive

Set `P4KX_DATA_P4K` to the installed archive and use the usual real-archive
key configuration. Set `P4KX_FLIGHT_CONFIGURATION_WITNESS` to a private
UTF-8 selector file with seven lines in this order.

1. Spacecraft Record path.
2. Record path of the flight-controller item installed on that spacecraft.
3. Ground vehicle Record path.
4. Record path of an installed main thruster.
5. Authored role enum label of that thruster.
6. Record path of an installed maneuvering thruster.
7. Authored role enum label of that thruster.

Omit `.xml` from Record paths. Choose the parts from the spacecraft's default
Loadout. The test checks the original component fields and their provenance
and compares both vehicles' exported Loadouts to the original resolutions.
The synthetic placement tests exercise repeated, invisible and docked ports.

```powershell
cargo test -p p4kx-export --test real_archive --locked installed_flight_configurations_match_original_components_and_loadouts -- --ignored
```
