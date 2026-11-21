// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Receipt, TestResult, catalog, compact_fixture, index, path, receipt, verify};

#[test]
fn kind_health_rejects_missing_duplicate_extra_and_inconsistent_rows() -> TestResult {
    for (pointer, replacement, expected) in [
        ("/kinds", None, "missing catalog kind health"),
        (
            "/kinds/0/kind",
            Some(json!(Kind::Items)),
            "catalog kind health expected",
        ),
        (
            "/kinds/0/kind",
            Some(json!("unregistered")),
            "catalog kind health expected",
        ),
        (
            "/kinds/0/sourceRecords/selected",
            Some(json!(1)),
            "sourceRecords does not reconcile",
        ),
        (
            "/kinds/0/sourceEntries/selected",
            Some(json!(1)),
            "sourceEntries does not reconcile",
        ),
        (
            "/selectedKindRecords",
            Some(json!(2)),
            "selectedKindRecords total does not reconcile",
        ),
        (
            "/writtenRows",
            Some(json!(3)),
            "writtenRows total does not reconcile",
        ),
    ] {
        let mut document = catalog();
        let health = path(&mut document, "/exportHealth")?;
        match replacement {
            Some(value) => *path(health, pointer)? = value,
            None => {
                health
                    .as_object_mut()
                    .ok_or("health object")?
                    .remove(pointer.trim_start_matches('/'))
                    .ok_or("health field to remove")?;
            }
        }
        assert_replay_error(&document, expected)?;
    }
    let mut document = catalog();
    let kinds = path(&mut document, "/exportHealth/kinds")?
        .as_array_mut()
        .ok_or("kind health array")?;
    kinds.pop().ok_or("last Kind")?;
    assert_replay_error(&document, "does not contain exactly the registered Kinds")?;
    Ok(())
}

#[test]
fn complete_selection_refuses_unknown_and_duplicate_kind_keys() -> TestResult {
    let mut document = catalog();
    path(&mut document, "/completeness")?
        .as_object_mut()
        .ok_or("completeness map")?
        .insert("unregistered".to_owned(), json!({"status": "complete"}));
    assert_replay_error(&document, "unexpected Catalog Kind selection")?;

    let raw = serde_json::to_string(&catalog())?;
    let duplicate = raw.replacen(
        r#""completeness":{"#,
        r#""completeness":{"items":{"status":"complete"},"#,
        1,
    );
    let Err(error) = index(duplicate.as_bytes()) else {
        return Err("duplicate completeness key was accepted".into());
    };
    assert!(
        error
            .to_string()
            .contains("duplicate Catalog Kind selection")
    );
    Ok(())
}

fn assert_replay_error(document: &Value, expected: &str) -> TestResult {
    let raw = compact_fixture(document)?;
    let input: Receipt = serde_json::from_value(receipt(&raw, &raw))?;
    let binding = input.binding()?;
    let indexed = index(raw.as_slice())?;
    let error = verify(raw.as_slice(), &indexed, &binding).expect_err("invalid replay accounting");
    assert!(error.to_string().contains(expected), "{error}");
    Ok(())
}
