// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Authored Generator scopes preserve six filters and owner-bound weak references.

use p4kx_testkit::{TestInstance, TestValue};
use serde_json::Value;

use crate::mission_property_sets_fixture::{
    self as fixture, GENERATOR, generator, level, parameters,
};
use crate::scoped_fixture::{self, TestResult, object, property, scoped};

fn filter(source_type: &str, field: &str, value: TestValue) -> TestValue {
    object(TestInstance::new(source_type).value(field, value))
}

fn filters() -> TestInstance {
    scoped([
        (
            "factionFilter",
            filter(
                "LocationCondition_ExcludedFactionHostility",
                "hostility",
                TestValue::text("FutureHostility"),
            ),
        ),
        (
            "locationClassFilter",
            filter(
                "LocationCondition_RequiredLocationClasses",
                "locationClasses",
                TestValue::Array(vec![
                    TestValue::Reference(scoped_fixture::DEFINITION_A),
                    TestValue::Null,
                    TestValue::Reference(scoped_fixture::DEFINITION_A),
                ]),
            ),
        ),
        (
            "locationSizeFilter",
            filter(
                "LocationCondition_ExcludedLocationSize",
                "locationSize",
                TestValue::Reference(scoped_fixture::SIZE_A),
            ),
        ),
        (
            "locationFunctionFilter",
            filter(
                "LocationCondition_RequiredLocationFunction",
                "locationFunction",
                TestValue::Reference(scoped_fixture::FUNCTION_A),
            ),
        ),
        (
            "otherLocationFilter",
            filter(
                "LocationCondition_SameLocation",
                "location",
                TestValue::pointer("source-origin"),
            ),
        ),
        (
            "relativeLocationFilter",
            object(
                TestInstance::new("LocationCondition_DifferentScope")
                    .value("scope", TestValue::text("FutureScope"))
                    .value("location", TestValue::pointer("source-origin")),
            ),
        ),
    ])
}

#[test]
fn each_generator_scope_retains_the_complete_six_filter_payload() -> TestResult {
    let origin = object(
        TestInstance::new("MissionProperty")
            .named("source-origin")
            .value("missionVariableName", TestValue::text("Origin")),
    );
    let report = fixture::build(
        generator([
            parameters(vec![origin, property("Handler", filters())]),
            parameters(vec![property("Contract", filters())]),
            parameters(vec![property("Sub", filters())]),
        ]),
        None,
    )?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    for (source_level, variable) in [
        ("handler", "Handler"),
        ("contract", "Contract"),
        ("subContract", "Sub"),
    ] {
        let set = level(&report.document, source_level)?;
        let scoped = &fixture::property(set, variable)?["value"];
        assert_eq!(scoped["factionFilter"]["hostility"], "FutureHostility");
        assert_eq!(
            scoped["locationClassFilter"]["type"],
            "requiredLocationClasses"
        );
        let classes = scoped["locationClassFilter"]["locationClasses"]
            .as_array()
            .ok_or("classes")?;
        assert_eq!(classes.len(), 3);
        assert_eq!(classes.first(), classes.get(2));
        assert_eq!(classes.get(1), Some(&Value::Null));
        assert_eq!(scoped["locationSizeFilter"]["type"], "excludedLocationSize");
        assert_eq!(
            scoped["locationFunctionFilter"]["type"],
            "requiredLocationFunction"
        );
        assert_eq!(scoped["relativeLocationFilter"]["scope"], "FutureScope");
        for field in ["otherLocationFilter", "relativeLocationFilter"] {
            let target = &scoped[field]["location"];
            assert_eq!(target["recordGuid"], GENERATOR);
            assert_eq!(
                target["location"],
                "generators[0].contractParams.propertyOverrides[0]"
            );
            assert_eq!(target["variable"], "Origin");
            assert_eq!(target["status"], "resolved");
        }
    }
    Ok(())
}

#[test]
fn generator_weak_links_retain_foreign_identity_without_resolving_its_values() -> TestResult {
    let scoped = scoped([(
        "otherLocationFilter",
        object(
            TestInstance::new("LocationCondition_NotLocation")
                .value("location", TestValue::pointer("MissionProperty.Foreign")),
        ),
    )]);
    let report = fixture::build(
        generator([
            parameters(vec![property("Foreign", scoped)]),
            parameters(vec![]),
            parameters(vec![]),
        ]),
        None,
    )?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let property = fixture::property(level(&report.document, "handler")?, "Foreign")?;
    let target = &property["value"]["otherLocationFilter"]["location"];
    assert_eq!(target["recordGuid"], "33000000-0000-4000-8000-000000000001");
    assert_eq!(target["status"], "outsideOwner");
    assert_eq!(target["variable"], Value::Null);
    assert_eq!(target["targetType"], "MissionProperty");
    Ok(())
}

#[test]
fn direct_subcontract_fallback_keeps_its_own_authored_source_position() -> TestResult {
    let record = fixture::with_sub(
        [parameters(vec![]), parameters(vec![])],
        &TestInstance::new("SubContract").value(
            "propertyOverrides",
            TestValue::Array(vec![property("Direct", scoped([]))]),
        ),
    );
    let report = fixture::build(record, None)?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let set = level(&report.document, "subContract")?;
    assert!(
        set["sourceLocation"]
            .as_str()
            .is_some_and(|location| location.ends_with(".propertyOverrides")
                && !location.contains("subContracts[0].paramOverrides")),
        "direct declarations keep their authored sub-contract location"
    );
    assert_eq!(
        fixture::property(set, "Direct")?["value"]["type"],
        "scopedLocation"
    );
    Ok(())
}
