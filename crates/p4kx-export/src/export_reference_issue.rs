// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::export_diagnostic::ExportSourceRole;
use crate::guid_text::CatalogGuid;
use crate::selection::Kind;

/// The retained source state of a Reference or instance pointer that cannot be followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub enum ExportReferenceStatus {
    /// A declared GUID names no source Record.
    Dangling,
    /// A weak pointer names an instance outside the original owning Record.
    OutsideOwner,
    /// A pointer names an unavailable struct or instance.
    InvalidPointer,
    /// The declared target type does not match the actual target.
    UnexpectedTargetType,
    /// A strong pointer attempts to own an instance a second time.
    OwnedTwice,
    /// A present source target is outside the catalog kind's record rule.
    OutsideCatalogSelection,
    /// The catalog Filter intentionally excludes the target.
    ExcludedByFilter,
    /// The catalog development-leftover policy intentionally excludes the target.
    ExcludedByPolicy,
    /// The target's source metadata or canonical tree could not be read.
    TargetReadFailed,
    /// The target source could not be projected into its catalog row.
    TargetProjectionFailed,
    /// A projected target has no emitted row or exact intentional row exclusion.
    TargetNotEmitted,
    /// A persistent catalog identity cannot identify one physical emitted target.
    AmbiguousIdentity,
}

/// Available raw source coordinates of an unresolved instance pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportSourcePointer {
    /// The source struct index when available.
    pub struct_index: Option<u32>,
    /// The source instance index when available.
    pub instance_index: Option<u32>,
    /// The field's declared source struct index when available.
    pub declared_type_index: Option<u32>,
}

/// One unresolved source link with its exact owner and field identity, without sampling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportUnresolvedReference {
    /// The physical owning Record-table index in this source build.
    pub record_index: u32,
    /// The affected catalog kind, null for a catalog-wide configuration or dependency.
    pub kind: Option<Kind>,
    /// The relationship of the owner to the affected kind.
    pub source_role: ExportSourceRole,
    /// The original owning source Record GUID.
    #[schemars(with = "CatalogGuid")]
    pub record_guid: String,
    /// The original owning source Record path.
    pub record_path: String,
    /// The canonical field location inside the original owner.
    pub field: String,
    /// The source struct that declares the field.
    pub source_type: String,
    /// The declared target GUID when available, including a dangling GUID.
    #[schemars(with = "Option<CatalogGuid>")]
    pub target_guid: Option<String>,
    /// The resolved target's physical Record-table index in this source Build, null when unknown.
    pub target_record_index: Option<u32>,
    /// The known target Record path, null for an unavailable target.
    pub target_record_path: Option<String>,
    /// The known target struct name, null for an unavailable target.
    pub target_type: Option<String>,
    /// The known target's canonical location, null when unavailable.
    pub target_location: Option<String>,
    /// The retained source state.
    pub status: ExportReferenceStatus,
    /// Available stored instance coordinates, null for a GUID-only Reference.
    pub source_pointer: Option<ExportSourcePointer>,
}
