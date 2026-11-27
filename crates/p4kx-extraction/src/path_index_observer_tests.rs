// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Entry;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{Archive, PathIndexBuildError, PathKeys};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn observed_keys_visit_original_physical_inputs_once_in_build_order() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.ext", b"one"))
        .entry(TestEntry::stored("same.ext", b"two"))
        .entry(TestEntry::stored("other.ext", b"three"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let mut originals = Vec::new();
    let keys = PathKeys::try_new_observed(
        &archive,
        |_| Ok::<_, &'static str>(()),
        |entry| {
            assert!(
                std::ptr::eq(
                    archive
                        .entries()
                        .get(entry.index())
                        .ok_or("physical input")?,
                    entry
                ),
                "the observer receives original borrowed Entries"
            );
            originals.push(entry.index());
            Ok(())
        },
    )
    .map_err(|_| "observed build")?;
    assert_eq!(
        originals,
        [0, 1, 2],
        "duplicates retain separate original physical positions"
    );
    let found = keys
        .index(&archive)
        .matching_entries_with("same.ext", |_| Ok::<_, &'static str>(()))
        .map_err(|_| "matching keys")?
        .map(|entry| entry.map(Entry::index))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "matching input")?;
    assert_eq!(
        found,
        [0, 1],
        "observed construction preserves the actual lookup and duplicate order"
    );
    Ok(())
}

#[test]
fn original_admission_and_observer_errors_stop_the_single_build_loop() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("a.ext", b"a"))
        .entry(TestEntry::stored("b.ext", b"b"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let mut observed = 0;
    let result = PathKeys::try_new_observed(
        &archive,
        |_| Err("admission"),
        |_| {
            observed += 1;
            Ok(())
        },
    );
    assert!(
        matches!(result, Err(PathIndexBuildError::Interrupted("admission"))),
        "existing admission fails before observations"
    );
    assert_eq!(
        observed, 0,
        "failed initial admission cannot create extra origin metadata"
    );
    let result = PathKeys::try_new_observed(
        &archive,
        |_| Ok(()),
        |_| {
            observed += 1;
            Err("observer")
        },
    );
    assert!(
        matches!(result, Err(PathIndexBuildError::Interrupted("observer"))),
        "the original observer failure survives"
    );
    assert_eq!(
        observed, 1,
        "later physical inputs are not visited after failure"
    );
    Ok(())
}
