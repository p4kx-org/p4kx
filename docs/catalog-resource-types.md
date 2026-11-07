# Resource catalog contract

The catalog uses schema version 3; the additional ResourceTypes kind is independently additive. See [ADR-0352](adr/0352-resources-preserve-authored-facts-and-physical-reference-provenance.md) for the ownership, identity and source-health decisions.

## Commodity identity and resource joins

`commodities[].guid` identifies a commodity Record. `subtypeGuid` is a nullable, non-unique ResourceType GUID. Multiple commodity Records can share one ResourceType. Consumers join ingredients and other `resourceGuid` fields to `resourceTypes[].guid` for resource facts, and to every matching `commodities[].subtypeGuid` when they need commodity alternatives. They must not index commodities uniquely by subtypeGuid or assume a resource has a commodity.

The commodity ResourceType is the first non-null `entry` in the first ResourceContainer's ordered `defaultComposition`. An explicitly configured dangling first GUID is retained; a later valid link does not replace it. A resolved link uses its actual physical target Record, even when another Record has the same persistent GUID. `typeGuid` remains the first selected, readable, non-leftover group in source Record order that directly lists that ResourceType. This existing singular field does not describe every group membership.

The existing ResourceType-backed commodity fallback remains limited to otherwise unrepresented resources used by blueprint costs or kept quality quantizations. It preserves the resource GUID as both `guid` and `subtypeGuid`. It does not determine which ResourceTypes are exported independently. `densityGramsPerCubicCentimetre` remains null for authored kg/m³ or molar-mass sources; no implicit unit conversion changes its meaning.

## ResourceTypes

`resourceTypes` contains every selected, readable ResourceType Record under `resourcetypedatabase`, excluding the declared development-leftover policy, in canonical GUID order. A ResourceType needs no commodity or recipe to qualify.

| Field | Source and contract |
| --- | --- |
| `guid`, `className`, `recordPath` | The ResourceType's persistent GUID, unprefixed Record name and normalized Record path. |
| `name`, `description` | `displayName` and `description` as LocalisedText. Description is game text (`x-gameText`). |
| `groupGuids` | All distinct selected groups that directly list this resource, sorted by GUID. Does not infer transitive membership. |
| `refinedVersion` | The authored ResourceType Record reference, including its configured GUID and source resolution status. |
| `density` | The actual `ResourceTypeDensityType` struct with its `sourceType` and tagged `payload`. |
| `defaultThumbnailPath`, `defaultThumbnailPathSVG` | Raw source strings, preserving empty strings. These fields describe asset paths; cataloging does not extract the images. |
| `rttThumbnailEntityClass` | The authored EntityClassDefinition reference used for the RTT thumbnail. The source Entity need not be a catalog row. |
| `validateDefaultCargoBox` | The nullable authored Bool. `false` is retained. |
| `defaultCargoContainers` | Every named field of the cargo-container source as `{slot, entityClass}`, in source declaration order, including null slots. |
| `properties` | Nullable ordered list of nullable authored properties, retaining duplicate names and values. |

A property holds `sourceType`, nullable raw `name`, and a tagged `payload`: `base`, `volatility`, `craftingData` or `unsupported`. Volatility keeps raw nullable `volatility` and `healthDecayPerSecond`; no clamping, simulation or choice of one property is applied. CraftingData retains three independent nullable sources: `qualityDistribution`, `qualityLocationOverride`, `qualityQuantization`.

The exact cargo slot names are `one_eighthSCU`, `one_quarterSCU`, `one_halfSCU`, `oneSCU`, `twoSCU`, `fourSCU`, `eightSCU`, `sixteenSCU`, `twentyFourSCU` and `thirtyTwoSCU`. The slot label is a source name. It does not assert a container's measured capacity or derive one from mesh or component data.

