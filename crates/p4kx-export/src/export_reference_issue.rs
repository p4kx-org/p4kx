// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

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

pub use crate::catalog_read::original::{ExportSourcePointer, ExportUnresolvedReference};
