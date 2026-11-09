// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::contract_recipe::{BlueprintDismantle, BlueprintTier};
use crate::guid_text::CatalogGuid;

/// A crafting blueprint: a `CraftingBlueprintRecord` Record under `crafting/blueprints/crafting`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Blueprint<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `BP_CRAFT_behr_lmg_ballistic_01_mag`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The blueprint key, the Record name.
    pub(crate) key: &'a str,
    /// The name, from `CraftingBlueprint.blueprintName`.
    pub(crate) name: LocalisedText<'a>,
    /// The GUID of the category Record, the `guid` of a `blueprintCategories` entry; `null`
    /// without a category or when its Record is a left-out leftover or unreadable.
    pub(crate) category_guid: Option<CatalogGuid>,
    /// The GUID of the entity the blueprint produces, `null` without one or for a Reference to
    /// a Record the DataForge lacks.
    pub(crate) produced_item_guid: Option<CatalogGuid>,
    /// The craft time of the first tier in seconds, `null` without one.
    pub(crate) craft_time_seconds: Option<i64>,
    /// The ingredients of the first tier in stored order, derived from `tiers`: a selection is
    /// flattened depth first, so its count, quantity multipliers and quality modifiers are not
    /// represented here.
    pub(crate) ingredients: Vec<Ingredient<'a>>,
    /// Every tier in stored order with its craft time and its costs, selections kept as
    /// `select` lines. The first one is the tier `craftTimeSeconds` and `ingredients` describe.
    pub(crate) tiers: Vec<BlueprintTier<'a>>,
    /// Whether the blueprint is one the player has by default
    /// (`CraftingGlobalParams.defaultBlueprintSelection`).
    pub(crate) default_available: bool,
    /// What dismantling the produced item returns, `null` without a produced item, for an item
    /// that cannot be dismantled, and unless the Build holds exactly one dismantle process.
    pub(crate) dismantle: Option<BlueprintDismantle>,
}

/// One ingredient line of a blueprint: a resource or an item with its quantity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Ingredient<'a> {
    /// The GUID of the `ResourceType` Record of a resource line, `null` for an item line.
    pub(crate) resource_guid: Option<CatalogGuid>,
    /// The GUID of the entity of an item line, `null` for a resource line.
    pub(crate) item_guid: Option<CatalogGuid>,
    /// The quantity of a resource line in standard cargo units (SCU), `null` for an item line.
    pub(crate) quantity_scu: Option<f32>,
    /// The quantity of an item line in units, `null` for a resource line.
    pub(crate) quantity_units: Option<i64>,
    /// The minimum quality the ingredient must have, `null` without one.
    pub(crate) min_quality: Option<i64>,
    /// The debug name of the slot the ingredient fills, `null` without one.
    pub(crate) slot: Option<&'a str>,
    /// The localisation key of the slot name, `null` without one or for a placeholder.
    pub(crate) slot_key: Option<&'a str>,
}

/// A blueprint category: a `BlueprintCategoryRecord` Record under `crafting/blueprintcategories`,
/// the target of the `categoryGuid` of a blueprint.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintCategory<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `FPSArmours`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name; the Record holds no field for one, so the key is `null`.
    pub(crate) name: LocalisedText<'a>,
}
