// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage, ExportSourceRole, Kind};
use serde_json::Value;

use super::super::error::{Failure, MetadataFamily, OutputFault};
use super::super::{TestResult, health};
use super::fixture::Fixture;
use super::{privacy, wire};

fn rejected(document: &Value) -> TestResult {
    let failure = health::read(document)
        .err()
        .ok_or("a malformed Health identity was silently dropped")?;
    assert_eq!(
        failure,
        Failure::OutputParse(OutputFault::HealthShape),
        "static malformed-wire classification"
    );
    privacy::assert_sanitized(failure);
    Ok(())
}

#[test]
fn property_set_health_absent_subject_never_becomes_physical_record_zero() -> TestResult {
    let fixture = Fixture::new()?;
    let mut document = fixture.catalog()?;
    let mut diagnostic = wire::diagnostic(&mut document, 0, "MissionPropertyValue_")?;
    let example = diagnostic
        .get_mut("examples")
        .and_then(Value::as_array_mut)
        .and_then(|examples| examples.first_mut())
        .ok_or("diagnostic subject")?;
    for name in ["recordIndex", "recordGuid", "recordPath"] {
        *example.get_mut(name).ok_or("subject field")? = wire::absent();
    }
    wire::diagnostics(&mut document, vec![diagnostic])?;
    assert_eq!(
        health::unsupported_family(&health::read(&document)?, 0),
        MetadataFamily::ExampleUnavailable,
        "inapplicable sourced subjects do not classify physical index zero"
    );
    Ok(())
}

fn failed_document(fixture: &Fixture) -> TestResult<Value> {
    let mut document = fixture.catalog()?;
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("original source")?;
    assert!(
        record.tree().is_ok(),
        "the independent source remains genuinely readable"
    );
    let failed = wire::failure(
        &mut document,
        &record,
        Kind::MissionPropertySets,
        ExportSourceRole::DerivedInput,
        ExportDiagnosticStage::SourceRead,
        ExportDiagnosticCode::SourceReadFailure,
    )?;
    wire::push_failure(&mut document, failed)?;
    Ok(document)
}

#[derive(Clone, Copy)]
enum IndexMutation {
    Missing,
    Bare,
    Unreadable,
}

#[test]
fn property_set_health_requires_each_physical_index_cell_without_legacy_fallback() -> TestResult {
    let fixture = Fixture::new()?;
    let original = failed_document(&fixture)?;
    for state in [
        IndexMutation::Missing,
        IndexMutation::Bare,
        IndexMutation::Unreadable,
    ] {
        let mut document = original.clone();
        let row = document
            .pointer_mut("/exportHealth/failedRecords/0")
            .ok_or("failure")?;
        match state {
            IndexMutation::Missing => {
                row.as_object_mut()
                    .ok_or("failure object")?
                    .remove("recordIndex");
            }
            IndexMutation::Bare => {
                *row.get_mut("recordIndex").ok_or("position")? =
                    serde_json::json!(privacy::SENTINEL);
            }
            IndexMutation::Unreadable => {
                let cell = row.get_mut("recordIndex").ok_or("position")?;
                *cell.get_mut("state").ok_or("state")? = serde_json::json!("unreadable");
                *cell.get_mut("value").ok_or("value")? = Value::Null;
            }
        }
        rejected(&document)?;
    }
    Ok(())
}

#[test]
fn property_set_health_text_refs_resolve_only_inside_the_same_producer_document() -> TestResult {
    let fixture = Fixture::new()?;
    let original = failed_document(&fixture)?;
    for atom in ["00", privacy::SENTINEL, "18446744073709551615"] {
        let mut document = original.clone();
        *document
            .pointer_mut("/exportHealth/failedRecords/0/recordPath/value/atomIndex")
            .ok_or("actual wire atom reference")? = serde_json::json!(atom);
        rejected(&document)?;
    }
    Ok(())
}
