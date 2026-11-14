// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
};
use crate::export_ledger::{DerivedFailure, ExportAccountingError};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::health_failed_observation::FailedRecordObservation as FailedExportRecord;

/// Retains distinct failed operations and merges their already paired original subjects.
pub(crate) fn failures(
    failures: Vec<DerivedFailure>,
) -> Result<(Vec<FailedExportRecord>, Vec<ExportDiagnostic>), ExportAccountingError> {
    let mut distinct = BTreeMap::<_, DerivedFailure>::new();
    for failure in failures {
        failure.check()?;
        let key = (
            failure.affected_kind.index(),
            failure.record_index,
            failure.output_kind.index(),
            failure.role,
        );
        if let Some(retained) = distinct.get_mut(&key) {
            retained.merge(failure)?;
        } else {
            distinct.insert(key, failure);
        }
    }
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();
    for failure in distinct.into_values() {
        let code = failure
            .diagnosis
            .code
            .unwrap_or(ExportDiagnosticCode::DerivedProjectionFailure);
        records.push(FailedExportRecord::new(
            Some(failure.affected_kind),
            failure.subject().clone(),
            failure.role,
            ExportDiagnosticStage::DerivedProjection,
            code,
        )?);
        diagnostics.push(ExportDiagnostic {
            kind: Some(failure.affected_kind),
            stage: ExportDiagnosticStage::DerivedProjection,
            code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(failure.role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Projections,
            examples: vec![failure.diagnosis.example(failure.subject().clone())?],
        });
    }
    Ok((records, diagnostics))
}
