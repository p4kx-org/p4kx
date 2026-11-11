// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::CatalogError;
use crate::export_diagnostic::{
    ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::ExportLedger;
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::record_observation::RecordObservation;
use crate::selection::Kind;
use crate::source_fields::FieldError;

/// Retains projected-field diagnostics with the actual owning Record admission.
pub(crate) fn record(
    kind: Kind,
    role: ExportSourceRole,
    admission: &RecordObservation<'_, '_>,
    tree: &RecordTree<'_>,
    notes: Vec<FieldError>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    record_notes(Some(kind), role, admission, tree, notes, ledger)
}

/// Retains projected-field diagnoses for a selected configuration without a Catalog kind.
pub(crate) fn record_dependency(
    admission: &RecordObservation<'_, '_>,
    tree: &RecordTree<'_>,
    notes: Vec<FieldError>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    record_notes(
        None,
        ExportSourceRole::Dependency,
        admission,
        tree,
        notes,
        ledger,
    )
}

fn record_notes(
    kind: Option<Kind>,
    role: ExportSourceRole,
    admission: &RecordObservation<'_, '_>,
    tree: &RecordTree<'_>,
    notes: Vec<FieldError>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    admission.check_tree(tree)?;
    for note in notes {
        let description = note.description();
        let code = crate::export_projection_fields::code(description.issue);
        let diagnosis = crate::export_projection_fields::field(&description, note.body());
        let example = diagnosis.example(admission.subject().clone())?;
        ledger.field_diagnostic(DiagnosticObservation {
            kind,
            stage: ExportDiagnosticStage::Projection,
            code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Values,
            examples: vec![example],
        });
    }
    Ok(())
}
