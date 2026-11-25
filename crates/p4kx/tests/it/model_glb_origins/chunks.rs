// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, test_key};
use serde_json::{Value, json};

use super::{BASE, MODEL, TestResult};

const MODEL_UNKNOWN_TYPE: u32 = 0x00C0_FFEE;
const GEOMETRY_UNKNOWN_TYPE: u32 = 0x00C0_FFEF;
const CHUNK_FIELDS: [&str; 5] = ["type_id", "type", "index", "file", "usage"];
const IVO_TABLE_OFFSET: Range<usize> = 12..16;
const IVO_TABLE_ROW_BYTES: usize = 16;

#[test]
fn unknown_chunks_in_glb_report_keep_independent_original_words() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("fixture.p4k");
    let (model, geometry) = model_bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL, model.clone()).encrypted())
        .entry(TestEntry::zstd(BASE, geometry.clone()))
        .write_to(&archive)?;
    let report = report(&archive, Some(MODEL), &directory.path().join("cube.glb"))?;
    assert_emitted_chunks(&report);
    assert_field_origins(&report, 0, Some(true))?;
    assert_field_origins(&report, 1, Some(false))?;
    assert_type_word(&report, 0, &model, MODEL, 4)?;
    assert_type_word(&report, 1, &geometry, BASE, 1)?;
    Ok(())
}

#[test]
fn unknown_chunks_in_loose_glb_report_keep_explicit_unknown_origin() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("cube.cgf");
    let (model, geometry) = model_bytes();
    fs::write(&input, model)?;
    fs::write(directory.path().join("cube.cgfm"), geometry)?;
    let report = report(&input, None, &directory.path().join("cube.glb"))?;
    assert_emitted_chunks(&report);
    assert_field_origins(&report, 0, None)?;
    assert_field_origins(&report, 1, None)?;
    assert_eq!(
        report.get("provenance"),
        Some(&json!({"sources": [], "uses": []})),
        "a loose model contributes no bound Archive Entry"
    );
    Ok(())
}

fn model_bytes() -> (Vec<u8>, Vec<u8>) {
    let model = TestModel::cube()
        .with_chunk(MODEL_UNKNOWN_TYPE, [7; 12])
        .with_geometry_chunk(GEOMETRY_UNKNOWN_TYPE, [8; 12]);
    (model.cgf(), model.cgfm())
}

fn report(input: &Path, entry: Option<&str>, output: &Path) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.arg("--json");
    if entry.is_some() {
        command.args(["--key", &test_key().to_hex()]);
    }
    command.args(["model", "glb"]).arg(input);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let result = command.arg("--out").arg(output).output()?;
    assert!(result.status.success(), "standalone GLB report: {result:?}");
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn assert_emitted_chunks(report: &Value) {
    assert_eq!(
        report.get("unknown_chunks"),
        Some(&json!([
            {"file": "model", "index": 4, "type": null, "type_id": MODEL_UNKNOWN_TYPE, "usage": "unknown"},
            {"file": "geometry", "index": 1, "type": null, "type_id": GEOMETRY_UNKNOWN_TYPE, "usage": "unknown"}
        ])),
        "known chunks are filtered without changing the original order or values"
    );
}

fn assert_field_origins(report: &Value, row: usize, encrypted: Option<bool>) -> TestResult {
    for field in CHUNK_FIELDS {
        let path = format!("/unknown_chunks/{row}/{field}");
        let origin = field_origin(report, &path)?;
        let complete = encrypted.is_some() && matches!(field, "type_id" | "type" | "index");
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(complete)),
            "source completeness at {path}"
        );
        let expected = encrypted.filter(|encrypted| complete || *encrypted);
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(expected)),
            "encryption at {path}"
        );
        let references = origin
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source references")?;
        assert_eq!(
            references.is_empty(),
            encrypted.is_none(),
            "evidence at {path}"
        );
    }
    Ok(())
}

fn assert_type_word(
    report: &Value,
    row: usize,
    bytes: &[u8],
    entry: &str,
    table_index: usize,
) -> TestResult {
    let path = format!("/unknown_chunks/{row}/type_id");
    let use_ids = field_origin(report, &path)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("chunk type origins")?;
    let [id] = use_ids.as_slice() else {
        return Err("the type ID must have one original word".into());
    };
    let use_row = origin_row(report, "/provenance/uses", id)?;
    assert_eq!(use_row.get("role"), Some(&json!("payload")), "{path} role");
    let owner = use_row.get("source").ok_or("physical chunk source")?;
    let source = origin_row(report, "/provenance/sources", owner)?;
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!(entry)),
        "only the owning Entry supplies {path}"
    );
    let table_word = bytes.get(IVO_TABLE_OFFSET).ok_or("IVO table offset word")?;
    let table = usize::try_from(u32::from_le_bytes(table_word.try_into()?))?;
    let start = table + table_index * IVO_TABLE_ROW_BYTES;
    let end = start + size_of::<u32>();
    assert_eq!(
        use_row.get("location"),
        Some(&json!({
            "type": "subresource",
            "namespace": "model-byte-range",
            "indices": [start.to_string(), end.to_string()],
            "field": "typeId",
        })),
        "exact original table word for {path}"
    );
    let word = bytes.get(start..end).ok_or("type ID word")?;
    let type_id = u32::from_le_bytes(word.try_into()?);
    assert_eq!(
        report.pointer(&path),
        Some(&json!(type_id)),
        "decoded original word at {path}"
    );
    Ok(())
}

fn field_origin<'a>(report: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing field origin at {path}").into())
}

fn origin_row<'a>(report: &'a Value, path: &str, id: &Value) -> Result<&'a Value, Box<dyn Error>> {
    report
        .pointer(path)
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(id)))
        .ok_or_else(|| format!("unresolved origin {id} in {path}").into())
}
