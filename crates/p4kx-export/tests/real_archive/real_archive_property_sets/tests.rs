// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod classification;
mod fixture;
mod privacy;

use p4kx_export::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportHealth, ExportHealthStatus,
    ExportSourceRole, ExportStageStatus, FailedExportRecord, Kind,
};
use p4kx_extraction::{Record, TreeBuilder};
use serde_json::Value;

use super::error::{
    ArrayShape, Failure, MetadataFamily, ProductionFailure, SourceFault, SourceLevel,
};
use super::published::Published;
use super::requirements::Requirements;
use super::{Counts, TestResult, compare, health};

fn remove_source(document: &mut Value, owner: &str) -> TestResult {
    for key in ["missionPropertySets", "contracts"] {
        let rows = document
            .get_mut(key)
            .and_then(Value::as_array_mut)
            .ok_or("published rows absent")?;
        rows.retain(|row| row["recordGuid"] != owner);
        assert!(
            !rows.is_empty(),
            "another source survives, so a nonempty-output check cannot catch this mutation"
        );
    }
    Ok(())
}

fn report_projection_omission(health: &mut ExportHealth, record: &Record<'_>) -> TestResult {
    health.status = ExportHealthStatus::Incomplete;
    health.projection = ExportStageStatus::Incomplete;
    let affected = [
        (Kind::Contracts, ExportSourceRole::Selected),
        (Kind::MissionPropertySets, ExportSourceRole::DerivedInput),
    ];
    health.written_rows -= affected.len();
    for (kind, role) in affected {
        let source = health
            .kinds
            .iter_mut()
            .find(|source| source.kind == kind)
            .ok_or("source kind absent")?;
        source.projection = ExportStageStatus::Incomplete;
        source.source_records.projected -= 1;
        source.source_records.omitted_after_projection_failure += 1;
        source.written_rows -= 1;
        let failed = projection_failure(record, kind, role)?;
        source.failed_records.push(failed.clone());
        health.diagnostics.push(projection_diagnostic(&failed));
        health.failed_records.push(failed);
        assert_eq!(
            source.source_records.selected,
            source.source_records.projected
                + source.source_records.omitted_after_projection_failure,
            "the mutated production source accounting truthfully explains its omitted source"
        );
    }
    Ok(())
}

fn projection_failure(
    record: &Record<'_>,
    kind: Kind,
    role: ExportSourceRole,
) -> TestResult<FailedExportRecord> {
    let (stage, code) = if role == ExportSourceRole::DerivedInput {
        (
            ExportDiagnosticStage::DerivedProjection,
            ExportDiagnosticCode::DerivedProjectionFailure,
        )
    } else {
        (
            ExportDiagnosticStage::Projection,
            ExportDiagnosticCode::ProjectionFailure,
        )
    };
    Ok(FailedExportRecord {
        kind: Some(kind),
        record_index: record.index(),
        guid: record.guid().to_string(),
        record_path: Some(record.record_path()?.to_owned()),
        role,
        stage,
        code,
    })
}

fn projection_diagnostic(failed: &FailedExportRecord) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: failed.kind,
        stage: failed.stage,
        code: failed.code,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(failed.role),
        warning_code: None,
        count: 1,
        count_unit: ExportDiagnosticUnit::Records,
        examples: vec![ExportDiagnosticExample {
            record_index: Some(failed.record_index),
            record_guid: Some(failed.guid.clone()),
            record_path: failed.record_path.clone(),
            field: None,
            source_type: None,
            target_guid: None,
        }],
    }
}

#[test]
fn readable_source_omission_is_rejected_even_when_production_health_explains_it() -> TestResult {
    let fixture = fixture::Fixture::new()?;
    let (mut document, mut report) = fixture.catalog()?;
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("first source absent")?;
    assert!(
        record.tree().is_ok(),
        "the independently readable source is real"
    );
    remove_source(&mut document, &record.guid().to_string())?;
    report_projection_omission(&mut report, &record)?;
    let published = Published::read(&document)?;
    let mut checked = Counts::default();
    let failure = compare::check(
        &record,
        &mut TreeBuilder::new(),
        &published,
        &report,
        &mut checked,
    )
    .err()
    .ok_or("a production omission was accepted")?;
    assert_eq!(
        failure,
        Failure::SourceCompare(SourceFault::MissingAuthoredArray {
            level: SourceLevel::Handler,
            shape: ArrayShape::Populated,
            production: ProductionFailure::Reported {
                stage: ExportDiagnosticStage::DerivedProjection,
                code: ExportDiagnosticCode::DerivedProjectionFailure,
                family: MetadataFamily::NotApplicable,
            },
        }),
        "the source position class, array shape and actual typed production failure are retained"
    );
    assert_eq!(
        checked.failed, 0,
        "a readable source is never called unreadable"
    );
    assert!(
        health::reconcile(&report, 2, 0).is_err(),
        "complete output of all independently readable sources is required"
    );
    Ok(())
}

#[test]
fn a_retained_field_failure_is_not_treated_as_a_whole_source_omission() -> TestResult {
    let fixture = fixture::Fixture::new()?;
    let (document, mut report) = fixture.catalog()?;
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("first source absent")?;
    report.failed_records.push(FailedExportRecord {
        kind: Some(Kind::MissionPropertySets),
        record_index: record.index(),
        guid: record.guid().to_string(),
        record_path: Some(record.record_path()?.to_owned()),
        role: ExportSourceRole::DerivedInput,
        stage: ExportDiagnosticStage::Projection,
        code: ExportDiagnosticCode::UnreadableSource,
    });
    let published = Published::read(&document)?;
    let mut checked = Counts::default();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        compare::check(&record, &mut trees, &published, &report, &mut checked)?;
    }
    assert_eq!(
        checked.failed, 0,
        "retained field failures do not omit their readable owner"
    );
    assert_eq!(
        checked.scoped, 2,
        "both independently read scoped values are checked"
    );
    health::reconcile(&report, checked.sources, checked.failed)?;
    Ok(())
}

#[test]
fn positive_scoped_location_coverage_cannot_pass_with_zero_checked_values() {
    let requirements = Requirements {
        minimum_scoped_values: 1,
    };
    assert!(
        requirements.check(0).is_err(),
        "explicit PTU coverage requires an actual scoped value"
    );
    assert!(
        requirements.check(1).is_ok(),
        "one independently checked scoped value meets the bound"
    );
    assert!(
        Requirements {
            minimum_scoped_values: 0
        }
        .check(0)
        .is_ok(),
        "a generic source run can explicitly omit a scoped-value coverage requirement"
    );
}
