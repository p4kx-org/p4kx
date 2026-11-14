// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract_hauling::{HaulingItemKind, HaulingSource};
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_refs::SourceFieldRef;

/// A selected order group whose original requirements stay in their canonical source cells.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateHaulingOrder {
    pub(crate) source: HaulingSource,
    pub(crate) property: Option<SourceValueRef>,
    pub(crate) variable: Option<SourceFieldRef>,
    pub(crate) requirements: Vec<TemplateHaulingRequirement>,
}

/// The established OR/AND grouping over actual original hauling item occurrences.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateHaulingRequirement {
    pub(crate) options: Vec<Vec<TemplateHaulingItem>>,
}

/// A recognized item kind without a second copy of its quantities or target identities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateHaulingItem {
    pub(crate) source: SourceValueRef,
    pub(crate) kind: HaulingItemKind,
}
