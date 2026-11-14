// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use jsonschema::Validator;
use serde_json::{Value, json};

use super::TestResult;

pub(super) fn assert_unknown_nested_fields(validator: &Validator, catalog: Value) -> TestResult {
    let mut unknown_kind = catalog.clone();
    for (path, key, value) in [
        ("", "starmap", json!([{"guid": "x"}])),
        ("/completeness", "starmap", json!({"status": "complete"})),
        ("/source", "extra", json!(true)),
        ("/completeness/items", "extra", json!(true)),
        ("/items/0/name", "extra", json!(true)),
    ] {
        let object = unknown_kind
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .ok_or(path)?;
        assert!(
            object.insert(key.to_owned(), value).is_none(),
            "{path}/{key} is a new field"
        );
    }
    assert!(
        validator.is_valid(&unknown_kind),
        "unknown kinds and fields everywhere"
    );

    let mut unknown_ingredient = catalog;
    let ingredient = unknown_ingredient
        .pointer_mut("/projections/blueprintPresentation/rows/0/ingredients/0")
        .and_then(Value::as_object_mut)
        .ok_or("the blueprint ingredient")?;
    assert!(
        ingredient.insert("extra".to_owned(), json!(true)).is_none(),
        "extra is a new ingredient field"
    );
    assert!(
        validator.is_valid(&unknown_ingredient),
        "unknown fields in an ingredient"
    );
    Ok(())
}

pub(super) fn assert_unknown_canonical_fields(
    validator: &Validator,
    catalog: &Value,
) -> TestResult {
    for pointer in [
        "/definitions/blueprintCategories/rows/0",
        "/definitions/gameplayProperties/rows/0",
        "/definitions/commodities/rows/0",
        "/projections/commodityPresentation/rows/0",
    ] {
        let mut unknown = catalog.clone();
        unknown
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .ok_or("the canonical row")?
            .insert("extra".to_owned(), json!(true));
        assert!(validator.is_valid(&unknown), "unknown fields in {pointer}");
    }

    Ok(())
}
