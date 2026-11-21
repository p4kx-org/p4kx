// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Index, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn media_cli_exports_source_layout_with_an_explicit_shared_work_budget() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("a", b"a"))
        .write_to(&source)?;
    let output = folder.path().join("media");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args([
            "--kind",
            "archive",
            "--max-metadata-work",
            "1000001",
            "--json",
        ])
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert_eq!(index.limits.metadata_work, 1_000_001);
    assert_eq!(
        index.source_entries.written, 1,
        "source-directory records are not fake physical Entries"
    );
    let rows: Vec<MediaRecord> =
        fs::read_to_string(output.join(&index.artifact_base).join("archive.jsonl"))?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
    assert!(
        rows.iter()
            .any(|row| row.subresource.namespace == "archive-layout")
    );
    assert!(
        rows.iter()
            .any(|row| row.subresource.namespace == "archive-raw-chunk")
    );
    let schema = p4kx_testkit::p4kx()?
        .args(["media", "schema", "--record"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let validator =
        jsonschema::validator_for(&serde_json::from_slice::<serde_json::Value>(&schema)?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[test]
fn media_cli_rejects_zero_work_and_preserves_the_current_index_when_work_is_exhausted() -> TestResult
{
    p4kx_testkit::p4kx()?
        .args([
            "media",
            "export",
            "unused.p4k",
            "--out",
            "unused",
            "--max-metadata-work",
            "0",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--max-metadata-work"));
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("a", b"a"))
        .write_to(&source)?;
    let output = folder.path().join("media");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "archive"])
        .assert()
        .code(0);
    let current = fs::read(output.join("media-index.json"))?;
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&source)
        .arg("--out")
        .arg(&output)
        .args([
            "--kind",
            "archive",
            "--conflict",
            "overwrite",
            "--max-metadata-work",
            "1",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("metadata_work"));
    assert_eq!(fs::read(output.join("media-index.json"))?, current);
    Ok(())
}
