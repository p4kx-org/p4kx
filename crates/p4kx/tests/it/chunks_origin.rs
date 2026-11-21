// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::{MODEL_ENTRY, TestResult, crch_model, ivo_model, p4kx};

#[test]
fn encrypted_ivo_fields_refer_to_their_table_words_and_size_inputs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL_ENTRY, ivo_model()).encrypted())
        .write_to(&archive)?;
    let key = test_key().to_hex();
    let output = p4kx()?
        .args(["--key", &key, "--json", "chunks", "list"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let fields = report
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    assert_eq!(fields["/file"]["containsEncryptedSource"], false);
    assert_eq!(fields["/container"]["containsEncryptedSource"], true);
    let row = report["chunks"][2].as_object().ok_or("chunk row")?;
    for field in row.keys() {
        let path = format!("/chunks/2/{field}");
        let value = fields.get(&path).ok_or_else(|| format!("missing {path}"))?;
        assert_eq!(value["containsEncryptedSource"], true, "{path}");
        assert_eq!(value["sourceSetComplete"], true, "{path}");
    }
    let kind = source_location(&report, "/chunks/2/type_id", "type")?;
    assert_eq!(kind["indices"], json!(["2", "48", "52"]));
    let current = source_location(&report, "/chunks/2/size", "offset")?;
    assert_eq!(current["indices"], json!(["2", "56", "64"]));
    let offsets = source_location(&report, "/chunks/2/size", "offsetColumn")?;
    assert_eq!(offsets["indices"], json!(["16", "80"]));
    source_location(&report, "/chunks/2/size", "decodedExtent")?;
    Ok(())
}

#[test]
fn standalone_crch_reports_declared_size_and_unknown_p4k_lineage() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("model.cgf");
    fs::write(&file, crch_model())?;
    let output = p4kx()?
        .args(["--json", "chunks", "list"])
        .arg(&file)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["chunks"][1]["byte_order"], "big");
    assert_eq!(report["chunks"][1]["size"], 7);
    let fields = report
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    let size = fields.get("/chunks/1/size").ok_or("size origin")?;
    assert_eq!(size["containsEncryptedSource"], Value::Null);
    assert_eq!(size["sourceSetComplete"], true);
    let stored = source_location(&report, "/chunks/1/size", "size")?;
    assert_eq!(stored["indices"], json!(["1", "40", "44"]));
    let order = source_location(&report, "/chunks/1/byte_order", "version")?;
    assert_eq!(order["indices"], json!(["1", "34", "36"]));
    Ok(())
}

fn source_location<'a>(
    report: &'a Value,
    path: &str,
    field: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let references = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .and_then(|origin| origin.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("origin references")?;
    report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source use inventory")?
        .iter()
        .filter(|usage| usage.get("id").is_some_and(|id| references.contains(id)))
        .filter_map(|usage| usage.get("location"))
        .find(|location| location.get("field").and_then(Value::as_str) == Some(field))
        .ok_or_else(|| format!("missing source location {field} for {path}").into())
}
