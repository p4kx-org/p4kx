// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDiagnosis;
use crate::mission_property_fields::PropertyFieldError;
use crate::scoped_location_fields::FieldError;

/// Extracts malformed and unreadable field categories from strict typed source mappers.
pub(crate) fn diagnosis(error: &(dyn Error + 'static)) -> Option<SourceDiagnosis> {
    let (field, source_type, code) = if let Some(error) = error.downcast_ref::<FieldError>() {
        match error {
            FieldError::Malformed { owner, field, .. } => (
                *field,
                Some(owner.clone()),
                ExportDiagnosticCode::MalformedSource,
            ),
            FieldError::Unreadable { owner, field, .. } => (
                *field,
                Some(owner.clone()),
                ExportDiagnosticCode::UnreadableSource,
            ),
        }
    } else {
        let error = error.downcast_ref::<PropertyFieldError>()?;
        match error {
            PropertyFieldError::Malformed { field, .. } => {
                (*field, None, ExportDiagnosticCode::MalformedSource)
            }
            PropertyFieldError::Unreadable { field, .. } => {
                (*field, None, ExportDiagnosticCode::UnreadableSource)
            }
        }
    };
    Some(SourceDiagnosis {
        code: Some(code),
        field: Some(field.to_owned()),
        source_type,
        target_guid: None,
    })
}
