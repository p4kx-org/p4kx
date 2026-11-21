// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entry level of a Comparison through the public API.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use p4kx_comparison::{
    Archive, Build, CancelToken, ChangeKind, CompareOptions, ComparisonError, ComparisonProgress,
    DEFAULT_CONTEXT, EntryChange, Filter, Levels, compare, line_diff,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};

type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn entries_only() -> CompareOptions<'static> {
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

pub(super) fn pair() -> Result<(tempfile::TempDir, Archive, Archive), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let old = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\same.txt", b"same".to_vec()))
        .entry(TestEntry::stored("Data\\removed.txt", b"gone".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\size.xml", b"<a/>".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\crc.xml", b"<a/>".to_vec()))
        .entry(TestEntry::deflated(
            "Engine\\recompressed.cfg",
            b"x=1".to_vec(),
        ));
    let new = ArchiveBuilder::v2()
        .entry(TestEntry::zstd("Engine\\recompressed.cfg", b"x=1".to_vec()))
        .entry(TestEntry::stored("data/libs/CRC.xml", b"<b/>".to_vec()))
        .entry(TestEntry::stored("Data\\Libs\\size.xml", b"<ab/>".to_vec()))
        .entry(TestEntry::stored("Data\\added.txt", b"new".to_vec()))
        .entry(TestEntry::stored("DATA\\SAME.TXT", b"same".to_vec()))
        .entry(TestEntry::stored("root.txt", b"r".to_vec()));
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
fn entries_are_joined_by_normalised_path_and_changed_only_by_size_or_crc() -> TestResult {
    let (_dir, old, new) = pair()?;

    let comparison = compare(&old, &new, None, &entries_only())?;

    let changes: Vec<(ChangeKind, &str)> = comparison
        .entry_changes()
        .iter()
        .map(|change| (change.kind(), change.path()))
        .collect();
    assert_eq!(
        changes,
        [
            (ChangeKind::Added, "Data\\added.txt"),
            (ChangeKind::Changed, "data\\libs\\CRC.xml"),
            (ChangeKind::Changed, "Data\\Libs\\size.xml"),
            (ChangeKind::Removed, "Data\\removed.txt"),
            (ChangeKind::Added, "root.txt"),
        ],
        "sorted by normalised path, unchanged Entries left out"
    );
    let counts = comparison.entry_counts();
    assert_eq!(
        (counts.added, counts.removed, counts.changed),
        (2, 1, 2),
        "counts per change kind"
    );
    assert!(comparison.has_differences(), "the Archives differ");
    Ok(())
}

#[test]
fn a_changed_entry_keeps_both_sides() -> TestResult {
    let (_dir, old, new) = pair()?;

    let comparison = compare(&old, &new, None, &entries_only())?;

    let size = comparison
        .entry_changes()
        .iter()
        .find(|change| change.path() == "Data\\Libs\\size.xml")
        .ok_or("size.xml is changed")?;
    let (Some(before), Some(after)) = (size.old_entry(), size.new_entry()) else {
        return Err("a changed Entry has both sides".into());
    };
    assert_eq!(
        (before.uncompressed_size(), after.uncompressed_size()),
        (4, 5),
        "sizes of both sides"
    );
    let added = comparison
        .entry_changes()
        .iter()
        .find(|change| change.kind() == ChangeKind::Added)
        .ok_or("an added Entry")?;
    assert!(
        added.old_entry().is_none() && added.new_entry().is_some(),
        "an added Entry has only the new side"
    );
    Ok(())
}

#[test]
fn counts_are_given_per_top_level_directory() -> TestResult {
    let (_dir, old, new) = pair()?;

    let comparison = compare(&old, &new, None, &entries_only())?;

    let directories: Vec<(&str, usize, usize, usize)> = comparison
        .directory_counts()
        .iter()
        .map(|d| {
            (
                d.directory,
                d.counts.added,
                d.counts.removed,
                d.counts.changed,
            )
        })
        .collect();
    assert_eq!(
        directories,
        [("", 1, 0, 0), ("Data", 1, 1, 2)],
        "root Entries under the empty directory, case and separators ignored"
    );
    Ok(())
}

