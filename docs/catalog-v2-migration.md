# Released catalog field migration

The frozen [v0.4.2 schema](../crates/p4kx-export/tests/fixtures/catalog-v0.4.2.schema.json)
matches `schemas/basetool-catalog.schema.json` at the `v0.4.2` release tag. Use that schema to
inventory the released fields. A synthetic v2 instance may leave entire lists empty.

The inventory tracks every declaration under the 53 released root fields. Each occurrence
has a wire path and a traversal path through the schema, including the keyword and index of
each tagged or nullable branch. A recursive reference is recorded at the point where it returns
to an active schema node. This finite inventory works for every root even when definitions are
shared. A schema keyword outside the supported vocabulary fails the inventory.

Verified migration dispositions below cover Blueprint Categories, Gameplay Properties,
Manufacturers and Commodities.
The rest of the released occurrences still need verified destinations or removal decisions.
P4KX 0.5.0 writes catalog schema v3. Re-export the original inputs to obtain the physical
identity and per-value provenance carried by v3.

| Released field | Catalog v3 |
| --- | --- |
| `blueprintCategories[].guid`, `className`, `recordPath` | Corresponding sourced field's `value` under `definitions.blueprintCategories.rows[]`. |
| `blueprintCategories[].name` and its `key`, `en`, `de` | The released writer always used an empty placeholder. The original category Records have no name field. Use the sourced class name when an application needs a category label. |
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

## External container links

Schema v3 includes `placements.unresolvedContainerLinks` as additive source evidence for a
reached external Child whose target lookup has zero or multiple winning candidates. Join an
occurrence's `parentInstanceId.value` to `placements.containerInstances.rows[].instanceId.value`
within the same Catalog. Resolve source-use IDs in that Catalog. Use `exportHealth` alongside
selection completeness for decisions about previously imported rows.

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
