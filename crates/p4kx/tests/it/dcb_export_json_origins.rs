// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "dcb_export_ndjson_tags.rs"]
mod ndjson_tags;

#[path = "dcb_export_json_annotation_assertions.rs"]
mod annotation_assertions;

use std::fs;
use std::path::Path;

use p4kx_extraction::{
    Archive, ConflictPolicy, DataForgeEntry, RecordExportOptions, RecordJson, TagTree,
    export_record_files,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};
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
fn an_unobserved_library_json_replacement_retires_the_old_companion_even_for_equal_bytes()
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
    let source = Archive::open_with_key(&archive, Some(p4kx_testkit::test_key()))?;
    let input = DataForgeEntry::read(&source)?;
    let forge = input.open()?;
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.name().ok() == Some("Manufacturer.Woodworks"))
        .collect();
    let tags = TagTree::default();
    let report = export_record_files::<RecordJson>(
        &forge,
        &records,
        &out,
        &RecordExportOptions {
            policy: ConflictPolicy::Overwrite,
            tags: Some(&tags),
            ..RecordExportOptions::default()
        },
    );
    assert_eq!(
        report.written(),
        1,
        "the ordinary library writer replaced the Record"
    );
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

#[test]
fn json_tags_and_localization_share_one_exact_origin_companion() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    write_annotated_archive(&archive)?;
    let output = directory.path().join("out");
    let report = export(
        &archive,
        &output,
        &[
            "--format", "json", "--filter", "kit", "--tags", "--lang", "en", "--json",
        ],
        0,
    )?;
    let report: Value = serde_json::from_str(&report)?;
    assert_eq!(report.get("tags"), Some(&Value::Bool(true)), "tags enabled");
    assert_eq!(
        report.get("language").and_then(Value::as_str),
        Some("en"),
        "English selected"
    );
    assert_eq!(
        report
            .get("origin_receipts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "one committed receipt"
    );
    let file = output.join("fixture/kit.json");
    let bytes = fs::read(&file)?;
    let payload: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        payload.pointer("/mount/tag").and_then(Value::as_str),
        Some("Weapon.Gun"),
        "selected tag path"
    );
    assert_eq!(
        payload.pointer("/label/text").and_then(Value::as_str),
        Some("A test kit"),
        "translated label"
    );
    let records: Vec<Value> =
        fs::read_to_string(file.with_file_name("kit.json.p4kx-origin.jsonl"))?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
    annotation_assertions::assert_receipt(&records, bytes.len())
}

fn observed_uses<'a>(
    records: &'a [Value],
    path: &str,
    encrypted: bool,
) -> Result<Vec<&'a Value>, Box<dyn std::error::Error>> {
    let field = records
        .iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
        .ok_or("missing annotation output address")?;
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "{path}"
    );
    assert_eq!(
        field
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "{path}"
    );
    let ids = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("origin references")?;
    Ok(records
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|use_row| use_row.get("id").is_some_and(|id| ids.contains(id)))
        .collect())
}

fn write_annotated_archive(file: &Path) -> TestResult {
    let reference = || TestType::ReferenceTo("Tag".to_owned());
    let tag = |id, name: &str, children| {
        TestRecord::new(id, "Tag", format!("Tag.{name}"), "fixture/tags.xml")
            .value("tagName", TestValue::text(name))
            .value("children", TestValue::Array(children))
    };
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", reference()),
        )
        .structure(TestStruct::new("TagDatabase").array("tags", reference()))
        .structure(
            TestStruct::new("Gear")
                .field("mount", reference())
                .field("label", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "TagDatabase", "TagDatabase.Roots", "fixture/roots.xml").value(
                "tags",
                TestValue::Array(vec![TestValue::Null, TestValue::Reference(2)]),
            ),
        )
        .record(tag(
            2,
            "Weapon",
            vec![TestValue::Null, TestValue::Reference(3)],
        ))
        .record(tag(3, "Gun", Vec::new()))
        .record(
            TestRecord::new(4, "Gear", "Gear.Kit", "fixture/kit.xml")
                .value("mount", TestValue::Reference(3))
                .value("label", TestValue::text("@fixture_Kit")),
        )
        .build();
    let english = GlobalIniBuilder::new()
        .text("fixture_Kit", "A test kit")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            english,
        ))
        .write_to(file)?;
    Ok(())
}
