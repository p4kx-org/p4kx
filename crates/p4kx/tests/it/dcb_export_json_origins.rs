// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::Value;

use super::{TestResult, export, write_archive};

#[test]
fn the_normal_record_json_export_publishes_bound_fields_and_keeps_prior_proof() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    let record = out.join("libs/foundry/records/vehicles/cart.json");
    let companion = record.with_file_name("cart.json.p4kx-origin.jsonl");

    let summary = export(
        &archive,
        &out,
        &["--format", "json", "--filter", "cart", "--json"],
        0,
    )?;
    let summary: Value = serde_json::from_str(&summary)?;
    assert!(
        summary
            .pointer("/origin_receipts/0")
            .and_then(Value::as_str)
            .is_some_and(|path| path.ends_with("/cart.json.p4kx-origin.jsonl")),
        "summary points to the committed companion"
    );
    let bytes = fs::read(&record)?;
    let original = fs::read_to_string(&companion)?;
    assert_companion(&bytes, &original)?;

    for policy in ["skip", "if-changed"] {
        export(
            &archive,
            &out,
            &["--format", "json", "--filter", "cart", "--conflict", policy],
            0,
        )?;
        assert_eq!(
            fs::read_to_string(&companion)?,
            original,
            "{policy} retains the existing proof"
        );
    }
    Ok(())
}

#[test]
fn an_ordinary_annotated_json_replacement_retires_the_old_companion_even_for_equal_bytes()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("out");
    let record = out.join("libs/foundry/records/makers/woodworks/Manufacturer.Woodworks.json");
    let companion = record.with_file_name("Manufacturer.Woodworks.json.p4kx-origin.jsonl");

    export(
        &archive,
        &out,
        &["--format", "json", "--filter", "makers"],
        0,
    )?;
    assert!(
        companion.is_file(),
        "the normal export publishes the original proof"
    );
    let plain = fs::read(&record)?;
    export(
        &archive,
        &out,
        &[
            "--format",
            "json",
            "--filter",
            "makers",
            "--tags",
            "--conflict",
            "overwrite",
        ],
        0,
    )?;
    assert_eq!(
        fs::read(&record)?,
        plain,
        "the tag resolver has no applied tags here"
    );
    assert!(
        !companion.exists(),
        "a replacement must not reuse the earlier proof"
    );
    Ok(())
}

fn assert_companion(bytes: &[u8], original: &str) -> TestResult {
    let rows: Vec<Value> = original
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("artifact receipt header")?;
    assert_eq!(
        header
            .pointer("/binding/byteLength")
            .and_then(Value::as_str),
        Some(bytes.len().to_string().as_str()),
        "binding measures written bytes"
    );
    let field = rows
        .iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some("/wheels/1/size"))
        .ok_or("second physical array item field")?;
    assert_eq!(
        field
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "selected DCB is encrypted"
    );
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the selected field has all its contributors"
    );

    Ok(())
}

#[test]
fn a_selected_native_record_folder_takes_precedence_over_a_companion() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let output = directory.path().join("out");
    write_colliding_archive(&archive)?;

    let published = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(&archive)
        .arg("--out")
        .arg(&output)
        .args(["--format", "json", "--json"])
        .output()?;
    assert_eq!(
        published.status.code(),
        Some(1),
        "one origin companion cannot be published"
    );
    let summary: Value = serde_json::from_slice(&published.stdout)?;
    assert_eq!(
        summary.get("exported").and_then(Value::as_u64),
        Some(2),
        "both payloads committed"
    );
    assert!(
        summary
            .get("failed")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "no Record payload failed"
    );
    assert!(
        summary
            .get("origin_failures")
            .and_then(Value::as_array)
            .is_some_and(|items| items.len() == 1),
        "the conflict is reported separately"
    );
    assert!(
        output.join("native/item.json").is_file(),
        "the sibling payload survives"
    );
    assert!(
        output
            .join("native/item.json.p4kx-origin.jsonl/child.json")
            .is_file(),
        "the selected directory is preserved"
    );
    Ok(())
}

fn write_colliding_archive(path: &Path) -> TestResult {
    let data = DataForgeBuilder::new()
        .structure(TestStruct::new("Record"))
        .record(TestRecord::new(
            1,
            "Record",
            "Record.Root",
            "native/item.xml",
        ))
        .record(TestRecord::new(
            2,
            "Record",
            "Record.Child",
            "native/item.json.p4kx-origin.jsonl/child.xml",
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", data))
        .write_to(path)?;
    Ok(())
}
