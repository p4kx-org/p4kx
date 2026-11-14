# EMP configurations

`configurations.emp.rows` contains one row per physical `Components[SCItemEMPParams]`
occurrence on a selected item. Join `ownerGuid.value` to the item GUID. The
`ownerRecordIndex.value` and `componentOrdinal.value` pair identifies the original slot,
including repeated components. `sourceType` retains the exact original declaration.

| Row fields | Original fields and storage |
| --- | --- |
| `chargeTime`, `unleashTime`, `cooldownTime` | The three separate timing Singles |
| `empRadius`, `minEmpRadius`, `physRadius`, `minPhysRadius` | The four separate radius Singles |
| `distortionDamage`, `pressure` | The authored damage and pressure Singles |
| `idleState`, `chargingState`, `chargedState`, `releasingState` | The four WeakPointers to their original interaction state targets |
| `chargingParticle`, `chargedParticle` | `ChargingParticle`, `ChargedParticle` inline Classes with `sourceType`, `path` and original extra fields |
| `chargingTag`, `chargedTag` | `ChargingTag`, `ChargedTag` References |
| `startChargingTrigger`, `stopChargingTrigger`, `startChargedTrigger`, `stopChargedTrigger`, `startUnleashTrigger`, `stopUnleashTrigger` | The six separately authored start/stop References, with the original names capitalized |

Numbers retain their source units and bit representation. Each Single supplies `rawBits` and a
nullable `finite` decimal projection. Each value and selector has its own source-use references
and archive encryption marker. An empty original particle path remains a present empty string.
Weak links identify a Record or owned instance, and original References retain their target GUID
and resolution status. The shared v3 states distinguish absent fields, explicit nulls, readable
incompatible declarations and unreadable data.

`additionalFields` preserves other readable source members in their original order, including
a readable field whose typed projection failed in diagnostic publication. Resource wiring and
shared durability are available through their corresponding configuration tables and the same
physical owner identity.


The ignored `both_installed_archives_match_original_emp_configuration` test compares a
path-scoped export with the original EMP component, field origins and link targets in each
configured archive.
