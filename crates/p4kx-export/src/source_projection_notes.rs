// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::ExportLedger;
use crate::selection::Kind;
use crate::source_fields::FieldError;

/// Retains exact source identities for unsupported or malformed projected resource fields.
pub(crate) fn record(
    kind: Kind,
    role: ExportSourceRole,
    tree: &RecordTree<'_>,
    notes: Vec<FieldError>,
    ledger: &mut ExportLedger,
) {
    for note in notes {
        let (source_type, field, code) = match note {
            FieldError::Malformed {
                source_type, field, ..
            } => (source_type, field, ExportDiagnosticCode::MalformedSource),
            FieldError::Unreadable {
                source_type, field, ..
            } => (source_type, field, ExportDiagnosticCode::UnreadableSource),
            FieldError::Unsupported { source_type, field } => {
                (source_type, field, ExportDiagnosticCode::UnsupportedValue)
            }
        };
        ledger.field_diagnostic(ExportDiagnostic {
            kind: Some(kind),
            stage: ExportDiagnosticStage::Projection,
            code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Values,
            examples: vec![ExportDiagnosticExample {
                record_index: Some(tree.record_index()),
                record_guid: Some(tree.guid().to_string()),
                record_path: Some(tree.record_path().to_owned()),
                field: Some(field),
                source_type: Some(source_type),
                target_guid: None,
            }],
        });
    }
}
