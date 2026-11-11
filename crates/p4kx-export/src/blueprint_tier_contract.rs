// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::blueprint_research_contract::BlueprintResearch;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Contextual interpretation of one actual Blueprint tier and its authored recipe.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintTier {
    pub(crate) blueprint: PhysicalBlueprintRef,
    pub(crate) source: SourceValueRef,
    pub(crate) craft_time_seconds: SourcedValue<ResourceInt64>,
    pub(crate) mandatory: Option<SourceValueRef>,
    pub(crate) optional: Vec<SourceValueRef>,
    pub(crate) costs: Vec<BlueprintCost>,
    pub(crate) research: BlueprintResearch,
}

/// One interpreted cost whose authored source and context remain in the Blueprint arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintCost {
    pub(crate) source: SourceValueRef,
    pub(crate) context: crate::blueprint_context_contract::BlueprintCostContext,
    pub(crate) meaning: BlueprintCostMeaning,
}

/// The selection or ingredient interpretation of a recognized authored cost.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum BlueprintCostMeaning {
    Select {
        count: SourcedValue<ResourceInt64>,
        debug_name: Option<SourceValueRef>,
        #[schemars(extend("x-gameText" = true))]
        display_name: SourcedValue<GameTextRef>,
        options: Vec<SourceValueRef>,
    },
    Resource {
        resource_guid: SourcedValue<CatalogGuid>,
        quantity_scu: SourcedValue<f32>,
        min_quality: SourcedValue<ResourceInt64>,
    },
    Item {
        item_guid: SourcedValue<CatalogGuid>,
        quantity_units: SourcedValue<ResourceInt64>,
        min_quality: SourcedValue<ResourceInt64>,
    },
}
