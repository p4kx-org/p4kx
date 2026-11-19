// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ArchiveLayoutInfo, ArchiveLayoutOriginError, ArchiveLayoutOrigins};

#[path = "archive_summary_origins.rs"]
mod origins;

/// Cached layout and admitted Entry totals for one Archive view.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ArchiveSummary {
    /// Cached source layout without lazy metadata or payload validation.
    pub metadata: ArchiveLayoutInfo,
    /// The number of admitted Entries.
    pub entry_count: usize,
    /// The number of admitted Entries read as encrypted.
    pub encrypted_count: usize,
    /// The reported directory defect and recovery count.
    pub issue_count: usize,
    /// The sum of declared stored Entry sizes, saturating at `u64::MAX`.
    pub compressed_size: u64,
    /// The sum of declared decoded Entry sizes, saturating at `u64::MAX`.
    pub size: u64,
}

impl ArchiveSummary {
    /// Summarizes the cached Archive layout and admitted directory without reading payloads.
    #[must_use]
    pub fn of(archive: &Archive) -> Self {
        let entries = archive.entries();
        let mut summary = Self {
            metadata: ArchiveLayoutInfo::of(archive.metadata()),
            entry_count: entries.len(),
            encrypted_count: 0,
            issue_count: archive.issue_count(),
            compressed_size: 0,
            size: 0,
        };
        for entry in entries {
            summary.encrypted_count += usize::from(entry.is_encrypted());
            summary.compressed_size = summary
                .compressed_size
                .saturating_add(entry.compressed_size());
            summary.size = summary.size.saturating_add(entry.uncompressed_size());
        }
        summary
    }

    /// Projects the summary and known original sources under the given result JSON Pointer.
    ///
    /// # Errors
    /// Returns an error if source evidence or a field address cannot be represented.
    pub fn project_with_origins(
        archive: &Archive,
        address: &str,
    ) -> Result<(Self, ArchiveLayoutOrigins), ArchiveLayoutOriginError> {
        let summary = Self::of(archive);
        let origins = origins::capture(archive, &summary, address)?;
        Ok((summary, origins))
    }
}