#[test]
fn the_filter_selects_entries_on_both_sides_with_the_browsing_rules() -> TestResult {
    let (_dir, old, new) = pair()?;
    let filter = Filter::new("DATA/LIBS/*.xml");

    let comparison = compare(
        &old,
        &new,
        None,
        &CompareOptions {
            filter: Some(&filter),
            ..entries_only()
        },
    )?;

    let paths: Vec<&str> = comparison
        .entry_changes()
        .iter()
        .map(EntryChange::path)
        .collect();
    assert_eq!(
        paths,
        ["data\\libs\\CRC.xml", "Data\\Libs\\size.xml"],
        "only the Entries the Filter matches"
    );
    assert_eq!(
        comparison.filter(),
        Some("DATA/LIBS/*.xml"),
        "the Filter text"
    );
    Ok(())
}

#[test]
fn identical_archives_have_no_differences() -> TestResult {
    let (_dir, old, _) = pair()?;

    let comparison = compare(&old, &old, None, &entries_only())?;

    assert!(comparison.entry_changes().is_empty(), "no Entry changed");
    assert!(!comparison.has_differences(), "no differences");
    Ok(())
}

#[test]
fn the_entry_level_reads_no_entry_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let data = b"<config speed=\"1\"/>".to_vec();
    let corrupt = ArchiveBuilder::new().entry(
        TestEntry::zstd("Data\\config.xml", data.clone()).with_payload(b"no zstd frame".to_vec()),
    );
    let intact = ArchiveBuilder::v2().entry(TestEntry::stored("Data\\config.xml", data));
    let old = Archive::open_with_key(
        write(dir.path(), "LIVE", &corrupt)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let new = Archive::open_with_key(
        write(dir.path(), "PTU", &intact)?,
        Some(p4kx_testkit::test_key()),
    )?;

    let comparison = compare(&old, &new, None, &entries_only())?;

    assert!(
        !comparison.has_differences(),
        "equal by size and CRC: {:?}",
        comparison.entry_changes()
    );
    assert!(
        line_diff(&old, &new, "Data\\config.xml", DEFAULT_CONTEXT).is_err(),
        "the data of the old side does not decompress"
    );
    Ok(())
}

#[test]
fn both_sides_carry_channel_and_build() -> TestResult {
    let (dir, old, _) = pair()?;
    write_build_manifest(
        &dir.path().join("PTU"),
        "sc-alpha-4.11.0",
        "4.11.42.1234",
        "12999999",
        "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
    )?;
    let new = Archive::open_with_key(
        dir.path().join("PTU").join("Data.p4k"),
        Some(p4kx_testkit::test_key()),
    )?;

    let comparison = compare(&old, &new, None, &entries_only())?;

    assert_eq!(comparison.old_side().channel(), Some("LIVE"), "old Channel");
    assert!(comparison.old_side().build().is_none(), "old Build unknown");
    assert_eq!(comparison.new_side().channel(), Some("PTU"), "new Channel");
    assert_eq!(
        comparison.new_side().build().map(Build::version),
        Some("4.11.42.1234"),
        "new Build"
    );
    Ok(())
}

#[test]
fn progress_ends_with_every_selected_entry_done() -> TestResult {
    let (_dir, old, new) = pair()?;
    let seen = Mutex::new(Vec::new());
    let record = |progress: ComparisonProgress| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(progress);
        }
    };

    compare(
        &old,
        &new,
        None,
        &CompareOptions {
            progress: Some(&record),
            ..entries_only()
        },
    )?;

    let seen = seen.into_inner()?;
    let last = seen.last().ok_or("progress was reported")?;
    assert_eq!(
        (last.entries_done, last.entries_total),
        (11, 11),
        "every Entry of both sides is done"
    );
    Ok(())
}

#[test]
fn a_cancelled_comparison_returns_cancelled() -> TestResult {
    let (_dir, old, new) = pair()?;
    let cancel = CancelToken::new();
    cancel.cancel();

    let result = compare(
        &old,
        &new,
        None,
        &CompareOptions {
            cancel: Some(&cancel),
            ..entries_only()
        },
    );

    assert!(
        matches!(result, Err(ComparisonError::Cancelled)),
        "cancelled: {result:?}"
    );
    Ok(())
}

#[test]
fn a_comparison_without_a_level_is_rejected() -> TestResult {
    let (_dir, old, new) = pair()?;

    let result = compare(
        &old,
        &new,
        None,
        &CompareOptions {
            levels: Levels::NONE,
            ..entries_only()
        },
    );

    assert!(
        matches!(result, Err(ComparisonError::NoLevel)),
        "no level: {result:?}"
    );
    Ok(())
}
