// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Output;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{TestResult, archive, p4kx};

fn fixture(directory: &Path) -> Result<PathBuf, Box<dyn Error>> {
    archive(
        directory,
        vec![
            TestEntry::stored(
                "Data/Localization/english/global.ini",
                b"+same=English\nonly=Plain\nmissing=Present\n".to_vec(),
            ),
            TestEntry::stored(
                "Data/Localization/german_(germany)/global.ini",
                b"+SAME==Formula\\nNext\nmissing=\nzOnly=Encrypted\nbroken line\n".to_vec(),
            )
            .encrypted(),
        ],
    )
}

fn export(input: &Path, args: &[&str]) -> Result<Output, Box<dyn Error>> {
    let output = p4kx()?
        .args(["--key", &p4kx_testkit::test_key().to_hex(), "loc", "export"])
        .arg(input)
        .args(args)
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(output)
}

fn parse_receipt(receipt: &[u8], artifact: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let rows: Vec<Value> = std::str::from_utf8(receipt)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("receipt header")?;
    let end = rows.last().ok_or("receipt end")?;
    assert_eq!(
        header.get("format"),
        Some(&json!("p4kx-source-origin")),
        "wire format"
    );
    assert_eq!(end.get("type"), Some(&json!("end")), "mandatory end record");
    let mut digest = OriginDigestWriter::new(Vec::new());
    digest.write_all(artifact)?;
    let digest = serde_json::to_value(digest.finish()?)?;
    let binding = header.get("binding").ok_or("measured binding")?;
    assert_eq!(binding.get("sha256"), digest.get("sha256"), "exact digest");
    assert_eq!(
        binding.get("byteLength"),
        digest.get("byteLength"),
        "exact byte count"
    );
    for (kind, count) in [
        ("source", "sources"),
        ("sourceUse", "sourceUses"),
        ("field", "fields"),
    ] {
        let measured = rows
            .iter()
            .filter(|row| row_type(row) == Some(kind))
            .count();
        assert_eq!(
            end.get(count),
            Some(&json!(measured.to_string())),
            "{kind} count"
        );
    }
    assert_resolved_uses(&rows)?;
    Ok(rows)
}

fn assert_resolved_uses(rows: &[Value]) -> TestResult {
    let mut uses = BTreeSet::new();
    for row in rows {
        if row_type(row) == Some("sourceUse") {
            let id = row
                .pointer("/sourceUse/id")
                .and_then(Value::as_str)
                .ok_or("use ID")?;
            assert!(uses.insert(id), "unique source-use ID {id}");
        }
        if row_type(row) == Some("field") {
            let refs = row
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .ok_or("field refs")?;
            for value in refs {
                let id = value.as_str().ok_or("field use ID")?;
                assert!(
                    uses.contains(id),
                    "field references prior registered use {id}"
                );
            }
        }
    }
    Ok(())
}

fn row_type(row: &Value) -> Option<&str> {
    row.get("type").and_then(Value::as_str)
}

fn field<'rows>(rows: &'rows [Value], address: &str) -> Result<&'rows Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| {
            row_type(row) == Some("field")
                && ["/address/path", "/address/field"]
                    .into_iter()
                    .any(|pointer| row.pointer(pointer).and_then(Value::as_str) == Some(address))
        })
        .ok_or_else(|| format!("missing field {address}").into())
}

