# Mining globals and refining processes

The canonical v3 datasets are `definitions.refiningProcesses` and
`configurations.miningGlobal`. Each is a typed table with `availability` and `rows`.
The root arrays `refiningProcesses` and `miningGlobalParams` are removed. Schema version
remains 3; P4KX emits no v2 compatibility document.

## Physical owner and original values

Every row flattens the sourced physical Record header: `guid`, `recordName`, `className`,
`recordPath`, `recordIndex`, `sourceType`, `fileName` and `metadataTag`. Equal GUIDs retain
separate physical rows in textual GUID order, with physical index breaking ties. Authored
reference resolution remains separate from output selection. Join within the same Build and
physical source inventory; a nominal GUID alone does not disambiguate duplicate declarations.

`fields` contains the complete original owner-local `SourceArenaOutput` (`roots`, `values`).
It includes projected fields and all other declarations, including effects, sound and default
explosion content omitted by the legacy projection. The arena retains exact declarations,
ordered/repeated/null occurrences, original numeric width, string/Locale text domain, references
and original errors. Its local links resolve through that row's arena. References retain target
headers without opening target bodies. The arena is a lossless source representation, not an
engine interpretation or a reference-target materialization.

Each projected datum has a direct `SourcedValue` state, immediate source uses and encrypted-source
marker. Resolve uses through the returned snapshot's own `source_inventory()`, then follow its
physical source/ancestor/proof views. Plain, encrypted and unknown remain separate. Protected text
domain and input encryption are independent facts.

## Refining

`processName` is a protected `GameTextRef` into `texts.gameTexts`. Its original Locale key,
including any `@` token, and available source/display translations retain their evidence.
`refiningSpeed` and `refiningQuality` are independently sourced open String/Enum labels.
Original spelling and present empty values remain exact; a new readable label is not a malformed
exporter control. P4KX does not infer yields, duration or numeric speed multipliers from these labels.

## Mining globals

The 26 numeric root fields keep their original names, including the exact `cSCUPerVolume` spelling:

```
absorbableVolumeThreshold, cSCUPerVolume,
childRockInactivityLifetime, childRockInvulnerabilityTime, childRockLifeTimer,
childRockZeroGDamping, compositionMassMultiplier,
controlledBreakingDecayRate, controlledBreakingFillRate, controlledBreakingFillRateDanger,
dangerBreakingDecayRate, dangerBreakingFillRate, dangerBreakingFillRateExponent,
dangerToGadgetDamage, decayPerMass, defaultMass,
gadgetDestroyThreshold, gadgetDetachThreshold, modifierPersistenceTime,
optimalWindowFactor, optimalWindowMaxSize, optimalWindowSize,
optimalWindowThinnessCurveFactor, powerCapacityPerMass, resistanceCurveFactor,
terrainFactorStaticThreshold
```

Three independently sourced owned blocks retain all 11 typed child numbers:

| Block | Children |
| --- | --- |
| `hitConsistencyParams` | `extractionMagnitude`, `hitHistoryWindow`, `maxEffectOnInstability`, `minDeviation`, `standardDeviationMultiplier`, `timeExponent` |
| `mineableInstabilityParams` | `instabilityCurveFactor`, `instabilityWavePeriod`, `instabilityWaveVariance` |
| `mineableExplosionParams` | `dangerPoolFactor`, `defaultVolume` |

Every number is `SourcedValue<ResourceNumber>`. Int64/UInt64 use exact decimal strings;
Single/Double preserve IEEE bits and their width-correct nullable finite representation.
Recover an exact Single from `rawBits` before promoting it to binary64. Its finite JSON convenience
uses the normal shortest Single decimal. Legal NaN/infinity and signed zero remain source facts;
no narrowing, clamping, integer conversion or zero substitution is performed.

The existing source contract identifies `childRockInactivityLifetime`,
`childRockInvulnerabilityTime`, `childRockLifeTimer` and `modifierPersistenceTime` as seconds.
Other values remain in their original source units. P4KX does not infer a mass/volume unit,
percentage, normalized range or conversion from a field name; `cSCUPerVolume` is not rescaled.

`wasteResourceType` is the original sourced reference with retained GUID, physical target header
and resolution status. A missing or incompatible target is not a fabricated null GUID or a reason
to open a ResourceType body. An absent block, null block, present empty block and a failed typed
projection remain distinct. The complete original arena remains available when a readable field
uses an unsupported typed representation; its projection failure still affects health.

## Selective SDK access

Use `p4kx_export::catalog_queries::mining::{with_refining_processes,
with_refining_processes_from, with_mining_global_params, with_mining_global_params_from}`.
`CatalogQuerySession` and its parsed view expose the corresponding methods. Reuse a parsed view
to share the decoded DataForge and indexes. All, GUID and RecordIndex selectors retain the ordinary
Catalog policy and every admitted physical alias. Borrowed rows and text stay inside the callback;
return consumer-owned values when needed afterward.

Only selected owner bodies are materialized. Refining reads requested locales after a readable
owner needs them. Mining-global projection opens no locale payload. Neither endpoint builds a
Catalog, discovers world placements, surveys Resource groups or reads ResourceType bodies.
No fixed quantity quota is introduced. Source declaration availability is distinct from selected
row count. Examine owner/body/field coverage, dependency coverage and sourced states before
accepting a cache update. Publish only after the outer query succeeds, including its final
Archive-change and cancellation checks.

## Migration from the legacy rows

Old headers become sourced header cells. Old refining name key/en/de routes resolve through the
protected text registry; speed and quality use their sourced values. Mining's old root numbers
move to sourced numbers, with `cScuPerVolume` renamed `cSCUPerVolume`. `hitConsistency` becomes
`hitConsistencyParams`; `instability` becomes `mineableInstabilityParams`, whose shortened child
names become their original `instability*` names; `explosion` becomes `mineableExplosionParams`.
`wasteResourceTypeGuid` becomes `wasteResourceType.value.guid.value`, alongside the original target
header/status. Missing declarations retain absence rather than a legacy null shortcut.
The released v2 schema is frozen as migration evidence and is not supported as an output format.

Decision: [ADR0408](adr/0408-canonical-mining-and-refining-tables-preserve-original-owner-data.md).
