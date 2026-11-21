// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{PathKeys, path_key};
use crate::{Archive, PathIndexBuildError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn all_matches_verify_colliding_hashes_and_retain_duplicate_physical_positions() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("test.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/unrelated.mtl", b""))
        .entry(TestEntry::stored("Data/same.mtl", b"one"))
        .entry(TestEntry::stored("data\\SAME.MTL", b"two"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let key = path_key("Data/same.mtl");
    let keys = PathKeys {
        keys: vec![(key, 0), (key, 1), (key, 2)],
    };
    let index = keys.index(&archive);
    let mut work = 0;
    let found: Result<Vec<_>, _> = index
        .matching_entries_with("data/SAME.mtl", |count| {
            work += count;
            Ok::<_, io::Error>(())
        })?
        .map(|entry| entry.map(crate::Entry::index))
        .collect();
    assert_eq!(
        found?,
        [1, 2],
        "same hash alone must not admit an unrelated path"
    );
    assert!(
        work >= 6,
        "query hashing, binary search and unmatched candidates are admitted"
    );
    let built = PathKeys::try_new_with(&archive, |_| Ok::<_, io::Error>(()))?;
    assert_eq!(
        built
            .index(&archive)
            .find("DATA/same.MTL")
            .ok_or("first duplicate")?
            .index(),
        1,
        "all matches verify colliding hashes and retain duplicate physical positions"
    );
    Ok(())
}

#[test]
fn hashing_sorting_and_candidate_visits_preserve_the_original_admission_error() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("test.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.mtl", b""))
        .entry(TestEntry::stored("Data/b.mtl", b""))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let mut admitted = 0;
    let failure = PathKeys::try_new_with(&archive, |count| {
        admitted += count;
        if admitted > 2 {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    })
    .err()
    .ok_or("sort admission did not interrupt")?;
    assert!(
        matches!(failure, PathIndexBuildError::Interrupted(error)
        if error.kind() == io::ErrorKind::PermissionDenied),
        "hashing sorting and candidate visits preserve the original admission error"
    );
    let keys = PathKeys::new(&archive);
    let index = keys.index(&archive);
    let failure = index
        .matching_entries_with("Data/a.mtl", |_| {
            Err(io::Error::from(io::ErrorKind::Interrupted))
        })
        .err()
        .ok_or("query admission did not interrupt")?;
    assert!(
        matches!(failure, PathIndexBuildError::Interrupted(error)
        if error.kind() == io::ErrorKind::Interrupted),
        "hashing sorting and candidate visits preserve the original admission error"
    );
    Ok(())
}

#[test]
fn stable_hash_sort_keeps_source_order_for_equal_hashes_and_full_width_words() -> TestResult {
    let mut keys = [(u64::MAX, 0), (1, 1), (0, 2), (1_u64 << 32, 3), (1, 4)];
    crate::path_key_sort::sort(&mut keys, &mut |_| Ok::<_, io::Error>(()))?;
    assert_eq!(
        keys,
        [(0, 2), (1, 1), (1, 4), (1_u64 << 32, 3), (u64::MAX, 0)],
        "stable hash sort keeps source order for equal hashes and full width words"
    );
    let mut empty = [];
    crate::path_key_sort::sort(&mut empty, &mut |_| Ok::<_, io::Error>(()))?;
    Ok(())
}
