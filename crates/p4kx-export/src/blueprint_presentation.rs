// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Per-Blueprint lookups and first-tier convenience navigation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintPresentation<Category> {
    pub(crate) blueprint: PhysicalBlueprintRef,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) name: SourcedValue<GameTextRef>,
    pub(crate) category: Category,
    pub(crate) produced_item: SourcedValue<CatalogGuid>,
    pub(crate) first_tier: Option<SourceValueRef>,
    pub(crate) ingredients: Vec<BlueprintIngredient>,
    pub(crate) default_available: SourcedValue<bool>,
    pub(crate) dismantle: SourcedValue<BlueprintDismantle>,
}

/// The first-tier ingredient and its nearest enclosing named selection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintIngredient {
    pub(crate) cost: SourceValueRef,
    pub(crate) slot: Option<SourceValueRef>,
}

/// The selected generic process and the tier whose costs it returns.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintDismantle {
    pub(crate) process_guid: CatalogGuid,
    pub(crate) tier: Option<SourceValueRef>,
}
