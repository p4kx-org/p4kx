// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::loot_definition_catalog::LootDefinition;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;

/// One original older `LootTable` and its ordered weighted rows.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacyLootTable<'source> {
    /// The physical Record and its exact owner-local source arena.
    #[serde(flatten)]
    pub original: LootDefinition<'source>,
    /// Original lootArchetypes slots in source order.
    pub entries: SourcedValue<Vec<SourcedValue<LegacyLootTableEntry>>>,
}

/// A weighted older table entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacyLootTableEntry {
    /// The original weighting number.
    pub weight: SourcedValue<ResourceNumber>,
    /// The linked archetype GUID from the original Reference slot.
    pub archetype_guid: SourcedValue<CatalogGuid>,
    /// The original minimum result count.
    pub min_results: SourcedValue<ResourceNumber>,
    /// The original maximum result count.
    pub max_results: SourcedValue<ResourceNumber>,
}

/// One original older `LootArchetype` and its ordered weighted tag groups.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacyLootArchetype<'source> {
    /// The physical Record and its exact owner-local source arena.
    #[serde(flatten)]
    pub original: LootDefinition<'source>,
    /// The ordered excluded Tag Reference slots.
    pub excluded_tag_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    /// The ordered original primary group slots.
    pub entries: SourcedValue<Vec<SourcedValue<LegacyLootEntry<'source>>>>,
    /// The ordered original secondary groups.
    pub secondary_groups: SourcedValue<Vec<SourcedValue<LegacySecondaryGroup<'source>>>>,
}

/// One authored primary entry of an older archetype.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacyLootEntry<'source> {
    /// The original designer label.
    pub name: SourcedValue<&'source str>,
    /// The original weighting number.
    pub weight: SourcedValue<ResourceNumber>,
    /// The original primary Tag Reference.
    pub tag_guid: SourcedValue<CatalogGuid>,
    /// The ordered positive Tag Reference slots.
    pub additional_positive_tag_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    /// The ordered negative Tag Reference slots.
    pub additional_negative_tag_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    /// The first authored stack-size selection with a usable endpoint.
    pub stack_size: SourcedValue<LegacyLootQuantity>,
    /// The matching original `SpawnWith` slots in source order.
    pub spawn_with: SourcedValue<Vec<SourcedValue<LegacySpawnWith<'source>>>>,
}

/// The sourced ends of an older loot amount range.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacyLootQuantity {
    /// The original minimum amount.
    pub min: SourcedValue<ResourceNumber>,
    /// The original maximum amount.
    pub max: SourcedValue<ResourceNumber>,
}

/// One ordered older-loot `SpawnWith` entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacySpawnWith<'source> {
    /// The original designer label.
    pub name: SourcedValue<&'source str>,
    /// The original selection mode.
    pub mode: SourcedValue<&'source str>,
    /// The original minimum amount.
    pub min: SourcedValue<ResourceNumber>,
    /// The original maximum amount.
    pub max: SourcedValue<ResourceNumber>,
    /// The ordered positive Tag Reference slots.
    pub positive_tag_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    /// The ordered negative Tag Reference slots.
    pub negative_tag_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
}

/// One original secondary group of an older archetype.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacySecondaryGroup<'source> {
    /// The original designer label.
    pub group_name: SourcedValue<&'source str>,
    /// The original entry slots in authored order.
    pub entries: SourcedValue<Vec<SourcedValue<LegacySecondaryEntry>>>,
}

/// One weighted Tag Reference in a secondary group.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LegacySecondaryEntry {
    /// The original weighting number.
    pub weight: SourcedValue<ResourceNumber>,
    /// The original Tag Reference.
    pub tag_guid: SourcedValue<CatalogGuid>,
}
