// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value, json};

use super::Case;
use crate::commodity_v3_fixture::TestResult;

const UNREADABLE_NAME: &str = "/names/value/3";

pub(super) fn verify(
    cell: &Map<String, Value>,
    header_marker: Option<bool>,
    case: Case,
    path: &str,
) -> TestResult {
    let unreadable_name = matches!(case, Case::NameSlots) && path == UNREADABLE_NAME;
    let marker = if unreadable_name {
        if cell.get("state").and_then(Value::as_str) != Some("unreadable")
            || cell.get("sourceSetComplete").and_then(Value::as_bool) != Some(false)
        {
            return Err(format!("case={case:?} pointer={path}: original unreadable name requires its exact incomplete source state").into());
        }
        header_marker.filter(|encrypted| *encrypted)
    } else {
        header_marker
    };
    if cell.get("containsEncryptedSource") != Some(&json!(marker)) {
        return Err(format!(
            "case={case:?} pointer={path} state={:?} complete={:?} refs={:?}: expected marker {marker:?}, found {:?}",
            cell.get("state"),
            cell.get("sourceSetComplete"),
            cell.get("sourceUsesRefs").and_then(Value::as_array).map(Vec::len),
            cell.get("containsEncryptedSource"),
        ).into());
    }
    Ok(())
}

#[test]
fn unreadable_plain_name_rejects_false_marker_or_complete_source_set() -> TestResult {
    let mut cell = unreadable_cell(None)?;
    verify(&cell, Some(false), Case::NameSlots, UNREADABLE_NAME)?;
    cell.insert("containsEncryptedSource".to_owned(), json!(false));
    assert!(
        verify(&cell, Some(false), Case::NameSlots, UNREADABLE_NAME).is_err(),
        "an incomplete plain datum cannot claim a wholly plain source set"
    );
    cell.insert("containsEncryptedSource".to_owned(), Value::Null);
    cell.insert("sourceSetComplete".to_owned(), json!(true));
    assert!(
        verify(&cell, Some(false), Case::NameSlots, UNREADABLE_NAME).is_err(),
        "the malformed source slot cannot become complete"
    );
    Ok(())
}

#[test]
fn unreadable_name_retains_known_encryption_and_unknown_source_states() -> TestResult {
    let mut cell = unreadable_cell(Some(true))?;
    verify(&cell, Some(true), Case::NameSlots, UNREADABLE_NAME)?;
    cell.insert("containsEncryptedSource".to_owned(), Value::Null);
    assert!(
        verify(&cell, Some(true), Case::NameSlots, UNREADABLE_NAME).is_err(),
        "the actual encrypted owner selector remains a known contributor"
    );
    verify(&cell, None, Case::NameSlots, UNREADABLE_NAME)?;
    cell.insert("containsEncryptedSource".to_owned(), json!(false));
    assert!(
        verify(&cell, None, Case::NameSlots, UNREADABLE_NAME).is_err(),
        "unknown source metadata cannot become plain"
    );
    Ok(())
}

#[test]
fn unreadable_name_exception_cannot_hide_unrelated_unknown_markers() -> TestResult {
    let mut cell = unreadable_cell(None)?;
    assert!(
        verify(&cell, Some(false), Case::NameSlots, "/guid").is_err(),
        "the exception does not affect unrelated header cells"
    );
    assert!(
        verify(&cell, Some(false), Case::Present, UNREADABLE_NAME).is_err(),
        "the exception does not affect another original source form"
    );
    cell.insert("state".to_owned(), json!("present"));
    cell.insert("sourceSetComplete".to_owned(), json!(true));
    cell.insert("containsEncryptedSource".to_owned(), json!(false));
    verify(&cell, Some(false), Case::NameSlots, "/guid")?;
    Ok(())
}

fn unreadable_cell(marker: Option<bool>) -> TestResult<Map<String, Value>> {
    json!({
        "state": "unreadable",
        "value": null,
        "sourceSetComplete": false,
        "containsEncryptedSource": marker,
        "sourceUsesRefs": [],
    })
    .as_object()
    .cloned()
    .ok_or_else(|| "synthetic immediate cell".into())
}
