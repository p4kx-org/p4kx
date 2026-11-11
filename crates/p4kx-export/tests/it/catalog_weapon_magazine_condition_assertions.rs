// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value as Raw};
use serde_json::{Value, json};

use super::{TestResult, body, origins, proof, source};

pub(super) fn check<'source>(
    catalog: &Value,
    row: &Value,
    tree: &RecordTree<'source>,
    original: &Raw<'source>,
    marker: bool,
) -> TestResult {
    if matches!(original, Raw::Null) {
        proof::cell(row, "condition", "null")?;
        return Ok(());
    }
    let actual = source::structure(original)?;
    let cell = proof::cell(row, "condition", "present")?;
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "the authored condition retains its original marker"
    );
    condition_arena(catalog, cell, tree, original, actual, marker)
}

fn condition_arena<'source>(
    catalog: &Value,
    cell: &Value,
    tree: &RecordTree<'source>,
    original: &Raw<'source>,
    actual: &StructValue<'source>,
    marker: bool,
) -> TestResult {
    let [root] = proof::rows(cell, "/value/roots")? else {
        return Err("exactly one source root for this original condition slot".into());
    };
    proof::present(root, "name", &json!("condition"), Some(marker))?;
    assert_eq!(
        proof::at(root, "/value/valueIndex")?,
        &json!("0"),
        "the selected root indexes its own local arena"
    );
    let nodes = proof::rows(cell, "/value/values")?;
    assert_eq!(
        nodes.len(),
        1 + actual.fields().len(),
        "only this original condition and its actual child values are retained"
    );
    let root_value = proof::at(
        nodes.first().ok_or("actual condition root")?,
        "/value/value",
    )?;
    let source_type = proof::present(
        root_value,
        "sourceType",
        &json!(actual.type_name()),
        Some(marker),
    )?;
    origins::exact(
        catalog,
        source_type,
        &[body(tree, original, "$type", "selector")?],
    )?;
    if let Some(comparer) = actual.field("comparer") {
        let value = proof::present(
            nodes.get(1).ok_or("original comparator occurrence")?,
            "value",
            &json!({"type":"boolean","value":false}),
            Some(marker),
        )?;
        origins::exact(
            catalog,
            value,
            &[body(tree, comparer, "$value", "payload")?],
        )?;
    }
    Ok(())
}
