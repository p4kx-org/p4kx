// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, ValueError};

use crate::ExportAccountingError;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity};
use crate::export_field_collect::FieldCollector;

/// Records decoder failures and preserves unresolved source identities without graph following.
pub(crate) fn record(
    collector: &mut FieldCollector<'_, '_>,
    field: &str,
    source_type: &str,
    error: ValueError,
    body: Option<ExampleBodyReceipt>,
) -> Result<(), ExportAccountingError> {
    let (code, severity, target) = category(error);
    collector.note(field, source_type, code, severity, target, body)
}

const fn category(
    error: ValueError,
) -> (ExportDiagnosticCode, ExportDiagnosticSeverity, Option<Guid>) {
    match error {
        ValueError::DanglingReference { guid } => (
            ExportDiagnosticCode::DanglingReference,
            ExportDiagnosticSeverity::Information,
            Some(guid),
        ),
        ValueError::OutsideRecord { .. } => (
            ExportDiagnosticCode::OutsideOwner,
            ExportDiagnosticSeverity::Information,
            None,
        ),
        _ => (
            ExportDiagnosticCode::UnreadableSource,
            ExportDiagnosticSeverity::Error,
            None,
        ),
    }
}
