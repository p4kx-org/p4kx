// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::LoadoutFileObservation;

use p4kx_extraction::{InputScope, SourceInput, SourceLocation, SourceUseRole, SourceValue};

use super::FileSelection;
use super::OriginalDatum;
use super::health_entry_frame::{EntryCapture, EntryFrame};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// An original operation and its normalized actual selection links.
#[derive(Debug, Clone)]
pub(crate) struct LoadoutBodyReceipt {
    frame: Arc<EntryFrame>,
    selections: Vec<FileSelection>,
    path: HealthText,
}

impl LoadoutBodyReceipt {
    pub(crate) fn read(
        capture: &EntryCapture<'_>,
        selection: FileSelection,
    ) -> Result<Self, CatalogError> {
        let original = selection.original()?;
        let source = original.source();
        if let Some(input) = source.archive_input_origin() {
            capture.check_archive(input)?;
        }
        let path = if let Some(origin) = source.entry_origin() {
            capture.frame().check_origin(origin)?;
            capture.text(origin.entry().path())?
        } else {
            let candidate = selection.candidate()?;
            if candidate.as_str() != source.identity().path {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            candidate.text().clone()
        };
        Ok(Self {
            frame: capture.frame(),
            selections: vec![selection],
            path,
        })
    }

    pub(crate) const fn path(&self) -> &HealthText {
        &self.path
    }

    pub(super) fn original(&self) -> Result<LoadoutFileObservation, CatalogError> {
        self.selections
            .first()
            .ok_or(ExportAccountingError::UnobservedFailure)?
            .original()
    }

    pub(super) fn check_frame(&self, frame: &EntryFrame) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(frame)
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.frame.check_input(input)?;
        for selection in &self.selections {
            selection.check_initial_input(input)?;
        }
        Ok(())
    }

    pub(super) fn path_fact(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let mut inputs = Vec::new();
        let mut complete = true;
        for selection in &self.selections {
            let intrinsic = self.intrinsic(selection)?;
            let (original, known) = intrinsic.contributions();
            inputs.extend_from_slice(original);
            complete &= known;
            for path in selection.initial_selectors()? {
                let (selected, known) = path.selector_inputs()?;
                inputs.extend(selected);
                complete &= known;
            }
        }
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.path.clone()),
            inputs,
            complete,
        ))
    }

    fn intrinsic(
        &self,
        selection: &FileSelection,
    ) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let original = selection.original()?;
        let candidate = selection.candidate()?;
        let Some(origin) = original.source().entry_origin() else {
            return candidate.retain();
        };
        let scope = self.frame.sources().observe(
            SourceInput::Archive(origin),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let mut inputs = vec![scope.at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Payload,
        )?];
        let (selected, complete) = candidate.selector_inputs()?;
        inputs.extend(selected);
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.path.clone()),
            inputs,
            complete,
        ))
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(&other.frame)?;
        if self.path != other.path {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.selections.extend(other.selections);
        Ok(())
    }
}