#[test]
fn table_json_keeps_mixed_origins_and_exact_empty_and_missing_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = fixture(directory.path())?;
    let artifact = directory.path().join("loc.json");
    let file = artifact.to_str().ok_or("artifact path")?;
    let summary = export(&input, &["--out", file])?;
    assert!(std::str::from_utf8(&summary.stdout)?.contains("4 keys in 2 languages"));
    let output = fs::read(&artifact)?;
    assert_eq!(output, export(&input, &[])?.stdout);
    let receipt = fs::read(directory.path().join("loc.json.p4kx-origin.jsonl"))?;
    let rows = parse_receipt(&receipt, &output)?;
    let document: Value = serde_json::from_slice(&output)?;
    assert_eq!(document["entries"][0]["key"], "+same");
    assert_eq!(document["entries"][0]["texts"]["de"], "=Formula\nNext");
    assert_eq!(document["entries"][1]["texts"]["de"], "");
    assert!(document["entries"][2]["texts"]["de"].is_null());
    for (path, encrypted) in [
        ("/entries/0/key", false),
        ("/entries/0/texts/en", false),
        ("/entries/0/texts/de", true),
        ("/entries/1/texts/de", true),
        ("/entries/2/texts/de", true),
        ("/entries/3/key", true),
        ("/entries/3/texts/en", true),
    ] {
        let origin = field(&rows, path)?;
        assert_eq!(origin["containsEncryptedSource"], encrypted, "{path}");
        assert_eq!(origin["sourceSetComplete"], true, "{path}");
    }
    assert_eq!(rows.last().ok_or("end")?["fields"], "14");
    Ok(())
}

#[test]
fn table_csv_keeps_exact_spans_and_raw_stdout_with_both_receipts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = fixture(directory.path())?;
    let receipt = directory.path().join("stdout.jsonl");
    let args = ["--format", "csv", "--lang", "de,en"];
    let plain = export(&input, &args)?;
    let observed = export(
        &input,
        &[
            args[0],
            args[1],
            args[2],
            args[3],
            "--origin-out",
            receipt.to_str().ok_or("receipt path")?,
            "--origin-stderr",
        ],
    )?;
    assert_eq!(plain.stdout, observed.stdout);
    let companion = fs::read(receipt)?;
    assert!(observed.stderr.starts_with(b"note: "));
    assert!(observed.stderr.ends_with(&companion));
    let rows = parse_receipt(&companion, &observed.stdout)?;
    for (path, bytes) in [
        ("/key", "key"),
        ("/languages/0", "de"),
        ("/entries/0/key", "'+same"),
        ("/entries/0/texts/de", "\"'=Formula\nNext\""),
        ("/entries/1/texts/de", ""),
        ("/entries/2/texts/de", ""),
    ] {
        let address = &field(&rows, path)?["address"];
        let start: usize = address["start"].as_str().ok_or("start")?.parse()?;
        let end: usize = address["endExclusive"].as_str().ok_or("end")?.parse()?;
        assert_eq!(
            observed.stdout.get(start..end),
            Some(bytes.as_bytes()),
            "{path}"
        );
    }
    Ok(())
}

#[test]
fn table_summary_and_file_receipt_publish_with_exact_source_independence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = fixture(directory.path())?;
    let output = directory.path().join("table.json");
    let receipt = directory.path().join("table.json.p4kx-origin.jsonl");
    let run = |archive: &Path| -> Result<Output, Box<dyn Error>> {
        Ok(p4kx()?
            .args([
                "--json",
                "--key",
                &p4kx_testkit::test_key().to_hex(),
                "loc",
                "export",
            ])
            .arg(archive)
            .args(["--out", output.to_str().ok_or("output")?])
            .output()?)
    };
    let summary = run(&input)?;
    assert!(summary.status.success(), "{summary:?}");
    let summary: Value = serde_json::from_slice(&summary.stdout)?;
    assert_eq!(summary["originReceipt"], "table.json.p4kx-origin.jsonl");
    assert_eq!(summary["languages"], json!(["en", "de"]));
    assert_eq!(
        summary["fieldOrigins"]["/keys"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        summary["fieldOrigins"]["/languages/1"]["containsEncryptedSource"],
        false
    );
    let original = (fs::read(&output)?, fs::read(&receipt)?);
    let invalid = directory.path().join("invalid");
    fs::create_dir(&invalid)?;
    let invalid = archive(&invalid, vec![TestEntry::stored("Data/no-locale", vec![0])])?;
    assert!(!run(&invalid)?.status.success());
    assert_eq!((fs::read(output)?, fs::read(receipt)?), original);
    Ok(())
}
