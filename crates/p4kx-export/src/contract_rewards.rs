// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;

/// A gameplay property definition: a `CraftingGameplayPropertyDef` Record under
/// `crafting/craftedproperties`, the target of the `propertyGuid` of a modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GameplayProperty<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name of the property; a placeholder key leaves `en` and `de` `null`.
    pub(crate) name: LocalisedText<'a>,
}

/// One entry of a blueprint pool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PoolEntry {
    /// The GUID of the blueprint, `null` for a null Reference.
    pub(crate) blueprint_guid: Option<CatalogGuid>,
    /// The weight of the entry among the pool's, `null` when the Record holds none.
    pub(crate) weight: Option<f32>,
}

/// A place that rewards a blueprint of a pool with a chance: a `BlueprintRewards` result of a
/// contract.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RewardSource<'a> {
    /// The GUID of the Record that holds the reward.
    pub(crate) record_guid: CatalogGuid,
    /// The record path of that Record, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `id` of the nearest enclosing struct that has one, e.g. the contract.
    pub(crate) contract_id: Option<CatalogGuid>,
    /// The `debugName` of that struct.
    pub(crate) contract_name: Option<&'a str>,
    /// The chance that the reward is given as the Record states it, 0 to 1 on LIVE and not
    /// clamped, `null` when the Record holds none.
    pub(crate) chance: Option<f32>,
}

/// A blueprint pool: a `BlueprintPoolRecord` under `crafting/blueprintrewards` that a reward
/// draws a blueprint from.
///
/// A source gives the blueprint of an entry with the chance of the source times the weight of the
/// entry over `totalWeight`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintPool<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The sum of the positive weights of the entries.
    pub(crate) total_weight: f32,
    /// The entries in stored order.
    pub(crate) entries: Vec<PoolEntry>,
    /// The places that reward a blueprint of the pool, found through their Reference to it.
    pub(crate) reward_sources: Vec<RewardSource<'a>>,
}

/// A generic dismantle process: a `CraftingBlueprintRecord` under `crafting/blueprints/dismantle`
/// whose process is a `GenericCraftingProcess_Dismantle`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DismantleProcess<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The share of the costs a dismantled item returns, 0 to 1, `null` when the Record holds
    /// none.
    pub(crate) efficiency: Option<f32>,
    /// The dismantle time in whole seconds, `null` when the Record holds none.
    pub(crate) time_seconds: Option<i64>,
}

/// The `CraftingGlobalParams` Records, merged: what applies to every dismantle.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CraftingGlobal {
    /// The `ResourceType` Records dismantling never returns.
    pub(crate) dismantle_excluded_resource_guids: Vec<CatalogGuid>,
    /// The entities that cannot be dismantled; their blueprints have no `dismantle`.
    pub(crate) dismantle_excluded_item_guids: Vec<CatalogGuid>,
}
