// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// The stored struct and instance coordinates of an unresolved source pointer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
pub(crate) struct SourcePointer {
    /// The stored struct index.
    pub(crate) struct_index: u32,
    /// The stored instance index.
    pub(crate) instance_index: u32,
}
