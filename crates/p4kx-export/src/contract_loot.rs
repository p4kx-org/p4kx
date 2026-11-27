// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::source_value_catalog::SourceValueRef;

/// A loot table: a `LootTableV3Record` Record under `lootgeneration/loottables`, the weighted
/// archetypes a container draws from.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootTable<'a> {
    /// The GUID of the Record.
    pub guid: CatalogGuid,
    /// The Record name without its type, e.g. `V3LootTable_Actor_CQC`.
    pub class_name: &'a str,
    /// The record path, without `.xml`.
    pub record_path: &'a str,
    /// The physical owner to join with `definitions.lootTables`.
    pub record_index: u32,
    /// The entries of `lootTable.lootArchetypes` in stored order.
    pub entries: Vec<LootTableEntry<'a>>,
}

/// One entry of a loot table: an archetype and the odds it is drawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootTableEntry<'a> {
    /// The designer's label of the entry.
    pub name: Option<&'a str>,
    /// The `weight` of the entry among the entries of the table.
    pub weight: Option<f32>,
    /// The original weight occurrence and the immediate origin of the convenience value.
    pub weight_source: SourcedValue<SourceValueRef>,
    /// The `LootArchetypeV3Record` the entry links to; `null` for an archetype written out in
    /// place.
    pub archetype_guid: Option<CatalogGuid>,
    /// The entries of an archetype written out in place; `null` for a linked one.
    pub archetype: Option<Vec<LootArchetypeEntry<'a>>>,
    /// `priorityCount` of the `LootTableOptionalDataV3_PriorityChoice` of the entry.
    pub priority_count: Option<i64>,
    /// `choiceLimit` of the `LootTableOptionalDataV3_ChoiceLimit` of the entry.
    pub choice_limit: Option<i64>,
    /// `dupeLimit` of the `LootTableOptionalDataV3_DupeLimit` of the entry.
    pub dupe_limit: Option<i64>,
    /// The selected duplicate-limit occurrence and the immediate origin of its selection.
    pub dupe_limit_source: SourcedValue<SourceValueRef>,
    /// `chanceToExist` of the `LootTableOptionalDataV3_ChanceToExist` of the entry, from 0 to 1.
    pub chance_to_exist: Option<f32>,
}

/// A loot archetype: a `LootArchetypeV3Record` Record under `lootgeneration/lootarchetypes`, the
/// weighted kinds of item a table entry stands for.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootArchetype<'a> {
    /// The GUID of the Record.
    pub guid: CatalogGuid,
    /// The Record name without its type, e.g. `V3LootArchetype_Antiques`.
    pub class_name: &'a str,
    /// The record path, without `.xml`.
    pub record_path: &'a str,
    /// The physical owner to join with `definitions.lootArchetypes`.
    pub record_index: u32,
    /// The entries of `lootArchetype.entries` in stored order.
    pub entries: Vec<LootArchetypeEntry<'a>>,
}

/// One entry of a loot archetype: which items it selects and the odds it is drawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootArchetypeEntry<'a> {
    /// The designer's label of the entry.
    pub name: Option<&'a str>,
    /// The `weight` of the entry among the entries of the archetype.
    pub weight: Option<f32>,
    /// The original weight occurrence and the immediate origin of the convenience value.
    pub weight_source: SourcedValue<SourceValueRef>,
    /// Which items the entry selects.
    pub selector: LootSelector<'a>,
    /// The `stackSize` of the `ArchetypeOptionalDataV3_StackSize` of the entry.
    pub stack_size: Option<QuantityRange>,
    /// The `ArchetypeOptionalDataV3_SpawnWith` items spawned along with the entry, in stored
    /// order.
    pub spawn_with: Vec<SpawnWith<'a>>,
}

/// Which items a loot entry selects: the items that carry every positive tag and none of the
/// negative ones, or the listed entity classes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootSelector<'a> {
    /// The struct of the selector as stored, e.g. `LootArchetypeV3Selector_Tags`; informational,
    /// not a selector; `null` when the entry has none.
    pub selector_type: Option<&'a str>,
    /// The GUIDs of the `Tag` Records in `positiveTags`, in stored order.
    pub positive_tag_guids: Vec<CatalogGuid>,
    /// The GUIDs of the `Tag` Records in `negativeTags`, in stored order.
    pub negative_tag_guids: Vec<CatalogGuid>,
    /// The GUIDs of the entity classes of a manual entity class list, in stored order.
    pub entity_class_guids: Vec<CatalogGuid>,
}

/// A range of amounts, both ends included.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct QuantityRange {
    /// `min`.
    pub min: Option<f32>,
    /// `max`.
    pub max: Option<f32>,
}

/// Items an entry spawns along with its own (`ArchetypeOptionalDataV3_SpawnWith`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SpawnWith<'a> {
    /// The designer's label.
    pub name: Option<&'a str>,
    /// `chanceToSpawnWith`, from 0 to 1.
    pub chance: Option<f32>,
    /// `mode`, e.g. `MostSimilar`.
    pub mode: Option<&'a str>,
    /// `amountToSpawn`.
    pub amount: Option<QuantityRange>,
    /// Which items are spawned.
    pub selector: LootSelector<'a>,
}

/// An object container placed by an entity with a harvestable multi-config.
///
/// Each row belongs to one system, container and Starmap ancestor.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootContainer {
    /// The `SubHarvestableMultiConfigRecord` the placing entity carries (`multiConfigRef`), an
    /// entry of `subHarvestableMultiConfigs`.
    pub multi_config_guid: SourcedValue<CatalogGuid>,
    /// The path of the object container the entity places.
    pub container: SourcedValue<String>,
    /// The star system folder of the system container the walk started in.
    pub system: SourcedValue<String>,
    /// The `guid` of the `starmapObjects` entry the walk's root container hangs below; `null` for
    /// none or an object the catalog lacks.
    pub starmap_object_guid: SourcedValue<CatalogGuid>,
    /// The total entities from the verified `.soc` files of the placed container, including zero.
    pub entity_count: SourcedValue<u32>,
    /// Entity counts by GUID from the verified SOC survey, ordered by GUID.
    pub tag_counts: SourcedValue<Vec<TagCount>>,
}

/// How many entities carry one tag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct TagCount {
    /// The GUID from the first accepted original `TagId` attribute.
    pub tag_guid: SourcedValue<CatalogGuid>,
    /// The number of distinct entities that carry the tag in the complete SOC survey.
    pub entities: SourcedValue<u32>,
}

/// The loot a slot of a harvestable generates (`lootConfig`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LootConfig {
    /// The `LootTable` Record the config links to, an entry of `legacyLootTables`.
    pub loot_table_guid: Option<CatalogGuid>,
    /// The `LootTableV3Record` the config links to, an entry of `lootTables`.
    pub loot_table_v3_guid: Option<CatalogGuid>,
    /// The `PoolFilterRecord` of the constraints, an entry of `definitions.lootPoolFilters.rows`.
    pub pool_filter_guid: Option<CatalogGuid>,
    /// `totalResultsLimit`.
    pub total_results_limit: Option<i64>,
    /// `chanceToGenerate`, from 0 to 1.
    pub chance_to_generate: Option<f32>,
    /// `chanceToGenerateAdditionalAttachedInventories`, from 0 to 1.
    pub chance_to_generate_additional_inventories: Option<f32>,
    /// The `fullnessFactorRange` of the constraints: how full the container is filled.
    pub fullness: Option<QuantityRange>,
}
