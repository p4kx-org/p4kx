// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_comparison::{Archive, ChangeKind, CompareOptions, Entry, Filter, compare};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::entries::{entries_only, pair};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn exact_join_operands_distinguish_size_checksum_and_ordering() -> TestResult {
    let (_dir, old, new) = pair()?;
    let report = serde_json::to_value(compare(&old, &new, None, &entries_only())?.report())?;
    let sources = report
        .pointer("/entries/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("comparison physical sources")?;
    assert!(
        sources.iter().any(|source| {
            source.get("kind").and_then(Value::as_str) == Some("archiveEntryProof")
                && source.get("entryIndex").and_then(Value::as_str).is_some()
                && source.get("name").is_none()
        }),
        "the unchanged opposing entry has compact physical proof"
    );
    let pair = [
        ("path", "selector"),
        ("path", "selector"),
        ("uncompressed_size", "conversion"),
        ("uncompressed_size", "conversion"),
    ];
    check(&report, "Data\\Libs\\size.xml", true, pair.to_vec())?;
    let checksums = [("checksum", "conversion"), ("checksum", "conversion")];
    check(
        &report,
        "data\\libs\\CRC.xml",
        true,
        checksums.into_iter().chain(pair).collect(),
    )?;
    check(
        &report,
        "Data\\added.txt",
        false,
        vec![("path", "selector"), ("path", "selector")],
    )?;
    check(
        &report,
        "root.txt",
        false,
        vec![("owner_entry_index", "selector")],
    )
}

#[test]
fn an_unchanged_case_alias_is_kept_before_reporting_a_removed_entry() -> TestResult {
    const REMOVED: &str = "Data\\shared.txt";
    const RETAINED: &str = "data\\shared.txt";
    let dir = tempfile::tempdir()?;
    let before = dir.path().join("before.p4k");
    let after = dir.path().join("after.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(REMOVED, b"AAAA"))
        .entry(TestEntry::stored(RETAINED, b"BBBB"))
        .write_to(&before)?;
    ArchiveBuilder::v2()
        .entry(TestEntry::stored(RETAINED, b"BBBB"))
        .write_to(&after)?;
    let before = Archive::open(before)?;
    let after = Archive::open(after)?;
    let comparison = compare(&before, &after, None, &entries_only())?;
    let changes = comparison.entry_changes();
    assert_eq!(
        changes.len(),
        1,
        "the unchanged case alias does not become a change or removal: {changes:?}"
    );
    let change = changes.first().ok_or("removed physical Entry")?;
    assert_eq!(change.kind(), ChangeKind::Removed, "one physical removal");
    assert_eq!(
        change.path(),
        REMOVED,
        "the removed occurrence's original spelling"
    );
    assert_eq!(
        change.old_entry().map(Entry::index),
        Some(0),
        "the report keeps the removed Entry's original physical index"
    );
    let report = serde_json::to_value(comparison.report())?;
    check(&report, REMOVED, false, collision_operands(3))?;
    Ok(())
}

#[test]
fn a_filtered_unpaired_duplicate_does_not_prove_global_absence() -> TestResult {
    const PATH: &str = "Data/shared.txt";
    let dir = tempfile::tempdir()?;
    let before = dir.path().join("before.p4k");
    let after = dir.path().join("after.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, b"first"))
        .entry(TestEntry::stored("Data/excluded.txt", b"old"))
        .write_to(&before)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, b"first"))
        .entry(TestEntry::stored(PATH, b"second"))
        .write_to(&after)?;
    let before = Archive::open(before)?;
    let after = Archive::open(after)?;
    let filter = Filter::new("*/shared.txt");
    let options = CompareOptions {
        filter: Some(&filter),
        ..entries_only()
    };
    let report = serde_json::to_value(compare(&before, &after, None, &options)?.report())?;
    assert!(
        report.pointer("/entries/changes/0/old").is_none(),
        "the first opposite occurrence was already paired"
    );
    check(&report, PATH, false, collision_operands(3))
}

fn collision_operands(candidates: usize) -> Vec<(&'static str, &'static str)> {
    [
        ("owner_entry_index", "selector"),
        ("path", "selector"),
        ("uncompressed_size", "conversion"),
        ("checksum", "conversion"),
    ]
    .repeat(candidates)
}

fn check(
    report: &Value,
    path: &str,
    complete: bool,
    mut expected: Vec<(&str, &str)>,
) -> TestResult {
    let index = report
        .pointer("/entries/changes")
        .and_then(Value::as_array)
        .ok_or("entry changes")?
        .iter()
        .position(|change| change.get("path").and_then(Value::as_str) == Some(path))
        .ok_or("selected entry change")?;
    let fields = report
        .pointer("/entries/fieldOrigins")
        .ok_or("field origins")?;
    let field = fields
        .get(format!("/entries/changes/{index}/change"))
        .ok_or("immediate change origin")?;
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(complete),
        "coverage of the decision for {path}"
    );
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&if complete {
            Value::Bool(false)
        } else {
            Value::Null
        }),
        "incomplete one-sided evidence does not prove clear input"
    );
    let sites = report
        .pointer("/entries/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("shared origin use table")?;
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate source use refs")?;
    let mut actual = refs
        .iter()
        .map(|reference| input(sites, reference))
        .collect::<Result<Vec<_>, _>>()?;
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "only interpreted source facts for {path}");
    Ok(())
}

fn input<'site>(
    sites: &'site [Value],
    reference: &Value,
) -> Result<(&'site str, &'site str), &'static str> {
    let source = sites
        .iter()
        .find(|site| site.get("id") == Some(reference))
        .ok_or("reported use must resolve")?;
    Ok((
        source
            .pointer("/location/field")
            .and_then(Value::as_str)
            .ok_or("original directory fact")?,
        source
            .get("role")
            .and_then(Value::as_str)
            .ok_or("actual role")?,
    ))
}
