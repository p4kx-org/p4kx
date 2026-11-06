// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Unsupported Generator properties reject their source Record without partial set output.

use p4kx_testkit::{TestInstance, TestValue};

use crate::mission_property_sets_fixture::{self as fixture, generator, parameters, rows};
use crate::scoped_fixture::{TestResult, object, property, scoped};

#[test]
fn unsupported_value_variants_fail_the_generator_without_leaving_partial_sets() -> TestResult {
    let source = generator([
        parameters(vec![property("Valid", scoped([]))]),
        parameters(vec![property(
            "Unknown",
            TestInstance::new("MissionPropertyValue_Unknown"),
        )]),
        parameters(vec![]),
    ]);
    let report = fixture::build(source, None)?;
    assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|reason| reason.contains("unknown struct")
                && reason.contains("MissionPropertyValue_Unknown")),
        "the concrete unsupported source type is diagnosed"
    );
    assert!(
        rows(&report.document)?.is_empty(),
        "a failed Generator emits no partial source sets"
    );
    assert_eq!(
        report.document["contracts"]
            .as_array()
            .ok_or("contracts")?
            .len(),
        0
    );
    Ok(())
}

#[test]
fn unreadable_value_pointers_are_not_exported_as_a_null_declaration() -> TestResult {
    let invalid: Vec<u8> = (u32::MAX - 1)
        .to_le_bytes()
        .into_iter()
        .chain(3_u32.to_le_bytes())
        .collect();
    let bad = object(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", TestValue::text("Unreadable"))
            .value("value", TestValue::Raw(invalid)),
    );
    let report = fixture::build(
        generator([
            parameters(vec![bad]),
            parameters(vec![]),
            parameters(vec![]),
        ]),
        None,
    )?;
    assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|reason| reason.contains("value") && reason.contains("unreadable")),
        "a present unreadable value is diagnosed"
    );
    assert!(
        rows(&report.document)?.is_empty(),
        "an unreadable source Record is not partially emitted"
    );
    Ok(())
}

#[test]
fn an_unsupported_scoped_variant_on_a_subcontract_fails_its_original_generator() -> TestResult {
    let bad = scoped([(
        "relativeLocationFilter",
        object(TestInstance::new("LocationCondition_UnknownRelative")),
    )]);
    let report = fixture::build(
        generator([
            parameters(vec![]),
            parameters(vec![]),
            parameters(vec![property("Bad", bad)]),
        ]),
        None,
    )?;
    assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|reason| reason.contains("LocationCondition_UnknownRelative")),
        "the unsupported concrete variant is diagnosed"
    );
    assert!(
        rows(&report.document)?.is_empty(),
        "a failed Generator emits no source sets"
    );
    Ok(())
}

#[test]
fn an_unreadable_override_array_is_not_an_empty_source_set() -> TestResult {
    let invalid: Vec<u8> = 1_u32
        .to_le_bytes()
        .into_iter()
        .chain(u32::MAX.to_le_bytes())
        .collect();
    let overrides = object(
        TestInstance::new("ContractParamOverrides")
            .value("propertyOverrides", TestValue::Raw(invalid)),
    );
    let report = fixture::build(
        generator([overrides, parameters(vec![]), parameters(vec![])]),
        None,
    )?;
    assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
    assert!(
        report
            .failures
            .iter()
            .all(|reason| reason.contains("propertyOverrides") && reason.contains("unreadable")),
        "an unreadable array is distinct from an empty authored array"
    );
    assert!(
        rows(&report.document)?.is_empty(),
        "an unreadable source emits no empty replacement set"
    );
    Ok(())
}
