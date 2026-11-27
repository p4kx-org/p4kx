// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use p4kx_comparison::{
    Archive, CompareOptions, DataForgeEntry, DataForges, Levels, OwnedComparison, OwnedDataForges,
    compare, compare_with_dataforge_entries,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SHARED: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0001;
const BROKEN: u128 = SHARED + 1;

fn database(old: bool) -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new()
        .version(if old { 6 } else { 8 })
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .structure(
            TestStruct::new("Hull")
                .field("armor", TestType::Int32)
                .declared_size(if old { 4 } else { 8 }),
        )
        .record(TestRecord::new(SHARED, "Ship", "Ship.One", "first.xml"))
        .record(
            TestRecord::new(
                SHARED,
                "Ship",
                "Ship.Twin",
                if old { "second.xml" } else { "moved.xml" },
            )
            .tag("TestTag"),
        );
    if old {
        builder = builder.record(TestRecord::new(SHARED, "Ship", "Ship.Third", "third.xml"));
    }
    builder.record(TestRecord::new(BROKEN, "Hull", "Hull.Broken", "broken.xml"))
}

fn write(path: &Path, old: bool) -> Result<Archive, Box<dyn Error>> {
    let entry = TestEntry::deflated("Data\\Game2.dcb", database(old).build());
    ArchiveBuilder::new()
        .entry(if old { entry.encrypted() } else { entry })
        .write_to(path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn options() -> CompareOptions<'static> {
    CompareOptions {
        levels: Levels::RECORDS,
        ..CompareOptions::default()
    }
}

#[test]
fn record_headers_use_exact_old_new_payloads_and_duplicate_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let old = Arc::new(write(&directory.path().join("old.p4k"), true)?);
    let new = Arc::new(write(&directory.path().join("new.p4k"), false)?);
    let entries = OwnedDataForges {
        old: Arc::new(DataForgeEntry::read(&old)?),
        new: Arc::new(DataForgeEntry::read(&new)?),
    };
    let comparison = OwnedComparison::compare(old, new, Some(entries), &options())?;
    let json = comparison.recompare(None, None, |comparison| {
        serde_json::to_value(comparison.report())
    })??;

    let moved = change(&json, "moved")?;
    let guid = site(&json, moved, "guid", false)?;
    check_record(guid, 1, "guid", "payload");
    let prior = site(&json, moved, "old/record_path", true)?;
    check_record(prior, 1, "fileName", "conversion");
    let current = site(&json, moved, "new/name", false)?;
    check_record(current, 1, "recordName", "conversion");
    let tag = site(&json, moved, "new/tag", false)?;
    check_record(tag, 1, "tag", "conversion");
    assert!(
        origin(&json, moved, "old/tag").is_none(),
        "version 6 omitted tag"
    );
    assert_ne!(
        prior.get("source"),
        current.get("source"),
        "same Entry path, two sources"
    );
    check_type(&json, moved, "old/type", true)?;

    let removed = change(&json, "removed")?;
    let guid = site(&json, removed, "guid", true)?;
    check_record(guid, 2, "guid", "payload");
    assert!(
        origin(&json, removed, "new/record_path").is_none(),
        "omitted side"
    );

    let unreadable = change(&json, "unreadable")?;
    let guid = site(&json, unreadable, "guid", false)?;
    check_record(guid, 2, "guid", "payload");
    let path = site(&json, unreadable, "old/record_path", true)?;
    check_record(path, 3, "fileName", "conversion");
    let new_name = site(&json, unreadable, "new/name", false)?;
    check_record(new_name, 2, "recordName", "conversion");
    assert!(
        origin(&json, unreadable, "fields/0/old").is_none(),
        "failed tree emitted no body"
    );
    Ok(())
}

