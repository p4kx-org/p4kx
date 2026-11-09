// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_number_health::{Coverage, NumberRequirement};
use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnostic, ExportDiagnosticCode,
    ExportDiagnosticExample, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
};
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::SourceObservation;

/// Counts all uncovered original numeric tokens with bounded owner/field examples.
pub(crate) fn uncovered(
    inspection: &FieldInspection,
    source: &SourceObservation,
    coverage: &mut Coverage<'_, '_>,
) -> Option<ExportDiagnostic> {
    let mut diagnostic = ExportDiagnostic {
        kind: source.kind,
        stage: ExportDiagnosticStage::Projection,
        code: ExportDiagnosticCode::UnsupportedValue,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(source.role),
        warning_code: None,
        count: 0,
        count_unit: ExportDiagnosticUnit::Values,
        examples: Vec::new(),
    };
    for required in &inspection.number_requirements {
        if coverage.take(required) {
            continue;
        }
        diagnostic.count += 1;
        retain_example(&mut diagnostic, inspection, required);
    }
    (diagnostic.count > 0).then_some(diagnostic)
}

fn retain_example(
    diagnostic: &mut ExportDiagnostic,
    inspection: &FieldInspection,
    required: &NumberRequirement,
) {
    if diagnostic.examples.len() == EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT
        && diagnostic
            .examples
            .last()
            .and_then(|example| example.field.as_deref())
            .is_some_and(|last| required.field.as_str() > last)
    {
        return;
    }
    diagnostic.examples.push(ExportDiagnosticExample {
        record_index: Some(inspection.record_index),
        record_guid: Some(inspection.guid.to_string()),
        record_path: Some(inspection.record_path.clone()),
        field: Some(required.field.clone()),
        source_type: Some(required.source_type.clone()),
        target_guid: None,
    });
    diagnostic.examples.sort();
    diagnostic.examples.dedup();
    diagnostic
        .examples
        .truncate(EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT);
}
