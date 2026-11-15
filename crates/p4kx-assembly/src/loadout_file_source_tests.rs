// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::LoadoutFileSource;
use crate::{ArchiveFiles, LoadoutFileError, LoadoutFileRead, LoadoutFiles};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SELECTOR: &str = "synthetic/shared.xml";
const OTHER: &str = "synthetic/other.xml";

fn archive() -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            format!("Data/{SELECTOR}"),
            b"<Loadout/>".as_slice(),
        ))
        .entry(TestEntry::stored(
            format!("Data/{OTHER}"),
            b"<Loadout/>".as_slice(),
        ))
        .write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

fn source(archive: &Archive, path: &str) -> Result<LoadoutFileSource, Box<dyn std::error::Error>> {
    let read = ArchiveFiles::new(archive).read_observed(path);
    assert!(
        read.outcome().is_ok(),
        "the genuine original byte read completed"
    );
    let source = read.source();
    let origin = source.entry_origin().ok_or("actual Archive admission")?;
    assert!(
        archive.owns_origin(origin),
        "the original admission belongs to this Archive"
    );
    Ok(source.clone())
}

#[test]
fn cloned_unbound_admission_is_not_a_new_equal_text_admission() {
    let read = LoadoutFileRead::unbound(SELECTOR, Ok(b"<Loadout/>".to_vec()));
    let second = LoadoutFileRead::unbound(SELECTOR, Ok(b"<Loadout/>".to_vec()));
    let cloned = read.source().clone();
    assert_eq!(
        read.source().identity(),
        second.source().identity(),
        "descriptive coordinates deliberately agree"
    );
    assert!(
        read.source().is_same_admission(&cloned),
        "a whole genuine source clone retains its handle"
    );
    assert!(
        !read.source().is_same_admission(second.source()),
        "fresh unbound evidence has a distinct original handle"
    );
    assert!(
        !second.source().is_same_admission(read.source()),
        "admission equality is symmetric"
    );
}

#[test]
fn actual_archive_admission_is_physical_and_repeated_reads_preserve_it() -> TestResult {
    let (_directory, original) = archive()?;
    let first = source(&original, SELECTOR)?;
    let again = source(&original, SELECTOR)?;
    let other = source(&original, OTHER)?;
    let unbound = LoadoutFileRead::unbound(first.identity().path, Ok(Vec::new()));
    assert!(
        first.is_same_admission(&again),
        "the same physical Entry remains the same admission"
    );
    assert!(
        again.is_same_admission(&first),
        "actual Archive admission is symmetric"
    );
    assert!(
        !first.is_same_admission(&other),
        "another actual Entry is not the same admission"
    );
    assert!(
        !first.is_same_admission(unbound.source()),
        "equal text cannot bind generic bytes to Archive"
    );
    assert!(
        !unbound.source().is_same_admission(&first),
        "generic evidence cannot authorize Archive facts"
    );
    Ok(())
}

#[test]
fn equal_entry_coordinates_in_separate_archives_remain_foreign() -> TestResult {
    let (_left_directory, left) = archive()?;
    let (_right_directory, right) = archive()?;
    let first = source(&left, SELECTOR)?;
    let foreign = source(&right, SELECTOR)?;
    assert_eq!(
        first.identity(),
        foreign.identity(),
        "equal path/index are only descriptors"
    );
    assert!(
        first.is_same_admission(&first.clone()),
        "the original whole clone is a positive control"
    );
    assert!(
        !first.is_same_admission(&foreign),
        "separate original Archives cannot share admission"
    );
    assert!(
        !foreign.is_same_admission(&first),
        "foreign ownership is rejected in either direction"
    );
    Ok(())
}

#[test]
fn verified_archive_read_only_binds_complete_owned_bytes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let xml = b"<Loadout/>".as_slice();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\good.xml", xml))
        .entry(TestEntry::stored(r"Data\bad.xml", xml).with_payload(b"<Unbound/>".as_slice()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = |name| archive.entries().iter().find(|entry| entry.path() == name);
    let good = entry(r"Data\good.xml").ok_or("valid XML entry")?;
    let bad = entry(r"Data\bad.xml").ok_or("corrupt XML entry")?;
    let read = LoadoutFileRead::read_archive(&archive, good, &|| false);
    assert_eq!(read.outcome(), Ok(xml));
    let origin = read.source().entry_origin().ok_or("verified admission")?;
    assert!(archive.owns_origin(origin));

    let cancelled = LoadoutFileRead::read_archive(&archive, good, &|| true);
    assert_eq!(
        cancelled.outcome(),
        Err(&LoadoutFileError::ObservationStopped)
    );
    assert!(cancelled.source().entry_origin().is_none());

    let detached = Entry::clone(good);
    let unowned = LoadoutFileRead::read_archive(&archive, &detached, &|| false);
    assert_eq!(
        unowned.outcome(),
        Err(&LoadoutFileError::SourceEvidenceUnavailable)
    );
    assert!(unowned.source().entry_origin().is_none());

    let corrupt = LoadoutFileRead::read_archive(&archive, bad, &|| false);
    assert!(matches!(
        corrupt.outcome(),
        Err(LoadoutFileError::Unreadable { .. })
    ));
    assert!(corrupt.source().entry_origin().is_none());
    Ok(())
}
