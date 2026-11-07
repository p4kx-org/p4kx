// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityNode, LoadoutError};
use p4kx_extraction::Record;

use crate::Kind;
use crate::contract_loadout::EntityKind;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::{
    DerivedFailure, ExportAccountingError, SourceDiagnosis, SourceDisposition, SourceObservation,
};

use super::Capture;

pub(super) fn diagnostic(
    record: &Record<'_>,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
    severity: ExportDiagnosticSeverity,
    warning: &str,
) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(Kind::Loadouts),
        stage,
        code,
        severity,
        source_role: Some(ExportSourceRole::Dependency),
        warning_code: Some(warning.to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: vec![ExportDiagnosticExample {
            record_index: Some(record.index()),
            record_guid: Some(record.guid().to_string()),
            record_path: record.record_path().ok().map(str::to_owned),
            field: None,
            source_type: record.type_name().ok().map(str::to_owned),
            target_guid: None,
        }],
    }
}

impl Capture {
    pub(crate) fn root(
        &self,
        record: &Record<'_>,
        kind: EntityKind,
        outcome: Result<&EntityNode, &LoadoutError>,
    ) {
        if self.stopped() {
            return;
        }
        if matches!(outcome, Err(LoadoutError::ObservationStopped)) {
            self.record_error(crate::CatalogError::Cancelled);
            return;
        }
        let diagnostics = match outcome {
            Ok(node) => match super::typed::diagnostics(record, node) {
                Ok(diagnostics) => diagnostics,
                Err(error) => {
                    self.record_error(error.into());
                    return;
                }
            },
            Err(_) => Vec::new(),
        };
        let Ok(mut state) = self.lock() else {
            return;
        };
        let read_failed = state.ledger.sources.iter().any(|source| {
            source.record_index == record.index()
                && source.kind == Some(Kind::Loadouts)
                && source.role == ExportSourceRole::Dependency
                && source.disposition == SourceDisposition::ReadFailed
        });
        if !read_failed && !state.proofs.contains(&record.index()) {
            state.error = Some(ExportAccountingError::MissingFieldInspection.into());
            return;
        }
        let disposition = match outcome {
            Ok(_) => SourceDisposition::Projected,
            Err(_) if read_failed => SourceDisposition::ReadFailed,
            Err(_) => SourceDisposition::ProjectionFailed,
        };
        state.ledger.observe(SourceObservation::new(
            Kind::Loadouts,
            record,
            ExportSourceRole::DerivedInput,
            disposition,
        ));
        state.ledger.field_diagnostics.extend(diagnostics);
        if let Err(error) = outcome {
            state
                .ledger
                .derived_failure(secondary_failure(record, kind, error));
        }
    }
}

fn secondary_failure(
    record: &Record<'_>,
    kind: EntityKind,
    error: &LoadoutError,
) -> DerivedFailure {
    DerivedFailure {
        affected_kind: match kind {
            EntityKind::Ship => Kind::Ships,
            EntityKind::Item => Kind::Items,
        },
        output_kind: Kind::Loadouts,
        record_index: record.index(),
        role: ExportSourceRole::Selected,
        guid: record.guid(),
        record_path: record.record_path().ok().map(str::to_owned),
        diagnosis: SourceDiagnosis {
            code: Some(ExportDiagnosticCode::DerivedProjectionFailure),
            ..crate::export_source_failure::diagnosis(error)
        },
    }
}
