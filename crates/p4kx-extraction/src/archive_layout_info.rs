// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{ArchiveLayoutKind, ArchiveMetadata, ArchiveMetadataStatus};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ArchiveMetadataVersion, ArchiveV2Layout, ArchiveZipLayout, DecimalU64};

/// Actual reader route independently of an authored layout-version word.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveLayout {
    /// Classic ZIP declarations.
    Zip,
    /// ZIP64 declarations, including positively recognised P4K v1.
    Zip64,
    /// Fixed P4K v2 trailer and directory.
    P4kV2,
    /// Surviving records without a proved global layout.
    RecoveredUnknown,
    /// A reader route this projection does not interpret.
    Unknown,
}

/// Cached header declaration availability without a lazy-row or payload-verification claim.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveDeclarationState {
    /// Header extents are available; record and table checks remain lazy.
    Available,
    /// A retained extent declaration is malformed.
    Malformed,
    /// Recovery did not establish global declarations.
    Recovered,
    /// The reader exposes an uninterpreted declaration state.
    Unknown,
}

/// Shared cached root/nested layout declarations without raw sidecars or additional I/O.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = crate::archive_projection_schema::require_properties)]
#[non_exhaustive]
pub struct ArchiveLayoutInfo {
    /// Source-layout payload contract independently of any media envelope.
    pub version: ArchiveMetadataVersion,
    /// Immediately owning source length, not the backing root file length.
    pub source_length: DecimalU64,
    /// Reader route actually used for this source.
    pub layout: ArchiveLayout,
    /// Availability of cached headers, without claiming valid lazy records.
    pub declaration_state: ArchiveDeclarationState,
    /// Original ZIP/ZIP64 words, absent for other known routes.
    pub zip: Option<ArchiveZipLayout>,
    /// Original v2 words, absent for other known routes.
    pub v2: Option<ArchiveV2Layout>,
}

impl ArchiveLayoutInfo {
    /// Projects cached declarations without reading bodies, signatures or table rows.
    #[must_use]
    pub fn of(metadata: &ArchiveMetadata) -> Self {
        let layout = match metadata.layout() {
            ArchiveLayoutKind::Zip => ArchiveLayout::Zip,
            ArchiveLayoutKind::Zip64 => ArchiveLayout::Zip64,
            ArchiveLayoutKind::P4kV2 => ArchiveLayout::P4kV2,
            ArchiveLayoutKind::RecoveredUnknown => ArchiveLayout::RecoveredUnknown,
            _ => ArchiveLayout::Unknown,
        };
        let declaration_state = match metadata.status() {
            ArchiveMetadataStatus::Available => ArchiveDeclarationState::Available,
            ArchiveMetadataStatus::Malformed => ArchiveDeclarationState::Malformed,
            ArchiveMetadataStatus::Recovered => ArchiveDeclarationState::Recovered,
            _ => ArchiveDeclarationState::Unknown,
        };
        Self {
            version: ArchiveMetadataVersion::V1,
            source_length: DecimalU64::new(metadata.source_len()),
            layout,
            declaration_state,
            zip: metadata.zip_directory().map(ArchiveZipLayout::of),
            v2: metadata.v2_directory().map(ArchiveV2Layout::of),
        }
    }
}