Density payloads distinguish `density`, `molarMass`, `emptyBase` and `unsupported`. A density's nullable `densityUnit` independently carries its actual `sourceType` and a payload of `gramsPerCubicCentimeter`, `kilogramsPerCubicMeter`, `emptyBase` or `unsupported`. Each concrete unit keeps a raw nullable finite `value`. Molar mass keeps nullable finite `molarMass`; the source declaration supplies no unit label, so no unit is invented. Missing and null values remain null, and authored zeros remain zero.

## Quality source wrappers and exact curves

Each resource quality source retains its actual `sourceType` and a tagged payload: `inline` with a shared `value`, `recordRef` with a nullable `reference`, `emptyBase` for an explicitly known fieldless base, or `unsupported` with blocking health evidence. The wrapper retains ownership and storage form instead of copying a linked Record's values into the owner.

Inline and top-level quality distributions share one QualityCurve. Its nullable `min` and `max` are exact signed 32-bit integers; `mean` and `stddev` are nullable finite 32-bit floats. Every confirmed Normal/Uniform source declaration uses Int32 bounds. A wrong stored bound type fails projection with its exact source type and field path. A nonfinite float, or a finite Double that cannot be represented by a finite f32, also fails projection; it is never silently replaced with null. Known fieldless distribution bases retain their source kind and null parameters; unknown shapes remain unsupported or erroneous with explicit health evidence.

Location overrides preserve nullable ordered `locationOverrideList` entries, repeated locations and null positions. Each entry carries its actual source type and either entry fields (`location`, `qualityDistribution`), `emptyBase`, or `unsupported`. Inline quantizations preserve nullable ordered bands and reuse exact integer `start`, `end`, `mappedValue` payloads. The LIVE concrete-entry and PTU base-entry declarations are both read using the actual owned concrete entry type.

The public Rust QualityCurve API exposes `Option<i32>` bounds and the same shared payload from both existing reexport paths. JSON integer spelling follows the exact bound contract. Schema validation enforces the inclusive range −2147483648 to 2147483647 through explicit minimum and maximum constraints. Frozen schema-v2 compatibility fixtures remain unchanged.

## ResourceGroups

ResourceGroups retain their existing `name` and first-parent `parentGuid`, and add `description`, `defaultThumbnailPath`, `defaultThumbnailPathSVG`, `groups` and `resources`. The two source reference lists are nullable, ordered and preserve repeats and null entries. They retain cycles instead of recursively expanding them. Only selected groups affect legacy `parentGuid`, commodity `typeGuid`, and ResourceType `groupGuids`; additional reads made solely for health diagnostics do not affect these joins.

## Reference source availability and catalog materialization

A DefinitionReference's `present` status means the source reference resolved to a physical Record. It does not promise that a catalog row for that Record exists. Source-only Entity links used by packaging and thumbnails do not create a missing catalog row error.

The existing bounded, cancellable source field inspection captures only required resolved target metadata. Final catalog rows and the existing source observations determine materialization using the actual target Record index. A projected source alone does not prove that a row was emitted. A different emitted Record with the same GUID does not satisfy the link.

Reference health distinguishes `outsideCatalogSelection`, `excludedByFilter`, `excludedByPolicy`, `targetReadFailed`, `targetProjectionFailed`, `targetNotEmitted`, `ambiguousIdentity` and `unexpectedTargetType`. Expected selection exclusions are informational. Read and projection failures retain the existing blocking source failure. Unexplained missing output, wrong target families and ambiguous emitted identities block normal publication. Diagnostic partial publication retains readable rows and the exact evidence.

The additional resolved-reference metadata is limited to 4096 references and 1 MiB of logical metadata per physical source Record. Limit exhaustion produces `fieldWalkLimit` and incomplete field inventory. A limited walk cannot claim a complete empty reference inventory. No extra Record traversal or separate provenance ledger is introduced.

An unreadable Record type is an unknown selection candidate, including under the prefix shared by ResourceTypes and ResourceGroups. It cannot be guessed as the first kind of that prefix. A readable record path excluded by an explicit Filter is excluded before generic type classification.

