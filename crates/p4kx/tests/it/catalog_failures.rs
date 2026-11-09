// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selected source failures refuse normal publication and require explicit diagnostic output.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const BROKEN_PATH: &str = "libs/foundry/records/crafting/blueprints/crafting/broken";
const BROKEN_GUID: &str = "90000000-0000-4000-8000-000000000009";

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(
            TestRecord::new(
                0x1000_0000_0000_4000_8000_0000_0000_0001,
                "SCItemManufacturer",
                "SCItemManufacturer.ACAS",
                "libs/foundry/records/scitemmanufacturer/acas.xml",
            )
            .value("Code", TestValue::text("ACAS")),
        )
        .record(TestRecord::new(
            0x9000_0000_0000_4000_8000_0000_0000_0009,
            "CraftingBlueprintRecord",
            "CraftingBlueprintRecord.broken",
            format!("{BROKEN_PATH}.xml"),
        ));
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn catalog(
    args: &[&str],
) -> Result<(std::process::Output, PathBuf, tempfile::TempDir), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("catalog.json");
    let output = run(&archive, &out, args)?;
    Ok((output, out, dir))
}

fn run(archive: &Path, out: &Path, args: &[&str]) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?
        .arg("catalog")
        .arg(archive)
        .arg("--out")
        .arg(out)
        .args(args)
        .output()?)
}

#[test]
fn an_unreadable_record_refuses_normal_publication_before_creating_a_file() -> TestResult {
    let (output, out, _dir) = catalog(&[])?;

    assert_eq!(output.status.code(), Some(1), "partial failure: {output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("--diagnostic-partial"),
        "the refusal explains the explicit diagnostic option: {stderr}"
    );
    assert!(output.stdout.is_empty(), "no success summary is printed");
    assert!(!out.exists(), "no normal snapshot is created");
    Ok(())
}

#[test]
fn json_lists_the_failed_records() -> TestResult {
    let (output, _, _dir) = catalog(&["--json", "--diagnostic-partial"])?;

    assert_eq!(output.status.code(), Some(1), "partial failure: {output:?}");
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let failed = summary["failed"].as_array().ok_or("failed")?;
    assert_eq!(failed.len(), 1, "one failure");
    assert_eq!(failed[0]["guid"], BROKEN_GUID, "the GUID");
    assert_eq!(failed[0]["path"], BROKEN_PATH, "the record path");
    assert!(
        failed[0]["error"].as_str().is_some_and(|e| !e.is_empty()),
        "the reason"
    );
    assert_eq!(
        summary["export_health"]["status"], "incomplete",
        "explicit health"
    );
    assert_eq!(
        summary["export_health"]["publicationMode"], "diagnosticPartial",
        "the summary identifies diagnostic permission"
    );
    Ok(())
}

fn read_catalog(path: &Path) -> Result<serde_json::Value, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    if path.extension().is_some_and(|extension| extension == "zst") {
        let mut decoded = Vec::new();
        zstd::Decoder::new(bytes.as_slice())?.read_to_end(&mut decoded)?;
        Ok(serde_json::from_slice(&decoded)?)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[test]
fn incomplete_normal_output_preserves_existing_plain_and_compressed_destinations() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    for extension in ["json", "json.zst"] {
        for existing in [false, true] {
            let out = dir.path().join(format!("normal-{existing}.{extension}"));
            if existing {
                fs::write(&out, "previous verified snapshot")?;
            }
            let output = run(&archive, &out, &["--json"])?;
            assert_eq!(output.status.code(), Some(1), "normal refusal: {output:?}");
            assert!(output.stdout.is_empty(), "a refusal has no success JSON");
            if existing {
                assert_eq!(
                    fs::read(&out)?,
                    b"previous verified snapshot",
                    "existing bytes survive"
                );
            } else {
                assert!(!out.exists(), "a fresh destination stays absent");
            }
        }
    }
    Ok(())
}

#[test]
fn explicit_diagnostic_output_marks_the_file_and_retains_failed_source_identity() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    for extension in ["json", "json.zst"] {
        for existing in [false, true] {
            let out = dir
                .path()
                .join(format!("diagnostic-{existing}.{extension}"));
            if existing {
                fs::write(&out, "old")?;
            }
            let output = run(&archive, &out, &["--json", "--diagnostic-partial"])?;
            assert_eq!(
                output.status.code(),
                Some(1),
                "diagnostic stays incomplete: {output:?}"
            );
            assert!(
                String::from_utf8(output.stderr)?.contains("diagnosticPartial"),
                "explicit status"
            );
            let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            let artifact = read_catalog(&out)?;
            assert_eq!(
                artifact["exportHealth"], summary["export_health"],
                "file/summary agree"
            );
            assert_eq!(
                artifact["exportHealth"]["status"], "incomplete",
                "no false complete status"
            );
            let failed = artifact["exportHealth"]["failedRecords"]
                .as_array()
                .ok_or("failed Records")?;
            assert!(
                failed.iter().any(|record| record["guid"] == BROKEN_GUID),
                "full failed-source identity"
            );
            assert_eq!(
                artifact["definitions"]["manufacturers"]["rows"]
                    .as_array()
                    .map(Vec::len),
                Some(1),
                "readable data survives"
            );
        }
    }
    Ok(())
}

#[test]
fn a_healthy_filter_excludes_an_unneeded_failed_source_without_becoming_diagnostic() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("filtered.json");
    let output = run(
        &archive,
        &out,
        &["--json", "--filter", "scitemmanufacturer"],
    )?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "healthy selected sources publish: {output:?}"
    );
    let artifact = read_catalog(&out)?;
    assert_eq!(
        artifact["exportHealth"]["status"], "complete",
        "read/projection are healthy"
    );
    assert_eq!(
        artifact["exportHealth"]["publicationMode"], "normal",
        "normal permission"
    );
    assert_eq!(
        artifact["completeness"]["manufacturers"]["status"], "partial",
        "Filter scope stays separate"
    );
    Ok(())
}
