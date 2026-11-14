// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::record_observation::RecordObservation;
use p4kx_assembly::{EntityNode, LoadoutError};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::contract_loadout::EntityKind;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_ledger::{
    DerivedFailure, ExportAccountingError, SourceDiagnosis, SourceDisposition,
};
use crate::{CatalogError, Kind};

use super::Capture;
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};

pub(super) fn diagnostic(
    input: Option<&OriginalCatalogInput<'_, '_>>,
    admission: &RecordObservation<'_, '_>,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
    severity: ExportDiagnosticSeverity,
    warning: &str,
) -> Result<ExportDiagnostic, CatalogError> {
    Ok(from_example(
        stage,
        code,
        severity,
        warning,
        example(input, admission)?,
    ))
}

pub(super) fn from_example(
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
    severity: ExportDiagnosticSeverity,
    warning: &str,
    example: ExampleObservation,
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
        examples: vec![example],
    }
}

pub(super) fn example(
    input: Option<&OriginalCatalogInput<'_, '_>>,
    admission: &RecordObservation<'_, '_>,
) -> Result<ExampleObservation, CatalogError> {
    let subject = admission.subject().clone();
    let Some(input) = input else {
        return Ok(ExampleObservation::new(
            subject,
            None,
            admission.record().type_name().ok(),
            None,
        ));
    };
    let body = ExampleBodyReceipt::source_type_header(input, subject.clone())?;
    let (_, source_type, _) = body.description()?;
    Ok(ExampleObservation::captured(
        subject,
        None,
        source_type.as_deref(),
        None,
        body,
    )?)
}

impl Capture<'_> {
    pub(crate) fn root(
        &self,
        record: &p4kx_extraction::Record<'_>,
        kind: EntityKind,
        outcome: Result<&EntityNode, &LoadoutError>,
    ) {
        if self.stopped() {
            return;
        }
        if let Err(error) = self.check_record(record) {
            self.record_error(error);
            return;
        }
        if matches!(outcome, Err(LoadoutError::ObservationStopped)) {
            self.record_error(CatalogError::Cancelled);
            return;
        }
        let admission = match self.admission(record) {
            Ok(admission) => admission,
            Err(error) => {
                self.record_error(error);
                return;
            }
        };
        let diagnostics = match outcome {
            Ok(node) => match super::typed::diagnostics(self, &admission, node) {
                Ok(diagnostics) => diagnostics,
                Err(error) => {
                    self.record_error(error);
                    return;
                }
            },
            Err(_) => Vec::new(),
        };
        self.record_root(&admission, kind, outcome, diagnostics);
    }

    fn record_root(
        &self,
        admission: &RecordObservation<'_, '_>,
        kind: EntityKind,
        outcome: Result<&EntityNode, &LoadoutError>,
        diagnostics: Vec<ExportDiagnostic>,
    ) {
        let record = admission.record();
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
            state
                .error
                .get_or_insert_with(|| ExportAccountingError::MissingFieldInspection.into());
            return;
        }
        let disposition = match outcome {
            Ok(_) => SourceDisposition::Projected,
            Err(_) if read_failed => SourceDisposition::ReadFailed,
            Err(_) => SourceDisposition::ProjectionFailed,
        };
        let source = match admission.source(
            Some(Kind::Loadouts),
            ExportSourceRole::DerivedInput,
            disposition,
        ) {
            Ok(source) => source,
            Err(error) => {
                state.error.get_or_insert(error);
                return;
            }
        };
        state.ledger.observe(source);
        state.ledger.field_diagnostics.extend(diagnostics);
        if let Err(error) = outcome {
            match secondary_failure(admission, kind, error) {
                Ok(failure) => state.ledger.derived_failure(failure),
                Err(error) => {
                    state.error.get_or_insert_with(|| error.into());
                }
            }
        }
    }
}

fn secondary_failure(
    admission: &RecordObservation<'_, '_>,
    kind: EntityKind,
    error: &LoadoutError,
) -> Result<DerivedFailure, ExportAccountingError> {
    DerivedFailure::of_subject(
        admission.subject().clone(),
        match kind {
            EntityKind::Ship => Kind::Ships,
            EntityKind::Item => Kind::Items,
        },
        Kind::Loadouts,
        ExportSourceRole::Selected,
        SourceDiagnosis {
            code: Some(ExportDiagnosticCode::DerivedProjectionFailure),
            ..crate::export_source_failure::diagnosis(error)
        },
    )
}
