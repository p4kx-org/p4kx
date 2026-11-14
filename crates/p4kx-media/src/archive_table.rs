// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveMetadataVersion, ArchiveRegion, DecimalU64};
use schemars::JsonSchema;
use serde::Serialize;

use crate::archive_raw::ArchiveRawSpan;

/// Comparison of authored progress to an admitted physical Entry's stored size.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveProgressBound {
    /// An omitted record provides no admitted Entry size to compare.
    Unavailable,
    /// Progress does not exceed the stored size.
    WithinStoredSize,
    /// Progress exceeds the stored size.
    ExceedsStoredSize,
}

/// Separate meaning of an optional authored reserved word.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveReservedState {
    /// This row format has no reserved word.
    NotPresent,
    /// The original reserved word is zero.
    Zero,
    /// The original reserved word is malformed nonzero data.
    Nonzero,
}

/// Authored installation progress, not a verified amount of payload.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveInstallation {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Original table ordinal before folder admission.
    pub ordinal: DecimalU64,
    /// Exact original fixed row bytes.
    pub raw: ArchiveRawSpan,
    /// Original progress word.
    pub written: DecimalU64,
    /// Original reserved word, absent only in v2's eight-byte format.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub reserved: Option<u32>,
    /// Independent reserved-word validation.
    pub reserved_state: ArchiveReservedState,
    /// Independent progress bound check.
    pub bound: ArchiveProgressBound,
}

/// Structural validity of a declared free-space extent.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveFreeState {
    /// Nonempty extent inside the owning source.
    WithinSource,
    /// Authored extent has no bytes.
    ZeroSize,
    /// Authored extent exceeds the owning source.
    OutsideSource,
    /// Offset-plus-size overflows the source width.
    Overflow,
}

/// Original free-space row without interpreting filesystem allocation.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveFreeRange {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Original free-list ordinal.
    pub ordinal: DecimalU64,
    /// Exact original fixed row bytes.
    pub raw: ArchiveRawSpan,
    /// Original extent words and their independently readable prefix.
    pub extent: ArchiveRegion,
    /// Validation of this declared free range.
    pub state: ArchiveFreeState,
}
