// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutFileError, LoadoutFileIdentity, LoadoutFileObservation,
    LoadoutPath, LoadoutRequest, LoadoutSelection,
};

use crate::entry_source::EntrySource;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportAccountingError, SourceDisposition};
use crate::{CatalogError, HealthText};
use p4kx_extraction::{InputScope, SourceInput, SourceLocation, SourceUseRole};

use super::Capture;
use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, ExampleBodyReceipt, LoadoutBodyReceipt, RecordSubject,
};
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};

const fn operation(kind: LoadoutDocumentKind) -> EntryOperation {
    match kind {
        LoadoutDocumentKind::XmlLoadout => EntryOperation::XmlLoadout,
        LoadoutDocumentKind::VehicleImplementation => EntryOperation::VehicleImplementation,
        LoadoutDocumentKind::VehiclePatch => EntryOperation::VehiclePatch,
    }
}

const fn bytes_operation(kind: LoadoutDocumentKind) -> EntryOperation {
    match kind {
        LoadoutDocumentKind::XmlLoadout => EntryOperation::XmlLoadoutBytes,
        LoadoutDocumentKind::VehicleImplementation => EntryOperation::VehicleImplementationBytes,
        LoadoutDocumentKind::VehiclePatch => EntryOperation::VehiclePatchBytes,
    }
}