The internal catalog crafting scope reads unfiltered ResourceType reverse links when a selected non-leftover quantization target needs them; uncertain target metadata is conservative. Without such a target it reads only selected ResourceType inputs for the existing link counter. It does not claim unused out-of-filter resources as dependencies. The public `read_crafting` API keeps its unfiltered ResourceType-link semantics, and a needed out-of-filter dependency failure still blocks healthy catalog publication.

## Source coverage and synthetic verification

The declaration-only ResourceType/ResourceTypeGroup ownership closure has 29 types and 52 declared fields. Every declared field has an explicit owning path and export mapping in the inventory below. The only LIVE/PTU declaration difference in that closure is the location-override-list pointer target changing from CraftingQualityLocationOverrideEntry to CraftingQualityLocationOverrideEntry_Base. These observations concern declarations only and imply no populated Record counts or gameplay values.

The synthetic verification matrix covers both declaration targets, standalone resources and commodity n:1 cardinality, raw units, all cargo slots, zero/false/empty/null preservation, source-only Entity references, exact integer curves, inline and linked quality sources, group repeats/cycles, selected group isolation, physical GUID aliases, dangling-first and null-first composition links, failed dependencies, and bounded metadata. Test pass counts are recorded separately after the corresponding commands run.

## Declaration field inventory

