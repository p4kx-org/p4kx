// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod classification;
mod fixture;
mod health_wire;
mod privacy;
mod wire;

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole, Kind};
use p4kx_extraction::{Record, TreeBuilder};
use serde_json::Value;

use super::error::{
    ArrayShape, Failure, MetadataFamily, ProductionFailure, SourceFault, SourceLevel,
};
use super::published::{Published, contract_owner};
use super::requirements::Requirements;
use super::{Counts, TestResult, compare, health};
use crate::real_archive_contracts::v3;

fn remove_source(document: &mut Value, record: &Record<'_>) -> TestResult {
    let owner = record.guid().to_string();
    let removed = contract_occurrences(document, record.index(), &owner)?;
    let sets = document
        .get_mut("missionPropertySets")
        .and_then(Value::as_array_mut)
        .ok_or("published property sets absent")?;
    sets.retain(|row| row["recordGuid"] != owner);
    assert!(!sets.is_empty(), "another source set survives");
    let contracts = document
        .pointer_mut("/projections/contracts/rows")
        .and_then(Value::as_array_mut)
        .ok_or("published contract rows absent")?;
    contracts.retain(|row| {
        !row.get("source")
            .is_some_and(|source| removed.contains(source))
    });
    assert!(
        !contracts.is_empty(),
        "another source survives, so a nonempty-output check cannot catch this mutation"
    );
    let titles = document
        .pointer_mut("/projections/missionTitles/rows")
        .and_then(Value::as_array_mut)
        .ok_or("published mission title rows absent")?;
    titles.retain(|row| {
        !row.get("contract")
            .is_some_and(|source| removed.contains(source))
    });
    Ok(())
}

fn contract_occurrences(document: &Value, index: u32, owner: &str) -> TestResult<Vec<Value>> {
    let projections = v3::Projections::read(document)?;
    let mut removed = Vec::new();
    for row in v3::rows(document)? {
        if contract_owner(&projections, row)? == (u64::from(index), owner) {
            removed.push(
                row.get("source")
                    .ok_or("actual removed occurrence")?
                    .clone(),
            );
        }
    }
    assert!(
        !removed.is_empty(),
        "the mutation removes the independently read owner"
    );
    Ok(removed)
}

fn report_projection_omission(document: &mut Value, record: &Record<'_>) -> TestResult {
    let health = document.get_mut("exportHealth").ok_or("health absent")?;
    *health.get_mut("status").ok_or("status absent")? = serde_json::json!("incomplete");
    *health.get_mut("projection").ok_or("projection absent")? = serde_json::json!("incomplete");
    adjust(health, "writtenRows", -2)?;
    for (kind, role, stage, code) in [
        (
            Kind::Contracts,
            ExportSourceRole::Selected,
            ExportDiagnosticStage::Projection,
            ExportDiagnosticCode::ProjectionFailure,
        ),
        (
            Kind::MissionPropertySets,
            ExportSourceRole::DerivedInput,
            ExportDiagnosticStage::DerivedProjection,
            ExportDiagnosticCode::DerivedProjectionFailure,
        ),
    ] {
        let failed = wire::failure(document, record, kind, role, stage, code)?;
        let diagnostic = wire::projection_diagnostic(&failed)?;
        let health = document.get_mut("exportHealth").ok_or("health absent")?;
        adjust_kind(health, kind, &failed)?;
        health
            .get_mut("diagnostics")
            .and_then(Value::as_array_mut)
            .ok_or("diagnostics absent")?
            .push(diagnostic);
        wire::push_failure(document, failed)?;
    }
    Ok(())
}

fn adjust_kind(health: &mut Value, kind: Kind, failed: &Value) -> TestResult {
    let source = health
        .get_mut("kinds")
        .and_then(Value::as_array_mut)
        .and_then(|kinds| {
            kinds
                .iter_mut()
                .find(|source| source.get("kind").and_then(Value::as_str) == Some(kind.name()))
        })
        .ok_or("source kind absent")?;
    *source.get_mut("projection").ok_or("projection absent")? = serde_json::json!("incomplete");
    let counts = source
        .get_mut("sourceRecords")
        .ok_or("source counts absent")?;
    adjust(counts, "projected", -1)?;
    adjust(counts, "omittedAfterProjectionFailure", 1)?;
    assert_eq!(
        number(counts, "selected")?,
        number(counts, "projected")? + number(counts, "omittedAfterProjectionFailure")?,
        "the mutated production source accounting truthfully explains its omitted source"
    );
    adjust(source, "writtenRows", -1)?;
    source
        .get_mut("failedRecords")
        .and_then(Value::as_array_mut)
        .ok_or("kind failures absent")?
        .push(failed.clone());
    Ok(())
}

fn number(object: &Value, field: &str) -> TestResult<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| "production count absent".into())
}

fn adjust(object: &mut Value, field: &str, delta: i64) -> TestResult {
    let value = number(object, field)?;
    let value = value
        .checked_add_signed(delta)
        .ok_or("invalid count mutation")?;
    *object.get_mut(field).ok_or("count field absent")? = serde_json::json!(value);
    Ok(())
}

#[test]
fn readable_source_omission_is_rejected_even_when_production_health_explains_it() -> TestResult {
    let fixture = fixture::Fixture::new()?;
    let mut document = fixture.catalog()?;
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("first source absent")?;
    assert!(
        record.tree().is_ok(),
        "the independently readable source is real"
    );
    remove_source(&mut document, &record)?;
    report_projection_omission(&mut document, &record)?;
    let report = health::read(&document)?;
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
    let mut document = fixture.catalog()?;
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("first source absent")?;
    let failed = wire::failure(
        &mut document,
        &record,
        Kind::MissionPropertySets,
        ExportSourceRole::DerivedInput,
        ExportDiagnosticStage::Projection,
        ExportDiagnosticCode::UnreadableSource,
    )?;
    wire::push_failure(&mut document, failed)?;
    let report = health::read(&document)?;
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
