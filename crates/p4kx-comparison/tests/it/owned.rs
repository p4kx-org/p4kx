// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A Comparison that owns its Archives, its filtered view and the saved report.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_comparison::{
    Archive, ChangeKind, CompareOptions, Filter, Levels, OwnedComparison, compare,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn entries() -> CompareOptions<'static> {
    CompareOptions {
        levels: Levels::ENTRIES,
        ..CompareOptions::default()
    }
}

fn write(dir: &Path, channel: &str, builder: &ArchiveBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let folder = dir.join(channel);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(path)
}

fn pair() -> Result<(tempfile::TempDir, Archive, Archive), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let old = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::stored("Data\\removed.txt", b"gone".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\size.xml", b"<a/>".to_vec()))
        .entry(TestEntry::stored("Engine\\old.cfg", b"x=1".to_vec()));
    let new = ArchiveBuilder::new()
        .entry(TestEntry::stored("data/libs/SIZE.xml", b"<ab/>".to_vec()))
        .entry(TestEntry::stored("Data\\added.txt", b"new".to_vec()))
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::stored("Engine\\new.cfg", b"x=2".to_vec()));
    let old = Archive::open_with_key(
        write(dir.path(), "LIVE", &old)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let new = Archive::open_with_key(
        write(dir.path(), "PTU", &new)?,
        Some(p4kx_testkit::test_key()),
    )?;
    Ok((dir, old, new))
}

#[test]
fn an_owned_comparison_holds_the_same_changes_on_another_thread() -> TestResult {
    let (_dir, old, new) = pair()?;
    let expected: Vec<(ChangeKind, String)> = compare(&old, &new, None, &entries())?
        .entry_changes()
        .iter()
        .map(|change| (change.kind(), change.path().to_owned()))
        .collect();

    let owned = std::thread::spawn(move || {
        OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries())
    })
    .join()
    .map_err(|_| "the thread panicked")??;

    let changes: Vec<(ChangeKind, String)> = (0..owned.len())
        .filter_map(|index| owned.entry_change(index))
        .map(|change| (change.kind(), change.path().to_owned()))
        .collect();
    assert_eq!(changes, expected, "the same changes in the same order");
    assert_eq!(owned.len(), 5, "five changes");
    assert!(
        owned.entry_change(owned.len()).is_none(),
        "nothing past the end"
    );
    assert_eq!(owned.old_side().channel(), Some("LIVE"), "old Channel");
    assert_eq!(owned.new_side().channel(), Some("PTU"), "new Channel");
    Ok(())
}

#[test]
fn a_recomparison_with_a_filter_reports_like_comparing_with_it() -> TestResult {
    let (_dir, old, new) = pair()?;
    let filter = Filter::new("DATA/");
    let with_filter = CompareOptions {
        filter: Some(&filter),
        ..entries()
    };
    let expected = serde_json::to_value(compare(&old, &new, None, &with_filter)?)?;
    let owned = OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries())?;

    let report = owned.recompare(Some(&filter), None, |comparison| {
        (
            serde_json::to_value(comparison),
            comparison.entry_counts().total(),
        )
    })?;

    assert_eq!(report.0?, expected, "the same report");
    assert_eq!(report.1, 3, "the three Data changes");
    Ok(())
}
#[test]
fn the_saved_report_is_the_pretty_json_with_a_final_newline() -> TestResult {
    let (dir, old, new) = pair()?;
    let owned = OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries())?;
    let path = dir.path().join("report.json");

    owned.comparison().save_report(&path)?;

    let written = std::fs::read_to_string(&path)?;
    let expected = format!("{}\n", serde_json::to_string_pretty(&owned.comparison())?);
    assert_eq!(written, expected, "pretty JSON and a newline");
    Ok(())
}

#[test]
fn a_selection_counts_only_the_selected_changes() -> TestResult {
    let (_dir, old, new) = pair()?;
    let owned = OwnedComparison::compare(Arc::new(old), Arc::new(new), None, &entries())?;

    let selected = owned.comparison_of(&[true, false, true]);

    let counts = selected.entry_counts();
    assert_eq!(counts.total(), 2, "the first and third change");
    assert_eq!(
        owned.comparison_of(&[]).entry_counts().total(),
        0,
        "nothing selected"
    );
    Ok(())
}

#[test]
fn report_writes_refuse_both_source_installations_but_allow_the_outside_target() -> TestResult {
    let (dir, old, new) = pair()?;
    let old_parent = old.path().parent().ok_or("old parent")?;
    let new_parent = new.path().parent().ok_or("new parent")?;
    std::fs::create_dir(old_parent.join("Bin64"))?;
    std::fs::create_dir(new_parent.join("Bin64"))?;
    let comparison = compare(&old, &new, None, &entries())?;
    for parent in [old_parent, new_parent] {
        let path = parent.join("report.json");
        let error = comparison
            .save_report(&path)
            .expect_err("the actual source installation is protected");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied,
            "the source protection is explicit"
        );
        assert!(
            !path.exists(),
            "neither source installation receives an output"
        );
    }
    let outside = dir.path().join("outside-report.json");
    comparison.save_report(&outside)?;
    assert!(outside.is_file(), "an unrelated target remains writable");
    Ok(())
}

#[test]
fn a_cancelled_report_preserves_the_existing_file_and_creates_no_temporary_output() -> TestResult {
    let (dir, old, new) = pair()?;
    let comparison = compare(&old, &new, None, &entries())?;
    let target = dir.path().join("report.json");
    std::fs::write(&target, b"existing user report")?;
    let before = std::fs::read_dir(dir.path())?.count();
    let cancel = p4kx_extraction::CancelToken::new();
    cancel.cancel();
    let error = comparison
        .save_report_with_cancel(&target, Some(&cancel))
        .expect_err("cancelled");
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::Interrupted,
        "the cancellation retains its meaning"
    );
    assert_eq!(
        std::fs::read(&target)?,
        b"existing user report",
        "no replacement after cancellation"
    );
    assert_eq!(
        std::fs::read_dir(dir.path())?.count(),
        before,
        "no temporary output is left"
    );
    Ok(())
}
