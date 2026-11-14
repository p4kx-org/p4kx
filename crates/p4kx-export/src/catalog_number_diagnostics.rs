// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ExportAccountingError;
use crate::catalog_number_health::{Coverage, NumberRequirement};
use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnosticCode, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit,
};
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::SourceObservation;
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation as ExportDiagnosticExample,
};

/// Counts all uncovered original numeric tokens with bounded owner/field examples.
pub(crate) fn uncovered(
    inspection: &FieldInspection,
    source: &SourceObservation,
    coverage: &mut Coverage<'_, '_>,
) -> Result<Option<ExportDiagnostic>, ExportAccountingError> {
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
        retain_example(&mut diagnostic, inspection, required)?;
    }
    Ok((diagnostic.count > 0).then_some(diagnostic))
}

fn retain_example(
    diagnostic: &mut ExportDiagnostic,
    inspection: &FieldInspection,
    required: &NumberRequirement,
) -> Result<(), ExportAccountingError> {
    if diagnostic.examples.len() == EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT
        && diagnostic
            .examples
            .last()
            .and_then(|example| example.field.as_deref())
            .is_some_and(|last| required.field.as_str() > last)
    {
        return Ok(());
    }
    let example = match &required.body {
        Some(body) => ExportDiagnosticExample::captured(
            inspection.subject.clone(),
            Some(&required.field),
            Some(&required.source_type),
            None,
            body.clone(),
        )?,
        None => ExportDiagnosticExample::new(
            inspection.subject.clone(),
            Some(&required.field),
            Some(&required.source_type),
            None,
        ),
    };
    diagnostic.examples.push(example);
    diagnostic.examples =
        crate::health_example_samples::retain(std::mem::take(&mut diagnostic.examples))?;
    Ok(())
}
