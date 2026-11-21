// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::manufacturer_v3_fixture::{self as fixture, Slot, State, TestResult};

#[test]
fn generated_schema_requires_all_direct_manufacturer_fields() -> TestResult {
    let (_, catalog) = fixture::normal(&fixture::database())?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&catalog),
        "unmutated real producer positive control"
    );
    for field in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "code",
        "localization",
    ] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut("/definitions/manufacturers/rows/0")
            .and_then(Value::as_object_mut)
            .ok_or("canonical row mutation target")?
            .remove(field)
            .ok_or("required row member")?;
        assert!(
            !validator.is_valid(&missing),
            "removing {field} must fail the actual generated schema"
        );
    }
    let mut legacy = catalog;
    legacy
        .pointer_mut("/definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions mutation target")?
        .remove("manufacturers")
        .ok_or("canonical table")?;
    legacy
        .as_object_mut()
        .ok_or("catalog object")?
        .insert("manufacturers".to_owned(), json!([]));
    assert!(
        !validator.is_valid(&legacy),
        "the old root field cannot replace the canonical table"
    );
    Ok(())
}

#[test]
fn parent_states_and_nested_leaf_carriers_keep_required_null_and_state_correlation() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    for state in [
        State::Absent,
        State::Null,
        State::Present,
        State::Invalid,
        State::Unreadable,
    ] {
        let (_, catalog) = fixture::build(
            &fixture::state_database(Slot::Localization, state),
            DcbOrigin::Plain,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        assert!(
            validator.is_valid(&catalog),
            "producer parent {state:?} validates"
        );
        let mut bad = catalog;
        let parent = bad
            .pointer_mut("/definitions/manufacturers/rows/0/localization")
            .and_then(Value::as_object_mut)
            .ok_or("parent source carrier")?;
        parent.insert("state".to_owned(), json!("absent"));
        parent.insert("value".to_owned(), json!({}));
        assert!(
            !validator.is_valid(&bad),
            "non-present parents cannot hide an object payload"
        );
    }
    let (_, catalog) = fixture::normal(&fixture::database())?;
    for field in ["sourceType", "name", "description"] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut("/definitions/manufacturers/rows/0/localization/value")
            .and_then(Value::as_object_mut)
            .ok_or("present localization")?
            .remove(field)
            .ok_or("required localization member")?;
        assert!(
            !validator.is_valid(&missing),
            "nested {field} is a required sourced carrier"
        );
    }
    Ok(())
}
