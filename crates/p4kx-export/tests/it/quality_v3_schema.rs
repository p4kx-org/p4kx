// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

#[test]
fn quality_v3_schema_requires_exact_numbers_closed_states_and_immediate_origin_cells() -> TestResult
{
    let (_, catalog) = support::normal(&values::healthy())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    let _row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    assert!(validator.is_valid(&catalog));
    let mut missing = catalog.clone();
    missing
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions")?
        .remove("qualityDistributions");
    assert!(
        !validator.is_valid(&missing),
        "the new canonical family is required by the published v3 contract"
    );
    for field in [
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        let mut changed = catalog.clone();
        mean(&mut changed)?.remove(field);
        assert!(
            !validator.is_valid(&changed),
            "each source-backed numeric value requires its immediate provenance carrier"
        );
    }
    let mut unknown_state = catalog.clone();
    mean(&mut unknown_state)?.insert("state".to_owned(), json!("guessed"));
    assert!(
        !validator.is_valid(&unknown_state),
        "unknown source controls cannot imply a successful raw fact"
    );
    for (field, value) in [("type", "float128"), ("rawBits", "deadbeef")] {
        let mut changed = catalog.clone();
        mean(&mut changed)?
            .get_mut("value")
            .and_then(Value::as_object_mut)
            .ok_or("raw number")?
            .insert(field.to_owned(), json!(value));
        assert!(
            !validator.is_valid(&changed),
            "a known f64 token retains its closed width and exact sixteen-digit bits"
        );
    }
    Ok(())
}

fn mean(catalog: &mut Value) -> TestResult<&mut Map<String, Value>> {
    catalog.pointer_mut("/definitions/qualityDistributions/rows/0/qualityDistribution/value/payload/value/value/value/mean")
        .and_then(Value::as_object_mut).ok_or_else(|| "source mean carrier is absent".into())
}
