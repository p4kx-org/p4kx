// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_tags_fixture::{self as fixture, TestResult, at};

#[test]
fn actual_schema_requires_canonical_tag_tables_and_all_authored_facts() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::entities())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "actual producer positive control"
    );
    for path in [
        "/definitions/tags",
        "/declarations/tagRoots",
        "/declarations/entityTags",
        "/projections/tagPaths",
    ] {
        let mut missing = catalog.clone();
        let (parent, member) = path.rsplit_once('/').ok_or("table path")?;
        missing
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or("parent")?
            .remove(member)
            .ok_or("table")?;
        assert!(!validator.is_valid(&missing), "required {path}");
    }
    for field in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "tagName",
        "legacyGuid",
        "children",
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut("/definitions/tags/rows/0")
            .and_then(Value::as_object_mut)
            .ok_or("tag row")?
            .remove(field)
            .ok_or("field")?;
        assert!(!validator.is_valid(&missing), "required {field}");
    }
    Ok(())
}

#[test]
fn nested_carriers_require_orthogonal_states_origins_and_typed_reference_payloads() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::entities())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for path in [
        "/definitions/tags/rows/0/tagName",
        "/definitions/tags/rows/0/children/value/0",
        "/definitions/tags/rows/0/children/value/0/value/guid",
        "/projections/tagPaths/rows/1/path",
    ] {
        for field in [
            "state",
            "value",
            "sourceUsesRefs",
            "sourceSetComplete",
            "containsEncryptedSource",
        ] {
            let mut missing = catalog.clone();
            missing
                .pointer_mut(path)
                .and_then(Value::as_object_mut)
                .ok_or("carrier")?
                .remove(field)
                .ok_or("carrier member")?;
            assert!(!validator.is_valid(&missing), "required {path}/{field}");
        }
        let mut inconsistent = catalog.clone();
        inconsistent
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .ok_or("carrier")?
            .insert("state".to_owned(), json!("absent"));
        assert!(
            !validator.is_valid(&inconsistent),
            "nonpresent states require null payloads"
        );
    }
    let mut invalid = catalog.clone();
    *invalid
        .pointer_mut("/definitions/tags/rows/0/legacyGuid/value")
        .ok_or("legacy identifier")? = json!(-1);
    assert!(!validator.is_valid(&invalid), "stored identifier is UInt32");
    let mut future = catalog;
    future
        .as_object_mut()
        .ok_or("catalog")?
        .insert("futureMetadata".to_owned(), json!({"extra":true}));
    assert!(
        validator.is_valid(&future),
        "additive metadata remains ignorable"
    );
    assert_eq!(at(&future, "/schemaVersion")?, &json!(3));
    Ok(())
}

#[test]
fn duplicate_or_unknown_source_controls_cannot_masquerade_as_healthy_tag_data() -> TestResult {
    let (_, catalog) = fixture::diagnostic(&fixture::entities())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (field, value) in [
        ("state", json!("futureState")),
        ("sourceSetComplete", json!(false)),
    ] {
        let mut invalid = catalog.clone();
        invalid
            .pointer_mut("/definitions/tags/rows/0/tagName")
            .and_then(Value::as_object_mut)
            .ok_or("carrier")?
            .insert(field.to_owned(), value);
        assert!(
            !validator.is_valid(&invalid),
            "unknown state or false complete-proof contradiction"
        );
    }
    let mut duplicate = catalog;
    let uses = duplicate
        .pointer_mut("/definitions/tags/rows/0/tagName/sourceUsesRefs")
        .and_then(Value::as_array_mut)
        .ok_or("immediate uses")?;
    let first = uses.first().ok_or("nonempty direct use set")?.clone();
    uses.push(first);
    assert!(
        !validator.is_valid(&duplicate),
        "immediate uses are distinct"
    );
    Ok(())
}
