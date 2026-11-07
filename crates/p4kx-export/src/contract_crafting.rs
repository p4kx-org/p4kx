// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::contract_recipe::{BlueprintDismantle, BlueprintTier};
use crate::definition_reference::DefinitionReference;
use crate::guid_text::CatalogGuid;

/// A commodity: an `EntityClassDefinition` Record under `entities/commodities` or a
/// `ResourceType` Record that a blueprint cost or a quality quantization uses and no commodity
/// Record carries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Commodity<'a> {
    /// The unique row identity: the Entity GUID or the resource-only `ResourceType` GUID.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `Gold`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name, from `SCItemPurchasableParams.displayName`.
    pub(crate) name: LocalisedText<'a>,
    /// The description, from the entity's display parameters or, without one, from the
    /// `ResourceType` Record of the commodity.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
    /// The GUID of the `ResourceTypeGroup` Record that directly lists the `ResourceType`, the
    /// `guid` of a `resourceGroups` entry; `null` without a `ResourceType` or a group.
    pub(crate) type_guid: Option<CatalogGuid>,
    /// The nullable `ResourceType` link from the first non-null default-composition Reference.
    /// Several commodities can share it; reverse joins return zero, one or many commodities.
    pub(crate) subtype_guid: Option<CatalogGuid>,
    /// The display type of the purchasable parameters, e.g. `@items_commodities_type_metal`;
    /// without a key for a `ResourceType` entry.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: LocalisedText<'a>,
    /// The density of the `ResourceType` in grams per cubic centimetre (game field
    /// `gramsPerCubicCentimeter`), `null` without a `ResourceType`, a density or a density in
    /// another unit.
    pub(crate) density_grams_per_cubic_centimetre: Option<f32>,
    /// The GUID of the `ResourceType` Record `refinedVersion` of the `ResourceType` links to; `null`
    /// without a `ResourceType` or a `refinedVersion`. It equals the `subtypeGuid` of the refined
    /// commodity only when the catalog has that commodity.
    pub(crate) refined_subtype_guid: Option<CatalogGuid>,
}

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

/// A resource group: a `ResourceTypeGroup` Record under `resourcetypedatabase`, the type the
/// `typeGuid` of a commodity points at.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceGroup<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `Metal`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name, from `displayName`; a placeholder key leaves `en` and `de` `null`.
    pub(crate) name: LocalisedText<'a>,
    /// The GUID of the group that lists this group in its `groups`, the `guid` of another
    /// `resourceGroups` entry; `null` when no selected group lists it. The first selected listing
    /// wins; raw `groups` retain all memberships, and parent traversal requires a cycle guard.
    pub(crate) parent_guid: Option<CatalogGuid>,
    /// The localised source description.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
    /// The raw source thumbnail path, including an empty string.
    pub(crate) default_thumbnail_path: Option<&'a str>,
    /// The raw source SVG thumbnail path, including an empty string.
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: Option<&'a str>,
    /// Every authored child-group Reference in stored order, including nulls and repetitions.
    pub(crate) groups: Option<Vec<Option<DefinitionReference>>>,
    /// Every authored resource Reference in stored order, including nulls and repetitions.
    pub(crate) resources: Option<Vec<Option<DefinitionReference>>>,
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
