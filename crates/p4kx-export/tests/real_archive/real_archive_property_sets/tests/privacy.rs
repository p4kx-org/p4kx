// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::{TreeBuilder, Value as SourceValue};
use serde_json::Value;

use super::super::error::{
    AccountingFault, ArchiveFault, ArrayShape, CatalogFault, CountFault, Failure, HealthFault,
    JoinFault, OutputFault, ProductionFailure, ReadFault, SetupFault, SourceFault, SourceLevel,
};
use super::super::published::Published;
use super::super::{Counts, TestResult, compare, health, values};
use super::fixture::Fixture;

pub(super) const SENTINEL: &str = "SENTINEL_PRIVATE_VALUE_GUID_RECORD_PATH";

#[derive(Debug)]
struct SentinelError(&'static str);

impl fmt::Display for SentinelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SentinelError {}

fn classifications() -> impl Iterator<Item = Failure> {
    [
        Failure::Setup(SetupFault::ArchivePath),
        Failure::ArchiveDcb(ArchiveFault::DcbRead),
        Failure::CatalogBuild(CatalogFault::Accounting(
            AccountingFault::MissingFieldInspection,
        )),
        Failure::OutputParse(OutputFault::JsonDecode),
        Failure::IndependentRead(ReadFault::UnreportedCanonicalFailure),
        missing_array_failure(),
        Failure::SourceCompare(SourceFault::DeclarationTextValue),
        Failure::Joins(JoinFault::Target),
        Failure::HealthReconcile(HealthFault::ProjectionOmission),
        Failure::PositiveScopedCoverage,
        Failure::CountReconcile(CountFault::Contracts),
    ]
    .into_iter()
}

const fn missing_array_failure() -> Failure {
    Failure::SourceCompare(SourceFault::MissingAuthoredArray {
        level: SourceLevel::Handler,
        shape: ArrayShape::Populated,
        production: ProductionFailure::NoneReported,
    })
}

pub(super) fn assert_sanitized(failure: Failure) {
    use std::error::Error as _;

    for output in [format!("{failure}"), format!("{failure:?}")] {
        assert!(
            !output.contains(SENTINEL),
            "error output contains only static discriminants"
        );
    }
    assert!(
        failure.source().is_none(),
        "the original error cannot escape through an error chain"
    );
}

fn compare_mutation(fixture: &Fixture, document: &Value) -> TestResult<Failure> {
    let published = Published::read(document)?;
    let report = health::read(document)?;
    let forge = fixture.entry.open()?;
    let mut trees = TreeBuilder::new();
    let mut checked = Counts::default();
    for record in forge.records() {
        if let Err(failure) = compare::check(&record, &mut trees, &published, &report, &mut checked)
        {
            return Ok(failure);
        }
    }
    Err("the mutated source output was accepted".into())
}

#[test]
fn every_stage_discards_the_original_error_before_display_or_debug() -> TestResult {
    for failure in classifications() {
        let result = Failure::classify(Err::<(), _>(SentinelError(SENTINEL)), failure);
        let mapped = result.err().ok_or("the synthetic failure was accepted")?;
        assert_eq!(
            mapped, failure,
            "classification keeps only the requested typed stage"
        );
        assert_sanitized(mapped);
    }
    assert_eq!(
        missing_array_failure().to_string(),
        "property_set_oracle SourceCompare(MissingAuthoredArray { level: Handler, shape: Populated, production: NoneReported })",
        "the diagnostic exposes an actionable static subcategory"
    );
    Ok(())
}

#[test]
fn declaration_value_mismatches_retain_no_source_or_output_content() -> TestResult {
    let fixture = Fixture::new()?;
    let mut document = fixture.catalog()?;
    let variable = document
        .get_mut("missionPropertySets")
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.first_mut())
        .and_then(|set| set.get_mut("properties"))
        .and_then(Value::as_array_mut)
        .and_then(|properties| properties.first_mut())
        .and_then(|property| property.get_mut("variable"))
        .ok_or("source declaration absent")?;
    *variable = Value::String(SENTINEL.to_owned());
    let failure = compare_mutation(&fixture, &document)?;
    assert_eq!(
        failure,
        Failure::SourceCompare(SourceFault::DeclarationTextValue),
        "value mismatches are distinct from missing arrays"
    );
    assert_sanitized(failure);
    Ok(())
}

#[test]
fn invalid_raw_shapes_retain_no_source_value() -> TestResult {
    let actual = serde_json::json!([null]);
    let failure = values::check(&[SourceValue::String(SENTINEL)], &actual, 0)
        .err()
        .ok_or("a raw string declaration was accepted")?;
    assert_eq!(
        failure,
        Failure::SourceCompare(SourceFault::SourceStructShape),
        "source shape errors are typed independently of their contents"
    );
    assert_sanitized(failure);
    Ok(())
}

#[test]
fn unresolved_joins_retain_no_configured_target_identity() -> TestResult {
    let fixture = Fixture::new()?;
    let mut document = fixture.catalog()?;
    let links = document
        .pointer_mut("/projections/contracts/rows")
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.first_mut())
        .and_then(|row| row.get_mut("propertySetGuids"))
        .and_then(Value::as_array_mut)
        .ok_or("contract links absent")?;
    *links = vec![Value::String(SENTINEL.to_owned())];
    let failure = compare_mutation(&fixture, &document)?;
    assert_eq!(
        failure,
        Failure::Joins(JoinFault::Target),
        "joins have a separate static failure stage"
    );
    assert_sanitized(failure);
    Ok(())
}
