// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use serde_json::{Value, json};

use super::{TestResult, export, write_archive};

const PAYLOAD: &str = "records.ndjson";
const COMPANION: &str = "records.ndjson.p4kx-origin.jsonl";

fn json_lines(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

#[test]
fn ndjson_companion_addresses_only_fields_of_the_committed_physical_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, &["--format", "ndjson"], 0)?;

    let bytes = fs::read(out.join(PAYLOAD))?;
    let mut measured = OriginDigestWriter::new(std::io::sink());
    measured.write_all(&bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let output = json_lines(&out.join(PAYLOAD))?;
    let proof = json_lines(&out.join(COMPANION))?;
    let header = proof.first().ok_or("receipt header")?;
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "the binding measures the complete output"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the digest authenticates the exact output"
    );
    assert_actual_fields(&output, &proof)?;
    let wheel = proof
        .iter()
        .find(|row| {
            row.pointer("/address/line").and_then(Value::as_str) == Some("2")
                && row.pointer("/address/path").and_then(Value::as_str) == Some("/wheels/1/size")
        })
        .ok_or("selected wheel size")?;
    assert_eq!(
        output.get(2).and_then(|row| row.pointer("/wheels/1/size")),
        Some(&json!(17)),
        "the last physical line holds the selected wheel"
    );
    assert_eq!(
        wheel.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the original DataForge Entry is encrypted"
    );
    assert_eq!(
        wheel.get("sourceSetComplete"),
        Some(&json!(true)),
        "the original wheel evidence is complete"
    );
    assert_eq!(
        proof.last().and_then(|row| row.get("type")),
        Some(&json!("end")),
        "the companion is complete"
    );
    Ok(())
}

fn assert_actual_fields(output: &[Value], proof: &[Value]) -> TestResult {
    let mut counts = vec![0_u32; output.len()];
    for row in proof
        .iter()
        .filter(|row| row.get("type").and_then(Value::as_str) == Some("field"))
    {
        assert_eq!(
            row.pointer("/address/type").and_then(Value::as_str),
            Some("ndjsonField"),
            "each field uses the physical line address"
        );
        let index = row
            .pointer("/address/line")
            .and_then(Value::as_str)
            .ok_or("physical line")?
            .parse::<usize>()?;
        let pointer = row
            .pointer("/address/path")
            .and_then(Value::as_str)
            .ok_or("relative pointer")?;
        let value = output.get(index).ok_or("output line")?;
        assert!(
            value.pointer(pointer).is_some(),
            "{index}:{pointer} addresses actual data"
        );
        let count = counts.get_mut(index).ok_or("line count")?;
        *count += 1;
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "each physical line has its own observations"
    );
    Ok(())
}

#[test]
fn ndjson_keep_preserves_its_proof_and_overwrite_republishes_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    export(&archive, &out, &["--format", "ndjson"], 0)?;
    let bytes = fs::read(out.join(PAYLOAD))?;
    let companion = out.join(COMPANION);
    let proof = fs::read(&companion)?;
    for policy in ["skip", "if-changed"] {
        export(
            &archive,
            &out,
            &["--format", "ndjson", "--conflict", policy],
            0,
        )?;
        assert_eq!(
            fs::read(&companion)?,
            proof,
            "{policy} retains source proof"
        );
    }
    export(
        &archive,
        &out,
        &["--format", "ndjson", "--tags", "--conflict", "overwrite"],
        0,
    )?;
    assert_eq!(
        fs::read(out.join(PAYLOAD))?,
        bytes,
        "the test archive adds no tags"
    );
    assert!(
        companion.exists(),
        "the equal-byte replacement publishes its own proof"
    );
    Ok(())
}

#[test]
fn ndjson_reports_a_foreign_companion_after_committing_the_payload() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    let companion = out.join(COMPANION);
    fs::write(&companion, b"foreign")?;
    let stdout = export(&archive, &out, &["--format", "ndjson", "--json"], 1)?;
    let report: Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        report.get("exported"),
        Some(&json!(3)),
        "all Records committed"
    );
    assert!(
        report.get("origin_receipts").is_none(),
        "foreign bytes are not current proof"
    );
    assert_eq!(
        report
            .get("origin_failures")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "the report includes the single companion failure"
    );
    assert_eq!(
        json_lines(&out.join(PAYLOAD))?.len(),
        3,
        "every committed line parses"
    );
    assert_eq!(
        fs::read(&companion)?,
        b"foreign",
        "the foreign companion stays unchanged"
    );
    assert_eq!(
        fs::read_dir(&out)?.count(),
        2,
        "owned temporary and lock were retired"
    );
    Ok(())
}
