// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_read_error::MetadataReadError;
use crate::metadata_region::{MetadataRegion, MetadataRegionStatus};
use crate::metadata_section::MetadataSection;
use crate::v2_directory_metadata::V2DirectoryMetadata;
use crate::zip_directory_metadata::ZipDirectoryMetadata;

/// Layout route actually used by the reader, without inventing a declared v1 word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArchiveLayoutKind {
    /// Classic ZIP declarations without a selected ZIP64 locator.
    Zip,
    /// A ZIP64 directory, including P4K v1.
    Zip64,
    /// A fixed v2 trailer and directory.
    P4kV2,
    /// Surviving records were scanned without a proved global directory layout.
    RecoveredUnknown,
}

/// Availability of layout declarations, independently of payload verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArchiveMetadataStatus {
    /// Header declarations have complete bounded extents; lazy row checks remain separate.
    Available,
    /// A retained declaration has an invalid or truncated metadata extent.
    Malformed,
    /// A recovered inventory has no proved global layout declaration.
    Recovered,
}

#[derive(Debug, PartialEq, Eq)]
enum Layout {
    Zip(Box<ZipDirectoryMetadata>),
    V2(V2DirectoryMetadata),
    Recovered,
}

/// Read-only source-relative layout declarations for one owning Archive source.
#[derive(Debug, PartialEq, Eq)]
pub struct ArchiveMetadata {
    source_len: u64,
    layout: Layout,
}

impl ArchiveMetadata {
    pub(crate) fn zip(source_len: u64, metadata: &ZipDirectoryMetadata) -> Self {
        Self {
            source_len,
            layout: Layout::Zip(Box::new(*metadata)),
        }
    }

    pub(crate) const fn v2(source_len: u64, metadata: V2DirectoryMetadata) -> Self {
        Self {
            source_len,
            layout: Layout::V2(metadata),
        }
    }

    pub(crate) const fn recovered(source_len: u64) -> Self {
        Self {
            source_len,
            layout: Layout::Recovered,
        }
    }

    /// Returns this source's length, which can differ from the backing Archive file length.
    #[must_use]
    pub const fn source_len(&self) -> u64 {
        self.source_len
    }

    /// Returns the actual layout route or explicitly unknown recovered layout.
    #[must_use]
    pub fn layout(&self) -> ArchiveLayoutKind {
        match &self.layout {
            Layout::Zip(value) if value.zip64.is_some() => ArchiveLayoutKind::Zip64,
            Layout::Zip(_) => ArchiveLayoutKind::Zip,
            Layout::V2(_) => ArchiveLayoutKind::P4kV2,
            Layout::Recovered => ArchiveLayoutKind::RecoveredUnknown,
        }
    }

    /// Returns original ZIP declarations, absent for v2 or an unknown recovered layout.
    #[must_use]
    pub fn zip_directory(&self) -> Option<&ZipDirectoryMetadata> {
        match &self.layout {
            Layout::Zip(value) => Some(value),
            _ => None,
        }
    }

    /// Returns original v2 declarations, absent for ZIP or an unknown recovered layout.
    #[must_use]
    pub const fn v2_directory(&self) -> Option<&V2DirectoryMetadata> {
        match &self.layout {
            Layout::V2(value) => Some(value),
            _ => None,
        }
    }

    /// Returns header extent availability without changing payload or installation status.
    #[must_use]
    pub fn status(&self) -> ArchiveMetadataStatus {
        match &self.layout {
            Layout::Recovered => ArchiveMetadataStatus::Recovered,
            Layout::Zip(value) if !value.is_complete() => ArchiveMetadataStatus::Malformed,
            Layout::V2(value)
                if value.trie(self.source_len).status() != MetadataRegionStatus::Complete =>
            {
                ArchiveMetadataStatus::Malformed
            }
            _ => ArchiveMetadataStatus::Available,
        }
    }

    pub(crate) fn region(
        &self,
        section: MetadataSection,
    ) -> Result<Option<MetadataRegion>, MetadataReadError> {
        match &self.layout {
            Layout::Zip(value) => crate::metadata_regions::zip(value, section, self.source_len),
            Layout::V2(value) => crate::metadata_regions::v2(value, section, self.source_len),
            Layout::Recovered => Err(MetadataReadError::UnavailableLayout),
        }
    }
}
