// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract;
use crate::guid_text::CatalogGuid;

/// The source identity and availability of an authored Record Reference.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct DefinitionReference {
    /// The resolved target position in this Build, null when no physical target is known.
    pub(crate) target_record_index: Option<u32>,
    /// The target GUID, retained when its Record is missing.
    pub(crate) guid: CatalogGuid,
    /// The target Record path when its source identity is available.
    pub(crate) record_path: Option<String>,
    /// The source target struct name when available.
    pub(crate) target_type: Option<String>,
    /// The source availability and declared-type compatibility of the target.
    pub(crate) status: DefinitionStatus,
}

/// Source availability of a Record Reference, independently of catalog selection.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum DefinitionStatus {
    /// The `DataForge` identifies the target Record.
    Present,
    /// The declared target GUID has no Record.
    Dangling,
    /// The target does not derive from the field's declared Record type.
    UnexpectedTargetType,
}
