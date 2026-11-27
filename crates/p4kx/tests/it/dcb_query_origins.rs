// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Output;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const QUERY: &str = "from Ship select label, label";
const PAYLOAD: &str = "=a,\"b\"";

#[test]
fn query_file_receipt_is_optional_and_binds_exact_json_cells() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(r"Data\Game2.dcb", database()).encrypted())
        .write_to(&archive)?;
    let out = directory.path().join("result.json");
    let companion = out.with_file_name("result.json.p4kx-origin.jsonl");
    let command = [
        "--format",
        "json",
        "--out",
        out.to_str().ok_or("result path")?,
    ];
    let raw = query(&archive, &command)?;
    let original = fs::read(&out)?;
    let raw_summary: Value = serde_json::from_slice(&raw.stdout)?;
    assert!(
        raw_summary.get("originReceipt").is_none(),
        "default summary"
    );
    assert!(!companion.exists(), "a plain write leaves only the result");

    let mut flags = command.to_vec();
    flags.push("--origin-receipt");
    let measured = query(&archive, &flags)?;
    let summary: Value = serde_json::from_slice(&measured.stdout)?;
    assert_eq!(
        summary.get("originReceipt"),
        Some(&json!("result.json.p4kx-origin.jsonl")),
        "the opt-in summary identifies its sibling receipt"
    );
    let bytes = fs::read(&out)?;
    assert_eq!(bytes, original, "the receipt does not alter query JSON");
    let rows = receipt(&fs::read(&companion)?)?;
    assert_binding(&rows, &bytes, "file")?;
    let label = field(&rows, "/address/path", "/0/label")?;
    assert_eq!(
        label.get("containsEncryptedSource"),
        Some(&json!(true)),
        "encrypted input"
    );
    assert_eq!(
        label.get("sourceSetComplete"),
        Some(&json!(true)),
        "observed scalar input"
    );
    assert_root_selector(&rows, label)?;
    assert_eq!(
        rows.iter()
            .filter(|row| row.get("type") == Some(&json!("field")))
            .count(),
        2,
        "the repeated JSON heading produces a single field address"
    );
    query(&archive, &command)?;
    assert!(
        !companion.exists(),
        "a plain replacement retires the prior binding"
    );
    Ok(())
}

#[test]
fn text_receipts_measure_transformed_csv_and_aligned_table_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let dcb = directory.path().join("standalone.dcb");
    fs::write(&dcb, database())?;
    for (format, mode, expected) in [
        ("csv", None, "\"'=a,\"\"b\"\"\""),
        ("csv", Some("--csv-raw"), "\"=a,\"\"b\"\"\""),
        ("csv-strict", None, "\"=a,\"\"b\"\"\""),
        ("table", None, PAYLOAD),
    ] {
        let mut flags = vec!["--format", format];
        flags.extend(mode);
        let raw = query(&dcb, &flags)?;
        flags.push("--origin-stderr");
        let measured = query(&dcb, &flags)?;
        assert_eq!(measured.stdout, raw.stdout, "unchanged {format} bytes");
        let rows = receipt(&measured.stderr)?;
        assert_binding(&rows, &measured.stdout, "stdout")?;
        for column in ["/0/1", "/0/2"] {
            let label = field(&rows, "/address/field", column)?;
            let start = offset(label, "/address/start")?;
            let end = offset(label, "/address/endExclusive")?;
            assert_eq!(
                measured.stdout.get(start..end),
                Some(expected.as_bytes()),
                "{format} field bytes"
            );
            assert_eq!(
                label.get("containsEncryptedSource"),
                Some(&Value::Null),
                "standalone file ancestry"
            );
            assert_root_selector(&rows, label)?;
        }
    }
    Ok(())
}

#[test]
fn stdout_companion_can_be_saved_and_printed_for_the_same_result() -> TestResult {
    let directory = tempfile::tempdir()?;
    let dcb = directory.path().join("standalone.dcb");
    let companion = directory.path().join("stdout.origin.jsonl");
    fs::write(&dcb, database())?;
    let measured = query(
        &dcb,
        &[
            "--format",
            "json",
            "--origin-out",
            companion.to_str().ok_or("companion path")?,
            "--origin-stderr",
        ],
    )?;
    assert_eq!(
        fs::read(&companion)?,
        measured.stderr,
        "both sinks serialize the same observed companion"
    );
    assert_binding(&receipt(&measured.stderr)?, &measured.stdout, "stdout")
}

fn assert_root_selector(receipt: &[Value], field: &Value) -> TestResult {
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field uses")?;
    let root = receipt
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .any(|source| {
            source.pointer("/location/namespace") == Some(&json!("dataforgeInstance"))
                && source.pointer("/location/indices") == Some(&json!(["1", "0"]))
                && source.get("id").is_some_and(|id| refs.contains(id))
        });
    assert!(root, "the cell cites the actual selected Record instance");
    Ok(())
}

fn assert_binding(rows: &[Value], bytes: &[u8], kind: &str) -> TestResult {
    let header = rows.first().ok_or("receipt header")?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(bytes)?;
    let digest = digest.finish()?;
    assert_eq!(
        header.pointer("/binding/type"),
        Some(&json!(kind)),
        "sink type"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        Some(&json!(digest.sha256())),
        "artifact SHA-256"
    );
    assert_eq!(
        header.pointer("/binding/byteLength"),
        Some(&json!(bytes.len().to_string())),
        "artifact size"
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "complete receipt"
    );
    Ok(())
}

fn field<'a>(rows: &'a [Value], key: &str, name: &str) -> TestResult<&'a Value> {
    rows.iter()
        .find(|row| row.pointer(key).and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("missing addressed field {name}").into())
}

fn offset(field: &Value, key: &str) -> TestResult<usize> {
    Ok(field
        .pointer(key)
        .and_then(Value::as_str)
        .ok_or("byte offset")?
        .parse()?)
}

fn receipt(bytes: &[u8]) -> TestResult<Vec<Value>> {
    Ok(std::str::from_utf8(bytes)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

fn query(input: &Path, flags: &[&str]) -> TestResult<Output> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "query"])
        .arg(input)
        .arg(QUERY)
        .args(flags)
        .args(if flags.contains(&"--out") {
            &["--json"][..]
        } else {
            &[]
        })
        .output()?;
    assert!(output.status.success(), "query exit: {output:?}");
    Ok(output)
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Before").field("unused", TestType::Int32))
        .structure(TestStruct::new("Ship").field("label", TestType::String))
        .record(TestRecord::new(
            1,
            "Before",
            "Earlier",
            "fixture/earlier.xml",
        ))
        .record(
            TestRecord::new(2, "Ship", "Selected", "fixture/ship.xml")
                .value("label", TestValue::text(PAYLOAD)),
        )
        .build()
}
