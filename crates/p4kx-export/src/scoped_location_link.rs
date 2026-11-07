// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// The source identity and availability of a location-vocabulary Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DefinitionReference {
    /// The resolved target position in this Build, null when no physical target is known.
    pub(crate) target_record_index: Option<u32>,
    /// The target GUID, retained when its Record is missing.
    pub(crate) guid: CatalogGuid,
    /// The target Record path when its source identity is available.
    pub(crate) record_path: Option<String>,
    /// The source target struct name when available.
    pub(crate) target_type: Option<String>,
    /// Whether the target Record is present in the source `DataForge`.
    pub(crate) status: DefinitionStatus,
}

/// Source availability of a definition Reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum DefinitionStatus {
    /// The `DataForge` identifies the target Record.
    Present,
    /// The declared target GUID has no Record.
    Dangling,
}

/// A source weak pointer and its resolution within its original owning Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PropertyReference {
    /// The resolved target position in this Build, null when no physical target is known.
    pub(crate) target_record_index: Option<u32>,
    /// The target Record GUID when known.
    pub(crate) record_guid: Option<CatalogGuid>,
    /// The target Record path when known.
    pub(crate) record_path: Option<String>,
    /// The source target struct name when known.
    pub(crate) target_type: Option<String>,
    /// The canonical source location within the target Record.
    pub(crate) location: Option<String>,
    /// The resolved `missionVariableName`, retaining an empty source string.
    pub(crate) variable: Option<String>,
    /// The owner-scoped resolution outcome.
    pub(crate) status: PropertyReferenceStatus,
    /// Raw instance coordinates retained for an unresolved source pointer.
    pub(crate) source_pointer: Option<SourcePointer>,
    /// The source read error when available, without a fabricated target value.
    pub(crate) source_error: Option<String>,
}

/// An unresolved weak pointer's stored struct and instance coordinates.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourcePointer {
    /// The stored struct index.
    pub(crate) struct_index: u32,
    /// The stored instance index.
    pub(crate) instance_index: u32,
}

/// The result of resolving a weak link within its original owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum PropertyReferenceStatus {
    /// The original owner contains the linked mission property.
    Resolved,
    /// The declared target cannot be located.
    MissingTarget,
    /// The target belongs outside the original owner.
    OutsideOwner,
    /// The linked instance has an unsupported target struct.
    UnsupportedTargetType,
    /// The target property's variable cannot be read.
    UnreadableTarget,
}
