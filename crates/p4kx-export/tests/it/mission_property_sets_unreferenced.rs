// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Authored source sets survive missing, excluded and deduplicated contract rows.

use p4kx_testkit::{TestInstance, TestValue};
use serde_json::Value;

use crate::mission_property_sets_fixture::{self as fixture, parameters, rows};
use crate::scoped_fixture::{TestResult, object, property, scoped};

fn owner(handler: TestInstance) -> p4kx_testkit::TestRecord {
    fixture::owner().value("generators", TestValue::Array(vec![object(handler)]))
}

fn declarations(variable: &str) -> TestValue {
    parameters(vec![property(variable, scoped([]))])
}

fn contracts(document: &Value) -> Result<&Vec<Value>, &'static str> {
    document["contracts"].as_array().ok_or("contracts")
}

#[test]
fn an_authored_handler_set_is_retained_without_any_contract_rows() -> TestResult {
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("contractParams", declarations("UnusedHandler"));
    let report = fixture::build(owner(handler), None)?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        contracts(&report.document)?.len(),
        0,
        "no contract rows were authored"
    );
    let sets = rows(&report.document)?;
    assert_eq!(
        sets.len(),
        1,
        "an authored source set does not depend on row fanout"
    );
    let set = sets.first().ok_or("handler source set")?;
    assert_eq!(
        set["level"], "handler",
        "the original authoring level is retained"
    );
    assert_eq!(
        fixture::property(set, "UnusedHandler")?["value"]["type"],
        "scopedLocation",
        "the unreferenced payload is retained"
    );
    Ok(())
}

#[test]
fn authored_contract_and_subcontract_sets_do_not_require_row_ids() -> TestResult {
    let sub =
        object(TestInstance::new("SubContract").value("paramOverrides", declarations("NoSubId")));
    let contract = object(
        TestInstance::new("CareerContract")
            .value("paramOverrides", declarations("NoContractId"))
            .value("subContracts", TestValue::Array(vec![sub])),
    );
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("contracts", TestValue::Array(vec![contract]));
    let report = fixture::build(owner(handler), None)?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        contracts(&report.document)?.len(),
        0,
        "missing row ids prevent derived rows"
    );
    assert_eq!(
        rows(&report.document)?.len(),
        2,
        "both authored source arrays are retained"
    );
    assert_eq!(
        fixture::property(
            fixture::level(&report.document, "contract")?,
            "NoContractId"
        )?["value"]["type"],
        "scopedLocation",
        "the contract source payload survives"
    );
    assert_eq!(
        fixture::property(fixture::level(&report.document, "subContract")?, "NoSubId")?["value"]["type"],
        "scopedLocation",
        "the sub-contract source payload survives"
    );
    Ok(())
}

#[test]
fn row_debug_and_duplicate_selection_do_not_discard_distinct_authored_sets() -> TestResult {
    let id = 0x8800_0000_0000_4000_8000_0000_0000_0001;
    let values = [
        ("First", "First"),
        ("Duplicate", "Second"),
        ("Debug", "Debug"),
    ]
    .into_iter()
    .map(|(name, variable)| {
        object(
            TestInstance::new("CareerContract")
                .value("id", TestValue::Guid(id))
                .value("debugName", TestValue::text(name))
                .value("paramOverrides", declarations(variable)),
        )
    })
    .collect();
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("contracts", TestValue::Array(values));
    let report = fixture::build(owner(handler), None)?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        contracts(&report.document)?.len(),
        1,
        "derived row selection still excludes debug and duplicate rows"
    );
    let sets = rows(&report.document)?;
    assert_eq!(
        sets.len(),
        3,
        "every authored array has its own source identity"
    );
    for variable in ["First", "Second", "Debug"] {
        assert!(
            sets.iter()
                .any(|set| fixture::property(set, variable).is_ok()),
            "the authored {variable} declaration is preserved"
        );
    }
    Ok(())
}

#[test]
fn absent_and_null_parameter_blocks_do_not_create_synthetic_empty_sets() -> TestResult {
    for handler in [
        TestInstance::new("ContractGeneratorHandler_Career"),
        TestInstance::new("ContractGeneratorHandler_Career")
            .value("contractParams", TestValue::Null),
    ] {
        let report = fixture::build(owner(handler), None)?;
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert!(
            rows(&report.document)?.is_empty(),
            "no authored array means no invented empty set"
        );
    }
    Ok(())
}

#[test]
fn unsupported_unreferenced_values_are_diagnosed_instead_of_disappearing() -> TestResult {
    let handler = TestInstance::new("ContractGeneratorHandler_Career").value(
        "contractParams",
        parameters(vec![property(
            "Unsupported",
            TestInstance::new("MissionPropertyValue_Unknown"),
        )]),
    );
    let report = fixture::build(owner(handler), None)?;
    assert_eq!(
        report.failures.len(),
        1,
        "an unsupported authored value fails its original Generator"
    );
    assert!(
        report
            .failures
            .iter()
            .all(|error| error.contains("MissionPropertyValue_Unknown")),
        "the concrete unsupported type is identified"
    );
    assert!(
        rows(&report.document)?.is_empty(),
        "a failed Generator has no partial replacement sets"
    );
    Ok(())
}
