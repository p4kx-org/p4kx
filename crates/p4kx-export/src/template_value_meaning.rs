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

#[path = "template_mission_item_selection.rs"]
mod mission_item;

pub(crate) use self::mission_item::dataset_conditions;
pub use self::mission_item::{TemplateMissionItemSelection, TemplateSpecificItems};

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
pub struct TemplateDatasetCondition {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: SourcedValue<Option<LocationConditionKind>>,
    pub(crate) specific_items: Option<TemplateSpecificItems>,
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
    pub(crate) mission_item: Option<TemplateMissionItemSelection>,
    pub(crate) scoped_filters: Vec<TemplateScopedFilter>,
}

impl TemplateDatasetCondition {
    /// The original condition occurrence in the source arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// The recognized condition operation and its source evidence.
    #[must_use]
    pub const fn kind(&self) -> &SourcedValue<Option<LocationConditionKind>> {
        &self.kind
    }

    /// The authored specific-item inputs when that interpretation applies.
    #[must_use]
    pub const fn specific_items(&self) -> Option<&TemplateSpecificItems> {
        self.specific_items.as_ref()
    }
}
