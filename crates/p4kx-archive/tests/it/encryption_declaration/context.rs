// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_archive::{DeclaredEncryption, OpenOptions, StoredEncryptionDeclaration};
use p4kx_testkit::{TestContainer, TestEntry};

use super::fixture::{self, FixtureResult, PAYLOAD};

#[test]
fn a_missing_cig_declaration_and_plain_zip_remain_unknown() -> FixtureResult<()> {
    let missing =
        fixture::missing_declaration(fixture::v1(TestEntry::stored("Data\\item.bin", PAYLOAD)))?;
    let zip = TestContainer::new()
        .entry(TestEntry::stored("Data\\item.bin", PAYLOAD))
        .build();
    for bytes in [missing, zip] {
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let entry = fixture::entry(&archive, 0)?;
        assert_eq!(
            entry.metadata().encryption_declarations().state(),
            DeclaredEncryption::Unknown
        );
        assert_eq!(entry.metadata().encryption_declarations().iter().len(), 0);
        assert!(!entry.is_encrypted());
        assert_eq!(
            archive.issues(),
            [],
            "valid metadata produces no Entry issues"
        );
        let mut decoded = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut decoded)?;
        assert_eq!(decoded, PAYLOAD);
    }
    Ok(())
}

#[test]
fn keys_and_failed_payload_reads_do_not_change_stored_declarations() -> FixtureResult<()> {
    let bytes = fixture::v1(TestEntry::stored("Data\\bad-cipher.bin", PAYLOAD).flagged_encrypted());
    for options in [
        OpenOptions::new(),
        OpenOptions::new().key(Some(p4kx_testkit::test_key())),
    ] {
        let (_directory, archive) = fixture::open(&bytes, options)?;
        let entry = fixture::entry(&archive, 0)?;
        let before = entry.metadata().encryption_declarations().clone();
        let result = archive.open_entry_verified(entry).map(|mut reader| {
            let mut decoded = Vec::new();
            reader.read_to_end(&mut decoded)
        });
        assert!(matches!(result, Err(_) | Ok(Err(_))));
        assert_eq!(before.state(), DeclaredEncryption::Encrypted);
        assert_eq!(entry.metadata().encryption_declarations(), &before);
    }
    Ok(())
}

#[test]
fn container_offsets_use_the_immediately_owning_source_origin() -> FixtureResult<()> {
    let inner = fixture::v1(TestEntry::stored("Data\\inner.bin", PAYLOAD));
    let expected = fixture::cig_offsets(&inner)?;
    for container in [
        TestEntry::stored("Data\\room.socpak", inner.as_slice()),
        TestEntry::deflated("Data\\room.socpak", inner.as_slice()),
    ] {
        let bytes = fixture::v1(container);
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let nested = archive.open_container(fixture::entry(&archive, 0)?)?;
        let declarations = fixture::entry(&nested, 0)?
            .metadata()
            .encryption_declarations();
        assert_eq!(
            declarations
                .iter()
                .map(StoredEncryptionDeclaration::value_offset)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(declarations.state(), DeclaredEncryption::Unencrypted);
    }
    Ok(())
}

#[test]
fn cig_declarations_and_zip_flags_remain_separate_source_facts() -> FixtureResult<()> {
    for flags in [0x0001, 0x0040, 0x0041] {
        let bytes = fixture::zip_flags(
            fixture::v1(TestEntry::stored("Data\\item.bin", PAYLOAD)),
            flags,
        )?;
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let entry = fixture::entry(&archive, 0)?;
        let metadata = entry.metadata();
        assert_eq!(
            metadata
                .zip_header()
                .ok_or("ZIP header missing")?
                .general_purpose_flags(),
            flags
        );
        assert_eq!(
            metadata.encryption_declarations().state(),
            DeclaredEncryption::Unencrypted
        );
        assert!(!entry.is_encrypted());
    }
    Ok(())
}
