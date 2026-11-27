// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entry level over two plain-ZIP containers, whose Entries carry the plain CRC-32.

use std::error::Error;
use std::path::PathBuf;

use p4kx_comparison::{Archive, ChangeKind, CompareOptions, Levels, compare};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const SOCPAK: &str = "Data\\ObjectContainers\\room.socpak";

fn container(room: &str) -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::stored(
            "room\\room.xml",
            room.as_bytes().to_vec(),
        ))
        .entry(TestEntry::stored("room\\same.xml", b"<Same/>".to_vec()))
}

fn open_container(
    dir: &std::path::Path,
    name: &str,
    room: &str,
    encrypted: bool,
) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join(name);
    let outer = TestEntry::stored(SOCPAK, container(room).build());
    let outer = if encrypted { outer.encrypted() } else { outer };
    ArchiveBuilder::new().entry(outer).write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == SOCPAK)
        .ok_or("no container Entry")?;
    Ok(archive.open_container(entry)?)
}

fn schema_violations(report: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/comparison-report.schema.json");
    let schema: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let validator = jsonschema::draft202012::new(&schema)?;
    Ok(validator
        .iter_errors(report)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect())
}

#[test]
fn two_containers_compare_by_plain_crc32_and_the_report_matches_the_schema() -> TestResult {
    let dir = tempfile::tempdir()?;
    let old = open_container(dir.path(), "old.p4k", "<Room/>", true)?;
    let new = open_container(dir.path(), "new.p4k", "<Room2/>", false)?;

    let comparison = compare(
        &old,
        &new,
        None,
        &CompareOptions {
            levels: Levels::ENTRIES,
            ..CompareOptions::default()
        },
    )?;

    let changes: Vec<_> = comparison
        .entry_changes()
        .iter()
        .map(|change| (change.kind(), change.path()))
        .collect();
    assert_eq!(
        changes,
        [(ChangeKind::Changed, "room\\room.xml")],
        "only the Entry with other data differs"
    );
    let report = serde_json::to_value(comparison.report())?;
    let violations = schema_violations(&report)?;
    assert!(violations.is_empty(), "the report is valid: {violations:?}");
    let change = report
        .pointer("/entries/changes/0")
        .or_else(|| report.pointer("/changes/0"))
        .ok_or("no change in the report")?;
    assert!(
        change.pointer("/old/crc32").is_some() && change.pointer("/new/crc32").is_some(),
        "both sides report the plain CRC-32: {change}"
    );
    assert!(
        change.pointer("/old/crc32c").is_none(),
        "and no CRC-32C: {change}"
    );
    check_origins(&report)?;
    check_aggregates(&report)?;
    check_historical_schema(report)
}

fn check_aggregates(report: &Value) -> TestResult {
    let origins = report
        .pointer("/entries/fieldOrigins")
        .ok_or("Entry report field origins")?;
    for path in ["/entries/counts/changed", "/entries/directories/0/changed"] {
        let origin = origins.get(path).ok_or("aggregate field origin")?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "the aggregate uses a child comparison inside an encrypted container",
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "the matched decision and group assignments are complete",
        );
    }
    let label = origins
        .get("/entries/directories/0/directory")
        .ok_or("directory label source")?;
    assert_eq!(
        label
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the displayed directory comes from the clear new side",
    );
    Ok(())
}

fn check_historical_schema(mut report: Value) -> TestResult {
    let entries = report
        .pointer_mut("/entries")
        .and_then(Value::as_object_mut)
        .ok_or("comparison Entry report")?;
    entries.remove("fieldOrigins");
    entries.remove("provenance");
    let violations = schema_violations(&report)?;
    assert!(
        violations.is_empty(),
        "a historical v1 report still validates: {violations:?}"
    );
    Ok(())
}

fn check_origins(report: &Value) -> TestResult {
    let origins = report
        .pointer("/entries/fieldOrigins")
        .ok_or("Entry report field origins")?;
    let mut side_refs = Vec::new();
    for (side, encrypted) in [("old", true), ("new", false)] {
        for (field, original) in [("size", "uncompressed_size"), ("crc32", "checksum")] {
            let path = format!("/entries/changes/0/{side}/{field}");
            let origin = origins.get(&path).ok_or("immediate side-field origin")?;
            assert_eq!(
                origin
                    .get("containsEncryptedSource")
                    .and_then(Value::as_bool),
                Some(encrypted),
                "directory metadata derives from the outer payload in {side}"
            );
            assert_eq!(
                origin.get("sourceSetComplete").and_then(Value::as_bool),
                Some(true),
                "the physical Entry and all ancestors were observed"
            );
            let usage = single_use(report, origin)?;
            assert_eq!(
                usage.pointer("/location/field").and_then(Value::as_str),
                Some(original),
                "physical source field for {path}"
            );
            if field == "size" {
                side_refs.push(usage.get("source").ok_or("physical source reference")?);
            }
        }
    }
    let [old, new] = side_refs.as_slice() else {
        return Err("two original size source references".into());
    };
    assert_ne!(
        old, new,
        "equal leaf paths belong to distinct physical inputs"
    );
    let path_origin = origins
        .get("/entries/changes/0/path")
        .ok_or("winning path origin")?;
    assert_eq!(
        path_origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the emitted path comes from the clear new side"
    );
    Ok(())
}

fn single_use<'report>(
    report: &'report Value,
    origin: &Value,
) -> Result<&'report Value, Box<dyn Error>> {
    let [reference] = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("actual source use references")?
        .as_slice()
    else {
        return Err("one directory metadata use".into());
    };
    report
        .pointer("/entries/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("report source use inventory")?
        .iter()
        .find(|usage| usage.get("id") == Some(reference))
        .ok_or_else(|| "source use resolves in this report".into())
}
