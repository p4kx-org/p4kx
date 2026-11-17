// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::guid_text::CatalogGuid;
use crate::source_pointer::SourcePointer;

/// A raw weak link resolved only within its original physical Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct OwnedReference {
    /// The actual target Record-table index when known.
    pub target_record_index: Option<u32>,
    /// The target Record GUID when known.
    pub record_guid: Option<CatalogGuid>,
    /// The target Record path when known.
    pub record_path: Option<String>,
    /// The target struct name when known.
    pub target_type: Option<String>,
    /// The canonical source location in the target Record.
    pub location: Option<String>,
    /// Availability within the original owner.
    pub status: OwnedReferenceStatus,
    /// Raw coordinates retained when the pointer cannot resolve.
    pub source_pointer: Option<SourcePointer>,
    /// The source decoding error when available.
    pub source_error: Option<String>,
}

/// The source availability of an owner-local weak link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum OwnedReferenceStatus {
    /// The original owner contains the target node.
    Resolved,
    /// The declared target is absent.
    MissingTarget,
    /// The target lies outside the original physical owner.
    OutsideOwner,
}
