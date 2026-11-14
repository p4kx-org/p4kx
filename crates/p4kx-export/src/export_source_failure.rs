// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDiagnosis;
use crate::record_source_status::DiagnosisBody;
use crate::template_issues::{TemplateDescription, TemplateError};

/// Extracts technical field metadata from typed errors without parsing display strings.
pub(crate) fn diagnosis(error: &(dyn Error + 'static)) -> SourceDiagnosis {
    let mut cause = Some(error);
    while let Some(error) = cause {
        if let Some(diagnosis) = crate::export_projection_fields::diagnosis(error) {
            return diagnosis;
        }
        let template =
            error.downcast_ref::<TemplateError>().or_else(|| match error
                .downcast_ref::<crate::spawn_error::SpawnError>(
            ) {
                Some(crate::spawn_error::SpawnError::Template(template)) => Some(template),
                _ => None,
            });
        if let Some(error) = template {
            let TemplateDescription::UnknownStruct { field, name } = error.description();
            return SourceDiagnosis {
                code: Some(ExportDiagnosticCode::UnsupportedValue),
                field: Some(field.to_owned()),
                source_type: Some(name.to_owned()),
                target_guid: None,
                body: error
                    .body()
                    .cloned()
                    .map_or(DiagnosisBody::Pending, DiagnosisBody::Captured),
            };
        }
        cause = error.source();
    }
    SourceDiagnosis::default()
}
