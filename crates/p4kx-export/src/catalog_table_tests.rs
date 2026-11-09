// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use schemars::{JsonSchema, SchemaGenerator};
use serde_json::json;

use super::{CatalogTable, DatasetAvailability};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn adding_another_row_family_preserves_existing_table_references_and_definitions() -> TestResult {
    let mut initial = SchemaGenerator::default();
    let expected = initial.subschema_for::<CatalogTable<u32>>();
    let name = CatalogTable::<u32>::schema_name();
    let definition = initial
        .definitions()
        .get(name.as_ref())
        .ok_or("initial table definition")?;
    let mut expanded = SchemaGenerator::default();
    expanded.subschema_for::<CatalogTable<String>>();
    expanded.subschema_for::<CatalogTable<bool>>();
    let actual = expanded.subschema_for::<CatalogTable<u32>>();
    assert_eq!(
        actual, expected,
        "adding row families must not renumber an existing dataset reference"
    );
    assert_eq!(
        expanded.definitions().get(name.as_ref()),
        Some(definition),
        "adding row families must preserve the existing dataset contract"
    );
    Ok(())
}

#[test]
fn named_table_definitions_keep_typed_rows_and_required_availability() -> TestResult {
    let schema = schemars::schema_for!(CatalogTable<u32>);
    let validator = jsonschema::validator_for(&schema.to_value())?;
    let original = serde_json::to_value(CatalogTable {
        availability: DatasetAvailability::Available,
        rows: vec![0_u32],
    })?;
    assert!(
        validator.is_valid(&original),
        "the actual table representation remains valid"
    );
    let mut wrong = original.clone();
    *wrong.pointer_mut("/rows/0").ok_or("typed table row")? = json!("wrong type");
    assert!(
        !validator.is_valid(&wrong),
        "the table name must not erase its row payload type"
    );
    let mut wrong = original;
    wrong
        .as_object_mut()
        .ok_or("actual table object")?
        .remove("availability");
    assert!(
        !validator.is_valid(&wrong),
        "dataset availability remains a required independent control"
    );
    Ok(())
}
