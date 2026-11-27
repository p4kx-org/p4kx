// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_comparison::{Archive, ChangeKind, EntryChange, OwnedComparison, compare};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::entries::entries_only;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn equivalent_case_aliases_cancel_by_declared_content() -> TestResult {
    let (_directory, old, new) = pair(
        [("DATA/shared.txt", b"aaaa"), ("Data/shared.txt", b"bbbb")],
        [("data/shared.txt", b"bbbb"), ("dAtA/shared.txt", b"aaaa")],
    )?;
    let comparison = compare(&old, &new, None, &entries_only())?;
    assert!(
        comparison.entry_changes().is_empty(),
        "case aliases with equal declared content are unchanged"
    );
    Ok(())
}

#[test]
fn physically_indistinguishable_surplus_is_ambiguous() -> TestResult {
    let path = "Data/shared.txt";
    let (_directory, old, new) = pair([(path, b"same"), (path, b"same")], [(path, b"same")])?;
    let comparison = compare(&old, &new, None, &entries_only())?;
    let [change] = comparison.entry_changes() else {
        return Err("one duplicate occurrence removed".into());
    };
    assert_eq!(change.kind(), ChangeKind::Removed, "physical surplus");
    assert!(
        change.pairing_is_ambiguous(),
        "directory facts cannot establish which equal occurrence survived"
    );
    Ok(())
}

#[test]
fn unresolved_collision_reports_survivors_and_shared_sources() -> TestResult {
    let ambiguous = "Data/shared.txt";
    let (_directory, old, new) = pair(
        [
            ("Data/earlier.txt", b"aaaa"),
            ("data/earlier.txt", b"bbbb"),
            (ambiguous, b"aaaa"),
            (ambiguous, b"bbbb"),
        ],
        [
            ("data/earlier.txt", b"bbbb"),
            (ambiguous, b"cccc"),
            (ambiguous, b"dddd"),
        ],
    )?;
    let owned = OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries_only())?;
    let selected = owned.comparison_of(&[false, true, true, true, true]);
    let counts = selected.entry_counts();
    assert_eq!(
        (counts.added, counts.removed, counts.changed),
        (2, 2, 0),
        "indeterminate pairing preserves every original occurrence"
    );
    assert!(
        selected
            .entry_changes()
            .iter()
            .all(EntryChange::pairing_is_ambiguous),
        "all unresolved occurrences carry the marker"
    );
    let report = serde_json::to_value(selected.report())?;
    verify_origins(&report)?;
    Ok(())
}

fn verify_origins(report: &Value) -> TestResult {
    let fields = report
        .pointer("/entries/fieldOrigins")
        .ok_or("collision field origins")?;
    let changes = report
        .pointer("/entries/changes")
        .and_then(Value::as_array)
        .ok_or("physical collision rows")?;
    let mut first = None;
    for (index, change) in changes.iter().enumerate() {
        assert_eq!(
            change.get("pairing_ambiguous"),
            Some(&Value::Bool(true)),
            "pairing flag"
        );
        let path = format!("/entries/changes/{index}");
        let marker = fields
            .get(format!("{path}/pairing_ambiguous"))
            .ok_or("immediate pairing origin")?;
        let decision = fields
            .get(format!("{path}/change"))
            .ok_or("change origin")?;
        assert_eq!(
            marker, decision,
            "the marker cites its actual comparison inputs"
        );
        assert_eq!(
            marker.get("sourceSetComplete"),
            Some(&Value::Bool(false)),
            "the field does not claim a globally complete removal"
        );
        let refs = marker
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("collision candidate references")?;
        assert_eq!(
            refs.len(),
            16,
            "four original facts for each of four operands"
        );
        if let Some(first) = first {
            assert_eq!(
                refs, first,
                "every residual change cites the same physical bucket"
            );
        } else {
            first = Some(refs);
        }
    }
    Ok(())
}

fn pair(
    old: impl IntoIterator<Item = (&'static str, &'static [u8; 4])>,
    new: impl IntoIterator<Item = (&'static str, &'static [u8; 4])>,
) -> TestResult<(tempfile::TempDir, Archive, Archive)> {
    let folder = tempfile::tempdir()?;
    let before = folder.path().join("before.p4k");
    let after = folder.path().join("after.p4k");
    for (entries, path) in [
        (builder(ArchiveBuilder::new(), old), &before),
        (builder(ArchiveBuilder::v2(), new), &after),
    ] {
        entries.write_to(path)?;
    }
    Ok((folder, Archive::open(before)?, Archive::open(after)?))
}

fn builder(
    initial: ArchiveBuilder,
    rows: impl IntoIterator<Item = (&'static str, &'static [u8; 4])>,
) -> ArchiveBuilder {
    rows.into_iter().fold(initial, |archive, (path, body)| {
        archive.entry(TestEntry::stored(path, body))
    })
}
