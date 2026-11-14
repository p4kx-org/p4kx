// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;

use super::data_type::SourceDataType;
use super::value::SourceValueRef;

/// One original field occurrence with its exact name and decode declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceField<'source> {
    pub(crate) name: SourcedValue<&'source str>,
    pub(crate) declaration: SourceDeclaration,
    pub(crate) value: SourceValueRef,
}

/// The actual original decoder storage of a field and its elements.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceDeclaration {
    pub(crate) data_type: SourcedValue<SourceDataType>,
    pub(crate) is_array: SourcedValue<bool>,
    pub(crate) target: SourcedValue<u16>,
}