impl Capture<'_> {
    fn file_observation(
        &self,
        original: &LoadoutFileObservation,
        operation: EntryOperation,
        disposition: SourceDisposition,
        code: Option<ExportDiagnosticCode>,
        receipt: Option<LoadoutBodyReceipt>,
    ) -> Result<ArchiveEntryObservation, CatalogError> {
        let diagnosis = EntryDiagnosis::loadout(code);
        match (self.input, receipt) {
            (Some(input), Some(receipt)) => {
                let source =
                    EntrySource::of_operation(&EntryCapture::for_input(input), receipt, operation)?
                        .dependency();
                Ok(source.observation(self.kind, disposition, diagnosis))
            }
            (None, None) => {
                let identity = original.source().identity();
                Ok(ArchiveEntryObservation {
                    operation,
                    kind: self.kind,
                    entry_index: identity.entry_index,
                    container_entry_index: None,
                    path: HealthText::new(identity.path),
                    role: ExportSourceRole::Dependency,
                    disposition,
                    diagnosis,
                    receipt: None,
                })
            }
            _ => Err(ExportAccountingError::UnboundHealthSubject.into()),
        }
    }

    fn checked_file(
        &self,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
    ) -> Result<(), CatalogError> {
        let Some(input) = self.input else {
            return Ok(());
        };
        let capture = EntryCapture::for_input(input);
        let source = original.source();
        if let Some(archive) = source.archive_input_origin() {
            capture.check_archive(archive)?;
        }
        if let Some(origin) = source.entry_origin() {
            capture.check_origin(origin)?;
        }
        let recipe = self.lock()?.paths.get(request)?;
        recipe.check_input(input)?;
        if source.entry_origin().is_none() && source.identity().path != request.value() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    fn attempted_file(&self, original: &LoadoutFileObservation) -> Result<(), CatalogError> {
        let Some(input) = self.input else {
            return Ok(());
        };
        if matches!(
            original.read_outcome(),
            Ok(()) | Err(LoadoutFileError::Unreadable { .. })
        ) {
            let source = original
                .source()
                .entry_origin()
                .map_or(SourceInput::Unknown, SourceInput::Archive);
            input
                .origins()
                .uses
                .observe(source, InputScope::Payload)?
                .at(
                    SourceLocation::Subresource {
                        namespace: "loadoutFileRead".to_owned(),
                        indices: Vec::new(),
                        field: "content".to_owned(),
                    },
                    SourceUseRole::Attempted,
                )?;
        }
        Ok(())
    }

    pub(super) fn file_read(
        &self,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
        caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        if let Some(error) = read_stop(original.read_outcome()) {
            return self.fail(error);
        }
        let result = (|| {
            self.checked_file(original, request)?;
            self.attempted_file(original)?;
            let source = original.source();
            let missing = matches!(original.read_outcome(), Err(LoadoutFileError::NotFound));
            let (disposition, code) = read_status(original.read_outcome());
            let mut state = self.lock()?;
            state.files.read(original)?;
            let receipt = if let Some(input) = self.input {
                let candidate = state.paths.get(request)?;
                let xml_caller = xml_selection(caller);
                let caller = state.paths.request(caller)?;
                let selected = state.graph.read(original, candidate, caller, xml_caller)?;
                Some(LoadoutBodyReceipt::read(
                    &EntryCapture::for_input(input),
                    selected,
                )?)
            } else {
                None
            };
            let entry = self.file_observation(
                original,
                bytes_operation(original.kind()),
                disposition,
                code,
                receipt.clone(),
            )?;
            if missing {
                state
                    .ledger
                    .field_diagnostic(missing_file(self.kind, source.identity(), receipt)?);
            }
            if original.read_outcome().is_err() && !missing {
                state.ledger.incomplete_inventory(self.kind);
            }
            state.ledger.entries.push(entry);
            drop(state);
            Ok::<_, CatalogError>(())
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn file_projection(
        &self,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
        caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        let Some(outcome) = original.projection_outcome() else {
            return self.fail(ExportAccountingError::UnobservedFailure.into());
        };
        let result = (|| {
            self.checked_file(original, request)?;
            let mut state = self.lock()?;
            state.files.projected(original)?;
            let receipt = if let Some(input) = self.input {
                let candidate = state.paths.get(request)?;
                let caller = state.paths.request(caller)?;
                state
                    .graph
                    .check_read_context(original, &candidate, &caller)?;
                Some(LoadoutBodyReceipt::read(
                    &EntryCapture::for_input(input),
                    state.graph.selection(original)?,
                )?)
            } else {
                None
            };
            let (disposition, code) = projection_status(outcome);
            state.ledger.entries.push(self.file_observation(
                original,
                operation(original.kind()),
                disposition,
                code,
                receipt,
            )?);
            if outcome.is_err() {
                state.ledger.incomplete_inventory(self.kind);
            }
            drop(state);
            Ok::<_, CatalogError>(())
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn file_reused(
        &self,
        original: &LoadoutFileObservation,
        request: &LoadoutPath,
        caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        if let Some(error) = read_stop(original.read_outcome()) {
            return self.fail(error);
        }
        let result = (|| {
            self.checked_file(original, request)?;
            let state = self.lock()?;
            if !state.files.contains(original) {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            if self.input.is_some() {
                let candidate = state.paths.get(request)?;
                state.graph.check_candidate(original, &candidate)?;
                if let LoadoutRequest::Xml { path } = caller {
                    let recipe = state.paths.get(path)?;
                    let selection = path
                        .selection()
                        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
                    state.graph.selected(original, selection, recipe)?;
                } else {
                    state.paths.validate_request(caller)?;
                }
            }
            drop(state);
            Ok::<_, CatalogError>(())
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn incomplete_files(&self) -> ControlFlow<()> {
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        state.ledger.incomplete_inventory(self.kind);
        drop(state);
        ControlFlow::Continue(())
    }
}

const fn xml_selection(caller: &LoadoutRequest) -> Option<&LoadoutSelection> {
    match caller {
        LoadoutRequest::Xml { path } => path.selection(),
        _ => None,
    }
}

fn read_stop(outcome: Result<(), &LoadoutFileError>) -> Option<CatalogError> {
    match outcome {
        Err(LoadoutFileError::ObservationStopped) => Some(CatalogError::Cancelled),
        Err(error @ LoadoutFileError::SourceEvidenceUnavailable) => {
            Some(CatalogError::LoadoutSourceEvidence(error.clone()))
        }
        Ok(()) | Err(_) => None,
    }
}

const fn read_status(
    outcome: Result<(), &LoadoutFileError>,
) -> (SourceDisposition, Option<ExportDiagnosticCode>) {
    match outcome {
        Ok(()) => (SourceDisposition::Projected, None),
        Err(LoadoutFileError::NotFound) => (
            SourceDisposition::SkippedBeforeRead,
            Some(ExportDiagnosticCode::DanglingReference),
        ),
        Err(_) => (
            SourceDisposition::ReadFailed,
            Some(ExportDiagnosticCode::SourceReadFailure),
        ),
    }
}

const fn projection_status(
    outcome: Result<(), &str>,
) -> (SourceDisposition, Option<ExportDiagnosticCode>) {
    match outcome {
        Ok(()) => (SourceDisposition::Projected, None),
        Err(_) => (
            SourceDisposition::ProjectionFailed,
            Some(ExportDiagnosticCode::ProjectionFailure),
        ),
    }
}

fn missing_file(
    kind: crate::Kind,
    source: LoadoutFileIdentity<'_>,
    receipt: Option<LoadoutBodyReceipt>,
) -> Result<ExportDiagnostic, CatalogError> {
    let example = match receipt {
        Some(receipt) => ExampleObservation::captured(
            RecordSubject::absent(),
            Some(source.path),
            None,
            None,
            ExampleBodyReceipt::loadout(receipt),
        )?,
        None => ExampleObservation::new(RecordSubject::absent(), Some(source.path), None, None),
    };
    Ok(ExportDiagnostic {
        kind: Some(kind),
        stage: ExportDiagnosticStage::SourceRead,
        code: ExportDiagnosticCode::DanglingReference,
        severity: ExportDiagnosticSeverity::Information,
        source_role: Some(ExportSourceRole::Dependency),
        warning_code: Some("missing_loadout_file".to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: vec![example],
    })
}

#[cfg(test)]
#[path = "files_admission_tests.rs"]
mod tests;
