// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole, Kind};
use p4kx_extraction::SourceValueState;
use serde_json::Value;

use super::super::OracleResult;
use super::super::error::{Failure, HealthFault, MetadataFamily};
use super::{Counts, Diagnostic, Example, FailedRecord, Health, cells, controls};

pub(super) fn read(document: &Value) -> OracleResult<Health<'_>> {
    let health = document.get("exportHealth").ok_or_else(cells::fault)?;
    let texts = health
        .get("metadata")
        .and_then(|metadata| metadata.get("texts"))
        .and_then(Value::as_array)
        .ok_or_else(cells::fault)?;
    let records = array(health, "failedRecords")?
        .iter()
        .map(|row| failed(row, texts))
        .collect::<OracleResult<Vec<_>>>()?;
    let diagnostics = array(health, "diagnostics")?
        .iter()
        .map(|row| diagnostic(row, texts))
        .collect::<OracleResult<Vec<_>>>()?;
    Ok(Health {
        _document: document,
        records,
        diagnostics,
        counts: counts(health)?,
    })
}

fn array<'value>(object: &'value Value, field: &str) -> OracleResult<&'value [Value]> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(cells::fault)
}

fn failed(row: &Value, texts: &[Value]) -> OracleResult<FailedRecord> {
    let index = cells::index(&cells::field(row, "recordIndex")?)?;
    cells::string(&cells::field(row, "guid")?)?;
    cells::text(&cells::field(row, "recordPath")?, texts)?;
    cells::text(&cells::field(row, "sourceType")?, texts)?;
    let kind = controls::optional_text(row, "kind")?;
    let role = controls::role(row, "role")?.ok_or_else(cells::fault)?;
    Ok(FailedRecord {
        index,
        relevant: kind == Some(Kind::MissionPropertySets.name())
            && role == ExportSourceRole::DerivedInput,
        stage: controls::stage(row)?,
        code: controls::code(row)?,
    })
}

fn diagnostic(row: &Value, texts: &[Value]) -> OracleResult<Diagnostic> {
    let kind = controls::optional_text(row, "kind")?;
    let role = controls::role(row, "sourceRole")?;
    let stage = controls::stage(row)?;
    let code = controls::code(row)?;
    let examples = array(row, "examples")?
        .iter()
        .map(|row| example(row, texts))
        .collect::<OracleResult<Vec<_>>>()?;
    Ok(Diagnostic {
        relevant: kind == Some(Kind::MissionPropertySets.name())
            && role == Some(ExportSourceRole::DerivedInput)
            && code == ExportDiagnosticCode::UnsupportedValue
            && matches!(
                stage,
                ExportDiagnosticStage::Projection | ExportDiagnosticStage::DerivedProjection
            ),
        examples,
    })
}

fn example(row: &Value, texts: &[Value]) -> OracleResult<Example> {
    let position = cells::field(row, "recordIndex")?;
    let guid = cells::field(row, "recordGuid")?;
    let path = cells::field(row, "recordPath")?;
    let index = if position.state == SourceValueState::Absent {
        if guid.state != SourceValueState::Absent || path.state != SourceValueState::Absent {
            return Err(cells::fault());
        }
        None
    } else {
        let index = cells::index(&position)?;
        cells::string(&guid)?;
        cells::text(&path, texts)?;
        Some(index)
    };
    let family = controls::optional_text(row, "sourceType")?
        .map_or(MetadataFamily::MetadataAbsent, super::schema_family);
    Ok(Example { index, family })
}

fn count(row: &Value, name: &str) -> OracleResult<usize> {
    let number = row
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(cells::fault)?;
    usize::try_from(number).map_err(|_| cells::fault())
}

fn counts(health: &Value) -> OracleResult<Counts> {
    let rows = array(health, "kinds")?;
    for row in rows {
        controls::required_text(row, "kind")?;
    }
    let mut kinds = rows.iter().filter(|row| {
        row.get("kind").and_then(Value::as_str) == Some(Kind::MissionPropertySets.name())
    });
    let kind = kinds
        .next()
        .ok_or(Failure::HealthReconcile(HealthFault::MissingKind))?;
    if kinds.next().is_some() {
        return Err(cells::fault());
    }
    let source = kind.get("sourceRecords").ok_or_else(cells::fault)?;
    Ok(Counts {
        selected: count(source, "selected")?,
        read_omissions: count(source, "omittedAfterReadFailure")?,
        projection_omissions: count(source, "omittedAfterProjectionFailure")?,
        projected: count(source, "projected")?,
        skipped: count(source, "intentionallySkipped")?,
    })
}
