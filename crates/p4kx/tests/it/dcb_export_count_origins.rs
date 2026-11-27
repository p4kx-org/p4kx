// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const FORMS: &[(&str, &[&str])] = &[
    ("xml", &[]),
    ("json", &["--format", "json"]),
    ("combined", &["--combined"]),
    ("ndjson", &["--format", "ndjson"]),
    ("flat-csv", &["--flat"]),
    ("flat-raw", &["--flat", "--csv-raw"]),
    ("flat-strict", &["--flat", "--format", "csv-strict"]),
    ("flat-ndjson", &["--flat", "--format", "ndjson"]),
];

fn database() -> Vec<u8> {
    let row = |guid, name, path| {
        TestRecord::new(guid, "Item", name, path).value("value", TestValue::Int32(42))
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("Item").field("value", TestType::Int32))
        .structure(TestStruct::new("Empty"))
        .record(row(1, "Item.Zulu", "chosen/zulu.xml"))
        .record(row(2, "Item.Other", "other/ignored.xml"))
        .record(TestRecord::new(
            3,
            "Empty",
            "Empty.Alpha",
            "chosen/alpha.xml",
        ))
        .record(row(4, "Item.Beta", "chosen/beta.xml"))
        .build()
}

fn archive(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

fn export(input: &Path, target: &Path, args: &[&str], exit: i32) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["dcb", "export"])
        .arg(input)
        .arg("--out")
        .arg(target)
        .args(["--json", "--filter", "chosen/"])
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    let text = String::from_utf8(output.stdout)?;
    assert_eq!(text.lines().count(), 1, "one summary line: {text}");
    Ok(serde_json::from_str(&text)?)
}

#[test]
fn every_form_binds_the_count_to_the_committed_original_definitions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(dir.path())?;
    for &(name, args) in FORMS {
        let output = dir.path().join(name);
        let summary = export(&input, &output, args, 0)?;
        assert_count(&summary, 3, Some(true), &[[0, 1], [2, 4]])?;
        assert!(
            summary.get("origin_receipts").is_some(),
            "{name} retains the actual output companion paths"
        );
    }
    Ok(())
}

#[test]
fn kept_and_unchanged_outputs_supply_no_committed_definitions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(dir.path())?;
    for &(name, args) in FORMS {
        let target = dir.path().join(name);
        export(&input, &target, args, 0)?;
        for policy in ["skip", "if-changed"] {
            let mut arguments = args.to_vec();
            arguments.extend(["--conflict", policy]);
            let kept = export(&input, &target, &arguments, 0)?;
            assert_count(&kept, 0, Some(false), &[])?;
            assert!(
                kept.get("origin_receipts").is_none() && kept.pointer("/skipped/0").is_some(),
                "{name} {policy} reports the kept file only"
            );
        }
    }
    Ok(())
}

#[test]
fn a_failed_file_cannot_supply_a_committed_definition() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(dir.path())?;
    let target = dir.path().join("files");
    fs::create_dir_all(target.join("chosen/beta.xml"))?;
    let summary = export(&input, &target, &["--conflict", "overwrite"], 1)?;
    assert_count(&summary, 2, Some(true), &[[0, 1], [2, 3]])?;
    assert_eq!(
        summary.pointer("/failed/0/path"),
        Some(&json!("chosen/beta")),
        "the refused output's Record is reported separately"
    );
    Ok(())
}

#[test]
fn a_failed_stream_commit_has_zero_committed_definitions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(dir.path())?;
    let target = dir.path().join("stream");
    fs::create_dir_all(target.join("records.ndjson"))?;
    let summary = export(&input, &target, &["--format", "ndjson"], 1)?;
    assert_count(&summary, 0, Some(false), &[])?;
    assert!(
        summary.get("error").is_some_and(Value::is_string),
        "the original file error is still in the report"
    );
    Ok(())
}

#[test]
fn a_companion_failure_keeps_the_actual_committed_definition_evidence() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(dir.path())?;
    let target = dir.path().join("combined");
    fs::create_dir(&target)?;
    let foreign = target.join("Game2.xml.p4kx-origin.jsonl");
    fs::write(&foreign, b"foreign")?;
    let summary = export(&input, &target, &["--combined"], 1)?;
    assert_count(&summary, 3, Some(true), &[[0, 1], [2, 4]])?;
    assert!(
        summary.pointer("/origin_failures/0").is_some(),
        "the companion failure still appears separately"
    );
    assert_eq!(fs::read(&foreign)?, b"foreign", "foreign proof was kept");
    assert!(target.join("Game2.xml").is_file(), "XML committed");
    Ok(())
}

#[test]
fn a_standalone_definition_has_unknown_archive_encryption() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("Game2.dcb");
    fs::write(&input, database())?;
    let summary = export(&input, &dir.path().join("loose"), &["--combined"], 0)?;
    assert_count(&summary, 3, None, &[[0, 1], [2, 4]])
}

fn assert_count(
    report: &Value,
    exported: usize,
    encrypted: Option<bool>,
    expected: &[[u32; 2]],
) -> TestResult {
    assert_eq!(
        report.get("exported"),
        Some(&json!(exported)),
        "ordinary committed Record count"
    );
    assert!(
        report.get("provenanceError").is_none(),
        "count origin captured successfully"
    );
    let origin = report
        .pointer("/fieldOrigins/~1exported")
        .ok_or("exported count origin")?;
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "every committed definition is represented"
    );
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "only contributing source controls the encryption marker"
    );
    let mut indices = original_ranges(report, origin)?;
    indices.sort_unstable();
    assert_eq!(indices, expected, "exact committed definition positions");
    Ok(())
}

fn original_ranges(report: &Value, origin: &Value) -> Result<Vec<[u32; 2]>, Box<dyn Error>> {
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("count contributor references")?;
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("count source uses")?;
    let observed_definitions = inventory.iter().filter(|used| {
        used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeRecordTable")
            && used.pointer("/location/field").and_then(Value::as_str)
                == Some("committedDefinitions")
    });
    assert_eq!(
        observed_definitions.count(),
        references.len(),
        "no unaddressed committed definition ranges"
    );
    let mut ranges = Vec::new();
    for reference in references {
        let used = inventory
            .iter()
            .find(|used| used.get("id") == Some(reference))
            .ok_or("bound source reference")?;
        assert_eq!(
            used.get("role"),
            Some(&json!("conversion")),
            "derived count"
        );
        assert_eq!(
            used.pointer("/location/namespace"),
            Some(&json!("dataforgeRecordTable")),
            "original Record definition table"
        );
        assert_eq!(
            used.pointer("/location/field"),
            Some(&json!("committedDefinitions")),
            "committed subset"
        );
        let positions = used
            .pointer("/location/indices")
            .and_then(Value::as_array)
            .ok_or("definition range")?;
        let [start, end] = positions.as_slice() else {
            return Err("expected a half-open definition range".into());
        };
        ranges.push([
            start.as_str().ok_or("definition range start")?.parse()?,
            end.as_str().ok_or("definition range end")?.parse()?,
        ]);
    }
    Ok(ranges)
}

pub(super) fn remove_count_origin(summary: &mut Value) -> TestResult {
    let object = summary.as_object_mut().ok_or("legacy summary object")?;
    for field in ["fieldOrigins", "provenance"] {
        assert!(
            object.remove(field).is_some(),
            "count proof {field} is present"
        );
    }
    Ok(())
}
