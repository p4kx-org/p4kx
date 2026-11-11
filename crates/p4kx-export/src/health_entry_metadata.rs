// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Archive, Entry, EntryOrigin, InputScope, ObservedEntry, ObservedEntryBinding,
    SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
};

use super::{EntryCapture, EntryFrame};
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Original Entry ownership with shared registered declarations and exact pooled path text.
#[derive(Debug, Clone)]
pub(super) struct EntryMetadata {
    frame: Arc<EntryFrame>,
    scope: SourceScope,
    binding: ObservedEntryBinding,
    path: HealthText,
}

impl EntryMetadata {
    pub(super) fn observe(
        capture: &EntryCapture<'_>,
        archive: &Archive,
        entry: &Entry,
    ) -> Result<Self, CatalogError> {
        Self::from_view(capture, checked_view(capture, archive, entry)?)
    }

    pub(super) fn from_view(
        capture: &EntryCapture<'_>,
        view: ObservedEntry<'_>,
    ) -> Result<Self, CatalogError> {
        capture.check_archive(view.input_origin())?;
        let binding = view.binding();
        if binding.has_unknown_ancestry() {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(Self {
            frame: capture.frame(),
            scope: capture.frame().sources().observe(
                SourceInput::ArchiveView(view),
                InputScope::ArchiveDirectoryMetadata,
            )?,
            binding,
            path: capture.text(view.entry().path())?,
        })
    }

    pub(super) fn from_origin(
        capture: &EntryCapture<'_>,
        origin: &EntryOrigin,
    ) -> Result<Self, CatalogError> {
        capture.check_origin(origin)?;
        Ok(Self {
            frame: capture.frame(),
            scope: capture.frame().sources().observe(
                SourceInput::Archive(origin),
                InputScope::ArchiveDirectoryMetadata,
            )?,
            binding: origin.binding(),
            path: capture.text(origin.entry().path())?,
        })
    }

    pub(super) fn check_frame(&self, frame: &EntryFrame) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(frame)
    }

    pub(super) fn payload_at(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self
            .scope
            .for_scope(InputScope::Payload)
            .at(location, role)?)
    }

    pub(super) const fn binding(&self) -> &ObservedEntryBinding {
        &self.binding
    }
    pub(super) const fn source_ref(&self) -> &p4kx_extraction::PhysicalSourceRef {
        self.scope.source_ref()
    }

    pub(super) const fn path(&self) -> &HealthText {
        &self.path
    }
    pub(super) const fn index(&self) -> u32 {
        self.binding.entry_index()
    }

    pub(super) fn metadata(
        &self,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self.scope.at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            role,
        )?)
    }

    pub(super) fn check_entry(&self, archive: &Archive, entry: &Entry) -> Result<(), CatalogError> {
        let view = archive
            .observed_entry(entry)
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        if !self.scope.matches_entry(&view)? {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(super) fn attempted_payload(&self) -> Result<(), CatalogError> {
        self.scope.for_scope(InputScope::Payload).at(
            SourceLocation::Subresource {
                namespace: "archiveEntryRead".to_owned(),
                indices: Vec::new(),
                field: "content".to_owned(),
            },
            SourceUseRole::Attempted,
        )?;
        Ok(())
    }
}

pub(super) fn checked_view<'archive>(
    capture: &EntryCapture<'_>,
    archive: &'archive Archive,
    entry: &'archive Entry,
) -> Result<ObservedEntry<'archive>, CatalogError> {
    capture.check_archive(archive.input_origin())?;
    let view = archive
        .observed_entry(entry)
        .ok_or(SourceEvidenceError::ForeignEvidence)?;
    if view.binding().has_unknown_ancestry() {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    Ok(view)
}
