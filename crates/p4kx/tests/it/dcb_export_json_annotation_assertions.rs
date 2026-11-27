// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{TestResult, observed_uses};

pub(super) fn assert_receipt(records: &[Value], byte_length: usize) -> TestResult {
    assert_eq!(
        records
            .first()
            .and_then(|row| row.pointer("/binding/byteLength")),
        Some(&serde_json::json!(byte_length.to_string())),
        "receipt binds the exported bytes"
    );
    let tag = observed_uses(records, "/mount/tag", true)?;
    for (field, owner) in [("children", "1"), ("tags", "0")] {
        assert!(
            tag.iter().any(
                |used| used.get("role").and_then(Value::as_str) == Some("selector")
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("dataforgeTagRelations")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && used.pointer("/location/indices") == Some(&serde_json::json!([owner, "1"]))
            ),
            "the selected original {field} occurrence retains its physical slot"
        );
    }
    let text = observed_uses(records, "/label/text", true)?;
    assert!(
        text.iter().any(
            |used| used.pointer("/location/type").and_then(Value::as_str)
                == Some("localizationLookup")
        ),
        "the label retains the actual lookup"
    );
    assert!(
        text.iter().any(
            |used| used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("globalIniBytes")
        ),
        "the label retains the source INI byte range"
    );
    assert_eq!(
        records
            .last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "the receipt has its completion row"
    );
    Ok(())
}
