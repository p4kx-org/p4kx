// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every declared filter alternative, including schema-only alternatives, keeps its own type.

use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

use crate::scoped_fixture::{self, TestResult, object, property, scoped, value};

struct Case {
    source: &'static str,
    tag: &'static str,
    multiple: bool,
}

struct Family {
    field: &'static str,
    singular: &'static str,
    plural: &'static str,
    first: u128,
    second: u128,
    first_guid: &'static str,
    second_guid: &'static str,
    cases: [Case; 4],
}

const FAMILIES: [Family; 3] = [
    Family {
        field: "locationClassFilter",
        singular: "locationClass",
        plural: "locationClasses",
        first: scoped_fixture::DEFINITION_A,
        second: scoped_fixture::DEFINITION_B,
        first_guid: "44000000-0000-4000-8000-000000000001",
        second_guid: "44000000-0000-4000-8000-000000000002",
        cases: [
            Case {
                source: "LocationCondition_RequiredLocationClass",
                tag: "requiredLocationClass",
                multiple: false,
            },
            Case {
                source: "LocationCondition_ExcludedLocationClass",
                tag: "excludedLocationClass",
                multiple: false,
            },
            Case {
                source: "LocationCondition_RequiredLocationClasses",
                tag: "requiredLocationClasses",
                multiple: true,
            },
            Case {
                source: "LocationCondition_ExcludedLocationClasses",
                tag: "excludedLocationClasses",
                multiple: true,
            },
        ],
    },
    Family {
        field: "locationSizeFilter",
        singular: "locationSize",
        plural: "locationSizes",
        first: scoped_fixture::SIZE_A,
        second: scoped_fixture::SIZE_B,
        first_guid: "66000000-0000-4000-8000-000000000001",
        second_guid: "66000000-0000-4000-8000-000000000002",
        cases: [
            Case {
                source: "LocationCondition_RequiredLocationSize",
                tag: "requiredLocationSize",
                multiple: false,
            },
            Case {
                source: "LocationCondition_ExcludedLocationSize",
                tag: "excludedLocationSize",
                multiple: false,
            },
            Case {
                source: "LocationCondition_RequiredLocationSizes",
                tag: "requiredLocationSizes",
                multiple: true,
            },
            Case {
                source: "LocationCondition_ExcludedLocationSizes",
                tag: "excludedLocationSizes",
                multiple: true,
            },
        ],
    },
    Family {
        field: "locationFunctionFilter",
        singular: "locationFunction",
        plural: "locationFunctions",
        first: scoped_fixture::FUNCTION_A,
        second: scoped_fixture::FUNCTION_B,
        first_guid: "99000000-0000-4000-8000-000000000001",
        second_guid: "99000000-0000-4000-8000-000000000002",
        cases: [
            Case {
                source: "LocationCondition_RequiredLocationFunction",
                tag: "requiredLocationFunction",
                multiple: false,
            },
            Case {
                source: "LocationCondition_ExcludedLocationFunction",
                tag: "excludedLocationFunction",
                multiple: false,
            },
            Case {
                source: "LocationCondition_RequiredLocationFunctions",
                tag: "requiredLocationFunctions",
                multiple: true,
            },
            Case {
                source: "LocationCondition_ExcludedLocationFunctions",
                tag: "excludedLocationFunctions",
                multiple: true,
            },
        ],
    },
];

#[test]
fn every_required_excluded_singular_and_plural_definition_filter_is_exported() -> TestResult {
    for family in &FAMILIES {
        for case in &family.cases {
            let (field, payload) = input(family, case);
            let report = scoped_fixture::build(vec![property(
                "Scoped_BP",
                scoped([(
                    family.field,
                    object(TestInstance::new(case.source).value(field, payload)),
                )]),
            )])?;
            assert_eq!(
                report.failures,
                Vec::<String>::new(),
                "{} reads fully",
                case.source
            );
            for kind in ["contractTemplates", "missionBrokerEntries"] {
                check(
                    family,
                    case,
                    &value(&report.document, kind, "Scoped_BP")?[family.field],
                )?;
            }
        }
    }
    Ok(())
}

