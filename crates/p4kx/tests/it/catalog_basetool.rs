// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FORMAT: &str = "p4kx-basetool-catalog";
const PRECEDING: &[u8] = b"previous catalog generation";

#[test]
fn basetool_writes_a_deterministic_v1_frame_and_one_json_summary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let first = dir.path().join("first.json.zst");
    let second = dir.path().join("second.json.ZST");
    write_dataforge_archive(&archive)?;
    fs::write(&first, PRECEDING)?;

    let explicit = command(&archive, &first)?
        .env("SOURCE_DATE_EPOCH", "1")
        .args(["--exported-at", "0"])
        .output()?;
    let environment = command(&archive, &second)?
        .env("SOURCE_DATE_EPOCH", "0")
        .args(["--provenance", "markers-only"])
        .output()?;

    assert_eq!(
        explicit.status.code(),
        Some(0),
        "explicit export time: {explicit:?}"
    );
    assert_eq!(
        environment.status.code(),
        Some(0),
        "environment export time: {environment:?}"
    );
    let first_frame = fs::read(&first)?;
    let second_frame = fs::read(&second)?;
    assert_eq!(first_frame, second_frame, "identical fixed-time artifacts");
    let plain = check_frame(&first_frame)?;
    let root: Value = serde_json::from_slice(&plain)?;
    assert_eq!(root.get("format"), Some(&json!(FORMAT)));
    assert_eq!(root.get("formatVersion"), Some(&json!(1)));
    assert_eq!(root.pointer("/catalog/schemaVersion"), Some(&json!(3)));
    assert_eq!(
        root.pointer("/catalog/source/exportedAt"),
        Some(&json!("1970-01-01T00:00:00Z"))
    );
    assert_eq!(
        root.pointer("/catalog/exportHealth/status"),
        Some(&json!("complete"))
    );
    let schema = schema()?;
    jsonschema::validator_for(&schema)?
        .validate(&root)
        .map_err(|error| error.to_string())?;
    check_summary(&explicit, &archive, &first, plain.len(), first_frame.len())?;
    check_summary(
        &environment,
        &archive,
        &second,
        plain.len(),
        second_frame.len(),
    )
}

#[test]
fn basetool_rejects_plain_outputs_and_full_provenance_before_archive_open() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("missing.p4k");
    for (name, flags, diagnostic) in [
        (
            "catalog.json",
            &[][..],
            "requires an output ending in .json.zst",
        ),
        (
            "catalog.json.zst",
            &["--provenance", "full"][..],
            "fixed markers-only provenance profile",
        ),
    ] {
        let out = dir.path().join(name);
        fs::write(&out, PRECEDING)?;
        let output = command(&archive, &out)?.args(flags).output()?;
        assert_refusal(output, 2, diagnostic, &out)?;
    }
    Ok(())
}

#[test]
fn basetool_keeps_preceding_output_when_the_archive_lacks_a_dataforge() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/fixture.txt", b"valid Archive"))
        .write_to(&archive)?;
    let out = dir.path().join("catalog.json.zst");
    fs::write(&out, PRECEDING)?;
    let output = command(&archive, &out)?.output()?;
    assert_refusal(output, 5, "DataForge", &out)
}

fn schema() -> TestResult<Value> {
    let output = p4kx_testkit::p4kx()?
        .args(["catalog", "schema", "--profile", "basetool", "--json"])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "schema: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn command(archive: &Path, out: &Path) -> TestResult<Command> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(["catalog", "basetool"])
        .arg(archive)
        .arg("--out")
        .arg(out)
        .arg("--json")
        .env_remove("SOURCE_DATE_EPOCH");
    Ok(command)
}

fn write_dataforge_archive(archive: &Path) -> TestResult {
    let dcb = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("UnusedFixtureRecord"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dcb))
        .write_to(archive)?;
    Ok(())
}

fn check_frame(frame: &[u8]) -> TestResult<Vec<u8>> {
    let frame_size =
        zstd::zstd_safe::find_frame_compressed_size(frame).map_err(|_| "invalid zstd frame")?;
    assert_eq!(frame_size, frame.len(), "one frame without trailing bytes");
    let decoded = zstd::decode_all(frame)?;
    assert_eq!(decoded.last(), Some(&b'\n'), "JSON line terminator");
    let content_size =
        zstd::zstd_safe::get_frame_content_size(frame).map_err(|_| "invalid zstd frame header")?;
    assert_eq!(
        content_size,
        Some(u64::try_from(decoded.len())?),
        "content size includes the full reduced JSON"
    );
    Ok(decoded)
}

fn check_summary(
    output: &Output,
    archive: &Path,
    out: &Path,
    size: usize,
    compressed_size: usize,
) -> TestResult {
    let stdout = std::str::from_utf8(&output.stdout)?;
    assert!(stdout.ends_with('\n'), "the summary is newline terminated");
    assert_eq!(
        stdout.lines().count(),
        1,
        "stdout contains exactly one JSON line"
    );
    let summary: Value = serde_json::from_str(stdout)?;
    assert_eq!(
        summary,
        json!({
            "archive": archive.display().to_string(),
            "out": out.display().to_string(),
            "format": FORMAT,
            "format_version": 1,
            "size": size,
            "compressed_size": compressed_size,
        }),
        "the summary identifies the published artifact and reports both measured lengths"
    );
    Ok(())
}

fn assert_refusal(output: Output, expected: i32, diagnostic: &str, out: &Path) -> TestResult {
    assert_eq!(output.status.code(), Some(expected), "refusal: {output:?}");
    assert!(
        output.stdout.is_empty(),
        "no summary before successful commit"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains(diagnostic), "diagnostic: {stderr}");
    assert_eq!(
        fs::read(out)?,
        PRECEDING,
        "the existing file remains intact"
    );
    Ok(())
}
