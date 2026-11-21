// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exact Entry ownership, containing-source evidence and unchanged key-demand identities.

use std::error::Error;
use std::fs;
use std::io::Read as _;

use p4kx_archive::{Archive, Entry, OpenOptions, SourceEncryption};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const CONTENT: &[u8] = b"synthetic source bytes";
const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const ZIP_FLAGS_OFFSET: usize = 8;

fn open(bytes: &[u8], options: OpenOptions) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    fs::write(&path, bytes)?;
    Ok((directory, Archive::open_with(&path, options)?))
}

fn first(archive: &Archive) -> TestResult<&Entry> {
    archive
        .entries()
        .first()
        .ok_or_else(|| "Entry missing".into())
}

fn set_zip_flags(bytes: &mut [u8], flags: u16) -> TestResult {
    let record = bytes
        .windows(CENTRAL_SIGNATURE.len())
        .position(|window| window == CENTRAL_SIGNATURE)
        .ok_or("directory missing")?;
    let offset = record + ZIP_FLAGS_OFFSET;
    bytes
        .get_mut(offset..offset + 2)
        .ok_or("ZIP flags missing")?
        .copy_from_slice(&flags.to_le_bytes());
    Ok(())
}

#[test]
fn origin_requires_the_actual_borrowed_entry_slot() -> TestResult {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.bin", CONTENT))
        .build();
    let (_directory, archive) = open(&bytes, OpenOptions::new())?;
    let (_other_directory, other) = open(&bytes, OpenOptions::new())?;
    let owned = first(&archive)?;
    let clone = owned.clone();
    let foreign = first(&other)?;
    assert_eq!(owned.index(), foreign.index());
    assert_eq!(
        owned, foreign,
        "equal source declarations do not prove ownership"
    );
    assert!(
        archive.entry_origin(&clone).is_none(),
        "a clone is not an owned borrowed slot"
    );
    assert!(
        archive.entry_origin(foreign).is_none(),
        "same-index foreign Entries are not owned"
    );
    let origin = archive.entry_origin(owned).ok_or("owned origin missing")?;
    assert_eq!(origin.entry().index(), 0);
    assert_eq!(origin.entry().path(), "same.bin");
    assert_eq!(origin.entry().metadata(), owned.metadata());
    assert_eq!(origin.ancestors().len(), 0);
    assert!(
        archive.owns_origin(&origin),
        "the actual observed owner matches"
    );
    assert!(
        !other.owns_origin(&origin),
        "equal declarations in another input do not match"
    );
    assert!(
        origin.input_origin().is_same_input(archive.input_origin()),
        "root input is retained"
    );
    assert_eq!(origin.encryption(), SourceEncryption::Unencrypted);
    Ok(())
}

#[test]
fn a_detached_container_entry_keeps_an_unknown_ancestor_step() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.bin", CONTENT))
        .build();
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("room.socpak", inner))
        .build();
    let (_directory, archive) = open(&bytes, OpenOptions::new())?;
    let detached = first(&archive)?.clone();
    let container = archive.open_container(&detached)?;
    let origin = container
        .entry_origin(first(&container)?)
        .ok_or("child origin missing")?;
    assert_eq!(origin.ancestors().len(), 1);
    assert!(
        origin.ancestors().next().flatten().is_none(),
        "unproved ancestry remains explicit"
    );
    assert_eq!(origin.entry().encryption(), SourceEncryption::Unencrypted);
    assert_eq!(origin.encryption(), SourceEncryption::Unknown);
    assert!(
        !container.owns_origin(&origin),
        "unknown ancestor ownership cannot be proved"
    );
    Ok(())
}

#[test]
fn source_encryption_reconciles_cig_zero_with_zip_flags_conservatively() -> TestResult {
    for flag in [0, 1, 7] {
        for zip_flags in [0, 0x0001, 0x0040, 0x0041] {
            let mut bytes = ArchiveBuilder::new()
                .entry(TestEntry::stored("source.bin", CONTENT).with_encryption_field(flag))
                .build();
            set_zip_flags(&mut bytes, zip_flags)?;
            let (_directory, archive) = open(&bytes, OpenOptions::new())?;
            let origin = archive
                .entry_origin(first(&archive)?)
                .ok_or("origin missing")?;
            let expected = match (flag, zip_flags) {
                (1, _) => SourceEncryption::Encrypted,
                (0, 0) => SourceEncryption::Unencrypted,
                _ => SourceEncryption::Unknown,
            };
            assert_eq!(
                origin.encryption(),
                expected,
                "CIG {flag}, ZIP flags {zip_flags:#x}"
            );
            assert_eq!(
                origin
                    .entry()
                    .metadata()
                    .zip_header()
                    .ok_or("ZIP header missing")?
                    .general_purpose_flags(),
                zip_flags
            );
        }
    }
    Ok(())
}