fn input(family: &Family, case: &Case) -> (&'static str, TestValue) {
    if case.multiple {
        (
            family.plural,
            TestValue::Array(vec![
                TestValue::Reference(family.second),
                TestValue::Null,
                TestValue::Reference(family.first),
                TestValue::Reference(family.second),
            ]),
        )
    } else {
        (family.singular, TestValue::Reference(family.first))
    }
}

fn check(family: &Family, case: &Case, filter: &Value) -> TestResult {
    assert_eq!(
        filter["type"], case.tag,
        "{} keeps its source variant",
        case.source
    );
    if case.multiple {
        let references = filter[family.plural].as_array().ok_or("reference array")?;
        let guids: Vec<Value> = references
            .iter()
            .map(|entry| {
                if entry.is_null() {
                    Value::Null
                } else {
                    entry["guid"].clone()
                }
            })
            .collect();
        assert_eq!(
            guids,
            vec![
                json!(family.second_guid),
                Value::Null,
                json!(family.first_guid),
                json!(family.second_guid),
            ],
            "{} preserves source order, repetitions and null entries",
            case.source
        );
    } else {
        let target = filter.get(family.singular).ok_or("singular definition")?;
        assert_eq!(
            target.get("guid").and_then(Value::as_str),
            Some(family.first_guid),
            "{} target",
            case.source
        );
        assert_eq!(
            target.get("status").and_then(Value::as_str),
            Some("present"),
            "{} source availability",
            case.source
        );
        assert!(
            target.get("recordPath").is_some_and(Value::is_string),
            "present target path"
        );
        assert!(
            target.get("targetType").is_some_and(Value::is_string),
            "present target struct"
        );
    }
    Ok(())
}

#[test]
fn both_faction_variants_keep_the_raw_hostility() -> TestResult {
    for (source, tag) in [
        (
            "LocationCondition_RequiredFactionHostility",
            "requiredFactionHostility",
        ),
        (
            "LocationCondition_ExcludedFactionHostility",
            "excludedFactionHostility",
        ),
    ] {
        let report = scoped_fixture::build(vec![property(
            "Scoped_BP",
            scoped([(
                "factionFilter",
                object(TestInstance::new(source).value("hostility", TestValue::text("Hostile"))),
            )]),
        )])?;
        for kind in ["contractTemplates", "missionBrokerEntries"] {
            assert_eq!(
                value(&report.document, kind, "Scoped_BP")?["factionFilter"],
                json!({"type": tag, "hostility": "Hostile"})
            );
        }
    }
    Ok(())
}

#[test]
fn same_location_not_location_and_both_scope_variants_keep_their_identity() -> TestResult {
    for (family, source, tag) in [
        (
            "otherLocationFilter",
            "LocationCondition_NotLocation",
            "notLocation",
        ),
        (
            "otherLocationFilter",
            "LocationCondition_SameLocation",
            "sameLocation",
        ),
        (
            "relativeLocationFilter",
            "LocationCondition_SameScope",
            "sameScope",
        ),
        (
            "relativeLocationFilter",
            "LocationCondition_DifferentScope",
            "differentScope",
        ),
    ] {
        let mut condition =
            TestInstance::new(source).value("location", TestValue::pointer("origin"));
        if family == "relativeLocationFilter" {
            condition = condition.value("scope", TestValue::text("FutureScope"));
        }
        let report = scoped_fixture::build(vec![property(
            "Scoped_BP",
            scoped([(family, object(condition))]),
        )])?;
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        for kind in ["contractTemplates", "missionBrokerEntries"] {
            let filter = &value(&report.document, kind, "Scoped_BP")?[family];
            assert_eq!(filter["type"], tag);
            assert_eq!(filter["location"]["status"], "resolved");
            assert_eq!(filter["location"]["variable"], "Origin_BP");
        }
    }
    Ok(())
}
