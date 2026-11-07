// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source reference identities survive missing definitions and unresolved weak targets.

use p4kx_testkit::{TestInstance, TestValue};
use serde_json::Value;

use crate::scoped_fixture::{self, TestResult, object, property, scoped, value};

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
    for (kind, guid, location, record_index) in [
        (
            "contractTemplates",
            "11000000-0000-4000-8000-000000000001",
            "contractProperties[0]",
            1_u32,
        ),
        (
            "missionBrokerEntries",
            "22000000-0000-4000-8000-000000000001",
            "properties[0]",
            2_u32,
        ),
    ] {
        let filter = &value(&report.document, kind, "Scoped_BP")?["otherLocationFilter"];
        assert_eq!(filter["type"], "notLocations");
        let links = filter["locations"].as_array().ok_or("locations")?;
        assert_eq!(links.len(), 6);
        assert_eq!(links[0]["recordGuid"], guid);
        assert_eq!(
            links[0]["targetRecordIndex"], record_index,
            "weak links identify the physical owning source"
        );
        assert_eq!(links[0]["location"], location);
        assert_eq!(links[0]["targetType"], "MissionProperty");
        assert_eq!(links[0]["status"], "resolved");
        assert_eq!(links[0]["variable"], "Origin_BP");
        assert!(links[0]["recordPath"].is_string());
        assert_eq!(links[1], Value::Null);
        assert_eq!(links[2], links[0]);
        assert_eq!(links[3]["status"], "outsideOwner");
        assert!(links[3]["sourcePointer"]["structIndex"].is_u64());
        assert!(links[3]["sourcePointer"]["instanceIndex"].is_u64());
        assert_eq!(links[3]["recordGuid"], Value::Null);
        assert_eq!(
            links[3]["targetRecordIndex"],
            Value::Null,
            "unresolved instance coordinates do not fabricate a Record index"
        );
        assert_eq!(links[4]["status"], "outsideOwner");
        assert_eq!(
            links[4]["recordGuid"],
            "33000000-0000-4000-8000-000000000001"
        );
        assert_eq!(links[4]["variable"], Value::Null);
        assert_eq!(
            links[4]["targetRecordIndex"], 0,
            "the known outside-owner target retains its actual index"
        );
        assert_eq!(links[5]["status"], "missingTarget");
        assert_eq!(links[5]["sourcePointer"]["structIndex"], u32::MAX - 1);
        assert_eq!(links[5]["sourcePointer"]["instanceIndex"], 7_u32);
    }
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
    for kind in ["contractTemplates", "missionBrokerEntries"] {
        let link = &value(&report.document, kind, "Scoped_BP")?["otherLocationFilter"]["location"];
        assert_eq!(link["status"], "outsideOwner");
        assert_eq!(link["recordGuid"], "33000000-0000-4000-8000-000000000001");
        assert_eq!(
            link["recordPath"],
            "libs/foundry/records/properties/foreign"
        );
        assert_eq!(link["targetType"], "MissionProperty");
        assert_eq!(link["location"], Value::Null);
        assert_eq!(link["variable"], Value::Null);
    }
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
    let link = &value(&report.document, "contractTemplates", "Scoped_BP")?["otherLocationFilter"]["location"];
    assert_eq!(link["status"], "unsupportedTargetType");
    assert_eq!(link["targetType"], "OtherTarget");
    assert_eq!(link["location"], "wrongTarget");
    assert_eq!(link["variable"], Value::Null);
    Ok(())
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
    for kind in ["contractTemplates", "missionBrokerEntries"] {
        let link = &value(&report.document, kind, "Scoped_BP")?["otherLocationFilter"]["location"];
        assert_eq!(link["status"], "unreadableTarget");
        assert_eq!(link["variable"], Value::Null);
        assert!(
            link["sourceError"]
                .as_str()
                .is_some_and(|error| !error.is_empty())
        );
        assert!(link["recordGuid"].is_string());
        assert!(link["location"].is_string());
    }
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
    for kind in ["contractTemplates", "missionBrokerEntries"] {
        let references =
            &value(&report.document, kind, "Scoped_BP")?["locationClassFilter"]["locationClasses"];
        assert_eq!(
            references[0]["guid"],
            "44000000-0000-4000-8000-0000000000ff"
        );
        assert_eq!(references[0]["status"], "dangling");
        assert_eq!(
            references[0]["targetRecordIndex"],
            Value::Null,
            "a dangling GUID has no physical source position"
        );
        assert_eq!(references[0]["recordPath"], Value::Null);
        assert_eq!(references[1], Value::Null);
        assert_eq!(references[2]["status"], "present");
        assert_eq!(
            references[2]["targetRecordIndex"], 4,
            "the actual definition source position is preserved"
        );
    }
    Ok(())
}
