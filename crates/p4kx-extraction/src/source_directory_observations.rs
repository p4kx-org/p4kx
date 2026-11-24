// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_archive::Archive;

use crate::DecimalU64;
use crate::source_evidence_error::{self, SourceEvidenceError};
use crate::source_inventory::SourceEncryptionState;
use crate::source_scope::SourceScope;
use crate::source_use::{InputScope, SourceLocation, SourceUse, SourceUseRole};

use super::SourceUses;

impl SourceUses {
    /// Retains a complete survey of the admitted paths in this root Archive directory.
    /// Call after consulting every admitted path. A nested Archive returns no root survey.
    ///
    /// # Errors
    /// Returns an error when a mutation interrupted the shared evidence registry.
    pub fn observe_root_directory_paths(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory_path_prefix(archive, archive.entries().len())
    }

    /// Retains the admitted Entry count of the original root Archive directory.
    ///
    /// # Errors
    /// Returns an error if the shared evidence registry cannot register the observation.
    pub fn observe_root_directory_entry_count(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory(archive, |_| {
            Ok(SourceLocation::Metadata {
                field: "entryCount".into(),
            })
        })
    }

    /// Observes the original v2 root installation flags byte when available.
    ///
    /// # Errors
    /// Returns an error when the original source cannot register the use.
    pub fn observe_root_installation_flags(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        let Some(trailer) = archive.metadata().v2_directory() else {
            return Ok(None);
        };
        let Some(scope) = self.root_directory_scope(archive)? else {
            return Ok(None);
        };
        let offset = trailer.flags_offset();
        scope
            .at(
                SourceLocation::Subresource {
                    namespace: "archiveV2Trailer".into(),
                    indices: vec![offset, offset + 1],
                    field: "flags".into(),
                },
                SourceUseRole::Payload,
            )
            .map(Some)
    }

    /// Retains the consulted prefix of the original root Archive directory.
    /// The prefix is in admitted Entry order.
    ///
    /// # Errors
    /// Returns an error when the prefix exceeds the admitted directory or registration fails.
    pub fn observe_root_directory_path_prefix(
        &self,
        archive: &Archive,
        end: usize,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory(archive, |count| {
            let end = end as u64;
            if end > count.get() {
                return Err(SourceEvidenceError::InvalidDirectoryRange);
            }
            Ok(SourceLocation::ArchiveDirectoryPaths {
                start_entry_index: DecimalU64::new(0),
                end_entry_index: DecimalU64::new(end),
            })
        })
    }

    /// Retains the actual folder tree path range of a root Archive.
    /// Pass positions from the owning root Archive's `EntryTree`.
    ///
    /// # Errors
    /// Returns an error for a reversed or out-of-bounds range or interrupted evidence registry.
    pub fn observe_root_directory_tree_paths(
        &self,
        archive: &Archive,
        paths: Range<u32>,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observe_root_directory(archive, |count| {
            let start = u64::from(paths.start);
            let end = u64::from(paths.end);
            if start > end || end > count.get() {
                return Err(SourceEvidenceError::InvalidDirectoryRange);
            }
            Ok(SourceLocation::ArchiveDirectoryTreePaths {
                start_entry_index: DecimalU64::new(start),
                end_entry_index: DecimalU64::new(end),
            })
        })
    }

    fn observe_root_directory(
        &self,
        archive: &Archive,
        location: impl FnOnce(DecimalU64) -> Result<SourceLocation, SourceEvidenceError>,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        let Some(scope) = self.root_directory_scope(archive)? else {
            return Ok(None);
        };
        let count = DecimalU64::new(archive.entries().len() as u64);
        Ok(Some(scope.at(location(count)?, SourceUseRole::Selector)?))
    }

    pub(crate) fn root_directory_scope(
        &self,
        archive: &Archive,
    ) -> Result<Option<SourceScope>, SourceEvidenceError> {
        let Some(count) = archive.root_directory_entry_count() else {
            return Ok(None);
        };
        let source = source_evidence_error::lock(&self.registry)?
            .root_directory(archive.input_origin(), DecimalU64::new(count as u64));
        Ok(Some(SourceScope {
            registry: Arc::clone(&self.registry),
            source,
            payload: SourceEncryptionState::Unknown,
            metadata: SourceEncryptionState::Unencrypted,
            scope: InputScope::ArchiveDirectoryMetadata,
        }))
    }
}
