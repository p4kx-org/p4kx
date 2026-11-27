// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_archive::{
    DeclaredEncryption, EncryptionDeclarationSource, Entry, EntryIssue, EntryRecordFormat,
    OpenOptions, StoredEncryptionDeclaration,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture::{self, FixtureResult, PAYLOAD};

const V2_ENCRYPTION_OFFSET: u64 = 170;

#[test]
fn valid_v1_and_v2_declarations_preserve_decoder_and_payload_bytes() -> FixtureResult<()> {
    for v2 in [false, true] {
        for encrypted in [false, true] {
            let authored = TestEntry::stored("Data\\item.bin", PAYLOAD);
            let authored = if encrypted {
                authored.encrypted()
            } else {
                authored
            };
            let builder = if v2 {
                ArchiveBuilder::v2()
            } else {
                ArchiveBuilder::new()
            };
            let bytes = builder.entry(authored).build();
            let options = OpenOptions::new().key(Some(p4kx_testkit::test_key()));
            let (_directory, archive) = fixture::open(&bytes, options)?;
            let entry = fixture::entry(&archive, 0)?;
            let expected = if encrypted {
                DeclaredEncryption::Encrypted
            } else {
                DeclaredEncryption::Unencrypted
            };
            check_valid(entry, expected, &bytes)?;
            assert_eq!(entry.is_encrypted(), encrypted);
            assert_eq!(
                archive.issues(),
                [],
                "valid metadata produces no Entry issues"
            );
            let mut decoded = Vec::new();
            archive
                .open_entry_verified(entry)?
                .read_to_end(&mut decoded)?;
            assert_eq!(
                decoded, PAYLOAD,
                "decoder bytes and verification remain exact"
            );
        }
    }
    Ok(())
}

#[test]
fn v2_raw_words_keep_all_bits_in_the_metadata_projection() -> FixtureResult<()> {
    for word in [0, 1, 7, u16::MAX] {
        let bytes = ArchiveBuilder::v2()
            .entry(TestEntry::stored("Data\\item.bin", PAYLOAD).with_encryption_field(word))
            .build();
        let (_directory, archive) = fixture::open(&bytes, OpenOptions::new())?;
        let entry = fixture::entry(&archive, 0)?;
        let declarations = entry.metadata().encryption_declarations();
        let declaration = declarations.iter().next().ok_or("v2 word missing")?;
        assert_eq!(entry.metadata().raw_v2_encryption_word(), Some(word));
        assert_eq!(declaration.raw_value(), Some(word));
        assert_eq!(
            fixture::word(&bytes, usize::try_from(declaration.value_offset())?)?,
            word
        );
        assert_eq!(entry.is_encrypted(), word != 0);
        match word {
            0 => assert_eq!(declarations.state(), DeclaredEncryption::Unencrypted),
            1 => assert_eq!(declarations.state(), DeclaredEncryption::Encrypted),
            _ => {
                assert_eq!(declarations.state(), DeclaredEncryption::Unknown);
                assert_eq!(
                    archive.issues(),
                    [(0, EntryIssue::UnknownEncryption { value: word })]
                );
            }
        }
    }
    Ok(())
}

fn check_valid(entry: &Entry, expected: DeclaredEncryption, bytes: &[u8]) -> FixtureResult<()> {
    let declarations = entry.metadata().encryption_declarations();
    let word = u16::from(matches!(expected, DeclaredEncryption::Encrypted));
    assert_eq!(
        declarations.state(),
        expected,
        "valid fields establish the stored state"
    );
    let declaration = declarations.iter().next().ok_or("declaration missing")?;
    assert_eq!(
        declarations.iter().len(),
        1,
        "the fixture declares one field"
    );
    assert_eq!(
        declaration.raw_value(),
        Some(word),
        "the original word remains exact"
    );
    assert_eq!(
        declaration.state(),
        expected,
        "the field agrees with its collection"
    );
    assert_eq!(
        declaration.raw_value_bytes().collect::<Vec<_>>(),
        word.to_le_bytes(),
        "the original little-endian bytes remain exact"
    );
    match entry.metadata().record_format() {
        EntryRecordFormat::P4kV2 => check_v2(entry, declaration, word),
        EntryRecordFormat::ZipCentralDirectory => check_cig(entry, declaration, bytes)?,
        _ => return Err("unsupported fixture record format".into()),
    }
    Ok(())
}

fn check_v2(entry: &Entry, declaration: &StoredEncryptionDeclaration, word: u16) {
    assert_eq!(
        declaration.source(),
        EncryptionDeclarationSource::P4kV2Field,
        "v2 uses its fixed directory field"
    );
    assert_eq!(
        declaration.declared_block_len(),
        None,
        "v2 has no CIG block length"
    );
    assert_eq!(
        declaration.value_offset(),
        entry.metadata().record_range().offset() + V2_ENCRYPTION_OFFSET,
        "v2 coordinates address the original fixed word"
    );
    assert_eq!(
        entry.metadata().raw_v2_encryption_word(),
        Some(word),
        "the existing accessor preserves its original result"
    );
}

fn check_cig(
    entry: &Entry,
    declaration: &StoredEncryptionDeclaration,
    bytes: &[u8],
) -> FixtureResult<()> {
    assert_eq!(
        declaration.source(),
        EncryptionDeclarationSource::CigExtraBlock,
        "v1 uses the CIG extra block"
    );
    assert_eq!(
        declaration.declared_block_len(),
        Some(6),
        "the valid CIG block includes its four-byte header"
    );
    assert_eq!(
        entry
            .metadata()
            .encryption_declarations()
            .iter()
            .map(StoredEncryptionDeclaration::value_offset)
            .collect::<Vec<_>>(),
        fixture::cig_offsets(bytes)?,
        "v1 coordinates agree with the independent fixture"
    );
    assert_eq!(
        entry.metadata().raw_v2_encryption_word(),
        None,
        "the ZIP record does not acquire a v2 word"
    );
    Ok(())
}
