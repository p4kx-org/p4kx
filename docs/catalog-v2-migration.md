# Released catalog field migration

The frozen [v0.4.2 schema](../crates/p4kx-export/tests/fixtures/catalog-v0.4.2.schema.json)
matches `schemas/basetool-catalog.schema.json` at the `v0.4.2` release tag. Use that schema to
inventory the released fields. A synthetic v2 instance may leave entire lists empty.

The inventory tracks every declaration under the 53 released root fields. Each occurrence
has a wire path and a traversal path through the schema, including the keyword and index of
each tagged or nullable branch. A recursive reference is recorded at the point where it returns
to an active schema node. This finite inventory works for every root even when definitions are
shared. A schema keyword outside the supported vocabulary fails the inventory.

Verified migration dispositions below cover Blueprint Categories, Blueprint Pools, Gameplay
Properties, Manufacturers and Commodities.
The rest of the released occurrences still need verified destinations or removal decisions.
P4KX 0.5.0 writes catalog schema v3. Re-export the original inputs to obtain the physical
identity and per-value provenance carried by v3.

| Released field | Catalog v3 |
| --- | --- |
| `blueprintCategories[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.blueprintCategories.rows[]`. |
| `blueprintCategories[].name` and its `key`, `en`, `de` | The released writer always used an empty placeholder. The original category Records have no name field. Use the sourced class name when an application needs a category label. |
| `blueprintPools[].guid`, `className`, `recordPath` | Corresponding sourced value in `definitions.blueprintPools.rows[]`. Resolve nominal GUIDs to physical definitions before identifying an individual pool. |
| `blueprintPools[].entries[].blueprintGuid`, `weight` | Each `definitions.blueprintPools.rows[].blueprintRewards.value[]` position holds the original `blueprintRecord` Reference and exact `weight`. Retain their original states and positions. |
| `blueprintPools[].totalWeight` | `projections.blueprintPoolWeights.rows[].totalWeight`. Match the same input and `pool.recordIndex.value` to the definition. |
| `blueprintPools[].rewardSources[]` | In `declarations.blueprintRewardSources.rows[]`, `guid` and `recordPath` identify the original referencing Record. `sourceOrdinal` distinguishes its occurrences. Use `pool` for the authored target, `poolStorage` for its original storage type, and the sourced `contractId`, `contractName` and `chance` for authored facts. |
| `gameplayProperties[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.gameplayProperties.rows[]`. |
| `gameplayProperties[].name` | Join `definitions.gameplayProperties.rows[].propertyName.value` to `texts.publicNames[].id.value`. |
| `gameplayProperties[].name.key` | `key.value` of that public-name entry. |
| `gameplayProperties[].name.en`, `name.de` | `translations.value.en.displayText.value` or `translations.value.de.displayText.value` on that entry, when the language and value are available. |
| `manufacturers[].guid`, `className`, `recordPath`, `code` | Corresponding sourced field's `value` under `definitions.manufacturers.rows[]`. |
| `manufacturers[].name` | Join `localization.value.name.value` on the manufacturer row to `texts.publicNames[].id.value`. |
| `manufacturers[].description` | Join `localization.value.description.value` on the manufacturer row to `texts.gameTexts[].id.value`. |
| `manufacturers[].name.key`, `description.key` | `key.value` of the corresponding public-name or game-text entry. |
| `manufacturers[].name.en`, `name.de`, `description.en`, `description.de` | The corresponding entry's `translations.value.en.displayText.value` or `translations.value.de.displayText.value` when available. |
| `commodities[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.commodities.rows[]`. |
| `commodities[].subtypeGuid`, `refinedSubtypeGuid`, `typeGuid`, `densityGramsPerCubicCentimetre` | Corresponding nullable `value` under `projections.commodityPresentation.rows[]`. Join the old Commodity GUID to `commodityGuid.value`. |
| `commodities[].name`, `description`, `displayType` | Read the nullable text reference from the same presentation row. `name` joins `texts.publicNames[].id.value`. Both protected fields join `texts.gameTexts[].id.value`. |
| Commodity `name`, `description`, `displayType` subfields `key`, `en`, `de` | Use the joined text entry's `key.value` and `translations.value.en.displayText.value` or `translations.value.de.displayText.value` when available. |

