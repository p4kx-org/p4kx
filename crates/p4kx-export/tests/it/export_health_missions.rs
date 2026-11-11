// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mission projection failures and Generator fan-out retain their own source and row units.

use p4kx_testkit::TestInstance;
use serde_json::Value;

use crate::mission_property_sets_fixture::{self, generator, parameters};
use crate::scoped_fixture::{self, TestResult, object, property, scoped};

#[test]
fn unsupported_mission_values_are_explained_inside_the_file() -> TestResult {
    let report = scoped_fixture::diagnostic(vec![property(
        "Scoped_BP",
        scoped([(
            "locationClassFilter",
            object(TestInstance::new("LocationCondition_UnknownClass")),
        )]),
    )])?;
    let health = report.document.get("exportHealth").ok_or("exportHealth")?;
    assert_eq!(
        health.get("status").and_then(Value::as_str),
        Some("incomplete"),
        "the artifact itself describes the failed typed projection"
    );
    assert!(
        crate::mission_canonical_health::example(
            &report.document,
            Some("unsupportedValue"),
            "LocationCondition_UnknownClass",
            |_| true,
        )?,
        "the concrete unsupported variant is diagnosed without parsing an error-display string"
    );
    let failed = health
        .get("failedRecords")
        .and_then(Value::as_array)
        .ok_or("failedRecords")?;
    assert!(
        failed
            .iter()
            .any(|source| source.get("kind").and_then(Value::as_str) == Some("contractTemplates")),
        "template source retention identities are preserved"
    );
    assert!(
        failed.iter().any(
            |source| source.get("kind").and_then(Value::as_str) == Some("missionBrokerEntries")
        ),
        "broker source retention identities are preserved"
    );
    Ok(())
}

#[test]
fn generator_fanout_and_source_sets_do_not_inflate_original_record_counts() -> TestResult {
    let report = mission_property_sets_fixture::build(
        generator([parameters(vec![]), parameters(vec![]), parameters(vec![])]),
        None,
    )?;
    let health = report.document.get("exportHealth").ok_or("exportHealth")?;
    let kinds = health
        .get("kinds")
        .and_then(Value::as_array)
        .ok_or("kind health")?;
    let contracts = kinds
        .iter()
        .find(|kind| kind.get("kind").and_then(Value::as_str) == Some("contracts"))
        .ok_or("contract health")?;
    assert_eq!(
        contracts
            .get("sourceRecords")
            .and_then(|counts| counts.get("selected"))
            .and_then(Value::as_u64),
        Some(1),
        "one original Generator is selected despite its many rows"
    );
    assert_eq!(
        contracts.get("writtenRows").and_then(Value::as_u64),
        Some(3),
        "the contract and two sub-contracts remain row units"
    );
    assert_eq!(
        contracts
            .get("sourceRecords")
            .and_then(|counts| counts.get("intentionallySkipped"))
            .and_then(Value::as_u64),
        Some(0),
        "authored row selection does not invent a source-Record skip"
    );
    Ok(())
}
