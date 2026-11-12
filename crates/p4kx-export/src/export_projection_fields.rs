// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDiagnosis;
use crate::mission_property_fields::PropertyFieldError;
use crate::record_source_status::DiagnosisBody;
use crate::scoped_location_fields::FieldError;
use crate::source_fields::{self, FieldDescription, FieldIssue};

/// Borrows closed source-error metadata without parsing display text or recreating its receipt.
pub(crate) fn diagnosis(error: &(dyn Error + 'static)) -> Option<SourceDiagnosis> {
    if let Some(error) = error.downcast_ref::<source_fields::FieldError>() {
        return Some(field(&error.description(), error.body()));
    }
    if let Some(error) = error.downcast_ref::<FieldError>() {
        return Some(field(&error.description(), error.body()));
    }
    let error = error.downcast_ref::<PropertyFieldError>()?;
    Some(field(&error.description(), error.body()))
}

pub(crate) fn field(
    description: &FieldDescription<'_>,
    body: Option<&ExampleBodyReceipt>,
) -> SourceDiagnosis {
    let body = body.map_or(DiagnosisBody::Pending, |body| {
        body.clone()
            .select_description(Some(description.field), description.source_type, None)
            .map_or(DiagnosisBody::Conflicting, DiagnosisBody::Captured)
    });
    SourceDiagnosis {
        code: Some(code(description.issue)),
        field: Some(description.field.to_owned()),
        source_type: description.source_type.map(str::to_owned),
        target_guid: None,
        body,
    }
}

pub(crate) const fn code(issue: FieldIssue) -> ExportDiagnosticCode {
    match issue {
        FieldIssue::Malformed => ExportDiagnosticCode::MalformedSource,
        FieldIssue::Unreadable => ExportDiagnosticCode::UnreadableSource,
        FieldIssue::Unsupported | FieldIssue::RetainedVariant => {
            ExportDiagnosticCode::UnsupportedValue
        }
    }
}
