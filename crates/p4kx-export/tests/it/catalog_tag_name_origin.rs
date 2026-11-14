// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Value as OriginalValue};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};
use crate::health_origin_assertions;

#[test]
fn absent_tag_name_uses_its_complete_original_declaration_survey() -> TestResult {
    let database = database(TestStruct::new("Tag"), None);
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual absent-name Tag Record")?;
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0);
    assert!(
        tree.root().fields().is_empty(),
        "the original declaration list is empty"
    );
    assert!(
        tree.field("tagName").is_none(),
        "the name has no original field occurrence"
    );

    let (_, catalog) = fixture::diagnostic(&database)?;
    let raw = fixture::row(&catalog, "definitions/tags", 0)?;
    let path = fixture::row(&catalog, "projections/tagPaths", 0)?;
    assert_eq!(at(raw, "/tagName/state")?, &json!("absent"));
    assert!(at(raw, "/tagName/value")?.is_null());
    assert_eq!(at(path, "/path/value")?, &json!(""));
    assert_eq!(at(path, "/path/sourceSetComplete")?, &json!(true));
    for cell in [at(raw, "/tagName")?, at(path, "/path")?] {
        let inputs = health_origin_assertions::uses(&catalog, cell)?;
        assert!(
            inputs
                .iter()
                .all(|used| used.pointer("/location/field") != Some(&json!("tagName"))),
            "an undeclared field cannot have a physical slot"
        );
        assert_eq!(
            inputs.iter().filter(|used| survey(used)).count(),
            1,
            "the actual complete declaration survey determines absence"
        );
    }
    Ok(())
}

#[test]
fn declared_null_and_empty_tag_names_keep_their_distinct_original_field_occurrence() -> TestResult {
    for (field_type, value, state, role) in [
        (
            TestType::StrongPointer("OptionalName".to_owned()),
            TestValue::Null,
            "null",
            "selector",
        ),
        (
            TestType::String,
            TestValue::text(""),
            "present",
            "conversion",
        ),
    ] {
        let database = database(
            TestStruct::new("Tag").field("tagName", field_type),
            Some(value),
        );
        let bytes = database.build();
        let forge = DataForge::open(&bytes)?;
        let tree = forge
            .record(0)
            .ok_or("actual declared-name Tag Record")?
            .tree()?;
        assert_eq!(tree.error_count(), 0);
        match state {
            "null" => assert!(matches!(tree.field("tagName"), Some(OriginalValue::Null))),
            _ => assert!(matches!(
                tree.field("tagName"),
                Some(OriginalValue::String(""))
            )),
        }

        let (_, catalog) = fixture::diagnostic(&database)?;
        let raw = fixture::row(&catalog, "definitions/tags", 0)?;
        let path = fixture::row(&catalog, "projections/tagPaths", 0)?;
        assert_eq!(at(raw, "/tagName/state")?, &json!(state));
        assert_eq!(
            at(raw, "/tagName/value")?,
            &if state == "null" {
                Value::Null
            } else {
                json!("")
            }
        );
        assert_eq!(at(path, "/path/value")?, &json!(""));
        assert_eq!(at(path, "/path/sourceSetComplete")?, &json!(true));
        assert_name_use(&catalog, at(raw, "/tagName")?, "payload")?;
        assert_name_use(&catalog, at(path, "/path")?, role)?;
    }
    Ok(())
}

fn database(structure: TestStruct, value: Option<TestValue>) -> DataForgeBuilder {
    let mut record = TestRecord::new(fixture::ROOT, "Tag", "Tag.State", "synthetic/tags.xml");
    if let Some(value) = value {
        record = record.value("tagName", value);
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(structure)
        .structure(TestStruct::new("OptionalName"))
        .record(record)
}

fn survey(used: &Value) -> bool {
    used.pointer("/location/type") == Some(&json!("subresource"))
        && used.pointer("/location/namespace") == Some(&json!("dataforgeRecordDeclarations"))
        && used.pointer("/location/indices") == Some(&json!(["0"]))
        && used.pointer("/location/field") == Some(&json!("$fields"))
        && used.get("role") == Some(&json!("selector"))
}

fn assert_name_use(catalog: &Value, cell: &Value, role: &str) -> TestResult {
    let inputs = health_origin_assertions::uses(catalog, cell)?;
    assert!(
        !inputs.iter().any(|used| survey(used)),
        "a declared slot does not need an absence survey"
    );
    assert_eq!(
        inputs
            .iter()
            .filter(|used| {
                used.pointer("/location/type") == Some(&json!("dataForge"))
                    && used.pointer("/location/recordIndex") == Some(&json!(0))
                    && used.pointer("/location/field") == Some(&json!("tagName"))
                    && used.get("role") == Some(&json!(role))
            })
            .count(),
        1,
        "the original declared field contributes in its actual role"
    );
    Ok(())
}
