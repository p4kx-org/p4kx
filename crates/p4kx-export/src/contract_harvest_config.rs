// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::contract_harvest::HarvestableLink;
use crate::contract_loot::LootConfig;
use crate::guid_text::CatalogGuid;

/// A sub harvestable multi config: a `SubHarvestableMultiConfigRecord` Record under
/// `harvestable/slotpresets`, what the slots of an object can hold by tag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SubHarvestableMultiConfig<'a> {
    /// The GUID of the Record.
    pub guid: CatalogGuid,
    /// The Record name without its type.
    pub class_name: &'a str,
    /// The record path, without `.xml`.
    pub record_path: &'a str,
    /// The GUID of the Record the default config links to when it is a reference; `null` for none
    /// or a config of another form.
    pub default_config_guid: Option<CatalogGuid>,
    /// `ignoreAttachableTagsForTaggedConfigs`.
    pub ignore_attachable_tags: Option<bool>,
    /// The configs by tag, from `taggedConfigs` in stored order.
    pub tagged_configs: Vec<TaggedConfig<'a>>,
}

/// One config of a [`SubHarvestableMultiConfig`] and the tags it applies to.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct TaggedConfig<'a> {
    /// `name`.
    pub name: Option<&'a str>,
    /// The GUIDs of the `Tag` Records of the tag list, in stored order.
    pub tag_guids: Vec<CatalogGuid>,
    /// The GUID of the Record the config links to when it is a reference; `null` for a config
    /// written out in place.
    pub config_guid: Option<CatalogGuid>,
    /// `configRespawnTimeMultiplier` of a config written out in place.
    pub respawn_time_multiplier: Option<f32>,
    /// `initialSlotsProbability` of a config written out in place.
    pub initial_slots_probability: Option<f32>,
    /// The sub harvestables of a config written out in place, in stored order.
    pub sub_harvestables: Vec<SubHarvestable<'a>>,
}

/// One sub harvestable of a config.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SubHarvestable<'a> {
    /// What the sub harvestable spawns; `null` when it names neither a harvestable nor an entity.
    pub harvestable: Option<HarvestableLink<'a>>,
    /// The GUID of the Record the Reference `harvestableSetup` links to.
    pub setup_guid: Option<CatalogGuid>,
    /// `harvestableRespawnTimeMultiplier`.
    pub respawn_time_multiplier: Option<f32>,
    /// `relativeProbability`.
    pub relative_probability: Option<f32>,
    /// `probability` of `relativeProbabilityDeepest`; `null` when there is none.
    pub relative_probability_deepest: Option<f32>,
    /// The GUIDs of the `Tag` Records of `geometries`, in stored order.
    pub geometry_tag_guids: Vec<CatalogGuid>,
    /// The loot the slot generates (`lootConfig`); `null` when the slot has none.
    pub loot_config: Option<LootConfig>,
}
