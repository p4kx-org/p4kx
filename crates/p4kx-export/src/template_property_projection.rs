// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_property::PropertyKind;
use crate::template_property_kind::PropertyValueKind;
use crate::template_source_refs::DeclarationBinding;

/// The selected property view; every game value stays in its original canonical source cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplatePropertyProjection {
    pub(crate) source: SourceValueRef,
    pub(crate) objective: Option<SourceValueRef>,
    pub(crate) naming: SourceValueRef,
    pub(crate) kind: Option<PropertyKind>,
    pub(crate) variable: DeclarationBinding,
    pub(crate) token: DeclarationBinding,
    pub(crate) value: Option<SourceValueRef>,
    pub(crate) value_kind: Option<PropertyValueKind>,
    pub(crate) contract_property: Option<SourceValueRef>,
    pub(crate) linked_output: Option<SourceValueRef>,
}