| Declaring type.field | Stored type | LIVE target | PTU target | Owning paths | Export mapping |
| --- | --- | --- | --- | --- | --- |
| CraftingQualityDistributionNormal.min | Int32 |  |  | ResourceType.properties[].qualityDistribution.min OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.min | shared QualityCurve.min (one authoritative exact nullable i32; inline.value.min for resource sources) |
| CraftingQualityDistributionNormal.max | Int32 |  |  | ResourceType.properties[].qualityDistribution.max OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.max | shared QualityCurve.max (one authoritative exact nullable i32; inline.value.max for resource sources) |
| CraftingQualityDistributionNormal.mean | Single |  |  | ResourceType.properties[].qualityDistribution.mean OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.mean | shared QualityCurve.mean (raw finite Single) |
| CraftingQualityDistributionNormal.stddev | Single |  |  | ResourceType.properties[].qualityDistribution.stddev OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.stddev | shared QualityCurve.stddev (raw finite Single) |
| CraftingQualityDistributionUniform.min | Int32 |  |  | ResourceType.properties[].qualityDistribution.min OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.min | shared QualityCurve.min (one authoritative exact nullable i32; inline.value.min for resource sources) |
| CraftingQualityDistributionUniform.max | Int32 |  |  | ResourceType.properties[].qualityDistribution.max OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.max | shared QualityCurve.max (one authoritative exact nullable i32; inline.value.max for resource sources) |
| CraftingQualityDistribution_RecordRef.qualityDistributionRecord | Reference | CraftingQualityDistributionRecord | CraftingQualityDistributionRecord | ResourceType.properties[].qualityDistribution.qualityDistributionRecord OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.qualityDistributionRecord | qualityDistribution.payload.reference (target qualityDistributions; retain GUID/status) |
| CraftingQualityLocationOverride.locationOverrideList | StrongPointer[] | CraftingQualityLocationOverrideEntry | CraftingQualityLocationOverrideEntry_Base | ResourceType.properties[].qualityLocationOverride.locationOverrideList | qualityLocationOverride.payload.value.locationOverrideList (ordered nullable entry sources; LIVE/PTU base widening accepted) |
| CraftingQualityLocationOverrideEntry.location | Reference | StarMapObject | StarMapObject | ResourceType.properties[].qualityLocationOverride.locationOverrideList[].location | qualityLocationOverride.payload.value.locationOverrideList[].payload.location (StarMapObject Reference) |
| CraftingQualityLocationOverrideEntry.qualityDistribution | StrongPointer | CraftingQualityDistribution_Base | CraftingQualityDistribution_Base | ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution | qualityLocationOverride.payload.value.locationOverrideList[].payload.qualityDistribution (shared QualityCurve inline-or-RecordRef source wrapper) |
| CraftingQualityLocationOverride_RecordRef.locationOverrideRecord | Reference | CraftingQualityLocationOverrideRecord | CraftingQualityLocationOverrideRecord | ResourceType.properties[].qualityLocationOverride.locationOverrideRecord | qualityLocationOverride.payload.reference (target qualityDistributions override row) |
| CraftingQualityQuantization.bands | Class[] | CraftingQualityQuantizationBand | CraftingQualityQuantizationBand | ResourceType.properties[].qualityQuantization.bands | qualityQuantization.payload.value.bands (ordered shared QuantizationBand payloads) |
| CraftingQualityQuantizationBand.start | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].start | shared QuantizationBand.start (exact integer; existing i64 DTO) |
| CraftingQualityQuantizationBand.end | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].end | shared QuantizationBand.end (exact integer; existing i64 DTO) |
| CraftingQualityQuantizationBand.mappedValue | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].mappedValue | shared QuantizationBand.mappedValue (exact integer; existing i64 DTO) |
| CraftingQualityQuantization_RecordRef.qualityQuantizationRecord | Reference | CraftingQualityQuantizationRecord | CraftingQualityQuantizationRecord | ResourceType.properties[].qualityQuantization.qualityQuantizationRecord | qualityQuantization.payload.reference (target qualityQuantizations) |
| GramsPerCubicCentimeter.gramsPerCubicCentimeter | Single |  |  | ResourceType.densityType.densityUnit.gramsPerCubicCentimeter | resourceTypes[].density.payload.densityUnit.payload.value (raw g/cm3) |
| KilogramsPerCubicMeter.kilogramsPerCubicMeter | Single |  |  | ResourceType.densityType.densityUnit.kilogramsPerCubicMeter | resourceTypes[].density.payload.densityUnit.payload.value (raw kg/m3) |
| ResourceType.displayName | Locale |  |  | ResourceType.displayName | resourceTypes[].name (LocalisedText) |
| ResourceType.description | Locale |  |  | ResourceType.description | resourceTypes[].description (LocalisedText; x-gameText) |
| ResourceType.densityType | StrongPointer | ResourceTypeDensityType | ResourceTypeDensityType | ResourceType.densityType | resourceTypes[].density (actual sourceType + tagged payload) |
| ResourceType.defaultThumbnailPath | String |  |  | ResourceType.defaultThumbnailPath | resourceTypes[].defaultThumbnailPath (raw String) |
| ResourceType.defaultThumbnailPathSVG | String |  |  | ResourceType.defaultThumbnailPathSVG | resourceTypes[].defaultThumbnailPathSVG (raw String) |
| ResourceType.rttThumbnailEntityClass | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.rttThumbnailEntityClass | resourceTypes[].rttThumbnailEntityClass (Record Reference) |
| ResourceType.properties | StrongPointer[] | ResourceTypeProperties | ResourceTypeProperties | ResourceType.properties | resourceTypes[].properties (ordered nullable list, no deduplication) |
| ResourceType.refinedVersion | Reference | ResourceType | ResourceType | ResourceType.refinedVersion | resourceTypes[].refinedVersion (ResourceType Reference) |
| ResourceType.validateDefaultCargoBox | Bool |  |  | ResourceType.validateDefaultCargoBox | resourceTypes[].validateDefaultCargoBox (nullable Bool) |
| ResourceType.defaultCargoContainers | StrongPointer | SResourceTypeDefaultCargoContainers | SResourceTypeDefaultCargoContainers | ResourceType.defaultCargoContainers | resourceTypes[].defaultCargoContainers (ordered named slots) |
| ResourceTypeCraftingData.qualityDistribution | StrongPointer | CraftingQualityDistribution_Base | CraftingQualityDistribution_Base | ResourceType.properties[].qualityDistribution | resourceTypes[].properties[].payload.qualityDistribution (inline-or-RecordRef source wrapper) |
| ResourceTypeCraftingData.qualityLocationOverride | StrongPointer | CraftingQualityLocationOverride_Base | CraftingQualityLocationOverride_Base | ResourceType.properties[].qualityLocationOverride | resourceTypes[].properties[].payload.qualityLocationOverride (inline-or-RecordRef source wrapper) |
| ResourceTypeCraftingData.qualityQuantization | StrongPointer | CraftingQualityQuantization_Base | CraftingQualityQuantization_Base | ResourceType.properties[].qualityQuantization | resourceTypes[].properties[].payload.qualityQuantization (inline-or-RecordRef source wrapper) |
| ResourceTypeDensity.densityUnit | StrongPointer | BaseDensityUnit | BaseDensityUnit | ResourceType.densityType.densityUnit | resourceTypes[].density.payload.densityUnit (actual sourceType + tagged value/unit) |
| ResourceTypeGroup.displayName | Locale |  |  | ResourceTypeGroup.displayName | resourceGroups[].name (existing LocalisedText) |
| ResourceTypeGroup.description | Locale |  |  | ResourceTypeGroup.description | resourceGroups[].description (LocalisedText; x-gameText) |
| ResourceTypeGroup.defaultThumbnailPath | String |  |  | ResourceTypeGroup.defaultThumbnailPath | resourceGroups[].defaultThumbnailPath (raw String) |
| ResourceTypeGroup.defaultThumbnailPathSVG | String |  |  | ResourceTypeGroup.defaultThumbnailPathSVG | resourceGroups[].defaultThumbnailPathSVG (raw String) |
| ResourceTypeGroup.groups | Reference[] | ResourceTypeGroup | ResourceTypeGroup | ResourceTypeGroup.groups | resourceGroups[].groups (ordered nullable Record References) |
| ResourceTypeGroup.resources | Reference[] | ResourceType | ResourceType | ResourceTypeGroup.resources | resourceGroups[].resources (ordered nullable Record References) |
| ResourceTypeMolarMass.molarMass | Single |  |  | ResourceType.densityType.molarMass | resourceTypes[].density.payload.molarMass (raw Single, no invented unit) |
| ResourceTypeProperties.name | String |  |  | ResourceType.properties[].name | resourceTypes[].properties[].name (raw String, inherited) |
| ResourceTypeVolatility.volatility | Single |  |  | ResourceType.properties[].volatility | resourceTypes[].properties[].payload.volatility (raw Single) |
| ResourceTypeVolatility.healthDecayPerSecond | Single |  |  | ResourceType.properties[].healthDecayPerSecond | resourceTypes[].properties[].payload.healthDecayPerSecond (raw Single; source name describes rate) |
| SResourceTypeDefaultCargoContainers.one_eighthSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_eighthSCU | resourceTypes[].defaultCargoContainers[{slot=one_eighthSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.one_quarterSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_quarterSCU | resourceTypes[].defaultCargoContainers[{slot=one_quarterSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.one_halfSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_halfSCU | resourceTypes[].defaultCargoContainers[{slot=one_halfSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.oneSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.oneSCU | resourceTypes[].defaultCargoContainers[{slot=oneSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.twoSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.twoSCU | resourceTypes[].defaultCargoContainers[{slot=twoSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.fourSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.fourSCU | resourceTypes[].defaultCargoContainers[{slot=fourSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.eightSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.eightSCU | resourceTypes[].defaultCargoContainers[{slot=eightSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.sixteenSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.sixteenSCU | resourceTypes[].defaultCargoContainers[{slot=sixteenSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.twentyFourSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.twentyFourSCU | resourceTypes[].defaultCargoContainers[{slot=twentyFourSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.thirtyTwoSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.thirtyTwoSCU | resourceTypes[].defaultCargoContainers[{slot=thirtyTwoSCU}].entityClass (exact slot, nullable Entity Reference) |
