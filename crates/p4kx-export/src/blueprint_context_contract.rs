// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// The original context and the actual effective cost rules.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintCostContext {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) quantity_multiplier: Option<SourcedValue<f32>>,
    pub(crate) composition_inclusion: Option<SourceValueRef>,
    pub(crate) modifiers: Vec<BlueprintModifier>,
}

/// An original property modifier and its known ordered ranges.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintModifier {
    pub(crate) source: SourceValueRef,
    pub(crate) property_guid: SourcedValue<CatalogGuid>,
    pub(crate) ranges: Vec<BlueprintModifierRange>,
}

/// The exact original range and its known finite interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintModifierRange {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: BlueprintRangeKind,
    pub(crate) start_quality: SourcedValue<ResourceInt64>,
    pub(crate) end_quality: SourcedValue<ResourceInt64>,
    pub(crate) at_start: SourcedValue<f32>,
    pub(crate) at_end: SourcedValue<f32>,
}

/// The supported gameplay property range interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum BlueprintRangeKind {
    Multiplier,
    Additive,
}