The released English and German fields used null for missing languages, missing keys,
placeholders and empty display values. Read the sourced cell's state to preserve those
distinctions when importing v3 data.
The text registry carries the decoded source text separately from its display form. For
the full field origins and policies see the [Gameplay Property contract](catalog-gameplay-properties.md)
and the [shared Record header](catalog-record-headers.md).

For the Commodity GUID join, resource fallbacks and exact presentation rules, see the
[Commodity contract](catalog-commodity-types.md).

## Loadout identities

Read the original owner GUID and category from `loadouts[].entityGuid.value` and
`loadouts[].entityKind.value`. The first value joins to the owner and the second
identifies its family. Use `sourceRecordIndex.value` to distinguish physical rows
with equal GUIDs in one Build. Port names are under `ports[].name.value`.
Each of these values carries its own original source references and encryption marker.
Read the minimum and maximum accepted sizes from `ports[].minSize.value` and
`ports[].maxSize.value`. Their separate state and origin distinguish missing, null, unreadable
and invalid source values.
Read normalized flag and tag words from `flags.value`, `portTags.value` and
`requiredTags.value` on each resolved port. An explicitly empty source has an empty array.
For non-present states, use an empty list when applying port compatibility rules.
Read the original port localisation key from `ports[].displayNameKey.value`. The cell retains
authored empty and placeholder keys, which can be ignored when displaying a port name.

## External container links

Schema v3 includes `placements.unresolvedContainerLinks` as additive source evidence for a
reached external Child whose target lookup has zero or multiple winning candidates or whose
unique target body is unreadable or unparsed. Join the occurrence's `parentInstanceId.value`
to `placements.containerInstances.rows[].instanceId.value` within the same Catalog. Match the root
Entry index, Child ordinal sequence and `externalChildStepIndices` together. Resolve source-use
IDs in that Catalog. Use `exportHealth` alongside selection completeness for decisions
about previously imported rows.

