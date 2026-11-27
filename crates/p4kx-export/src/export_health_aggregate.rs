// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ExportAccountingError;
use crate::Kind;
use crate::export_diagnostic::{
    ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use std::collections::BTreeMap;
type DiagnosticKey = (
    Option<usize>,
    ExportDiagnosticStage,
    crate::ExportDiagnosticCode,
    ExportDiagnosticSeverity,
    Option<ExportSourceRole>,
    Option<String>,
    ExportDiagnosticUnit,
);

pub(crate) fn aggregate(
    diagnostics: impl IntoIterator<Item = ExportDiagnostic>,
) -> Result<Vec<ExportDiagnostic>, ExportAccountingError> {
    aggregate_fallible(diagnostics.into_iter().map(Ok))
}

pub(crate) fn aggregate_fallible<Error: From<ExportAccountingError>>(
    diagnostics: impl IntoIterator<Item = Result<ExportDiagnostic, Error>>,
) -> Result<Vec<ExportDiagnostic>, Error> {
    let mut grouped = BTreeMap::<DiagnosticKey, ExportDiagnostic>::new();
    for diagnostic in diagnostics {
        let mut diagnostic = diagnostic?;
        let key = (
            diagnostic.kind.map(Kind::index),
            diagnostic.stage,
            diagnostic.code,
            diagnostic.severity,
            diagnostic.source_role,
            diagnostic.warning_code.clone(),
            diagnostic.count_unit,
        );
        if let Some(existing) = grouped.get_mut(&key) {
            existing.count = existing
                .count
                .checked_add(diagnostic.count)
                .ok_or(ExportAccountingError::CounterOverflow)?;
            existing.examples.append(&mut diagnostic.examples);
            examples(existing)?;
        } else {
            examples(&mut diagnostic)?;
            grouped.insert(key, diagnostic);
        }
    }
    Ok(grouped.into_values().collect())
}

fn examples(diagnostic: &mut ExportDiagnostic) -> Result<(), ExportAccountingError> {
    diagnostic.examples =
        crate::health_example_samples::retain(std::mem::take(&mut diagnostic.examples))?;
    Ok(())
}
