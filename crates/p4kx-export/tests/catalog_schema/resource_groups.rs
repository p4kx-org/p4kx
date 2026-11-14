// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{TestResult, minimal_catalog};

#[test]
fn group_tables_accept_additive_metadata_and_require_each_canonical_owner() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let mut unknown_group = minimal_catalog();
    unknown_group
        .pointer_mut("/definitions/resourceGroups/rows/0")
        .and_then(Value::as_object_mut)
        .ok_or("canonical group fixture")?
        .insert("extra".to_owned(), json!(true));
    assert!(
        validator.is_valid(&unknown_group),
        "unknown group definition metadata stays ignorable"
    );

    for (path, field) in [
        ("/definitions/resourceGroups/rows/0", "recordIndex"),
        ("/declarations/resourceGroupMembers/rows/0", "groups"),
        ("/projections/resourceGroupParents/rows/0", "parent"),
    ] {
        let mut invalid = minimal_catalog();
        invalid
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .ok_or("canonical group fixture")?
            .remove(field)
            .ok_or("required group field")?;
        assert!(!validator.is_valid(&invalid), "{path} requires {field}");
    }
    Ok(())
}
