// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Generator source property sets retain typed payloads and fan out through ordered links.

use p4kx_testkit::{TestInstance, TestValue};
use serde_json::{Value, json};

use crate::mission_property_sets_fixture::{
    self as fixture, GENERATOR, generator, level, parameters, rows,
};
use crate::scoped_fixture::{TestResult, object, property, scoped};

fn boolean(value: bool) -> TestValue {
    property(
        "",
        TestInstance::new("MissionPropertyValue_Boolean").value("value", TestValue::Bool(value)),
    )
}

#[test]
fn authored_sets_keep_declarations_and_deduplicate_contract_fanout() -> TestResult {
    let source = generator([
        parameters(vec![
            boolean(false),
            TestValue::Null,
            property("Handler", scoped([])),
        ]),
        parameters(vec![property("Contract", scoped([]))]),
        parameters(vec![property("Sub", scoped([]))]),
    ]);
    let report = fixture::build(source, None)?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(rows(&report.document)?.len(), 4);
    let handler = level(&report.document, "handler")?;
    assert_eq!(handler["recordGuid"], GENERATOR);
    assert_eq!(handler["sourceType"], "ContractGenerator");
    assert_eq!(
        handler["sourceLocation"],
        "generators[0].contractParams.propertyOverrides"
    );
    assert_eq!(
        handler["properties"][0],
        json!({"variable":"", "token":"", "valueType":"Boolean", "value":{"type":"boolean", "value":false}})
    );
    assert_eq!(handler["properties"][1], Value::Null);
    let contract = level(&report.document, "contract")?;
    let contracts = report.document["contracts"].as_array().ok_or("contracts")?;
    assert_eq!(contracts.len(), 3);
    for row in contracts {
        let links = row["propertySetGuids"]
            .as_array()
            .ok_or("propertySetGuids")?;
        assert_eq!(links.first(), Some(&handler["guid"]));
        assert_eq!(links.get(1), Some(&contract["guid"]));
        let expected = if row["subContractGuid"].is_null() {
            2
        } else {
            3
        };
        assert_eq!(links.len(), expected);
        for guid in links {
            assert!(
                rows(&report.document)?
                    .iter()
                    .any(|set| &set["guid"] == guid),
                "every link joins its canonical source set"
            );
        }
    }
    Ok(())
}

#[test]
fn source_identity_survives_payload_changes_but_distinguishes_authored_positions() -> TestResult {
    let build = |value| {
        fixture::build(
            generator([
                parameters(vec![boolean(value)]),
                parameters(vec![boolean(value)]),
                parameters(vec![]),
            ]),
            None,
        )
    };
    let before = build(false)?;
    let after = build(true)?;
    for source_level in ["handler", "contract", "subContract"] {
        assert_eq!(
            level(&before.document, source_level)?["guid"],
            level(&after.document, source_level)?["guid"]
        );
    }
    let handler = level(&before.document, "handler")?;
    let contract = level(&before.document, "contract")?;
    assert_eq!(
        level(&before.document, "subContract")?["properties"],
        json!([])
    );
    assert_ne!(handler["guid"], contract["guid"]);
    assert_eq!(handler["properties"], contract["properties"]);
    assert_ne!(
        level(&before.document, "handler")?["properties"],
        level(&after.document, "handler")?["properties"]
    );
    Ok(())
}

#[test]
fn selected_generator_sources_respect_the_catalog_filter() -> TestResult {
    let record = generator([parameters(vec![]), parameters(vec![]), parameters(vec![])]);
    let excluded = fixture::build(record.clone(), Some("does-not-match"))?;
    assert_eq!(rows(&excluded.document)?.len(), 0);
    let selected = fixture::build(record, Some("scoped-source"))?;
    assert_eq!(rows(&selected.document)?.len(), 4);
    assert_eq!(
        selected.document["completeness"]["missionPropertySets"]["status"],
        "partial"
    );
    Ok(())
}

#[test]
fn null_value_pointers_preserve_the_authored_declaration() -> TestResult {
    let declaration = object(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", TestValue::text("Absent")),
    );
    let report = fixture::build(
        generator([
            parameters(vec![declaration]),
            parameters(vec![]),
            parameters(vec![]),
        ]),
        None,
    )?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let declaration = fixture::property(level(&report.document, "handler")?, "Absent")?;
    assert_eq!(declaration["valueType"], Value::Null);
    assert_eq!(declaration["value"], Value::Null);
    Ok(())
}
