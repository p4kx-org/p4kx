// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole};
use serde_json::Value;

use super::super::OracleResult;
use super::cells;

pub(super) fn required_text<'value>(row: &'value Value, field: &str) -> OracleResult<&'value str> {
    row.get(field)
        .and_then(Value::as_str)
        .ok_or_else(cells::fault)
}

pub(super) fn optional_text<'value>(
    row: &'value Value,
    field: &str,
) -> OracleResult<Option<&'value str>> {
    match row.get(field).ok_or_else(cells::fault)? {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        _ => Err(cells::fault()),
    }
}

pub(super) fn stage(row: &Value) -> OracleResult<ExportDiagnosticStage> {
    match required_text(row, "stage")? {
        "selection" => Ok(ExportDiagnosticStage::Selection),
        "sourceRead" => Ok(ExportDiagnosticStage::SourceRead),
        "projection" => Ok(ExportDiagnosticStage::Projection),
        "derivedProjection" => Ok(ExportDiagnosticStage::DerivedProjection),
        "sourceField" => Ok(ExportDiagnosticStage::SourceField),
        _ => Err(cells::fault()),
    }
}

pub(super) fn code(row: &Value) -> OracleResult<ExportDiagnosticCode> {
    let code = match required_text(row, "code")? {
        "optionalAbsent" => ExportDiagnosticCode::OptionalAbsent,
        "emptyValue" => ExportDiagnosticCode::EmptyValue,
        "danglingReference" => ExportDiagnosticCode::DanglingReference,
        "outsideOwner" => ExportDiagnosticCode::OutsideOwner,
        "unsupportedValue" => ExportDiagnosticCode::UnsupportedValue,
        "malformedSource" => ExportDiagnosticCode::MalformedSource,
        "unreadableSource" => ExportDiagnosticCode::UnreadableSource,
        "fieldWalkLimit" => ExportDiagnosticCode::FieldWalkLimit,
        "sourceReadFailure" => ExportDiagnosticCode::SourceReadFailure,
        "projectionFailure" => ExportDiagnosticCode::ProjectionFailure,
        "derivedProjectionFailure" => ExportDiagnosticCode::DerivedProjectionFailure,
        "selectionFailure" => ExportDiagnosticCode::SelectionFailure,
        "incompleteInventory" => ExportDiagnosticCode::IncompleteInventory,
        "ambiguousIdentity" => ExportDiagnosticCode::AmbiguousIdentity,
        "catalogTargetUnavailable" => ExportDiagnosticCode::CatalogTargetUnavailable,
        "sourceWarning" => ExportDiagnosticCode::SourceWarning,
        _ => return Err(cells::fault()),
    };
    Ok(code)
}

pub(super) fn role(row: &Value, field: &str) -> OracleResult<Option<ExportSourceRole>> {
    let role = match optional_text(row, field)? {
        None => None,
        Some("selected") => Some(ExportSourceRole::Selected),
        Some("derivedInput") => Some(ExportSourceRole::DerivedInput),
        Some("dependency") => Some(ExportSourceRole::Dependency),
        Some("selectionCandidate") => Some(ExportSourceRole::SelectionCandidate),
        _ => return Err(cells::fault()),
    };
    Ok(role)
}