#[test]
fn ordinary_zip_without_cig_and_malformed_duplicates_stay_unknown() -> TestResult {
    let zip = TestContainer::new()
        .entry(TestEntry::stored("source.bin", CONTENT))
        .build();
    let malformed = ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", CONTENT).with_encryption_block([0, 0, 9, 9]))
        .build();
    let duplicate = ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", CONTENT).with_empty_encryption_blocks(1))
        .build();
    for bytes in [zip, malformed, duplicate] {
        let (_directory, archive) = open(&bytes, OpenOptions::new())?;
        let origin = archive
            .entry_origin(first(&archive)?)
            .ok_or("origin missing")?;
        assert_eq!(origin.encryption(), SourceEncryption::Unknown);
    }
    Ok(())
}

#[test]
fn v2_origins_preserve_supported_and_unknown_word_meanings() -> TestResult {
    for word in [0, 1, 7, u16::MAX] {
        let bytes = ArchiveBuilder::v2()
            .entry(TestEntry::stored("source.bin", CONTENT).with_encryption_field(word))
            .build();
        let (_directory, archive) = open(&bytes, OpenOptions::new())?;
        let origin = archive
            .entry_origin(first(&archive)?)
            .ok_or("origin missing")?;
        let expected = match word {
            0 => SourceEncryption::Unencrypted,
            1 => SourceEncryption::Encrypted,
            _ => SourceEncryption::Unknown,
        };
        assert_eq!(origin.encryption(), expected);
        assert_eq!(
            origin.entry().metadata().raw_v2_encryption_word(),
            Some(word)
        );
    }
    Ok(())
}

#[test]
fn origin_observation_does_not_change_duplicate_entry_key_demand() -> TestResult {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.bin", CONTENT).encrypted())
        .entry(TestEntry::stored("same.bin", CONTENT).encrypted())
        .build();
    let (_directory, archive) = open(&bytes, OpenOptions::new())?;
    for entry in archive.entries() {
        let origin = archive.entry_origin(entry).ok_or("origin missing")?;
        assert_eq!(origin.entry().index(), entry.index());
        assert_eq!(origin.encryption(), SourceEncryption::Encrypted);
    }
    assert_eq!(archive.key_demand().snapshot().unread(), 0);
    for entry in archive.entries() {
        for _ in 0..2 {
            let error = archive
                .open_entry(entry)
                .err()
                .ok_or("key unexpectedly available")?;
            assert!(
                error.is_key_required(),
                "the source observer supplies no key"
            );
        }
    }
    let snapshot = archive.key_demand().snapshot();
    assert_eq!(snapshot.missing, 2, "indices distinguish duplicate paths");
    assert_eq!(
        snapshot.unread(),
        2,
        "repeat reads preserve physical-position deduplication"
    );
    Ok(())
}

#[test]
fn source_evidence_is_independent_of_missing_wrong_and_correct_keys() -> TestResult {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", CONTENT).encrypted())
        .build();
    let options = [
        (OpenOptions::new(), false),
        (
            OpenOptions::new().key(Some(p4kx_testkit::other_test_key())),
            false,
        ),
        (OpenOptions::new().key(Some(p4kx_testkit::test_key())), true),
    ];
    for (options, succeeds) in options {
        let (_directory, archive) = open(&bytes, options)?;
        let entry = first(&archive)?;
        let origin = archive.entry_origin(entry).ok_or("origin missing")?;
        let read = read_verified(&archive, entry);
        assert_eq!(
            read.is_ok(),
            succeeds,
            "the fixture exercises actual key/read outcomes"
        );
        if let Ok(decoded) = read {
            assert_eq!(decoded, CONTENT);
        }
        assert_eq!(origin.encryption(), SourceEncryption::Encrypted);
        assert_eq!(
            archive
                .entry_origin(entry)
                .ok_or("origin missing after read")?
                .encryption(),
            SourceEncryption::Encrypted
        );
    }
    Ok(())
}

fn read_verified(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut decoded = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut decoded)?;
    Ok(decoded)
}
