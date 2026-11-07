// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, FailedExportRecord,
};
use crate::export_ledger::DerivedFailure;

/// Retains one diagnostic for each distinct failed secondary operation.
pub(crate) fn failures(
    failures: Vec<DerivedFailure>,
) -> (Vec<FailedExportRecord>, Vec<ExportDiagnostic>) {
    let mut distinct = BTreeMap::new();
    for failure in failures {
        distinct
            .entry((
                failure.affected_kind.index(),
                failure.record_index,
                failure.output_kind.index(),
                failure.role,
            ))
            .or_insert(failure);
    }
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();
    for failure in distinct.into_values() {
        let code = failure
            .diagnosis
            .code
            .unwrap_or(ExportDiagnosticCode::DerivedProjectionFailure);
        records.push(FailedExportRecord {
            kind: Some(failure.affected_kind),
            record_index: failure.record_index,
            guid: failure.guid.to_string(),
            record_path: failure.record_path.clone(),
            role: failure.role,
            stage: ExportDiagnosticStage::DerivedProjection,
            code,
        });
        diagnostics.push(ExportDiagnostic {
            kind: Some(failure.affected_kind),
            stage: ExportDiagnosticStage::DerivedProjection,
            code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(failure.role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Projections,
            examples: vec![ExportDiagnosticExample {
                record_index: Some(failure.record_index),
                record_guid: Some(failure.guid.to_string()),
                record_path: failure.record_path,
                field: failure.diagnosis.field,
                source_type: failure.diagnosis.source_type,
                target_guid: failure.diagnosis.target_guid.map(|guid| guid.to_string()),
            }],
        });
    }
    (records, diagnostics)
}
