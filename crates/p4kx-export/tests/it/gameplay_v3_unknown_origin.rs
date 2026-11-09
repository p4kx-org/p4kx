// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_support as support;

pub(crate) fn check(catalog: &Value, database: &DataForgeBuilder) -> TestResult {
    let path = format!(
        "{}/displayTransformation/value/nodes/0/payload",
        support::FIRST
    );
    let mut independent = catalog.clone();
    let carrier = independent
        .pointer_mut(&path)
        .and_then(Value::as_object_mut)
        .ok_or("the actual future producer's Unsupported payload carrier")?;
    carrier.insert("containsEncryptedSource".to_owned(), json!(false));
    carrier.insert("sourceSetComplete".to_owned(), json!(true));
    carrier.insert("sourceUsesRefs".to_owned(), json!([]));
    let changed = support::at(&independent, &path)?;
    let before = support::at(catalog, &path)?;
    assert_eq!(
        changed.get("state"),
        before.get("state"),
        "only contribution metadata changes, not the Unsupported state"
    );
    assert_eq!(
        changed.get("value"),
        before.get("value"),
        "the actual unknown source payload value remains null"
    );
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&independent),
        "the counterfactual is a coherent independent source carrier in the actual schema"
    );
    support::row(&independent, 0)?;
    let error = original::check(&independent, database, 0)
        .err()
        .ok_or("unknown payload must retain its original class selector")?;
    assert!(
        error.to_string().contains("original contribution differs"),
        "the original per-payload selector oracle rejects the source-free mutation: {error}"
    );
    Ok(())
}
