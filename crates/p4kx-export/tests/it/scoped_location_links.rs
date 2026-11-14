// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source reference identities survive missing definitions and unresolved weak targets.

use p4kx_testkit::{TestInstance, TestValue};

use crate::scoped_fixture::{self, TestResult, object, property, scoped};

#[path = "scoped_canonical_links.rs"]
mod canonical;

fn weak_array_property() -> TestValue {
    const INVALID_STRUCT: u32 = u32::MAX - 1;
    const INVALID_INSTANCE: u32 = 7;
    let invalid: Vec<u8> = INVALID_STRUCT
        .to_le_bytes()
        .into_iter()
        .chain(INVALID_INSTANCE.to_le_bytes())
        .collect();
    property(
        "Scoped_BP",
        scoped([(
            "otherLocationFilter",
            object(TestInstance::new("LocationCondition_NotLocations").value(
                "locations",
                TestValue::Array(vec![
                    TestValue::pointer("origin"),
                    TestValue::Null,
                    TestValue::pointer("origin"),
                    TestValue::pointer("outside"),
                    TestValue::pointer("MissionProperty.Foreign"),
                    TestValue::Raw(invalid),
                ]),
            )),
        )]),
    )
}

#[test]
fn weak_arrays_keep_nulls_repetitions_owner_paths_and_unresolved_source_coordinates() -> TestResult
{
    let report = scoped_fixture::diagnostic(vec![weak_array_property()])?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    canonical::array(&report.document)?;
    Ok(())
}

#[test]
fn a_foreign_owner_is_never_resolved_against_a_same_named_local_property() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([(
            "otherLocationFilter",
            object(
                TestInstance::new("LocationCondition_SameLocation")
                    .value("location", TestValue::pointer("MissionProperty.Foreign")),
            ),
        )]),
    )])?;
    canonical::foreign_owner(&report.document)?;
    Ok(())
}

#[test]
fn a_non_property_owned_target_keeps_its_identity_without_a_fabricated_variable() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([(
            "otherLocationFilter",
            object(
                TestInstance::new("LocationCondition_SameLocation")
                    .value("location", TestValue::pointer("wrong")),
            ),
        )]),
    )])?;
    canonical::other_target(&report.document)
}

#[test]
fn an_unreadable_target_variable_is_distinct_from_a_missing_or_null_link() -> TestResult {
    let report = scoped_fixture::diagnostic_with_origin(
        vec![property(
            "Scoped_BP",
            scoped([(
                "otherLocationFilter",
                object(
                    TestInstance::new("LocationCondition_SameLocation")
                        .value("location", TestValue::pointer("origin")),
                ),
            )]),
        )],
        TestValue::Raw(u32::MAX.to_le_bytes().to_vec()),
    )?;
    canonical::unreadable_variable(&report.document)?;
    Ok(())
}

#[test]
fn a_dangling_definition_guid_is_not_dropped_or_resolved_by_name() -> TestResult {
    let report = scoped_fixture::build(vec![property(
        "Scoped_BP",
        scoped([(
            "locationClassFilter",
            object(
                TestInstance::new("LocationCondition_ExcludedLocationClasses").value(
                    "locationClasses",
                    TestValue::Array(vec![
                        TestValue::Reference(scoped_fixture::MISSING),
                        TestValue::Null,
                        TestValue::Reference(scoped_fixture::DEFINITION_A),
                    ]),
                ),
            ),
        )]),
    )])?;
    canonical::dangling_definition(&report.document)?;
    Ok(())
}
