// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_archive::Checksum;
use p4kx_comparison::{
    Archive, ChangeKind, CompareOptions, Filter, IncomparableReason, OwnedComparison,
};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::{Value, json};

use super::entries::entries_only;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SAME: &str = "Data/same.txt";
const SIZE: &str = "Data/size.txt";
const BODY: &[u8] = b"123456789";

#[test]
fn equal_verified_bytes_with_different_declared_algorithms_are_incomparable() -> TestResult {
    let (_directory, old, new) = mixed_pair(
        [TestEntry::stored(SAME, BODY), TestEntry::stored(SIZE, b"a")],
        [
            TestEntry::stored(SAME, BODY),
            TestEntry::stored(SIZE, b"ab"),
        ],
    )?;
    let filtered = Filter::new(SAME);
    let options = CompareOptions {
        filter: Some(&filtered),
        ..entries_only()
    };
    let selected = p4kx_comparison::compare(&old, &new, None, &options)?;
    assert!(
        !selected.has_differences(),
        "the matched pair supplies no confirmed change"
    );
    let [row] = selected.incomparable_entries() else {
        return Err("one equal-size mixed-checksum pair".into());
    };
    assert_eq!(
        row.reason(),
        IncomparableReason::ChecksumAlgorithmsDiffer,
        "the directory comparison describes the algorithm mismatch"
    );
    assert_mixed_operands(&old, &new, row.old_entry(), row.new_entry())?;
    assert_report(&serde_json::to_value(selected.report())?)?;
    let all = p4kx_comparison::compare(&old, &new, None, &entries_only())?;
    let [changed] = all.entry_changes() else {
        return Err("one proven size change".into());
    };
    assert_eq!(
        changed.kind(),
        ChangeKind::Changed,
        "size alone proves the change"
    );
    assert_eq!(changed.path(), SIZE, "the different-sized original pair");
    Ok(())
}

fn assert_mixed_operands(
    old: &Archive,
    new: &Archive,
    before: &p4kx_comparison::Entry,
    after: &p4kx_comparison::Entry,
) -> TestResult {
    use std::io::Read;

    assert_eq!(
        before.checksum(),
        Checksum::Crc32(0xcbf4_3926),
        "plain ZIP CRC"
    );
    assert_eq!(after.checksum(), Checksum::Crc32c(0xe306_9283), "P4K CRC");
    for (archive, entry) in [(old, before), (new, after)] {
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        assert_eq!(
            bytes, BODY,
            "both declared checksums independently verify the same fixture"
        );
    }
    Ok(())
}

fn assert_report(report: &Value) -> TestResult {
    let record = report
        .pointer("/entries/incomparable/0")
        .ok_or("typed incomparable row")?;
    assert_eq!(
        record,
        &json!({
            "path": SAME,
            "reason": "checksum_algorithms_differ",
            "old": {"size": 9, "crc32": 0xcbf4_3926_u32},
            "new": {"size": 9, "crc32c": 0xe306_9283_u32}
        }),
        "the report publishes both original declared operands"
    );
    let fields = report
        .pointer("/entries/fieldOrigins")
        .ok_or("entry origins")?;
    for path in ["path", "reason", "old/crc32", "new/crc32c"] {
        assert!(
            fields
                .get(format!("/entries/incomparable/0/{path}"))
                .is_some(),
            "the incomparable {path} has its own evidence"
        );
    }
    let reason = fields
        .get("/entries/incomparable/0/reason")
        .ok_or("immediate reason source")?;
    let refs = reason
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("reason sources")?;
    assert!(
        refs.len() > 6,
        "the matching facts also include both layouts"
    );
    let schema = p4kx_comparison::report_schema();
    let validator = jsonschema::draft202012::new(&schema)?;
    assert!(
        validator.is_valid(report),
        "the report matches the declared v1 schema"
    );
    Ok(())
}

#[test]
fn mixed_duplicates_with_unique_spellings_remain_separate_pairs() -> TestResult {
    let (_directory, old, new) = mixed_pair(
        [
            TestEntry::stored("Data/alias", b"a"),
            TestEntry::stored("data/alias", b"b"),
        ],
        [
            TestEntry::stored("data/alias", b"b"),
            TestEntry::stored("Data/alias", b"a"),
        ],
    )?;
    let comparison = OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries_only())?;
    assert_eq!(
        comparison.len(),
        0,
        "the extraction mask contains no unverified row"
    );
    let full = comparison.comparison();
    assert_eq!(
        full.incomparable_entries().len(),
        2,
        "both proven physical pairings survive"
    );
    for row in comparison.incomparable_entries() {
        assert_eq!(
            row.old_entry().path(),
            row.new_entry().path(),
            "the original spellings distinguish both physical pairs"
        );
    }
    assert!(
        comparison
            .comparison_of(&[])
            .incomparable_entries()
            .is_empty(),
        "the confirmed-change mask carries only its selected change rows"
    );
    Ok(())
}

#[test]
fn same_algorithm_unchanged_and_changed_entries_keep_the_existing_result() -> TestResult {
    let folder = tempfile::tempdir()?;
    let old = folder.path().join("old.zip");
    let new = folder.path().join("new.zip");
    for (path, body) in [(&old, b"a"), (&new, b"b")] {
        let zip = TestContainer::new()
            .entry(TestEntry::stored(SAME, BODY))
            .entry(TestEntry::stored("Data/change.txt", body));
        std::fs::write(path, zip.build())?;
    }
    let old = Archive::open(old)?;
    let new = Archive::open(new)?;
    let comparison = p4kx_comparison::compare(&old, &new, None, &entries_only())?;
    assert_eq!(
        comparison.entry_counts().changed,
        1,
        "one equal-size comparable change"
    );
    assert!(
        comparison.incomparable_entries().is_empty(),
        "the algorithms match"
    );
    assert!(
        serde_json::to_value(comparison.report())?
            .pointer("/entries/incomparable")
            .is_none(),
        "an empty outcome class leaves the v1 wire shape intact"
    );
    Ok(())
}

fn mixed_pair(
    old: impl IntoIterator<Item = TestEntry>,
    new: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive, Archive)> {
    let folder = tempfile::tempdir()?;
    let old_path = folder.path().join("before.zip");
    let new_path = folder.path().join("after.p4k");
    let zip = old
        .into_iter()
        .fold(TestContainer::new(), TestContainer::entry);
    std::fs::write(&old_path, zip.build())?;
    new.into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&new_path)?;
    Ok((folder, Archive::open(old_path)?, Archive::open(new_path)?))
}
