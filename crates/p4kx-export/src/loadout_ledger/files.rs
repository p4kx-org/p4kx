// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutDocumentKind, LoadoutFileError, LoadoutFileIdentity};

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
        source: LoadoutFileIdentity<'_>,
        operation: EntryOperation,
        disposition: SourceDisposition,
        code: Option<ExportDiagnosticCode>,
    ) -> ArchiveEntryObservation {
        ArchiveEntryObservation {
            operation,
            kind: self.kind,
            entry_index: source.entry_index,
            container_entry_index: None,
            path: source.path.to_owned(),
            role: ExportSourceRole::Dependency,
            disposition,
            diagnosis: SourceDiagnosis {
                code,
                field: Some(source.path.to_owned()),
                source_type: Some("LoadoutFile".to_owned()),
                target_guid: None,
            },
        }
    }

    pub(super) fn file_read(
        &self,
        source: LoadoutFileIdentity<'_>,
        kind: LoadoutDocumentKind,
        outcome: Result<(), &LoadoutFileError>,
    ) -> ControlFlow<()> {
        if matches!(outcome, Err(LoadoutFileError::ObservationStopped)) {
            return self.fail(CatalogError::Cancelled);
        }
        let missing = matches!(outcome, Err(LoadoutFileError::NotFound));
        let (disposition, code) = if outcome.is_ok() {
            (SourceDisposition::Projected, None)
        } else if missing {
            (
                SourceDisposition::SkippedBeforeRead,
                Some(ExportDiagnosticCode::DanglingReference),
            )
        } else {
            (
                SourceDisposition::ReadFailed,
                Some(ExportDiagnosticCode::SourceReadFailure),
            )
        };
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
                .field_diagnostic(missing_file(self.kind, source));
        }
        if outcome.is_err() && !missing {
            state.ledger.incomplete_inventory(self.kind);
        }
        drop(state);
        ControlFlow::Continue(())
    }

    pub(super) fn file_projection(
        &self,
        source: LoadoutFileIdentity<'_>,
        kind: LoadoutDocumentKind,
        outcome: Result<(), &str>,
    ) -> ControlFlow<()> {
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        if !state.ledger.entries.iter().any(|entry| {
            entry.operation == bytes_operation(kind)
                && entry.entry_index == source.entry_index
                && entry.path == source.path
                && entry.disposition == SourceDisposition::Projected
        }) {
            drop(state);
            return self.fail(ExportAccountingError::UnobservedFailure.into());
        }
        let (disposition, code) = if outcome.is_ok() {
            (SourceDisposition::Projected, None)
        } else {
            (
                SourceDisposition::ProjectionFailed,
                Some(ExportDiagnosticCode::ProjectionFailure),
            )
        };
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

    pub(super) fn incomplete_files(&self) -> ControlFlow<()> {
        let Ok(mut state) = self.lock() else {
            return ControlFlow::Break(());
        };
        state.ledger.incomplete_inventory(self.kind);
        drop(state);
        ControlFlow::Continue(())
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
