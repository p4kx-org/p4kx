// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{TestChf, p4kx};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn differing_text_keeps_original_bytes_and_binds_the_exact_emitted_values() -> TestResult {
    let dir = tempfile::tempdir()?;
    let before = dir.path().join("before.chf");
    let after = dir.path().join("after.chf");
    let durable = dir.path().join("stdout-origin.jsonl");
    let mut old = TestChf::v7();
    let mut new = TestChf::v7();
    old.materials.first_mut().ok_or("old material")?.flags = 1;
    new.materials.first_mut().ok_or("new material")?.flags = 2;
    old.materials.pop();
    fs::write(&before, old.to_bytes())?;
    fs::write(&after, new.to_bytes())?;
    let legacy = p4kx()?
        .args(["chf", "diff"])
        .arg(&before)
        .arg(&after)
        .output()?;
    let run = p4kx()?
        .args(["chf", "diff"])
        .arg(&before)
        .arg(&after)
        .arg("--origin-stderr")
        .arg("--origin-out")
        .arg(&durable)
        .output()?;
    assert_eq!(legacy.status.code(), Some(1), "{legacy:?}");
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert_eq!(
        run.stdout, legacy.stdout,
        "the normal textual diff stays byte-exact"
    );
    assert_eq!(
        fs::read(durable)?,
        run.stderr,
        "both explicit channels carry the same complete receipt"
    );
    let rows = rows(&run.stderr)?;
    check_stdout_binding(&rows, &run.stdout)?;
    for (field, value) in [
        ("/changes/0/old", b"0x00000001".as_slice()),
        ("/changes/0/new", b"0x00000002".as_slice()),
        ("/changes/1/status", b"added".as_slice()),
    ] {
        let datum = rows
            .iter()
            .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(field))
            .ok_or_else(|| format!("missing text origin field {field}"))?;
        assert_eq!(datum["containsEncryptedSource"], Value::Null, "{field}");
        assert_eq!(datum["address"]["type"], "textBytes", "{field}");
        assert_eq!(
            range(datum, &run.stdout),
            Some(value),
            "{field} uses exact emitted bytes"
        );
    }
    Ok(())
}

#[test]
fn identical_text_gets_a_single_comparison_datum_and_success_status() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("character.chf");
    fs::write(&input, TestChf::v7().to_bytes())?;
    let run = p4kx()?
        .args(["chf", "diff"])
        .arg(&input)
        .arg(&input)
        .arg("--origin-stderr")
        .output()?;
    assert_eq!(run.status.code(), Some(0), "{run:?}");
    assert_eq!(run.stdout, b"no differences\n");
    let rows = rows(&run.stderr)?;
    check_stdout_binding(&rows, &run.stdout)?;
    let datum = rows
        .iter()
        .find(|row| row.get("type").and_then(Value::as_str) == Some("field"))
        .ok_or("comparison datum")?;
    assert_eq!(datum["address"]["field"], "/identical");
    assert_eq!(
        range(datum, &run.stdout),
        Some(b"no differences".as_slice())
    );
    assert_eq!(rows.last().ok_or("receipt footer")?["fields"], "1");
    Ok(())
}

fn rows(bytes: &[u8]) -> Result<Vec<Value>, serde_json::Error> {
    bytes
        .split(|&byte| byte == b'\n')
        .filter(|row| !row.is_empty())
        .map(serde_json::from_slice)
        .collect()
}

fn check_stdout_binding(rows: &[Value], bytes: &[u8]) -> TestResult {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let measurement = serde_json::to_value(measured.finish()?)?;
    let hash = measurement.get("sha256").ok_or("measured stdout digest")?;
    let length = measurement
        .get("byteLength")
        .ok_or("measured stdout length")?;
    assert_eq!(
        rows.first().and_then(|row| row.get("binding")),
        Some(&json!({"type":"stdout", "sha256":hash, "byteLength":length})),
        "receipt binds the actual raw stream",
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "complete stream"
    );
    Ok(())
}

fn range<'bytes>(datum: &Value, bytes: &'bytes [u8]) -> Option<&'bytes [u8]> {
    let address = datum.get("address")?;
    let start = address.get("start")?.as_str()?.parse::<usize>().ok()?;
    let end = address
        .get("endExclusive")?
        .as_str()?
        .parse::<usize>()
        .ok()?;
    bytes.get(start..end)
}

#[test]
fn text_origin_options_require_text_comparison_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let before = dir.path().join("before.chf");
    let after = dir.path().join("after.chf");
    let run = p4kx()?
        .args(["--json", "chf", "diff"])
        .arg(&before)
        .arg(&after)
        .arg("--origin-stderr")
        .output()?;
    assert_eq!(run.status.code(), Some(2), "{run:?}");
    assert_eq!(run.stdout, b"");
    assert!(String::from_utf8_lossy(&run.stderr).contains("require text comparison output"));
    Ok(())
}