Schema v3 also adds `declarations.lootEntityContainerDocuments` for original container-link
attributes in the EntXML documents actually reached by the Loot walk. Document identity is the
containing/nested physical Entry, with document-local node and attribute ordinals. Normalized
nodes and configuration-reference sites are stored once per document; each link keeps its exact
selector, sourced lookup status and physical candidate set. This exposes configless and repeated
original occurrences without inventing Loot placements or changing normal-publication Health.
See [the original Loot document contract](catalog-loot-source.md#original-container-link-documents-reached-by-the-loot-walk).


## Mission Location source cells

The schema-3 missionLocationTemplates dataset remains at the same address and preserves original
Record selection, GUID sorting and physical ties. Its source-backed header and location facts use
one typed representation; regenerate older exports from the original input instead of assigning
source states or encryption markers from their serialized values.

| Previous field | Current schema 3 |
| --- | --- |
| guid, className, recordPath | The corresponding immediately sourced Header field; read its state, value and origin. |
| Record metadata | recordName, recordIndex, sourceType, fileName and metadataTag complete the same shared eight-field Header. |
| disabled, isSecurityNetworkHost | Sourced Bool values; present false remains false, independently of missing/null/invalid source states. |
| entityClusterMemberGuid | entityClusterMember carries the original Reference. Read value.guid.value for an available authored GUID and value.status for target resolution. Dangling GUIDs are retained rather than collapsed to null. |
| names[] | names.value[] contains sourced original slots; inspect each slot's state before reading its value. |
| names[].tag / names[].displayName | A present names.value[i].value retains tag and displayName. The protected displayName.value joins texts.gameTexts[].id.value. |

A present empty names.value array differs from absent or null. Repeated and nullable authored
positions are not compacted. The fixed released v2 fixture is a historical input, not another
writer mode. See the [Mission Location contract](catalog-location-configuration.md#source-identity-flags-cluster-link-and-names).

## Item identity, source selection and classifications

Schema v3 keeps `items` as a selected projection while source facts have one canonical family
location. This is the only writer form; released-v2 fixtures remain frozen historical inputs.
Re-export original inputs to obtain physical indices and immediate origins.

| Earlier Item field | Current schema 3 |
| --- | --- |
| `guid`, `className`, `recordPath` | Corresponding sourced header cell; read its `value`, state and origin. The complete shared header also carries recordName, recordIndex, sourceType, fileName and metadataTag. |
| `type`, `subType`, `size`, `grade` | Join Item `recordIndex.value` to `definitions.items.rows[].recordIndex.value`, then follow the sourced `selection.attachmentComponentOrdinal.value`. Read itemType, itemSubtype, size and grade in that component's `value.value`. Numbers retain original kinds and bits. |
| `manufacturerGuid` | The selected Attachment's original `manufacturer` retains Reference GUID/target state or authored storage. Do not replace an authored pointer or unresolved Reference with an invented GUID. |
| `tags` | The selected Attachment's sourced raw `tags` preserves whitespace, repeats and case; consumers may split whitespace. Original requiredTags is available independently. |
| `mass` | Follow `selection.physicsComponentOrdinal.value` to the physics component's exact `value.value.mass`. |
| Name, description and purchase display type | Follow the selected Attachment Localization or Purchasable source fields to the existing canonical text registries. Source text and display text remain separate. |
| `itemClass` | Sourced English ClassLine result; the original Description selector and actual English lookup/display evidence contribute even when requested output locales differ. |
| `playability` | Sourced result from actual key-only rule inputs and Record path; unrelated localized text does not contribute. |
| `variantOf` | Existing nominal GUID/null is unchanged in this bounded slice. Immediate source-backed physical target/status and unfiltered candidate evidence remain the explicit ADR-0410 variant scope. |

A selected null/unreadable first component is not replaced with a later one. Foreign original
AttachDef data remains in Item definitions even when the separate constraints family excludes it.
Respect the source-set completeness on selection and derived cells. These changes do not establish
complete root-Item/Ship/Loadout provenance or another output compatibility mode.


## Ship DCB/static source fields

Re-export original inputs to obtain canonical `ships` rows in schema v3.

| Earlier Ship field | Current schema 3 |
| --- | --- |
| `guid`, `className`, `recordPath` | Corresponding sourced Header cells. recordName, recordIndex, sourceType, fileName and metadataTag complete the shared original Header. Join physical owners by recordIndex.value. |
| `crewSize`, `size` | Sourced exact ResourceNumber. Read the original numeric type and value or IEEE bits. |
| `vehicleDefinition` | Sourced original String/Enum, including empty strings and whitespace. The XML reader normalizes the path for lookup. |
| `insurance` | Sourced first static declaration. Read value.staticElementOrdinal, value.restoreOnClaim and the independent value.shipInsuranceParams state. |
| Insurance class name, wait times and expedite fee | Sourced fields under `insurance.value.shipInsuranceParams.value`. Empty class names stay present and numbers retain their original widths and bits. `expediteFee` reads `baseExpeditingFee` in aUEC. |
| Crew-manifest input | `playabilityInputs.crewManifestComponentOrdinal` and `crewManifest` retain physical selection, the original reference and target header state. |

Each typed insurance leaf retains the parent choice uncertainty. A known absent declaration,
a null parameter pointer, an unreadable earlier static slot and a present zero/false/empty value
remain distinct.

Ship `mass`, `massBasis` and `hullHealth` are immediate sourced cells in v3. Read each cell's
`state` and `value`. Their numbers and basis use the same ShipReader rules as the standalone
ShipBase export. Each metric includes the physical Record, the relevant XML selection and fields,
and the original Physics operand when it supplies fallback mass. Coordinate loss affects
`sourceSetComplete` independently of the value state.

## Crafting stations

`crafters` moves to `definitions.crafters.rows`. Read sourced station values under
`component.value` and attachment restrictions under `attachment.value.attachDef`.
`projections.crafterHosts.rows` replaces `hostGuids` with resolved physical ship and
crafter identities plus each original mount position.