#[test]
fn records_without_exact_archive_and_byte_binding_remain_unknown() -> TestResult {
    let directory = tempfile::tempdir()?;
    let old_path = directory.path().join("old.p4k");
    let old = write(&old_path, true)?;
    let new = write(&directory.path().join("new.p4k"), false)?;
    let old_entry = DataForgeEntry::read(&old)?;
    let new_entry = DataForgeEntry::read(&new)?;
    let reopened = Archive::open_with_key(old_path, Some(p4kx_testkit::test_key()))?;
    let reopened_entry = DataForgeEntry::read(&reopened)?;
    let old_data = old_entry.open()?;
    let new_data = new_entry.open()?;
    let reopened_data = reopened_entry.open()?;
    let forges = DataForges {
        old: &old_data,
        new: &new_data,
    };
    let bare = compare(&old, &new, Some(forges), &options())?;
    let wrong_bytes = compare_with_dataforge_entries(
        &old,
        &new,
        forges,
        [&reopened_entry, &new_entry],
        &options(),
    )?;
    let wrong_owner = compare_with_dataforge_entries(
        &old,
        &new,
        DataForges {
            old: &reopened_data,
            new: &new_data,
        },
        [&reopened_entry, &new_entry],
        &options(),
    )?;

    for result in [&bare, &wrong_bytes, &wrong_owner] {
        let json = serde_json::to_value(result.report())?;
        let removed = change(&json, "removed")?;
        assert_eq!(
            origin(&json, removed, "guid"),
            Some(
                &json!({"containsEncryptedSource": null, "sourceSetComplete": false, "sourceUsesRefs": []})
            ),
            "missing exact byte or Archive evidence leaves the observed row unknown"
        );
    }
    let bound_new = serde_json::to_value(wrong_owner.report())?;
    let moved = change(&bound_new, "moved")?;
    check_record(
        site(&bound_new, moved, "guid", false)?,
        1,
        "guid",
        "payload",
    );
    Ok(())
}

fn change(report: &Value, kind: &str) -> Result<usize, &'static str> {
    report
        .pointer("/records/changes")
        .and_then(Value::as_array)
        .ok_or("record changes")?
        .iter()
        .position(|row| row.get("change").and_then(Value::as_str) == Some(kind))
        .ok_or("record change")
}

fn origin<'a>(report: &'a Value, row: usize, field: &str) -> Option<&'a Value> {
    report
        .pointer("/records/fieldOrigins")?
        .get(format!("/records/changes/{row}/{field}"))
}

fn sites<'a>(
    report: &'a Value,
    row: usize,
    field: &str,
    encrypted: bool,
) -> Result<Vec<&'a Value>, &'static str> {
    let origin = origin(report, row, field).ok_or("record field origin")?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "header marker for {field}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "header data complete for {field}"
    );
    let evidence = report
        .pointer("/records/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("report use inventory")?;
    origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field references")?
        .iter()
        .map(|reference| {
            evidence
                .iter()
                .find(|site| site.get("id") == Some(reference))
                .ok_or("resolved source use")
        })
        .collect()
}

fn site<'a>(
    report: &'a Value,
    row: usize,
    field: &str,
    encrypted: bool,
) -> Result<&'a Value, &'static str> {
    let origins = sites(report, row, field, encrypted)?;
    let [site] = origins.as_slice() else {
        return Err("one direct header source use");
    };
    Ok(site)
}

fn check_record(site: &Value, index: u32, field: &str, role: &str) {
    assert_eq!(
        site.get("location"),
        Some(&json!({"type": "dataForge", "recordIndex": index, "field": field})),
        "original Record coordinate for {field}"
    );
    assert_eq!(
        site.get("role"),
        Some(&json!(role)),
        "contribution for {field}"
    );
    assert_eq!(
        site.get("scope"),
        Some(&json!("payload")),
        "DataForge byte domain"
    );
}

fn check_type(report: &Value, row: usize, field: &str, encrypted: bool) -> TestResult {
    let evidence = sites(report, row, field, encrypted)?;
    assert_eq!(
        evidence.len(),
        2,
        "record struct selector and selected name"
    );
    let selector = evidence
        .iter()
        .find(|site| site.pointer("/location/field").and_then(Value::as_str) == Some("sourceType"))
        .ok_or("record struct selector")?;
    check_record(selector, 1, "sourceType", "selector");
    let name = evidence
        .iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeStructDefinition")
        })
        .ok_or("selected definition name")?;
    assert_eq!(
        name.get("location"),
        Some(
            &json!({"type": "subresource", "namespace": "dataforgeStructDefinition", "indices": ["0"], "field": "name"})
        ),
        "the exact selected struct definition index"
    );
    assert_eq!(
        name.get("role"),
        Some(&json!("payload")),
        "struct name is retained text"
    );
    Ok(())
}
