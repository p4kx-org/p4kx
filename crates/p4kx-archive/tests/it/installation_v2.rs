// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Installation declarations remain distinct from verified Entry payloads.

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_archive::{Archive, ArchiveError, InstallationStatus, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const TRAILER_LEN: usize = 175;

fn complete() -> Vec<u8> {
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data\\a.bin", b"abc".to_vec()))
        .build()
}

fn write(dir: &Path, bytes: &[u8]) -> Result<Archive, ArchiveError> {
    let path = dir.join("Data.p4k");
    std::fs::write(&path, bytes).map_err(|source| ArchiveError::Io {
        path: path.clone(),
        source,
    })?;
    Archive::open(&path)
}

fn field(bytes: &mut [u8], offset: usize, value: u64) -> TestResult {
    let at = bytes.len() - TRAILER_LEN + offset;
    bytes
        .get_mut(at..at + 8)
        .ok_or("synthetic trailer field")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn flags(bytes: &mut [u8], value: u8) -> TestResult {
    let at = bytes.len() - TRAILER_LEN + 104;
    *bytes.get_mut(at).ok_or("synthetic status flag")? = value;
    Ok(())
}

fn progress(bytes: &mut [u8], value: u64) -> TestResult {
    bytes
        .get_mut(3..11)
        .ok_or("synthetic progress field")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[test]
fn a_complete_declaration_retains_flags_and_verified_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write(dir.path(), &complete())?;
    assert_eq!(
        archive.installation_status(),
        InstallationStatus::DeclaredComplete { flags: 0 },
        "the supported declaration is retained"
    );
    let entry = archive.entries().first().ok_or("Entry")?;
    assert_eq!(
        entry.installed_bytes(),
        Some(3),
        "the stored counter is retained"
    );
    let mut output = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut output)?;
    assert_eq!(
        output, b"abc",
        "payload verification is independent of the declaration"
    );
    Ok(())
}

#[test]
fn a_valid_payload_does_not_override_an_incomplete_declaration_or_recovery() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = complete();
    flags(&mut bytes, 1)?;
    let failure = write(dir.path(), &bytes)
        .err()
        .ok_or("incomplete Archive opened")?;
    assert!(
        matches!(
            failure,
            ArchiveError::IncompleteInstallation { flags: 1, .. }
        ),
        "normal opens refuse explicitly incomplete installation"
    );
    let path = dir.path().join("Data.p4k");
    let options = OpenOptions::default().recover(true);
    let failure = Archive::open_with(path, options)
        .err()
        .ok_or("recovery bypassed status")?;
    assert!(
        matches!(
            failure,
            ArchiveError::IncompleteInstallation { flags: 1, .. }
        ),
        "v1 directory recovery is not a v2 incomplete-install bypass"
    );
    assert!(
        failure.to_string().contains("launcher"),
        "the error has an actionable next step"
    );
    Ok(())
}

#[test]
fn unknown_bits_are_preserved_without_invented_meanings() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = complete();
    flags(&mut bytes, 0x80)?;
    let archive = write(dir.path(), &bytes)?;
    assert_eq!(
        archive.installation_status().raw_flags(),
        Some(0x80),
        "unknown bits remain raw"
    );
    flags(&mut bytes, 0x81)?;
    assert!(
        matches!(
            write(dir.path(), &bytes),
            Err(ArchiveError::IncompleteInstallation { flags: 0x81, .. })
        ),
        "unknown bits cannot hide the known incomplete bit"
    );
    Ok(())
}

#[test]
fn progress_counters_are_bounded_source_facts_not_payload_integrity() -> TestResult {
    let dir = tempfile::tempdir()?;
    for value in [0, 2, 3] {
        let mut bytes = complete();
        progress(&mut bytes, value)?;
        let archive = write(dir.path(), &bytes)?;
        assert_eq!(
            archive
                .entries()
                .first()
                .and_then(p4kx_archive::Entry::installed_bytes),
            Some(value),
            "zero, partial and full source progress stay distinct"
        );
    }
    let mut bytes = complete();
    progress(&mut bytes, 4)?;
    assert!(
        matches!(write(dir.path(), &bytes),
        Err(ArchiveError::Corrupt { reason, .. }) if reason.contains("installed-byte")),
        "progress beyond stored bytes is malformed"
    );
    Ok(())
}

#[test]
fn truncated_or_overflowing_install_ranges_are_rejected() -> TestResult {
    let dir = tempfile::tempdir()?;
    for (offset, value) in [(64, 4), (64, u64::MAX), (72, 1), (72, u64::MAX)] {
        let mut bytes = complete();
        field(&mut bytes, offset, value)?;
        assert!(
            matches!(write(dir.path(), &bytes), Err(ArchiveError::Corrupt { .. })),
            "invalid install ranges fail before exposing Entries"
        );
    }
    Ok(())
}

#[test]
fn zero_entry_archives_still_obey_the_installation_declaration() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = ArchiveBuilder::v2().build();
    let archive = write(dir.path(), &bytes)?;
    assert_eq!(
        archive.entries().len(),
        0,
        "the complete empty Archive opens"
    );
    flags(&mut bytes, 1)?;
    assert!(
        matches!(
            write(dir.path(), &bytes),
            Err(ArchiveError::IncompleteInstallation { .. })
        ),
        "empty is not automatically complete"
    );
    Ok(())
}

#[test]
fn v1_absence_of_status_is_unavailable_instead_of_confirmed_complete() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\a.bin", b"abc".to_vec()))
        .build();
    let archive = write(dir.path(), &bytes)?;
    assert_eq!(
        archive.installation_status(),
        InstallationStatus::Unavailable,
        "v1 has no invented installation declaration"
    );
    assert_eq!(
        archive
            .entries()
            .first()
            .and_then(p4kx_archive::Entry::installed_bytes),
        None,
        "v1 has no invented progress counter"
    );
    Ok(())
}

#[test]
fn container_admission_preserves_its_own_status_and_progress() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut child = complete();
    flags(&mut child, 0x80)?;
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\test.socpak", child))
        .build();
    let archive = write(dir.path(), &bytes)?;
    let entry = archive.entries().first().ok_or("container Entry")?;
    let container = archive.open_container(entry)?;
    assert_eq!(
        archive.installation_status(),
        InstallationStatus::Unavailable,
        "the parent declaration is independent"
    );
    assert_eq!(
        container.installation_status().raw_flags(),
        Some(0x80),
        "the child declaration survives admission"
    );
    assert_eq!(
        container
            .entries()
            .first()
            .and_then(p4kx_archive::Entry::installed_bytes),
        Some(3),
        "the child counter survives admission"
    );
    Ok(())
}
