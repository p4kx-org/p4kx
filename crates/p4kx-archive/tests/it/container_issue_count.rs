// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, EntryIssue, MAX_ENTRY_ISSUES};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_zip_container_excludes_folder_issues_beyond_the_sample() -> TestResult {
    verify_retained_count(ArchiveBuilder::new())
}

#[test]
fn a_v2_container_excludes_folder_issues_beyond_the_sample() -> TestResult {
    verify_retained_count(ArchiveBuilder::v2())
}

fn verify_retained_count(mut inner: ArchiveBuilder) -> TestResult {
    let directory = tempfile::tempdir()?;
    for position in 0..MAX_ENTRY_ISSUES {
        inner = inner.entry(
            TestEntry::stored(format!("retained-{position}.bin"), b"x").with_encryption_field(3),
        );
    }
    inner = inner
        .entry(
            TestEntry::stored("dropped/", b"")
                .with_path_bytes(b"dropped\xff/")
                .with_encryption_field(3),
        )
        .entry(TestEntry::stored("retained-last.bin", b"x").with_encryption_field(3));
    let bytes = inner.build();
    let physical_path = directory.path().join("physical.p4k");
    std::fs::write(&physical_path, &bytes)?;
    let physical = Archive::open(&physical_path)?;
    let retained = MAX_ENTRY_ISSUES + 1;
    assert_eq!(
        physical.issue_count(),
        retained + 2,
        "the physical directory includes both distinct issues of the late empty folder"
    );
    assert_eq!(
        physical.issues().len(),
        MAX_ENTRY_ISSUES,
        "the empty folder is beyond the original diagnostic sample"
    );

    let outer_path = directory.path().join("outer.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.socpak", bytes))
        .write_to(&outer_path)?;
    let outer = Archive::open(&outer_path)?;
    let inner_entry = outer.entries().first().ok_or("container Entry")?;
    let admitted = outer.open_container(inner_entry)?;
    verify_admitted_count(&admitted, retained)
}

fn verify_admitted_count(admitted: &Archive, retained: usize) -> TestResult {
    assert_eq!(
        admitted.entries().len(),
        retained,
        "container admission removes only the empty directory Entry"
    );
    assert_eq!(
        admitted.issue_count(),
        retained,
        "all retained issues count, including the retained Entry beyond the sample"
    );
    assert_eq!(
        admitted.issues().len(),
        MAX_ENTRY_ISSUES,
        "admission preserves the original diagnostic sample size"
    );
    assert!(
        admitted
            .issues()
            .iter()
            .enumerate()
            .all(|(position, (index, issue))| {
                *index == position && matches!(issue, EntryIssue::UnknownEncryption { value: 3 })
            }),
        "the listed issues keep their original order and retained Entry positions"
    );
    let last = admitted.entries().last().ok_or("last retained Entry")?;
    assert_eq!(
        last.path(),
        "retained-last.bin",
        "the retained Entry after the dropped folder is admitted"
    );
    assert_eq!(
        last.index(),
        MAX_ENTRY_ISSUES,
        "the retained Entry is reindexed after folder removal"
    );
    assert_eq!(
        last.metadata().original_directory_ordinal(),
        Some(u64::try_from(MAX_ENTRY_ISSUES + 1)?),
        "the original directory position survives admission"
    );
    Ok(())
}
