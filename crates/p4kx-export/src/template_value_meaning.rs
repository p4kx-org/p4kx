// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::location_condition_kind::LocationConditionKind;
use crate::schema_fields::every_field_required;
use crate::scoped_location_kind::ScopedLocationFilterKind;
use crate::source_value_catalog::SourceValueRef;
use crate::template_property_kind::PropertyValueKind;

/// The result of the required property interpretation independently of raw readability.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum TemplateValueStatus {
    Recognized,
    Unsupported,
    Malformed,
    Unreadable,
}

/// A condition's recognized operation while its payload remains in the canonical D cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateDatasetCondition {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: SourcedValue<Option<LocationConditionKind>>,
}

/// A `ScopedLocation` filter operation without a copied raw filter DTO.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateScopedFilter {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: SourcedValue<Option<ScopedLocationFilterKind>>,
}

/// Required source-shape interpretation and contextual navigation for one selected value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateValueMeaning {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: SourcedValue<Option<PropertyValueKind>>,
    pub(crate) status: TemplateValueStatus,
    pub(crate) conditions: Vec<TemplateDatasetCondition>,
    pub(crate) scoped_filters: Vec<TemplateScopedFilter>,
}
