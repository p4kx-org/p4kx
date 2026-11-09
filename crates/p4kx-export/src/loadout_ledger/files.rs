// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutFileError, LoadoutFileIdentity, LoadoutFileObservation,
    LoadoutFileSource,
};

use crate::CatalogError;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_ledger::{ExportAccountingError, SourceDiagnosis, SourceDisposition};

use super::Capture;

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

impl Capture {
    fn file_observation(
        &self,
        source: &LoadoutFileSource,
        operation: EntryOperation,
        disposition: SourceDisposition,
        code: Option<ExportDiagnosticCode>,
    ) -> ArchiveEntryObservation {
        let identity = source.identity();
        ArchiveEntryObservation {
            operation,
            kind: self.kind,
            entry_index: identity.entry_index,
            container_entry_index: None,
            path: identity.path.to_owned(),
            role: ExportSourceRole::Dependency,
            disposition,
            diagnosis: SourceDiagnosis {
                code,
                field: Some(identity.path.to_owned()),
                source_type: Some("LoadoutFile".to_owned()),
                target_guid: None,
            },
        }
    }

    pub(super) fn file_read(
        &self,
        source: &LoadoutFileSource,
        kind: LoadoutDocumentKind,
        outcome: Result<(), &LoadoutFileError>,
    ) -> ControlFlow<()> {
        if let Some(error) = read_stop(outcome) {
            return self.fail(error);
        }
        let missing = matches!(outcome, Err(LoadoutFileError::NotFound));
        let (disposition, code) = read_status(outcome);
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        state.ledger.entries.push(self.file_observation(
            source,
            bytes_operation(kind),
            disposition,
            code,
        ));
        if missing {
            state
                .ledger
                .field_diagnostic(missing_file(self.kind, source.identity()));
        }
        if outcome.is_err() && !missing {
            state.ledger.incomplete_inventory(self.kind);
        }
        drop(state);
        ControlFlow::Continue(())
    }

    pub(super) fn file_projection(
        &self,
        source: &LoadoutFileSource,
        kind: LoadoutDocumentKind,
        outcome: Result<(), &str>,
    ) -> ControlFlow<()> {
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        let read = self.file_observation(
            source,
            bytes_operation(kind),
            SourceDisposition::Projected,
            None,
        );
        if !state.ledger.entries.contains(&read) {
            drop(state);
            return self.fail(ExportAccountingError::UnobservedFailure.into());
        }
        let (disposition, code) = projection_status(outcome);
        state.ledger.entries.push(self.file_observation(
            source,
            operation(kind),
            disposition,
            code,
        ));
        if outcome.is_err() {
            state.ledger.incomplete_inventory(self.kind);
        }
        drop(state);
        ControlFlow::Continue(())
    }

    pub(super) fn file_reused(&self, original: &LoadoutFileObservation) -> ControlFlow<()> {
        if let Some(error) = read_stop(original.read_outcome()) {
            return self.fail(error);
        }
        let (disposition, code) = read_status(original.read_outcome());
        let read = self.file_observation(
            original.source(),
            bytes_operation(original.kind()),
            disposition,
            code,
        );
        let projected = original.projection_outcome().map(|outcome| {
            let (disposition, code) = projection_status(outcome);
            self.file_observation(
                original.source(),
                operation(original.kind()),
                disposition,
                code,
            )
        });
        let Ok(state) = self.lock() else {
            return ControlFlow::Break(());
        };
        let retained = state.ledger.entries.contains(&read)
            && projected
                .as_ref()
                .is_none_or(|entry| state.ledger.entries.contains(entry));
        drop(state);
        if retained {
            ControlFlow::Continue(())
        } else {
            self.fail(ExportAccountingError::UnobservedFailure.into())
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

fn missing_file(kind: crate::Kind, source: LoadoutFileIdentity<'_>) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(kind),
        stage: ExportDiagnosticStage::SourceRead,
        code: ExportDiagnosticCode::DanglingReference,
        severity: ExportDiagnosticSeverity::Information,
        source_role: Some(ExportSourceRole::Dependency),
        warning_code: Some("missing_loadout_file".to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: vec![ExportDiagnosticExample {
            record_index: None,
            record_guid: None,
            record_path: None,
            field: Some(source.path.to_owned()),
            source_type: None,
            target_guid: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use std::ops::ControlFlow;

    use p4kx_assembly::{
        LoadoutDocumentKind, LoadoutFileError, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver,
    };

    use super::super::Capture;
    use super::CatalogError;
    use crate::export_ledger::ExportLedger;

    struct Unavailable;

    impl LoadoutFiles for Unavailable {
        fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
            assert_eq!(
                path, "synthetic/admission.xml",
                "the actual requested source"
            );
            Err(LoadoutFileError::SourceEvidenceUnavailable)
        }
    }

    #[test]
    fn reported_unavailable_loadout_admission_remains_the_first_fatal_adapter_cause() {
        let read = Unavailable.read_observed("synthetic/admission.xml");
        let capture = Capture::new(&ExportLedger::default(), None);
        let stopped = capture.observe(LoadoutReadEvent::FileRead {
            source: read.source(),
            kind: LoadoutDocumentKind::XmlLoadout,
            outcome: read.outcome().map(|_| ()),
        });
        assert!(
            matches!(stopped, ControlFlow::Break(())),
            "actual adapter stops immediately"
        );
        assert!(
            capture
                .observe(LoadoutReadEvent::InventoryIncomplete)
                .is_break(),
            "a subsequent event cannot resume the stopped capture"
        );
        let outcome = capture.finish();
        assert!(
            matches!(
                outcome,
                Err(CatalogError::LoadoutSourceEvidence(
                    LoadoutFileError::SourceEvidenceUnavailable
                ))
            ),
            "the first typed admission failure is neither cancellation nor an ordinary Health row"
        );
    }
}
