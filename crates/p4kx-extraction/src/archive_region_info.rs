// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{MetadataRegion, MetadataRegionStatus};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ArchiveByteRange, DecimalU64};

/// Structural availability of the exact authored extent.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveRegionState {
    /// The entire declared extent is available.
    Complete,
    /// Only a prefix is available.
    Truncated,
    /// The start lies outside the source.
    OutsideSource,
    /// Source-width arithmetic overflowed.
    Overflow,
    /// The reader exposes a declaration state this projection does not interpret.
    Unknown,
}

/// Source declarations distinguished from their readable prefix.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = crate::archive_projection_schema::require_properties)]
#[non_exhaustive]
pub struct ArchiveRegion {
    /// Authored source-relative start.
    pub offset: DecimalU64,
    /// Authored length, even when invalid.
    pub declared_length: DecimalU64,
    /// Actual readable prefix, including an explicitly empty range.
    pub available: Option<ArchiveByteRange>,
    /// Independent extent validation.
    pub state: ArchiveRegionState,
}

impl ArchiveRegion {
    /// Projects cached extent declarations without reading source bytes.
    #[must_use]
    pub fn of(region: MetadataRegion) -> Self {
        let state = match region.status() {
            MetadataRegionStatus::Complete => ArchiveRegionState::Complete,
            MetadataRegionStatus::Truncated => ArchiveRegionState::Truncated,
            MetadataRegionStatus::OutsideSource => ArchiveRegionState::OutsideSource,
            MetadataRegionStatus::Overflow => ArchiveRegionState::Overflow,
            _ => ArchiveRegionState::Unknown,
        };
        Self {
            offset: DecimalU64::new(region.offset()),
            declared_length: DecimalU64::new(region.declared_len()),
            available: region.available().map(ArchiveByteRange::of),
            state,
        }
    }
}
