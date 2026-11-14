// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scoped-location values reach the template, broker, objective and derived-contract export paths.

use p4kx_testkit::{TestInstance, TestValue};

use crate::scoped_canonical as canonical;
use crate::scoped_fixture::{self, TestResult, object, property, scoped};

#[test]
fn all_six_absent_filters_remain_nullable_and_the_property_is_not_dropped() -> TestResult {
    let report = scoped_fixture::build(vec![property("Scoped_BP", scoped([]))])?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    canonical::absent(&report.document)?;
    assert_eq!(
        crate::catalog_contracts::v3::rows(&report.document)?.len(),
        1
    );
    Ok(())
}

#[test]
fn raw_enum_names_are_retained_without_applying_hostility_or_scope_policy() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([
            (
                "factionFilter",
                object(
                    TestInstance::new("LocationCondition_ExcludedFactionHostility")
                        .value("hostility", TestValue::text("FutureHostility")),
                ),
            ),
            (
                "relativeLocationFilter",
                object(
                    TestInstance::new("LocationCondition_DifferentScope")
                        .value("scope", TestValue::text("FutureScope"))
                        .value("location", TestValue::pointer("origin")),
                ),
            ),
        ]),
    )])?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    canonical::raw_enums(&report.document)?;
    Ok(())
}

#[test]
fn empty_definition_arrays_are_not_absent_filters_or_absent_arrays() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([
            (
                "locationClassFilter",
                object(
                    TestInstance::new("LocationCondition_RequiredLocationClasses")
                        .value("locationClasses", TestValue::Array(vec![])),
                ),
            ),
            (
                "locationFunctionFilter",
                object(
                    TestInstance::new("LocationCondition_ExcludedLocationFunction")
                        .value("locationFunction", TestValue::Null),
                ),
            ),
        ]),
    )])?;
    canonical::empty(&report.document)?;
    Ok(())
}

#[test]
fn the_runtime_schema_accepts_complete_scoped_source_values() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([(
            "locationClassFilter",
            object(
                TestInstance::new("LocationCondition_RequiredLocationClass").value(
                    "locationClass",
                    TestValue::Reference(scoped_fixture::DEFINITION_A),
                ),
            ),
        )]),
    )])?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let errors: Vec<String> = validator
        .iter_errors(&report.document)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    Ok(())
}
