# Resource catalog contract

The [shared Record-header contract](catalog-record-headers.md) defines the eight sourced header
fields, exact original filename/metadata-tag states and separate Header/Body provenance coordinates.

The catalog uses schema version 3. The ResourceType family is published at
`definitions.resourceTypes.rows`, with immediate source origin on every source-backed field and
nested list element. Its localized names and descriptions use canonical text tables. Commodities
also use canonical definitions and presentation as described below. ResourceGroups have canonical
definition, membership and parent-projection tables with the same per-value origin contract.
Other catalog families still have their existing shapes; the complete canonical migration and origin coverage of
the 45 tool-output carrier families remain open work under [#1125](https://github.com/p4kx-org/p4kx/issues/1125).
[ADR-0356](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md) defines the full target;
[ADR-0357](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md) defines source origin.
Those targets do not imply that every family is already migrated.

## Commodity identity and resource joins

`definitions.commodities.rows[].guid.value` identifies a commodity Record. `subtypeGuid` is a nullable, non-unique
ResourceType GUID. Several commodity Records can share it, and a ResourceType can have no commodity.
Join ingredient `resourceGuid` values to `definitions.resourceTypes.rows[].guid.value` for resource
facts and to every matching `projections.commodityPresentation.rows[].subtypeGuid.value` for commodity alternatives. Read the field's
state before using its value. There is no additional root `resourceTypes` view in the v3 output.

The commodity ResourceType is the first non-null `entry` in the first ResourceContainer's ordered
`defaultComposition`. A dangling first GUID stays configured; a later valid link does not replace
it. A resolved link uses the actual physical target Record, even if another Record has the same
GUID. `typeGuid` stays the first selected, readable, non-leftover group in source Record order that
directly lists that resource; it is not the list of all group memberships.

The existing ResourceType-backed commodity fallback remains limited to otherwise unrepresented
resources used by blueprint costs or kept quality quantizations. It uses the resource GUID for
both `guid` and `subtypeGuid`. This does not limit independent ResourceType selection.
`densityGramsPerCubicCentimetre` retains its existing finite g/cm^3 meaning; other units are not
converted into it. The derived nullable value now uses its own direct field carrier in `projections.commodityPresentation`.
The [Commodity contract](catalog-commodity-types.md) documents the separate authored definitions, exact old-value reconstruction and precise selection origins.

## ResourceTypes

`definitions.resourceTypes` contains a closed `availability` state and `rows`. `available` means
the source family is declared, independently of its selected row count; `absent` means it is not
declared; `unavailable` means the family cannot be established from unreadable declarations. Rows
are selected, readable ResourceType Records under `resourcetypedatabase`, excluding the declared
development-leftover policy, in canonical GUID order. A row needs no commodity or recipe.

Every source-backed field below is a `SourcedValue<T>`; the table describes its `value` payload.
The same carrier surrounds nested fields and authored list elements. Object tags such as `type`,
reference `status` and table availability remain typed control metadata.

| Field | Source and value contract |
| --- | --- |
| `guid`, `recordName`, `className`, `recordPath`, `recordIndex`, `sourceType`, `fileName`, `metadataTag` | The shared eight-field original Record header, including exact filename and metadata declaration state. Every field has its own immediate origin. |
| `name`, `description` | Authored `displayName`/`description` keys projected to a `PublicNameRef`/`GameTextRef`; description carries `x-gameText`. |
| `groupGuids` | All distinct selected groups directly listing the resource, sorted by GUID; no inferred transitive membership. Each GUID has its own carrier. |
| `refinedVersion` | Authored ResourceType reference with sourced GUID, physical target index, path and type, plus source-resolution status. |
| `density` | Actual `ResourceTypeDensityType` struct with sourced `sourceType` and tagged `payload`. |
| `defaultThumbnailPath`, `defaultThumbnailPathSVG` | Raw strings, including empty strings; cataloging does not extract their images. |
| `rttThumbnailEntityClass` | Authored EntityClassDefinition reference; its source target need not be a catalog row. |
| `validateDefaultCargoBox` | Authored Bool; `false` is a present value. |
| `defaultCargoContainers` | Named source slots in declaration order, each with sourced `slot` and `entityClass`, including explicitly null references. |
| `properties` | Ordered authored properties, preserving null positions, repeated names and repeated values. |

A property carries sourced `sourceType`, `name` and `payload`. Present payload tags are `base`,
`volatility` and `craftingData`. An unknown shape has `payload.state: "unsupported"` and a null
payload value, with blocking health evidence; it is not a fabricated successful tagged variant.
Volatility retains `volatility` and `healthDecayPerSecond` without clamping or simulation.
CraftingData retains independent `qualityDistribution`, `qualityLocationOverride` and
`qualityQuantization` source values.

The exact cargo slot names are `one_eighthSCU`, `one_quarterSCU`, `one_halfSCU`, `oneSCU`, `twoSCU`,
`fourSCU`, `eightSCU`, `sixteenSCU`, `twentyFourSCU` and `thirtyTwoSCU`. A slot label does not assert
measured capacity or derive it from mesh/component data.

Density payload tags are `density`, `molarMass` and `emptyBase`. A density's `densityUnit` retains
its sourced concrete type and a payload tagged `gramsPerCubicCentimeter`, `kilogramsPerCubicMeter`
or `emptyBase`. Unknown density or unit shapes use the unsupported state. Concrete unit values
and molar mass preserve Single bits; the source supplies no molar-mass unit label, so none is
invented.

## Immediate value and origin carrier

Every applicable carrier has all five keys, including explicit nulls:

| Key | Contract |
| --- | --- |
| `state` | Closed interpretation state: `present`, `absent`, `null`, `unreadable`, `unsupported` or `invalid`. |
| `value` | Typed payload when present; null for every other state. A present payload may itself be nullable only when its declared type allows it. |
| `containsEncryptedSource` | `true`, `false` or null, derived from this value's actual contributing sources and ancestors. |
| `sourceUsesRefs` | Sorted, distinct references to the contributing entries in `provenance.uses`. Treat identifiers as opaque and document-local. |
| `sourceSetComplete` | Completeness of this value's contribution set, independently of selection and export health. |

`absent` means the source did not declare the field; `null` means it explicitly declared null.
`unreadable` means reading the declared value failed; `unsupported` means its concrete shape has
no typed payload; `invalid` means its value does not fit that field's contract. False, zero, empty
strings, empty arrays and legal nonfinite IEEE payloads remain present values. These states do not
replace the existing publication-health checks.

`containsEncryptedSource` is true if any actual contributor or containing Entry is proven encrypted,
even if another contribution is unknown. It is false only for a complete contribution set whose
contributors and ancestors are all proven unencrypted. Otherwise it is null. Thus a complete set
can still have unknown encryption, and an incomplete set can still contain a known encrypted
contributor. A tool-authored value with a complete empty physical-input set can be false.
A standalone DCB file without original P4K lineage stays unknown; a supplied localization Archive
does not make that DCB an Archive-derived input. Global `source.archiveKey` and unread-entry counts
are not substitutes for this field marker.

`provenance.sources` retains physical input identity, container ancestry and stored encryption
declarations. `provenance.uses` identifies each observed source, its payload/directory/file-metadata
scope, logical position and role. Only actual `payload`, `selector` and `conversion` contributions
enter a field's `sourceUsesRefs`; `attempted` reads and untouched `reference` targets remain
separate observations. A root Entry's directory metadata is not tainted by that Entry's payload
encryption, while encrypted container ancestors do affect nested directory metadata. DataForge
Record headers are payload facts. Local source metadata does not publish absolute host paths or keys.

## Quality source wrappers and exact curves

Fixed Single fields use `{ "rawBits": "7fc00001", "finite": null }` as an illustrative payload.
`rawBits` is exactly eight lowercase hexadecimal digits for f32 and sixteen for f64. `finite` is
the finite numeric projection or null; negative zero and NaN payload bits are preserved. The outer
carrier stays `present` for infinity and NaN; null in `finite` does not mean an absent source value.

Numeric fields supporting several source widths use a `type` tag: `int8`, `int16`, `int32`,
`int64`, `uint8`, `uint16`, `uint32`, `uint64`, `f32` or `f64`. Integer variants hold `value`;
Int64 and UInt64 use bounded canonical decimal strings, for example
`{ "type": "uint64", "value": "18446744073709551615" }`. Narrow integers remain exact JSON
numbers. Float variants hold the same `rawBits` and `finite` members as the fixed Single payload.
No generic-number path narrows a Double to Single or a 64-bit integer to a JSON number.

Resource quality sources retain sourced `sourceType` and `payload`. Present ownership tags are
`inline`, `recordRef` and `emptyBase`. Inline payloads carry a sourced `value`; a record reference
retains sourced target-header facts without copying or reading its target payload. Unknown shapes
are explicitly unsupported. Inline curves use sourced Int32 `min`/`max` and exact-width
ResourceNumber `mean`/`stddev`. Location overrides retain ordered nullable entries, concrete source
types, locations and nested distributions. Quantization bands preserve sourced Int32 `start`,
`end` and `mappedValue`.

The separate top-level `qualityDistributions` and public Rust QualityCurve API still use their
existing finite f32 projection and exact `Option<i32>` bounds. They are not migrated by this
ResourceType step. Their wrong numeric types and nonfinite values still fail projection. A
ResourceType's captured nonfinite numbers avoid a projection-loss failure only when that exact
physical source has a successfully emitted lossless ResourceType row; a filtered source or a
same-GUID alias cannot satisfy that proof.

## Canonical names and protected texts

A resource name references `texts.publicNames`; a description references `texts.gameTexts`.
Each canonical text row has sourced `id`, original `key` and `translations`. The latter's `value`
is a language-code map whose individual values have the same five-key carrier. All supported
languages with an available source Entry are included, using the actual winning Entry for each.
An absent language Entry produces no language-map member. Existing duplicate-key and duplicate-
Entry selection rules remain unchanged.

Texts decode escaped newlines and trim outer whitespace. An explicitly empty or whitespace-only
winning translation remains `state: "present", value: ""`; a missing key is `absent`. Placeholder
keys remain authored keys and produce absent translations without reading locale values. Bare
keys keep the existing lookup rules; an unmatched bare string is not emitted as a literal fallback.
Lookup provenance uses the exact localization Entry plus a reference to the DataForge key selector,
so a protected key or possible literal does not leak through global location strings or use IDs.

`x-gameText` and encryption origin are independent. The description reference, GameTextRef type
and complete `texts.gameTexts` table are protected even when every source is unencrypted. The same
key used as both a short name and a description has separate references and canonical rows in the
two domains. A consumer stripping game text must follow that protection through references and
retain closure of public-name references. An unencrypted marker does not grant publication rights.

## ResourceGroups

ResourceGroups use `definitions.resourceGroups`, `declarations.resourceGroupMembers` and
`projections.resourceGroupParents`. Direct scalar facts, every original ordered membership and the
existing first-selected parent choice have separate canonical owners and full immediate per-value
origins. Only selected groups affect commodity `typeGuid` and ResourceType `groupGuids`; extra
health-only reads do not change those joins. See [the ResourceGroup contract](catalog-resource-groups.md).

## Reference availability and catalog materialization

A reference's `present` status means it resolves to a physical Record, not that a catalog row was
emitted. Source-only Entity links for packaging and thumbnails do not require an Entity catalog
row. Target-header values keep their own sources; an untouched target payload is not a contributor.

Bounded, cancellable inspection captures required resolved target metadata. Final rows and source
observations determine materialization using the physical target Record index. A different emitted
Record with the same GUID does not satisfy the link. Health distinguishes `outsideCatalogSelection`,
`excludedByFilter`, `excludedByPolicy`, `targetReadFailed`, `targetProjectionFailed`,
`targetNotEmitted`, `ambiguousIdentity` and `unexpectedTargetType`. Expected selection exclusions
are informational; read/projection failures, unexplained missing output, wrong target families and
ambiguous emitted identities block normal publication. Diagnostic mode retains readable data and
the failure evidence.

Resolved-reference metadata is limited to 4096 references and 1 MiB of logical metadata per physical
source Record. Exhaustion produces `fieldWalkLimit` and incomplete field inventory. An unreadable
Record type remains an unknown selection candidate; a readable path excluded by a Filter is
excluded before generic type classification.

The internal crafting scope reads unfiltered ResourceType reverse links when a selected
non-leftover quantization target needs them; uncertain target metadata is conservative. Otherwise
it reads selected ResourceType inputs for the existing link counter. The public `read_crafting`
API keeps its unfiltered link semantics, and needed out-of-filter failures still block publication.

## Source coverage and verification

The declaration inventory below covers 29 types and 52 declared ResourceType/ResourceTypeGroup
fields. Its LIVE/PTU distinction concerns the location-override-list pointer target, from
CraftingQualityLocationOverrideEntry to CraftingQualityLocationOverrideEntry_Base; it asserts no
populated Record counts or gameplay values.

Synthetic tests cover both pointer declarations, independent resources, commodity n:1 joins,
physical GUID aliases, ordered/null cargo and quality values, direct origin/reference closure,
all available supported languages, protected/public text separation, exact IEEE and integer
payloads, filtered nonfinite sources, and shared plain/compressed serialization. Execution evidence
is recorded with the corresponding integration checks; the contract is not a full milestone or
real-game acceptance claim.

The [source verification guide](resource-source-verification.md) describes the independent
raw-value oracle, its bounds and the missing historical cardinality baseline.

## Declaration field inventory

The Export mapping column uses logical paths: follow `.value` at each sourced field or authored
list element. For example, a molar mass is
`definitions.resourceTypes.rows[i].density.value.payload.value.molarMass.value`.
ResourceGroup mappings use the canonical definition and membership tables described above.


| Declaring type.field | Stored type | LIVE target | PTU target | Owning paths | Export mapping |
| --- | --- | --- | --- | --- | --- |
| CraftingQualityDistributionNormal.min | Int32 |  |  | ResourceType.properties[].qualityDistribution.min OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.min | inline ResourceQualityCurve.min (SourcedValue<i32>) |
| CraftingQualityDistributionNormal.max | Int32 |  |  | ResourceType.properties[].qualityDistribution.max OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.max | inline ResourceQualityCurve.max (SourcedValue<i32>) |
| CraftingQualityDistributionNormal.mean | Single |  |  | ResourceType.properties[].qualityDistribution.mean OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.mean | inline ResourceQualityCurve.mean (SourcedValue<ResourceNumber>; exact source width) |
| CraftingQualityDistributionNormal.stddev | Single |  |  | ResourceType.properties[].qualityDistribution.stddev OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.stddev | inline ResourceQualityCurve.stddev (SourcedValue<ResourceNumber>; exact source width) |
| CraftingQualityDistributionUniform.min | Int32 |  |  | ResourceType.properties[].qualityDistribution.min OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.min | inline ResourceQualityCurve.min (SourcedValue<i32>) |
| CraftingQualityDistributionUniform.max | Int32 |  |  | ResourceType.properties[].qualityDistribution.max OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.max | inline ResourceQualityCurve.max (SourcedValue<i32>) |
| CraftingQualityDistribution_RecordRef.qualityDistributionRecord | Reference | CraftingQualityDistributionRecord | CraftingQualityDistributionRecord | ResourceType.properties[].qualityDistribution.qualityDistributionRecord OR ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution.qualityDistributionRecord | qualityDistribution.payload.reference (target qualityDistributions; retain GUID/status) |
| CraftingQualityLocationOverride.locationOverrideList | StrongPointer[] | CraftingQualityLocationOverrideEntry | CraftingQualityLocationOverrideEntry_Base | ResourceType.properties[].qualityLocationOverride.locationOverrideList | qualityLocationOverride.payload.value.locationOverrideList (ordered nullable entry sources; LIVE/PTU base widening accepted) |
| CraftingQualityLocationOverrideEntry.location | Reference | StarMapObject | StarMapObject | ResourceType.properties[].qualityLocationOverride.locationOverrideList[].location | qualityLocationOverride.payload.value.locationOverrideList[].payload.location (StarMapObject Reference) |
| CraftingQualityLocationOverrideEntry.qualityDistribution | StrongPointer | CraftingQualityDistribution_Base | CraftingQualityDistribution_Base | ResourceType.properties[].qualityLocationOverride.locationOverrideList[].qualityDistribution | qualityLocationOverride.payload.value.locationOverrideList[].payload.qualityDistribution (ResourceQualityCurve inline-or-RecordRef source wrapper) |
| CraftingQualityLocationOverride_RecordRef.locationOverrideRecord | Reference | CraftingQualityLocationOverrideRecord | CraftingQualityLocationOverrideRecord | ResourceType.properties[].qualityLocationOverride.locationOverrideRecord | qualityLocationOverride.payload.reference (target qualityDistributions override row) |
| CraftingQualityQuantization.bands | Class[] | CraftingQualityQuantizationBand | CraftingQualityQuantizationBand | ResourceType.properties[].qualityQuantization.bands | qualityQuantization.payload.value.bands (ordered shared QuantizationBand payloads) |
| CraftingQualityQuantizationBand.start | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].start | ResourceQuantizationBand.start (SourcedValue<i32>) |
| CraftingQualityQuantizationBand.end | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].end | ResourceQuantizationBand.end (SourcedValue<i32>) |
| CraftingQualityQuantizationBand.mappedValue | Int32 |  |  | ResourceType.properties[].qualityQuantization.bands[].mappedValue | ResourceQuantizationBand.mappedValue (SourcedValue<i32>) |
| CraftingQualityQuantization_RecordRef.qualityQuantizationRecord | Reference | CraftingQualityQuantizationRecord | CraftingQualityQuantizationRecord | ResourceType.properties[].qualityQuantization.qualityQuantizationRecord | qualityQuantization.payload.reference (target qualityQuantizations) |
| GramsPerCubicCentimeter.gramsPerCubicCentimeter | Single |  |  | ResourceType.densityType.densityUnit.gramsPerCubicCentimeter | definitions.resourceTypes.rows[].density.payload.densityUnit.payload.value (raw g/cm3) |
| KilogramsPerCubicMeter.kilogramsPerCubicMeter | Single |  |  | ResourceType.densityType.densityUnit.kilogramsPerCubicMeter | definitions.resourceTypes.rows[].density.payload.densityUnit.payload.value (raw kg/m3) |
| ResourceType.displayName | Locale |  |  | ResourceType.displayName | definitions.resourceTypes.rows[].name (PublicNameRef) |
| ResourceType.description | Locale |  |  | ResourceType.description | definitions.resourceTypes.rows[].description (GameTextRef; x-gameText) |
| ResourceType.densityType | StrongPointer | ResourceTypeDensityType | ResourceTypeDensityType | ResourceType.densityType | definitions.resourceTypes.rows[].density (actual sourceType + tagged payload) |
| ResourceType.defaultThumbnailPath | String |  |  | ResourceType.defaultThumbnailPath | definitions.resourceTypes.rows[].defaultThumbnailPath (raw String) |
| ResourceType.defaultThumbnailPathSVG | String |  |  | ResourceType.defaultThumbnailPathSVG | definitions.resourceTypes.rows[].defaultThumbnailPathSVG (raw String) |
| ResourceType.rttThumbnailEntityClass | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.rttThumbnailEntityClass | definitions.resourceTypes.rows[].rttThumbnailEntityClass (Record Reference) |
| ResourceType.properties | StrongPointer[] | ResourceTypeProperties | ResourceTypeProperties | ResourceType.properties | definitions.resourceTypes.rows[].properties (ordered nullable list, no deduplication) |
| ResourceType.refinedVersion | Reference | ResourceType | ResourceType | ResourceType.refinedVersion | definitions.resourceTypes.rows[].refinedVersion (ResourceType Reference) |
| ResourceType.validateDefaultCargoBox | Bool |  |  | ResourceType.validateDefaultCargoBox | definitions.resourceTypes.rows[].validateDefaultCargoBox (nullable Bool) |
| ResourceType.defaultCargoContainers | StrongPointer | SResourceTypeDefaultCargoContainers | SResourceTypeDefaultCargoContainers | ResourceType.defaultCargoContainers | definitions.resourceTypes.rows[].defaultCargoContainers (ordered named slots) |
| ResourceTypeCraftingData.qualityDistribution | StrongPointer | CraftingQualityDistribution_Base | CraftingQualityDistribution_Base | ResourceType.properties[].qualityDistribution | definitions.resourceTypes.rows[].properties[].payload.qualityDistribution (inline-or-RecordRef source wrapper) |
| ResourceTypeCraftingData.qualityLocationOverride | StrongPointer | CraftingQualityLocationOverride_Base | CraftingQualityLocationOverride_Base | ResourceType.properties[].qualityLocationOverride | definitions.resourceTypes.rows[].properties[].payload.qualityLocationOverride (inline-or-RecordRef source wrapper) |
| ResourceTypeCraftingData.qualityQuantization | StrongPointer | CraftingQualityQuantization_Base | CraftingQualityQuantization_Base | ResourceType.properties[].qualityQuantization | definitions.resourceTypes.rows[].properties[].payload.qualityQuantization (inline-or-RecordRef source wrapper) |
| ResourceTypeDensity.densityUnit | StrongPointer | BaseDensityUnit | BaseDensityUnit | ResourceType.densityType.densityUnit | definitions.resourceTypes.rows[].density.payload.densityUnit (actual sourceType + tagged value/unit) |
| ResourceTypeGroup.displayName | Locale |  |  | ResourceTypeGroup.displayName | definitions.resourceGroups.rows[].name (PublicNameRef) |
| ResourceTypeGroup.description | Locale |  |  | ResourceTypeGroup.description | definitions.resourceGroups.rows[].description (GameTextRef; x-gameText) |
| ResourceTypeGroup.defaultThumbnailPath | String |  |  | ResourceTypeGroup.defaultThumbnailPath | definitions.resourceGroups.rows[].defaultThumbnailPath (sourced raw String) |
| ResourceTypeGroup.defaultThumbnailPathSVG | String |  |  | ResourceTypeGroup.defaultThumbnailPathSVG | definitions.resourceGroups.rows[].defaultThumbnailPathSVG (sourced raw String) |
| ResourceTypeGroup.groups | Reference[] | ResourceTypeGroup | ResourceTypeGroup | ResourceTypeGroup.groups | declarations.resourceGroupMembers.rows[].groups (ordered sourced Record References) |
| ResourceTypeGroup.resources | Reference[] | ResourceType | ResourceType | ResourceTypeGroup.resources | declarations.resourceGroupMembers.rows[].resources (ordered sourced Record References) |
| ResourceTypeMolarMass.molarMass | Single |  |  | ResourceType.densityType.molarMass | definitions.resourceTypes.rows[].density.payload.molarMass (raw Single, no invented unit) |
| ResourceTypeProperties.name | String |  |  | ResourceType.properties[].name | definitions.resourceTypes.rows[].properties[].name (raw String, inherited) |
| ResourceTypeVolatility.volatility | Single |  |  | ResourceType.properties[].volatility | definitions.resourceTypes.rows[].properties[].payload.volatility (raw Single) |
| ResourceTypeVolatility.healthDecayPerSecond | Single |  |  | ResourceType.properties[].healthDecayPerSecond | definitions.resourceTypes.rows[].properties[].payload.healthDecayPerSecond (raw Single; source name describes rate) |
| SResourceTypeDefaultCargoContainers.one_eighthSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_eighthSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=one_eighthSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.one_quarterSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_quarterSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=one_quarterSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.one_halfSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.one_halfSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=one_halfSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.oneSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.oneSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=oneSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.twoSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.twoSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=twoSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.fourSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.fourSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=fourSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.eightSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.eightSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=eightSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.sixteenSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.sixteenSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=sixteenSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.twentyFourSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.twentyFourSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=twentyFourSCU}].entityClass (exact slot, nullable Entity Reference) |
| SResourceTypeDefaultCargoContainers.thirtyTwoSCU | Reference | EntityClassDefinition | EntityClassDefinition | ResourceType.defaultCargoContainers.thirtyTwoSCU | definitions.resourceTypes.rows[].defaultCargoContainers[{slot=thirtyTwoSCU}].entityClass (exact slot, nullable Entity Reference) |
