// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A loot table: a `LootTableV3Record` Record under `lootgeneration/loottables`, the weighted
/// archetypes a container draws from.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootTable<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `V3LootTable_Actor_CQC`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The entries of `lootTable.lootArchetypes` in stored order.
    pub(crate) entries: Vec<LootTableEntry<'a>>,
}

/// One entry of a loot table: an archetype and the odds it is drawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootTableEntry<'a> {
    /// The designer's label of the entry.
    pub(crate) name: Option<&'a str>,
    /// The `weight` of the entry among the entries of the table.
    pub(crate) weight: Option<f32>,
    /// The `LootArchetypeV3Record` the entry links to; `null` for an archetype written out in
    /// place.
    pub(crate) archetype_guid: Option<CatalogGuid>,
    /// The entries of an archetype written out in place; `null` for a linked one.
    pub(crate) archetype: Option<Vec<LootArchetypeEntry<'a>>>,
    /// `priorityCount` of the `LootTableOptionalDataV3_PriorityChoice` of the entry.
    pub(crate) priority_count: Option<i64>,
    /// `choiceLimit` of the `LootTableOptionalDataV3_ChoiceLimit` of the entry.
    pub(crate) choice_limit: Option<i64>,
    /// `dupeLimit` of the `LootTableOptionalDataV3_DupeLimit` of the entry.
    pub(crate) dupe_limit: Option<i64>,
    /// `chanceToExist` of the `LootTableOptionalDataV3_ChanceToExist` of the entry, from 0 to 1.
    pub(crate) chance_to_exist: Option<f32>,
}

/// A loot archetype: a `LootArchetypeV3Record` Record under `lootgeneration/lootarchetypes`, the
/// weighted kinds of item a table entry stands for.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootArchetype<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `V3LootArchetype_Antiques`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The entries of `lootArchetype.entries` in stored order.
    pub(crate) entries: Vec<LootArchetypeEntry<'a>>,
}

/// One entry of a loot archetype: which items it selects and the odds it is drawn.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootArchetypeEntry<'a> {
    /// The designer's label of the entry.
    pub(crate) name: Option<&'a str>,
    /// The `weight` of the entry among the entries of the archetype.
    pub(crate) weight: Option<f32>,
    /// Which items the entry selects.
    pub(crate) selector: LootSelector<'a>,
    /// The `stackSize` of the `ArchetypeOptionalDataV3_StackSize` of the entry.
    pub(crate) stack_size: Option<QuantityRange>,
    /// The `ArchetypeOptionalDataV3_SpawnWith` items spawned along with the entry, in stored
    /// order.
    pub(crate) spawn_with: Vec<SpawnWith<'a>>,
}

/// Which items a loot entry selects: the items that carry every positive tag and none of the
/// negative ones, or the listed entity classes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootSelector<'a> {
    /// The struct of the selector as stored, e.g. `LootArchetypeV3Selector_Tags`; informational,
    /// not a selector; `null` when the entry has none.
    pub(crate) selector_type: Option<&'a str>,
    /// The GUIDs of the `Tag` Records in `positiveTags`, in stored order.
    pub(crate) positive_tag_guids: Vec<CatalogGuid>,
    /// The GUIDs of the `Tag` Records in `negativeTags`, in stored order.
    pub(crate) negative_tag_guids: Vec<CatalogGuid>,
    /// The GUIDs of the entity classes of a manual entity class list, in stored order.
    pub(crate) entity_class_guids: Vec<CatalogGuid>,
}

/// A range of amounts, both ends included.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QuantityRange {
    /// `min`.
    pub(crate) min: Option<f32>,
    /// `max`.
    pub(crate) max: Option<f32>,
}

/// Items an entry spawns along with its own (`ArchetypeOptionalDataV3_SpawnWith`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SpawnWith<'a> {
    /// The designer's label.
    pub(crate) name: Option<&'a str>,
    /// `chanceToSpawnWith`, from 0 to 1.
    pub(crate) chance: Option<f32>,
    /// `mode`, e.g. `MostSimilar`.
    pub(crate) mode: Option<&'a str>,
    /// `amountToSpawn`.
    pub(crate) amount: Option<QuantityRange>,
    /// Which items are spawned.
    pub(crate) selector: LootSelector<'a>,
}

/// A container placed with a multi config: an entity that places an object container and carries
/// a `SubHarvestableMultiConfigRecord`, whose tag points the config fills with harvestables and
/// their loot. One entry per container, system and starmap object it hangs below.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootContainer {
    /// The `SubHarvestableMultiConfigRecord` the placing entity carries (`multiConfigRef`), an
    /// entry of `subHarvestableMultiConfigs`.
    pub(crate) multi_config_guid: CatalogGuid,
    /// The path of the object container the entity places.
    pub(crate) container: String,
    /// The star system folder of the system container the walk started in.
    pub(crate) system: String,
    /// The `guid` of the `starmapObjects` entry the walk's root container hangs below; `null` for
    /// none or an object the catalog lacks.
    pub(crate) starmap_object_guid: Option<CatalogGuid>,
    /// The number of entities in all `.soc` files of the placed container, 0 for none; `null`
    /// when a file cannot be read or the Archive lacks the container.
    pub(crate) entity_count: Option<u32>,
    /// How many entities of the placed container carry each tag, summed over its `.soc` files and
    /// ordered by GUID; empty when `entityCount` is `null`. The tags of `tagGuids` of the tagged
    /// configs select the slots.
    pub(crate) tag_counts: Vec<TagCount>,
}

/// How many entities carry one tag.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagCount {
    /// The `Tag` Record.
    pub(crate) tag_guid: CatalogGuid,
    /// The number of entities that carry the tag.
    pub(crate) entities: u32,
}

/// The loot a slot of a harvestable generates (`lootConfig`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootConfig {
    /// The `LootTable` Record the config links to, an entry of `legacyLootTables`.
    pub(crate) loot_table_guid: Option<CatalogGuid>,
    /// The `LootTableV3Record` the config links to, an entry of `lootTables`.
    pub(crate) loot_table_v3_guid: Option<CatalogGuid>,
    /// The `PoolFilterRecord` of the constraints, an entry of `definitions.lootPoolFilters.rows`.
    pub(crate) pool_filter_guid: Option<CatalogGuid>,
    /// `totalResultsLimit`.
    pub(crate) total_results_limit: Option<i64>,
    /// `chanceToGenerate`, from 0 to 1.
    pub(crate) chance_to_generate: Option<f32>,
    /// `chanceToGenerateAdditionalAttachedInventories`, from 0 to 1.
    pub(crate) chance_to_generate_additional_inventories: Option<f32>,
    /// The `fullnessFactorRange` of the constraints: how full the container is filled.
    pub(crate) fullness: Option<QuantityRange>,
}
