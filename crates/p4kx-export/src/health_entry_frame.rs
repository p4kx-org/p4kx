// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, ArchiveInputOrigin, EntryOrigin, SourceEvidenceError, SourceUses};

use super::health_reference_receipt::ReferenceFrame;
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

/// The actual root Archive and evidence inventory of one report preparation.
#[derive(Debug)]
pub(crate) struct EntryFrame {
    root: ArchiveInputOrigin,
    sources: SourceUses,
}

impl EntryFrame {
    pub(crate) fn new(archive: &Archive, sources: &SourceUses) -> Self {
        Self {
            root: archive.input_origin().clone(),
            sources: sources.clone(),
        }
    }

    pub(super) fn check_archive(&self, origin: &ArchiveInputOrigin) -> Result<(), CatalogError> {
        if !self.root.is_same_input(origin) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(super) fn check_origin(&self, origin: &EntryOrigin) -> Result<(), CatalogError> {
        if !self.root.is_same_input(origin.input_origin())
            || origin.binding().has_unknown_ancestry()
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        let other = input.origins().entry_frame();
        self.check_frame(other)
    }

    pub(super) fn check_frame(&self, other: &Self) -> Result<(), ExportAccountingError> {
        if !self.root.is_same_input(&other.root) || !self.sources.is_same_inventory(&other.sources)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }

    pub(super) const fn sources(&self) -> &SourceUses {
        &self.sources
    }
}

trait RecordFrameInput: Sync {
    fn check_record_frame(&self, frame: &ReferenceFrame) -> Result<(), ExportAccountingError>;
}

impl RecordFrameInput for OriginalCatalogInput<'_, '_> {
    fn check_record_frame(&self, frame: &ReferenceFrame) -> Result<(), ExportAccountingError> {
        frame.check_input(self)
    }
}

/// A borrowed capture context; owned receipts keep only its original immutable frame.
pub(crate) struct EntryCapture<'input> {
    frame: &'input Arc<EntryFrame>,
    texts: &'input super::HealthTextCapture,
    record_input: Option<&'input dyn RecordFrameInput>,
}

impl<'input> EntryCapture<'input> {
    pub(crate) fn for_input(input: &'input OriginalCatalogInput<'_, '_>) -> Self {
        Self {
            frame: input.origins().entry_frame(),
            texts: input.origins().health_texts(),
            record_input: Some(input),
        }
    }

    pub(crate) fn check_archive(&self, origin: &ArchiveInputOrigin) -> Result<(), CatalogError> {
        self.frame.check_archive(origin)
    }

    pub(crate) fn check_origin(&self, origin: &EntryOrigin) -> Result<(), CatalogError> {
        self.frame.check_origin(origin)
    }

    pub(super) fn check_record_frame(
        &self,
        frame: &ReferenceFrame,
    ) -> Result<(), ExportAccountingError> {
        self.record_input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?
            .check_record_frame(frame)
    }

    pub(super) fn frame(&self) -> Arc<EntryFrame> {
        Arc::clone(self.frame)
    }

    pub(crate) fn source_uses(&self) -> &SourceUses {
        self.frame.sources()
    }

    pub(super) fn text(&self, value: &str) -> Result<crate::HealthText, CatalogError> {
        Ok(self.texts.retain_text(value)?.text().clone())
    }
}

#[cfg(test)]
pub(crate) fn with_entry_capture<T>(
    archive: &Archive,
    read: impl FnOnce(&EntryCapture<'_>) -> Result<T, CatalogError>,
) -> Result<T, CatalogError> {
    let sources = SourceUses::new();
    let frame = Arc::new(EntryFrame::new(archive, &sources));
    let texts = super::HealthTextCapture::new();
    read(&EntryCapture {
        frame: &frame,
        texts: &texts,
        record_input: None,
    })
}

#[cfg(test)]
#[path = "health_starmap_attempt_tests.rs"]
mod starmap_attempt_tests;
